mod thumbnail;

use std::sync::{Arc, mpsc};
use std::time::Duration;

use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionMediaProperties,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use winisland_platform::{
    MediaCommand, MediaContext, MediaProvider, MediaSessionHandle, PlatformError, ThumbnailError,
    Timeline, TrackInfo,
};

pub struct WindowsMedia;

struct WinRtGuard;

impl WinRtGuard {
    fn new() -> Result<Self, PlatformError> {
        // SAFETY: Each successful initialization is balanced by this guard on the same thread.
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .map_err(|error| PlatformError::Backend(error.to_string()))?;
        Ok(Self)
    }
}

impl Drop for WinRtGuard {
    fn drop(&mut self) {
        // SAFETY: The guard exists only after a successful RoInitialize on this thread.
        unsafe { RoUninitialize() };
    }
}

struct WindowsMediaContext {
    manager: GlobalSystemMediaTransportControlsSessionManager,
    token: Option<i64>,
    event_rx: mpsc::Receiver<()>,
    _guard: WinRtGuard,
}

impl WindowsMediaContext {
    fn new() -> Result<Self, PlatformError> {
        let guard = WinRtGuard::new()?;
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .and_then(|operation| operation.join())
            .map_err(|error| PlatformError::Backend(error.to_string()))?;
        let (event_tx, event_rx) = mpsc::sync_channel(1);
        let handler = TypedEventHandler::new(move |_manager, _args| {
            let _ = event_tx.try_send(());
            Ok(())
        });
        let token = manager.SessionsChanged(&handler).ok();
        Ok(Self {
            manager,
            token,
            event_rx,
            _guard: guard,
        })
    }
}

impl MediaContext for WindowsMediaContext {
    fn sessions(&self) -> Vec<Arc<dyn MediaSessionHandle>> {
        let Ok(sessions) = self.manager.GetSessions() else {
            return Vec::new();
        };
        let Ok(count) = sessions.Size() else {
            return Vec::new();
        };
        (0..count)
            .filter_map(|index| sessions.GetAt(index).ok())
            .map(|session| Arc::new(WindowsMediaSession(session)) as Arc<dyn MediaSessionHandle>)
            .collect()
    }

    fn current(&self) -> Option<Arc<dyn MediaSessionHandle>> {
        self.manager
            .GetCurrentSession()
            .ok()
            .map(|session| Arc::new(WindowsMediaSession(session)) as Arc<dyn MediaSessionHandle>)
    }

    fn poll_events(&self) -> bool {
        self.event_rx.try_recv().is_ok()
    }
}

impl Drop for WindowsMediaContext {
    fn drop(&mut self) {
        if let Some(token) = self.token.take()
            && let Err(error) = self.manager.RemoveSessionsChanged(token)
        {
            log::warn!("SMTC: failed to remove session change handler: {error}");
        }
    }
}

struct WindowsMediaSession(GlobalSystemMediaTransportControlsSession);

impl WindowsMediaSession {
    fn media_type(&self) -> Option<windows::Media::MediaPlaybackType> {
        self.0
            .GetPlaybackInfo()
            .ok()?
            .PlaybackType()
            .ok()?
            .Value()
            .ok()
    }
}

impl MediaSessionHandle for WindowsMediaSession {
    fn source_app_id(&self) -> Option<String> {
        self.0.SourceAppUserModelId().ok().map(|id| id.to_string())
    }

    fn is_music(&self) -> bool {
        self.media_type() == Some(windows::Media::MediaPlaybackType::Music)
    }

    fn is_video(&self) -> bool {
        self.media_type() == Some(windows::Media::MediaPlaybackType::Video)
    }

    fn is_playing(&self) -> bool {
        self.playback().unwrap_or(false)
    }

    fn playback(&self) -> Result<bool, PlatformError> {
        let info = self
            .0
            .GetPlaybackInfo()
            .map_err(|error| PlatformError::Backend(error.to_string()))?;
        let status = info
            .PlaybackStatus()
            .map_err(|error| PlatformError::Backend(error.to_string()))?;
        Ok(status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
    }

    fn track(&self) -> Result<TrackInfo, PlatformError> {
        let props = self
            .0
            .TryGetMediaPropertiesAsync()
            .and_then(|operation| operation.join())
            .map_err(|error| PlatformError::Backend(error.to_string()))?;
        Ok(read_track(&props))
    }

    fn timeline(&self) -> Option<Timeline> {
        let props = self.0.GetTimelineProperties().ok()?;
        let position = props
            .Position()
            .ok()
            .map(|value| value.Duration.max(0) / 10_000)
            .unwrap_or(0);
        let duration = props
            .EndTime()
            .ok()
            .map(|value| value.Duration.max(0) / 10_000)
            .unwrap_or(0);
        Some(Timeline {
            position: Duration::from_millis(position as u64),
            duration: Duration::from_millis(duration as u64),
        })
    }

    fn send(&self, command: MediaCommand) -> Result<bool, PlatformError> {
        let operation = match command {
            MediaCommand::Toggle => {
                let Ok(is_playing) = self.playback() else {
                    return Ok(false);
                };
                if is_playing {
                    self.0.TryPauseAsync()
                } else {
                    self.0.TryPlayAsync()
                }
            }
            MediaCommand::Play => self.0.TryPlayAsync(),
            MediaCommand::Pause => self.0.TryPauseAsync(),
            MediaCommand::Next => self.0.TrySkipNextAsync(),
            MediaCommand::Previous => self.0.TrySkipPreviousAsync(),
            MediaCommand::Seek(position) => {
                let ticks = position.as_millis().min(i64::MAX as u128 / 10_000) as i64 * 10_000;
                return self
                    .0
                    .TryChangePlaybackPositionAsync(ticks)
                    .and_then(|operation| operation.join())
                    .map_err(|error| PlatformError::Backend(error.to_string()));
            }
        };
        operation
            .map(|_| true)
            .map_err(|error| PlatformError::Backend(error.to_string()))
    }

    fn thumbnail(&self, expected_title: &str) -> Result<Vec<u8>, ThumbnailError> {
        let _guard =
            WinRtGuard::new().map_err(|error| ThumbnailError::Backend(error.to_string()))?;
        thumbnail::load(&self.0, expected_title)
    }
}

fn read_string(value: windows::core::Result<windows::core::HSTRING>) -> String {
    value
        .map(|value| value.to_string().trim().to_string())
        .unwrap_or_default()
}

fn read_track(props: &GlobalSystemMediaTransportControlsSessionMediaProperties) -> TrackInfo {
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
    TrackInfo {
        title,
        artist,
        album,
        thumbnail: None,
    }
}

impl MediaProvider for WindowsMedia {
    fn open_context(&self) -> Result<Box<dyn MediaContext>, PlatformError> {
        Ok(Box::new(WindowsMediaContext::new()?))
    }

    fn detect_active_apps(&self) -> Vec<String> {
        let Ok(context) = WindowsMediaContext::new() else {
            return Vec::new();
        };
        let mut apps = Vec::new();
        for session in context.sessions() {
            if let Some(id) = session.source_app_id()
                && !apps.contains(&id)
            {
                apps.push(id);
            }
        }
        apps
    }
}
