use std::ffi::c_void;

use super::{ResourceId, Utf8Slice};
use crate::abi::PluginStatus;

/// The lyric line contains word-synchronised timing boundaries.
pub const LYRICS_TEXT_FLAG_WORD_SYNCED: u32 = 1 << 0;

/// A parsed lyric line supplied to a registered transformer.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LyricsTextV2 {
    /// Must be `size_of::<LyricsTextV2>()`.
    pub struct_size: u32,
    /// Combination of `LYRICS_TEXT_FLAG_*` values.
    pub flags: u32,
    /// Timestamp of this line in milliseconds.
    pub line_time_ms: u64,
    /// Borrowed UTF-8 text valid only for this callback.
    pub text: Utf8Slice,
}

impl Default for LyricsTextV2 {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            flags: 0,
            line_time_ms: 0,
            text: Utf8Slice::empty(),
        }
    }
}

/// Transform one parsed lyric line.
///
/// WinIsland calls this twice for each line. The first call has a null
/// `output` and zero `output_capacity`; write the required UTF-8 byte length
/// to `out_len`. The second call supplies that capacity; write the transformed
/// bytes and their actual length to `out_len`.
///
/// Word-synchronised lines must keep the same Unicode character count so the
/// host can preserve their timing boundaries.
pub type LyricsTransformFnV2 = unsafe extern "C" fn(
    callback_data: *mut c_void,
    resource_id: ResourceId,
    input: *const LyricsTextV2,
    output: *mut u8,
    output_capacity: u32,
    out_len: *mut u32,
) -> PluginStatus;

/// Registration data for a lyric text transformer.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LyricsTransformerDataV2 {
    /// Must be `size_of::<LyricsTransformerDataV2>()`.
    pub struct_size: u32,
    /// Reserved for future flags. Must be zero.
    pub flags: u32,
    pub on_transform: Option<LyricsTransformFnV2>,
    /// Opaque pointer passed back to `on_transform`.
    pub callback_data: *mut c_void,
}

impl Default for LyricsTransformerDataV2 {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            flags: 0,
            on_transform: None,
            callback_data: std::ptr::null_mut(),
        }
    }
}
