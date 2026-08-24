use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use skia_safe::Data;
use tokio::sync::watch;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionMediaProperties,
};
use windows::Storage::Streams::DataReader;

use crate::core::lyrics::LyricsMode;
use crate::utils::cover::compress_smtc_thumbnail;

use super::session::is_music_session;
use super::{LyricsFetchRequest, MediaInfo, WinRtGuard, spawn_lyrics_fetch};

const MAX_THUMBNAIL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_THUMBNAIL_STREAM_BYTES: u64 = 64 * 1024 * 1024;
const THUMBNAIL_STALE: windows::core::HRESULT = windows::core::HRESULT(-2);
const THUMBNAIL_TOO_LARGE: windows::core::HRESULT = windows::core::HRESULT(-3);
const THUMBNAIL_COMPRESSION_FAILED: windows::core::HRESULT = windows::core::HRESULT(-4);
static NEXT_TRACK_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct MediaMetadata {
    title: String,
    artist: String,
    album: String,
}

impl MediaMetadata {
    fn read(props: &GlobalSystemMediaTransportControlsSessionMediaProperties) -> Self {
        let raw_title = read_string(props.Title());
        let subtitle = read_string(props.Subtitle());
        let album = read_string(props.AlbumTitle());
        let raw_artist = read_string(props.Artist());
        let album_artist = read_string(props.AlbumArtist());

        let title = if !raw_title.is_empty() {
            raw_title
        } else if !album.is_empty() {
            album.clone()
        } else {
            subtitle.clone()
        };
        let artist = if !raw_artist.is_empty() {
            raw_artist
        } else if !album_artist.is_empty() {
            album_artist
        } else if subtitle != title {
            subtitle
        } else {
            String::new()
        };

        Self {
            title,
            artist,
            album,
        }
    }
}

fn read_string(value: windows::core::Result<windows::core::HSTRING>) -> String {
    value
        .map(|value| value.to_string().trim().to_string())
        .unwrap_or_default()
}

fn next_track_id() -> u64 {
    NEXT_TRACK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[derive(Clone)]
struct ThumbnailFetchRequest {
    session: GlobalSystemMediaTransportControlsSession,
    source_app_id: String,
    track_id: u64,
    title: String,
    artist: String,
    is_song_change: bool,
}

pub(super) struct ThumbnailFetcher {
    state: Arc<ThumbnailWorkerState>,
}

struct ThumbnailWorkerState {
    request: Mutex<Option<ThumbnailFetchRequest>>,
    changed: Condvar,
    closed: std::sync::atomic::AtomicBool,
}

impl ThumbnailFetcher {
    pub(super) fn new(info_tx: watch::Sender<MediaInfo>) -> Option<Self> {
        let state = Arc::new(ThumbnailWorkerState {
            request: Mutex::new(None),
            changed: Condvar::new(),
            closed: std::sync::atomic::AtomicBool::new(false),
        });
        let worker_state = Arc::clone(&state);
        let spawn_result = std::thread::Builder::new()
            .name("winisland-smtc-thumbnail".to_string())
            .spawn(move || {
                let _winrt_guard = match WinRtGuard::new() {
                    Ok(guard) => guard,
                    Err(error) => {
                        log::warn!("SMTC: failed to initialize thumbnail worker: {error}");
                        return;
                    }
                };
                loop {
                    let mut pending = worker_state
                        .request
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    while pending.is_none()
                        && !worker_state
                            .closed
                            .load(std::sync::atomic::Ordering::Acquire)
                    {
                        pending = worker_state
                            .changed
                            .wait(pending)
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                    }
                    if worker_state
                        .closed
                        .load(std::sync::atomic::Ordering::Acquire)
                    {
                        return;
                    }
                    let request = pending.take();
                    drop(pending);
                    if let Some(request) = request {
                        fetch_thumbnail(request, &info_tx);
                    }
                }
            });
        match spawn_result {
            Ok(_) => Some(Self { state }),
            Err(error) => {
                log::warn!("SMTC: failed to start thumbnail worker: {error}");
                None
            }
        }
    }

    fn request(
        &self,
        session: &GlobalSystemMediaTransportControlsSession,
        source_app_id: String,
        track_id: u64,
        title: String,
        artist: String,
        is_song_change: bool,
    ) {
        let mut request = self
            .state
            .request
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *request = Some(ThumbnailFetchRequest {
            session: session.clone(),
            source_app_id,
            track_id,
            title,
            artist,
            is_song_change,
        });
        drop(request);
        self.state.changed.notify_one();
    }
}

impl Drop for ThumbnailFetcher {
    fn drop(&mut self) {
        self.state
            .closed
            .store(true, std::sync::atomic::Ordering::Release);
        self.state.changed.notify_one();
    }
}

#[derive(Default)]
pub(super) struct TimelineCache {
    source_app_id: String,
    last_fetch: Option<Instant>,
    position_ms: u64,
    duration_secs: u64,
    duration_ms: u64,
}

impl TimelineCache {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

pub(super) fn fetch_properties(
    session: &GlobalSystemMediaTransportControlsSession,
    info_tx: &watch::Sender<MediaInfo>,
    lyrics_mode: LyricsMode,
    lyrics_source: &str,
    local_dir: Option<&str>,
    thumbnail_fetcher: Option<&ThumbnailFetcher>,
    timeline_cache: &mut TimelineCache,
) -> windows::core::Result<()> {
    if !is_music_session(session) {
        let info = info_tx.borrow();
        if !info.title.is_empty() {
            drop(info);
            let _ = info_tx.send(MediaInfo::default());
        }
        return Ok(());
    }

    let props = session.TryGetMediaPropertiesAsync()?.join()?;
    let pb_info = session.GetPlaybackInfo()?;
    let is_playing = pb_info.PlaybackStatus()? == windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;

    let metadata = MediaMetadata::read(&props);
    let new_title = &metadata.title;
    let new_artist = &metadata.artist;
    let new_album = &metadata.album;
    let source_app_id = session.SourceAppUserModelId()?.to_string();

    let song_changed = {
        let info = info_tx.borrow();
        is_new_track(&info, &metadata, &source_app_id)
    };

    let should_fetch = song_changed
        || (info_tx.borrow().is_playing != is_playing)
        || timeline_cache.source_app_id != source_app_id
        || timeline_cache
            .last_fetch
            .is_none_or(|last| last.elapsed() >= Duration::from_millis(500));

    let (smtc_pos, duration_secs, duration_ms_from_tl) = read_timeline(
        session,
        &source_app_id,
        should_fetch,
        song_changed,
        timeline_cache,
    );

    let mut lyrics_request = None;
    let mut thumbnail_request = None;

    info_tx.send_if_modified(|info| {
        let song_changed = is_new_track(info, &metadata, &source_app_id);
        let mut notify = false;
        if song_changed {
            log::info!(
                "SMTC: track changed -> {} - {} / {} ({})",
                new_title,
                new_artist,
                new_album,
                source_app_id
            );
            info.title.clone_from(new_title);
            info.artist.clone_from(new_artist);
            info.album.clone_from(new_album);
            info.source_app_id.clone_from(&source_app_id);
            info.track_id = next_track_id();
            info.duration_secs = duration_secs;
            info.duration_ms = duration_ms_from_tl;
            info.lyrics = None;
            info.lyrics_fetch_id = info.lyrics_fetch_id.wrapping_add(1);
            info.thumbnail = None;
            info.thumbnail_hash = 0;
            info.position_ms = smtc_pos;
            info.last_smtc_pos = smtc_pos;
            info.last_update = Instant::now();
            info.seek_target_ms = 0;
            info.seek_guard_until = None;
            info.last_thumbnail_fetch = Instant::now();
            lyrics_request = Some((
                info.title.clone(),
                info.artist.clone(),
                info.lyrics_fetch_id,
            ));
            thumbnail_request =
                Some((info.track_id, info.title.clone(), info.artist.clone(), true));
            notify = true;
        } else if new_title.is_empty()
            && info.source_app_id != source_app_id
            && !info.title.is_empty()
        {
            *info = MediaInfo::default();
            return true;
        } else if !new_title.is_empty() {
            let artist_enriched = info.artist.is_empty() && !new_artist.is_empty();
            if artist_enriched {
                info.artist.clone_from(new_artist);
                if info.lyrics.is_none() {
                    info.lyrics_fetch_id = info.lyrics_fetch_id.wrapping_add(1);
                    lyrics_request = Some((
                        info.title.clone(),
                        info.artist.clone(),
                        info.lyrics_fetch_id,
                    ));
                }
                notify = true;
            }
            if info.album.is_empty() && !new_album.is_empty() {
                info.album.clone_from(new_album);
                notify = true;
            }
            if info.thumbnail.is_none()
                && (info.is_playing != is_playing
                    || info.last_thumbnail_fetch.elapsed() >= Duration::from_secs(5))
            {
                info.last_thumbnail_fetch = Instant::now();
                thumbnail_request = Some((
                    info.track_id,
                    info.title.clone(),
                    info.artist.clone(),
                    false,
                ));
            }
        }
        let current_extrapolated = if info.is_playing {
            info.position_ms
                .saturating_add(info.last_update.elapsed().as_millis() as u64)
        } else {
            info.position_ms
        };

        let smtc_changed = smtc_pos != info.last_smtc_pos;
        let diff_with_extrapolated = (smtc_pos as i64 - current_extrapolated as i64).abs();

        let mut seek_guard_active = !song_changed
            && info
                .seek_guard_until
                .is_some_and(|until| Instant::now() < until);
        if !seek_guard_active && info.seek_guard_until.is_some() {
            info.seek_guard_until = None;
        }
        if seek_guard_active
            && smtc_pos > 0
            && (smtc_pos as i64 - info.seek_target_ms as i64).abs() < 1500
        {
            info.seek_guard_until = None;
            seek_guard_active = false;
        }

        let should_sync = song_changed
            || (info.is_playing != is_playing)
            || (smtc_pos > 0 && info.position_ms == 0)
            || (smtc_changed && (diff_with_extrapolated > 2000 || !is_playing));

        if should_sync && !seek_guard_active {
            if smtc_pos > 0 || !song_changed {
                info.position_ms = smtc_pos;
            }
            info.last_update = Instant::now();
            notify = true;
        }

        let was_playing = info.is_playing;
        info.last_smtc_pos = smtc_pos;
        info.is_playing = is_playing;
        if was_playing != is_playing {
            notify = true;
        }
        if !new_title.is_empty() && info.source_app_id != source_app_id {
            info.source_app_id.clone_from(&source_app_id);
            notify = true;
        }
        if !song_changed && was_playing != is_playing {
            log::info!(
                "SMTC: playback state -> {}",
                if is_playing { "Playing" } else { "Paused" }
            );
        }
        if info.duration_secs != duration_secs || info.duration_ms != duration_ms_from_tl {
            info.duration_secs = duration_secs;
            info.duration_ms = duration_ms_from_tl;
            notify = true;
        }
        notify
    });

    if let Some((track_id, title, artist, is_song_change)) = thumbnail_request
        && let Some(thumbnail_fetcher) = thumbnail_fetcher
    {
        thumbnail_fetcher.request(
            session,
            source_app_id.clone(),
            track_id,
            title,
            artist,
            is_song_change,
        );
    }

    if let Some((title, artist, request_id)) = lyrics_request {
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
    Ok(())
}

fn is_new_track(info: &MediaInfo, metadata: &MediaMetadata, source_app_id: &str) -> bool {
    if metadata.title.is_empty() {
        return false;
    }
    info.title.is_empty()
        || info.source_app_id != source_app_id
        || info.title != metadata.title
        || (!info.artist.is_empty()
            && !metadata.artist.is_empty()
            && info.artist != metadata.artist)
        || (!info.album.is_empty() && !metadata.album.is_empty() && info.album != metadata.album)
}

fn read_timeline(
    session: &GlobalSystemMediaTransportControlsSession,
    source_app_id: &str,
    should_fetch: bool,
    reset_cache: bool,
    cache: &mut TimelineCache,
) -> (u64, u64, u64) {
    if reset_cache || cache.source_app_id != source_app_id {
        cache.clear();
        cache.source_app_id = source_app_id.to_string();
    }
    if !should_fetch {
        return (cache.position_ms, cache.duration_secs, cache.duration_ms);
    }

    let Ok(tl) = session.GetTimelineProperties() else {
        cache.last_fetch = Some(Instant::now());
        return (cache.position_ms, cache.duration_secs, cache.duration_ms);
    };

    let smtc_pos = tl
        .Position()
        .ok()
        .map(|pos| {
            if pos.Duration > 0 {
                (pos.Duration / 10_000) as u64
            } else {
                0
            }
        })
        .unwrap_or(0);
    let (duration_secs, duration_ms) = tl
        .EndTime()
        .ok()
        .map(|end| {
            if end.Duration > 0 {
                (
                    (end.Duration / 10_000_000) as u64,
                    (end.Duration / 10_000) as u64,
                )
            } else {
                (0, 0)
            }
        })
        .unwrap_or((0, 0));

    cache.source_app_id.clear();
    cache.source_app_id.push_str(source_app_id);
    cache.last_fetch = Some(Instant::now());
    cache.position_ms = smtc_pos;
    cache.duration_secs = duration_secs;
    cache.duration_ms = duration_ms;
    (smtc_pos, duration_secs, duration_ms)
}

fn fetch_thumbnail(request: ThumbnailFetchRequest, info_tx: &watch::Sender<MediaInfo>) {
    if request.is_song_change {
        std::thread::sleep(Duration::from_millis(800));
    }
    for attempt in 0..10 {
        if !thumbnail_request_is_current(info_tx, &request) {
            return;
        }
        let res = (|| -> windows::core::Result<Vec<u8>> {
            let props = request.session.TryGetMediaPropertiesAsync()?.join()?;
            let fetched_metadata = MediaMetadata::read(&props);
            if fetched_metadata.title != request.title {
                return Err(windows::core::Error::new(
                    THUMBNAIL_STALE,
                    "Stale properties",
                ));
            }
            let thumb_ref = props.Thumbnail()?;
            let stream = thumb_ref.OpenReadAsync()?.join()?;
            let size = stream.Size()?;
            if size == 0 {
                return Err(windows::core::Error::new(
                    windows::core::HRESULT(-1),
                    "Empty thumbnail",
                ));
            }
            if size > MAX_THUMBNAIL_STREAM_BYTES {
                return Err(windows::core::Error::new(
                    THUMBNAIL_TOO_LARGE,
                    "Thumbnail exceeds 64 MiB",
                ));
            }
            if size > MAX_THUMBNAIL_BYTES {
                return compress_smtc_thumbnail(&stream, size).ok_or_else(|| {
                    windows::core::Error::new(
                        THUMBNAIL_COMPRESSION_FAILED,
                        "Failed to compress oversized thumbnail",
                    )
                });
            }
            let size = size as u32;
            let buffer = windows::Storage::Streams::Buffer::Create(size)?;
            let res_buffer = stream
                .ReadAsync(
                    &buffer,
                    size,
                    windows::Storage::Streams::InputStreamOptions::None,
                )?
                .join()?;
            let reader = DataReader::FromBuffer(&res_buffer)?;
            let actual_size = reader.UnconsumedBufferLength()?;
            if actual_size == 0 || actual_size > size {
                return Err(windows::core::Error::new(
                    windows::core::HRESULT(-1),
                    "Invalid thumbnail length",
                ));
            }
            let mut bytes = vec![0u8; actual_size as usize];
            reader.ReadBytes(&mut bytes)?;
            Ok(bytes)
        })();

        if let Ok(bytes) = res {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            let hash = hasher.finish();

            let byte_len = bytes.len();
            let data = Data::new_copy(&bytes);
            let applied = info_tx.send_if_modified(|current| {
                if current.track_id != request.track_id
                    || current.source_app_id != request.source_app_id
                    || current.thumbnail.is_some()
                    || current.thumbnail_hash == hash
                {
                    return false;
                }
                current.thumbnail = Some(data);
                current.thumbnail_hash = hash;
                true
            });
            if applied {
                log::info!(
                    "SMTC: thumbnail fetched ({} bytes, hash={:#x})",
                    byte_len,
                    hash
                );
            }
            return;
        }
        if res
            .as_ref()
            .is_err_and(|error| error.code() == THUMBNAIL_STALE)
        {
            return;
        }
        if res.as_ref().is_err_and(|error| {
            matches!(
                error.code(),
                THUMBNAIL_TOO_LARGE | THUMBNAIL_COMPRESSION_FAILED
            )
        }) {
            log::warn!(
                "SMTC: could not safely load oversized thumbnail for '{}' - '{}'",
                request.title,
                request.artist
            );
            return;
        }
        let delay = if attempt < 3 { 300 } else { 500 };
        std::thread::sleep(Duration::from_millis(delay));
    }
    log::warn!(
        "SMTC: thumbnail fetch failed for '{}' - '{}' after 10 attempts",
        request.title,
        request.artist
    );
}

fn thumbnail_request_is_current(
    info_tx: &watch::Sender<MediaInfo>,
    request: &ThumbnailFetchRequest,
) -> bool {
    let current = info_tx.borrow();
    current.track_id == request.track_id
        && current.source_app_id == request.source_app_id
        && current.thumbnail.is_none()
}
