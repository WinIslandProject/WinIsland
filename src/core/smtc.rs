use std::sync::Arc;
use std::time::{Duration, Instant};

use skia_safe::Data;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};

use crate::core::lyrics::{LyricHighlight, LyricLine, LyricsMode, fetch_lyrics};

mod properties;
mod session;
mod worker;

pub(super) struct WinRtGuard {
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl WinRtGuard {
    pub(super) fn new() -> windows::core::Result<Self> {
        // SAFETY: This initializes Windows Runtime for the current thread in the MTA. Every
        // successful call is balanced by WinRtGuard::drop on the same thread.
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }?;
        Ok(Self {
            _not_send: std::marker::PhantomData,
        })
    }
}

impl Drop for WinRtGuard {
    fn drop(&mut self) {
        // SAFETY: The guard is created only after RoInitialize succeeds and is dropped on the
        // thread that owns it.
        unsafe { RoUninitialize() };
    }
}

pub(crate) fn detect_active_apps_async() -> std::sync::mpsc::Receiver<Vec<String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let spawn_result = std::thread::Builder::new()
        .name("winisland-smtc-settings".to_string())
        .spawn(move || {
            use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

            let _winrt_guard = match WinRtGuard::new() {
                Ok(guard) => guard,
                Err(error) => {
                    log::warn!("SMTC: failed to initialize WinRT for app scan: {error}");
                    let _ = tx.send(Vec::new());
                    crate::utils::event_loop::wake();
                    return;
                }
            };

            let apps = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
                .ok()
                .and_then(|operation| operation.join().ok())
                .and_then(|manager| {
                    let sessions = manager.GetSessions().ok()?;
                    let count = sessions.Size().ok()?;
                    let mut apps = Vec::new();
                    for index in 0..count {
                        if let Ok(session) = sessions.GetAt(index)
                            && let Ok(id) = session.SourceAppUserModelId()
                        {
                            let app = id.to_string();
                            if !apps.contains(&app) {
                                apps.push(app);
                            }
                        }
                    }
                    Some(apps)
                })
                .unwrap_or_default();
            let _ = tx.send(apps);
            crate::utils::event_loop::wake();
        });
    if let Err(error) = spawn_result {
        log::warn!("Failed to start settings media app scan: {error}");
    }
    rx
}

#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub source_app_id: String,
    pub track_id: u64,
    pub is_playing: bool,
    pub thumbnail: Option<Data>,
    pub thumbnail_hash: u64,
    pub spectrum: [f32; 6],
    pub position_ms: u64,
    pub last_update: Instant,
    pub last_thumbnail_fetch: Instant,
    pub lyrics: Option<Arc<Vec<LyricLine>>>,
    pub lyrics_fetch_id: u64,
    pub last_smtc_pos: u64,
    pub duration_secs: u64,
    pub duration_ms: u64,
    pub seek_target_ms: u64,
    pub seek_guard_until: Option<Instant>,
}

impl Default for MediaInfo {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            source_app_id: String::new(),
            track_id: 0,
            is_playing: false,
            thumbnail: None,
            thumbnail_hash: 0,
            spectrum: [0.0; 6],
            position_ms: 0,
            last_update: Instant::now(),
            last_thumbnail_fetch: Instant::now() - Duration::from_secs(10),
            lyrics: None,
            lyrics_fetch_id: 0,
            last_smtc_pos: 0,
            duration_secs: 0,
            duration_ms: 0,
            seek_target_ms: 0,
            seek_guard_until: None,
        }
    }
}

impl MediaInfo {
    pub fn effective_duration_ms(&self) -> u64 {
        if self.duration_ms > 0 {
            self.duration_ms
        } else if self.duration_secs > 0 {
            self.duration_secs * 1000
        } else {
            0
        }
    }

    pub fn current_lyric(
        &self,
        delay_ms: i64,
        line_transition_lead_ms: u64,
    ) -> Option<CurrentLyric<'_>> {
        let lyrics = self.lyrics.as_ref()?;
        if lyrics.is_empty() {
            return None;
        }

        let raw_pos = if self.is_playing {
            self.position_ms
                .saturating_add(self.last_update.elapsed().as_millis() as u64)
        } else {
            self.position_ms
        };
        let current_pos = (raw_pos as i64 + delay_ms).max(0) as u64;
        let current_pos = if self.is_playing {
            let duration_ms = self.effective_duration_ms();
            if duration_ms > 0 && current_pos > duration_ms {
                current_pos % duration_ms
            } else {
                current_pos
            }
        } else {
            current_pos
        };

        let line_selection_pos = current_pos.saturating_add(line_transition_lead_ms);
        let index = lyrics.partition_point(|line| line.time_ms <= line_selection_pos);
        let index = index.checked_sub(1)?;
        let line = &lyrics[index];
        Some(CurrentLyric {
            text: &line.text,
            started: line.time_ms <= current_pos,
            highlight: line
                .highlight_at(current_pos, lyrics.get(index + 1).map(|next| next.time_ms)),
        })
    }
}

pub(crate) struct CurrentLyric<'a> {
    pub(crate) text: &'a str,
    pub(crate) started: bool,
    pub(crate) highlight: Option<LyricHighlight>,
}

#[derive(Debug, Clone)]
pub(super) enum PlaybackCommand {
    Toggle,
    Next,
    Prev,
}

pub struct SmtcListener {
    info_rx: watch::Receiver<MediaInfo>,
    enabled_tx: watch::Sender<bool>,
    seek_tx: mpsc::UnboundedSender<u64>,
    playback_tx: mpsc::UnboundedSender<PlaybackCommand>,
    lyrics_mode_tx: mpsc::UnboundedSender<LyricsMode>,
    lyrics_source_tx: mpsc::UnboundedSender<String>,
    lyrics_local_dir_tx: mpsc::UnboundedSender<Option<String>>,
    allowed_apps_tx: mpsc::UnboundedSender<Vec<String>>,
    cancel_token: CancellationToken,
}

impl SmtcListener {
    pub fn new(
        enabled: bool,
        mode: String,
        source: String,
        local_dir: Option<String>,
        allowed: Vec<String>,
        known_apps: Vec<String>,
    ) -> Self {
        let (info_tx, info_rx) = watch::channel(MediaInfo::default());
        let (enabled_tx, enabled_rx) = watch::channel(enabled);
        let (seek_tx, seek_rx) = mpsc::unbounded_channel();
        let (playback_tx, playback_rx) = mpsc::unbounded_channel();
        let (lyrics_mode_tx, lyrics_mode_rx) = mpsc::unbounded_channel();
        let (lyrics_source_tx, lyrics_source_rx) = mpsc::unbounded_channel();
        let (lyrics_local_dir_tx, lyrics_local_dir_rx) = mpsc::unbounded_channel();
        let (allowed_apps_tx, allowed_apps_rx) = mpsc::unbounded_channel();
        let cancel_token = CancellationToken::new();

        let _ = lyrics_mode_tx.send(mode.as_str().into());
        let _ = lyrics_source_tx.send(source);
        let _ = lyrics_local_dir_tx.send(local_dir);
        let _ = allowed_apps_tx.send(allowed);

        let cancel = cancel_token.clone();
        tokio::task::spawn_blocking(move || {
            worker::smtc_poll_loop(
                worker::WorkerChannels {
                    info_tx,
                    enabled_rx,
                    seek_rx,
                    playback_rx,
                    lyrics_mode_rx,
                    lyrics_source_rx,
                    lyrics_local_dir_rx,
                    allowed_apps_rx,
                    known_apps,
                },
                cancel,
            );
        });

        Self {
            info_rx,
            enabled_tx,
            seek_tx,
            playback_tx,
            lyrics_mode_tx,
            lyrics_source_tx,
            lyrics_local_dir_tx,
            allowed_apps_tx,
            cancel_token,
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled_tx.send_if_modified(|current| {
            if *current == enabled {
                false
            } else {
                *current = enabled;
                true
            }
        });
    }

    pub fn set_allowed_apps(&self, apps: Vec<String>) {
        let _ = self.allowed_apps_tx.send(apps);
    }

    pub fn set_lyrics_source(&self, source: String) {
        let _ = self.lyrics_source_tx.send(source);
    }

    pub fn set_lyrics_mode(&self, mode: String) {
        let _ = self.lyrics_mode_tx.send(mode.as_str().into());
    }

    pub fn set_lyrics_local_dir(&self, dir: Option<String>) {
        let _ = self.lyrics_local_dir_tx.send(dir);
    }

    pub fn take_info_if_changed(&mut self) -> Option<MediaInfo> {
        self.info_rx
            .has_changed()
            .ok()
            .filter(|changed| *changed)
            .map(|_| self.info_rx.borrow_and_update().clone())
    }

    pub fn request_seek(&self, position_ms: u64) {
        let _ = self.seek_tx.send(position_ms);
    }

    pub fn request_toggle_play(&self) {
        let _ = self.playback_tx.send(PlaybackCommand::Toggle);
    }

    pub fn request_next(&self) {
        let _ = self.playback_tx.send(PlaybackCommand::Next);
    }

    pub fn request_prev(&self) {
        let _ = self.playback_tx.send(PlaybackCommand::Prev);
    }
}

pub(super) struct LyricsFetchRequest {
    pub(super) title: String,
    pub(super) artist: String,
    pub(super) duration_secs: u64,
    pub(super) mode: LyricsMode,
    pub(super) source: String,
    pub(super) local_dir: Option<String>,
    pub(super) request_id: u64,
}

pub(super) fn spawn_lyrics_fetch(info_tx: &watch::Sender<MediaInfo>, request: LyricsFetchRequest) {
    let info_tx = info_tx.clone();
    tokio::spawn(async move {
        let lyrics = fetch_lyrics(
            &request.title,
            &request.artist,
            request.duration_secs,
            request.mode,
            &request.source,
            request.local_dir.as_deref(),
        )
        .await
        .map(crate::plugin::manager::apply_lyrics_transforms);
        let applied = info_tx.send_if_modified(|current| {
            if current.title != request.title
                || current.artist != request.artist
                || current.lyrics_fetch_id != request.request_id
            {
                return false;
            }
            current.lyrics = lyrics.clone();
            true
        });
        if !applied {
            return;
        }
        if let Some(lyrics) = lyrics {
            log::info!("SMTC: lyrics fetched ({} lines)", lyrics.len());
        } else {
            log::warn!(
                "SMTC: lyrics fetch returned none for '{}' - '{}'",
                request.title,
                request.artist
            );
        }
    });
}

impl Drop for SmtcListener {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}
