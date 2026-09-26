/// Playback — media players, podcasts, videos (lowest).
pub const PRIORITY_LOW: u32 = 0;
/// Activity — ongoing short-lived activities like timers, screen recording.
pub const PRIORITY_MEDIUM: u32 = 1;
/// Alert — notifications that need immediate attention (highest).
pub const PRIORITY_HIGH: u32 = 2;

/// Show this context in the compact island.
pub const CONTEXT_FLAG_SHOW_COMPACT: u32 = 1 << 0;

/// Context content owned by a plugin resource.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ContextDataV2 {
    /// Must be `size_of::<ContextDataV2>()`.
    pub struct_size: u32,
    /// Priority: [`PRIORITY_LOW`], [`PRIORITY_MEDIUM`], [`PRIORITY_HIGH`].
    pub priority: u32,
    /// Combination of `CONTEXT_FLAG_*` values.
    pub flags: u32,
    /// Remove the context after this many milliseconds. Zero means persistent.
    pub timeout_ms: u32,
    /// Primary text. Max 255 bytes plus NUL.
    pub title: [u8; 256],
    /// Secondary text. Max 511 bytes plus NUL.
    pub body: [u8; 512],
    /// Optional compact summary. Falls back to `title`. Max 127 bytes plus NUL.
    pub compact_text: [u8; 128],
}

impl Default for ContextDataV2 {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            priority: PRIORITY_MEDIUM,
            flags: CONTEXT_FLAG_SHOW_COMPACT,
            timeout_ms: 0,
            title: [0; 256],
            body: [0; 512],
            compact_text: [0; 128],
        }
    }
}

/// Snapshot of the current host state that a plugin can query.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HostStateV2 {
    /// Must be `size_of::<HostStateV2>()`.
    pub struct_size: u32,
    /// Reserved for future state flags.
    pub flags: u32,
    /// Currently playing media title. Max 255 bytes + NUL.
    pub media_title: [u8; 256],
    /// Currently playing media artist. Max 255 bytes + NUL.
    pub media_artist: [u8; 256],
    /// Whether media is currently playing.
    pub is_playing: u8,
    pub reserved: [u8; 7],
    /// Current theme: `"light"` or `"dark"`. Max 31 bytes + NUL.
    pub theme: [u8; 32],
}

impl Default for HostStateV2 {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            flags: 0,
            media_title: [0; 256],
            media_artist: [0; 256],
            is_playing: 0,
            reserved: [0; 7],
            theme: [0; 32],
        }
    }
}

/// Media is currently playing.
pub const MEDIA_FLAG_PLAYING: u32 = 1 << 0;

pub const MEDIA_CONTROL_TOGGLE_PLAY: u32 = 1 << 0;
pub const MEDIA_CONTROL_PREVIOUS: u32 = 1 << 1;
pub const MEDIA_CONTROL_NEXT: u32 = 1 << 2;
pub const MEDIA_CONTROL_SEEK: u32 = 1 << 3;

pub const MEDIA_COMMAND_TOGGLE_PLAY: u32 = 1;
pub const MEDIA_COMMAND_PREVIOUS: u32 = 2;
pub const MEDIA_COMMAND_NEXT: u32 = 3;
pub const MEDIA_COMMAND_SEEK: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MediaCommandV2 {
    pub struct_size: u32,
    pub command: u32,
    /// Used only by `MEDIA_COMMAND_SEEK`.
    pub position_ms: u64,
}

pub type MediaCommandFnV2 = unsafe extern "C" fn(
    callback_data: *mut std::ffi::c_void,
    resource_id: super::ResourceId,
    command: *const MediaCommandV2,
);

/// Display-only media source data supplied by a plugin.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MediaSourceDataV2 {
    /// Must be `size_of::<MediaSourceDataV2>()`.
    pub struct_size: u32,
    /// Combination of `MEDIA_FLAG_*` values.
    pub flags: u32,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Current playback position in milliseconds.
    pub position_ms: u64,
    /// Combination of `MEDIA_CONTROL_*` values.
    pub available_controls: u32,
    pub reserved: u32,
    /// Track title. Max 255 bytes + NUL.
    pub title: [u8; 256],
    /// Artist name. Max 255 bytes + NUL.
    pub artist: [u8; 256],
    /// Album name. Max 255 bytes + NUL.
    pub album: [u8; 256],
    /// Raw JPEG or PNG bytes. The host copies them before returning.
    pub cover: super::ByteSlice,
    /// Optional callback for controls declared in `available_controls`.
    pub on_command: Option<MediaCommandFnV2>,
    /// Opaque pointer passed back to `on_command`.
    pub callback_data: *mut std::ffi::c_void,
}

impl Default for MediaSourceDataV2 {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            flags: 0,
            duration_ms: 0,
            position_ms: 0,
            available_controls: 0,
            reserved: 0,
            title: [0; 256],
            artist: [0; 256],
            album: [0; 256],
            cover: super::ByteSlice::empty(),
            on_command: None,
            callback_data: std::ptr::null_mut(),
        }
    }
}
