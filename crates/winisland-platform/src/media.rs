use std::sync::Arc;

use crate::{MediaCommand, PlatformError, Timeline, TrackInfo};

pub enum ThumbnailError {
    Stale,
    Terminal,
    Backend(String),
}

/// Session handle for media and thumbnail workers. Calls may block and must stay off render.
pub trait MediaSessionHandle: Send + Sync {
    /// Returns the source ID, or `None` if the session has gone away.
    fn source_app_id(&self) -> Option<String>;
    /// Classifies music sessions; false on missing metadata.
    fn is_music(&self) -> bool;
    /// Classifies video sessions; false on missing metadata.
    fn is_video(&self) -> bool;
    /// Reads playback state; false on missing state.
    fn is_playing(&self) -> bool;
    /// Reads playback state with explicit failure for stale sessions.
    fn playback(&self) -> Result<bool, PlatformError>;
    /// Reads track metadata, or returns a stale/backend error.
    fn track(&self) -> Result<TrackInfo, PlatformError>;
    /// Reads timeline, or `None` if the session does not expose one.
    fn timeline(&self) -> Option<Timeline>;
    /// Sends a transport command; false means rejected, errors mean backend failure.
    fn send(&self, command: MediaCommand) -> Result<bool, PlatformError>;
    /// Reads a thumbnail; stale, terminal, and backend failures remain distinct.
    fn thumbnail(&self, expected_title: &str) -> Result<Vec<u8>, ThumbnailError>;
}

/// Worker-owned session manager. Polling is synchronous and must stay off render.
pub trait MediaContext {
    /// Lists current sessions; empty means no active media sessions.
    fn sessions(&self) -> Vec<Arc<dyn MediaSessionHandle>>;
    /// Returns the OS selected session, if any.
    fn current(&self) -> Option<Arc<dyn MediaSessionHandle>>;
    /// Consumes the session change flag; false means no change.
    fn poll_events(&self) -> bool;
}

/// Media operations block and must be called off the rendering thread. Session IDs remain
/// valid only while listed; stale sessions return errors and calls are not reentrant.
pub trait MediaProvider {
    /// Opens an owned manager, or returns an unsupported/backend error.
    fn open_context(&self) -> Result<Box<dyn MediaContext>, PlatformError>;
    /// Lists likely active source applications; empty means none found.
    fn detect_active_apps(&self) -> Vec<String>;
}
