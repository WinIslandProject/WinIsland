use std::time::{Duration, Instant};

use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

use crate::core::lyrics::LyricsMode;

use super::properties::{ThumbnailFetcher, TimelineCache, fetch_properties};
use super::session::{auto_allow_new_apps, get_target_session};
use super::{LyricsFetchRequest, MediaInfo, PlaybackCommand, WinRtGuard, spawn_lyrics_fetch};

pub(super) struct WorkerChannels {
    pub(super) info_tx: watch::Sender<MediaInfo>,
    pub(super) enabled_rx: watch::Receiver<bool>,
    pub(super) seek_rx: mpsc::UnboundedReceiver<u64>,
    pub(super) playback_rx: mpsc::UnboundedReceiver<PlaybackCommand>,
    pub(super) lyrics_mode_rx: mpsc::UnboundedReceiver<LyricsMode>,
    pub(super) lyrics_source_rx: mpsc::UnboundedReceiver<String>,
    pub(super) lyrics_local_dir_rx: mpsc::UnboundedReceiver<Option<String>>,
    pub(super) allowed_apps_rx: mpsc::UnboundedReceiver<Vec<String>>,
    pub(super) known_apps: Vec<String>,
}

pub(super) fn smtc_poll_loop(channels: WorkerChannels, cancel: CancellationToken) {
    let WorkerChannels {
        info_tx,
        mut enabled_rx,
        mut seek_rx,
        mut playback_rx,
        mut lyrics_mode_rx,
        mut lyrics_source_rx,
        mut lyrics_local_dir_rx,
        mut allowed_apps_rx,
        mut known_apps,
    } = channels;
    let _winrt_guard = match WinRtGuard::new() {
        Ok(guard) => guard,
        Err(error) => {
            log::error!("SMTC: failed to initialize WinRT: {error}");
            return;
        }
    };

    let manager = match GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
        Ok(op) => match op.join() {
            Ok(m) => m,
            Err(error) => {
                log::error!("SMTC: failed to get session manager: {error}");
                return;
            }
        },
        Err(error) => {
            log::error!("SMTC: RequestAsync failed: {error}");
            return;
        }
    };
    log::info!("SMTC: session manager created");

    let (event_tx, event_rx) = std::sync::mpsc::sync_channel::<()>(1);
    let handler = TypedEventHandler::new(move |_m, _| {
        let _ = event_tx.try_send(());
        Ok(())
    });
    let sessions_changed_token = manager.SessionsChanged(&handler).ok();
    let mut enabled = *enabled_rx.borrow_and_update();
    let mut thumbnail_fetcher = enabled
        .then(|| ThumbnailFetcher::new(info_tx.clone()))
        .flatten();

    let mut current_lyrics_mode = LyricsMode::Online;
    let mut current_lyrics_source = "163".to_string();
    let mut current_lyrics_local_dir: Option<String> = None;
    let mut current_allowed_apps: Vec<String> = Vec::new();

    while let Ok(mode) = lyrics_mode_rx.try_recv() {
        current_lyrics_mode = mode;
    }
    while let Ok(src) = lyrics_source_rx.try_recv() {
        current_lyrics_source = src;
    }
    while let Ok(dir) = lyrics_local_dir_rx.try_recv() {
        current_lyrics_local_dir = dir;
    }
    while let Ok(apps) = allowed_apps_rx.try_recv() {
        current_allowed_apps = apps;
    }

    let mut last_session_seen = Instant::now();
    let mut last_was_playing = false;
    let mut timeline_cache = TimelineCache::default();

    if enabled {
        for attempt in 0..10 {
            update_media_info(
                &manager,
                &info_tx,
                current_lyrics_mode,
                &current_lyrics_source,
                current_lyrics_local_dir.as_deref(),
                &mut current_allowed_apps,
                &mut known_apps,
                true,
                &mut last_session_seen,
                &mut last_was_playing,
                thumbnail_fetcher.as_ref(),
                &mut timeline_cache,
            );
            let info = info_tx.borrow();
            let timeline_ready = info.duration_ms > 0
                || info.position_ms > 0
                || !info.is_playing
                || info.title.is_empty();
            if timeline_ready {
                if attempt > 0 {
                    log::info!("SMTC: initial timeline ready after {} retries", attempt + 1);
                }
                drop(info);
                break;
            }
            drop(info);
            if attempt < 9 {
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }

    let mut last_regular_update = Instant::now();
    let mut regular_poll_count = 0u32;

    while !cancel.is_cancelled() {
        let mut lyrics_settings_changed = false;
        while let Ok(mode) = lyrics_mode_rx.try_recv() {
            if mode != current_lyrics_mode {
                current_lyrics_mode = mode;
                lyrics_settings_changed = true;
            }
        }
        while let Ok(src) = lyrics_source_rx.try_recv() {
            if src != current_lyrics_source {
                current_lyrics_source = src;
                lyrics_settings_changed = true;
            }
        }
        while let Ok(dir) = lyrics_local_dir_rx.try_recv() {
            if dir != current_lyrics_local_dir {
                current_lyrics_local_dir = dir;
                lyrics_settings_changed = true;
            }
        }
        if lyrics_settings_changed {
            refresh_current_lyrics(
                &info_tx,
                current_lyrics_mode,
                &current_lyrics_source,
                current_lyrics_local_dir.as_deref(),
            );
        }
        while let Ok(apps) = allowed_apps_rx.try_recv() {
            current_allowed_apps = apps;
        }

        let next_enabled = *enabled_rx.borrow_and_update();
        if next_enabled != enabled {
            enabled = next_enabled;
            if enabled {
                thumbnail_fetcher = ThumbnailFetcher::new(info_tx.clone());
                last_regular_update = Instant::now() - Duration::from_millis(301);
                last_session_seen = Instant::now();
                timeline_cache.clear();
                log::info!("SMTC: listener enabled");
            } else {
                thumbnail_fetcher = None;
                let _ = info_tx.send(MediaInfo::default());
                last_was_playing = false;
                timeline_cache.clear();
                log::info!("SMTC: listener disabled");
            }
        }

        if !enabled {
            while seek_rx.try_recv().is_ok() {}
            while playback_rx.try_recv().is_ok() {}
            while event_rx.try_recv().is_ok() {}
            std::thread::sleep(Duration::from_millis(300));
            continue;
        }

        let mut seek_pos = None;
        while let Ok(v) = seek_rx.try_recv() {
            seek_pos = Some(v);
        }
        if let Some(seek_pos) = seek_pos
            && let Some(session) = get_target_session(&manager, &current_allowed_apps)
        {
            let seek_pos = seek_pos.min(i64::MAX as u64 / 10_000);
            let source_app_id = session
                .SourceAppUserModelId()
                .map(|id| id.to_string())
                .unwrap_or_default();
            log::info!("SMTC: seek to {}ms", seek_pos);
            let ticks = seek_pos as i64 * 10_000;
            match session.TryChangePlaybackPositionAsync(ticks) {
                Ok(_) => {
                    info_tx.send_if_modified(|info| {
                        if info.source_app_id != source_app_id || info.title.is_empty() {
                            return false;
                        }
                        info.position_ms = seek_pos;
                        info.last_update = Instant::now();
                        info.seek_target_ms = seek_pos;
                        info.seek_guard_until = Some(Instant::now() + Duration::from_secs(4));
                        true
                    });
                }
                Err(error) => log::warn!("SMTC: failed to start seek: {error}"),
            }
        }

        while let Ok(cmd) = playback_rx.try_recv() {
            log::info!("SMTC: playback command {:?}", cmd);
            if let Some(session) = get_target_session(&manager, &current_allowed_apps) {
                match cmd {
                    PlaybackCommand::Toggle => {
                        if let Ok(pb_info) = session.GetPlaybackInfo()
                            && let Ok(status) = pb_info.PlaybackStatus()
                        {
                            if status == windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing {
                                    let _ = session.TryPauseAsync();
                                } else {
                                    let _ = session.TryPlayAsync();
                                }
                        }
                    }
                    PlaybackCommand::Next => {
                        let _ = session.TrySkipNextAsync();
                    }
                    PlaybackCommand::Prev => {
                        let _ = session.TrySkipPreviousAsync();
                    }
                }
            }
        }

        if event_rx.try_recv().is_ok() {
            log::info!("SMTC: session change event received, updating immediately");
            update_media_info(
                &manager,
                &info_tx,
                current_lyrics_mode,
                &current_lyrics_source,
                current_lyrics_local_dir.as_deref(),
                &mut current_allowed_apps,
                &mut known_apps,
                true,
                &mut last_session_seen,
                &mut last_was_playing,
                thumbnail_fetcher.as_ref(),
                &mut timeline_cache,
            );
            last_regular_update = Instant::now();
        }

        if last_regular_update.elapsed() > Duration::from_millis(300) {
            regular_poll_count += 1;
            let do_auto_allow = regular_poll_count.is_multiple_of(10);
            update_media_info(
                &manager,
                &info_tx,
                current_lyrics_mode,
                &current_lyrics_source,
                current_lyrics_local_dir.as_deref(),
                &mut current_allowed_apps,
                &mut known_apps,
                do_auto_allow,
                &mut last_session_seen,
                &mut last_was_playing,
                thumbnail_fetcher.as_ref(),
                &mut timeline_cache,
            );
            last_regular_update = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(300));
    }

    if let Some(token) = sessions_changed_token
        && let Err(error) = manager.RemoveSessionsChanged(token)
    {
        log::warn!("SMTC: failed to remove session change handler: {error}");
    }
}

#[allow(clippy::too_many_arguments)]
fn update_media_info(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    info_tx: &watch::Sender<MediaInfo>,
    lyrics_mode: LyricsMode,
    lyrics_source: &str,
    local_dir: Option<&str>,
    allowed_apps: &mut Vec<String>,
    known_apps: &mut Vec<String>,
    auto_allow: bool,
    last_session_seen: &mut Instant,
    last_was_playing: &mut bool,
    thumbnail_fetcher: Option<&ThumbnailFetcher>,
    timeline_cache: &mut TimelineCache,
) {
    if auto_allow {
        *allowed_apps = auto_allow_new_apps(manager, allowed_apps, known_apps);
    }

    if let Some(session) = get_target_session(manager, allowed_apps) {
        let result = fetch_properties(
            &session,
            info_tx,
            lyrics_mode,
            lyrics_source,
            local_dir,
            thumbnail_fetcher,
            timeline_cache,
        );
        match result {
            Ok(()) => {
                *last_session_seen = Instant::now();
                *last_was_playing = info_tx.borrow().is_playing;
            }
            Err(error) => {
                log::debug!("SMTC: failed to read active session: {error}");
                if last_session_seen.elapsed() > Duration::from_secs(5) {
                    let info = info_tx.borrow();
                    if !info.title.is_empty() {
                        drop(info);
                        let _ = info_tx.send(MediaInfo::default());
                        timeline_cache.clear();
                        *last_was_playing = false;
                        log::warn!("SMTC: active session failed for >5s, cleared media info");
                    }
                }
            }
        }
    } else if *last_was_playing {
        let info = info_tx.borrow();
        if !info.title.is_empty() {
            drop(info);
            let _ = info_tx.send(MediaInfo::default());
            timeline_cache.clear();
            log::info!("SMTC: app closed while playing, cleared immediately");
        }
        *last_was_playing = false;
    } else if last_session_seen.elapsed() > Duration::from_secs(15) {
        let info = info_tx.borrow();
        if !info.title.is_empty() {
            drop(info);
            let _ = info_tx.send(MediaInfo::default());
            timeline_cache.clear();
            log::info!("SMTC: paused session lost for >15s, cleared media info");
        }
    }
}

fn refresh_current_lyrics(
    info_tx: &watch::Sender<MediaInfo>,
    lyrics_mode: LyricsMode,
    lyrics_source: &str,
    local_dir: Option<&str>,
) {
    let mut request = None;
    info_tx.send_if_modified(|info| {
        if info.title.is_empty() {
            return false;
        }
        info.lyrics = None;
        info.lyrics_fetch_id = info.lyrics_fetch_id.wrapping_add(1);
        request = Some((
            info.title.clone(),
            info.artist.clone(),
            info.duration_secs,
            info.lyrics_fetch_id,
        ));
        true
    });
    let Some((title, artist, duration_secs, request_id)) = request else {
        return;
    };
    spawn_lyrics_fetch(
        info_tx,
        LyricsFetchRequest {
            title,
            artist,
            duration_secs,
            mode: lyrics_mode,
            source: lyrics_source.to_string(),
            local_dir: local_dir.map(str::to_string),
            request_id,
        },
    );
}
