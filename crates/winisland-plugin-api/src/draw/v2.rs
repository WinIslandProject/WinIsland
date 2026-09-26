pub const MAGIC: u32 = 0x5749_4432;
pub const VERSION: u32 = 2;
pub const MAX_LIST_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_COMMANDS: u32 = 4096;
pub const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_FAMILY_BYTES: usize = 255;

pub const PUSH_CLIP_RECT: u16 = 0x0001;
pub const PUSH_CLIP_ROUND_RECT: u16 = 0x0002;
pub const POP_CLIP: u16 = 0x0003;
pub const PUSH_TRANSFORM: u16 = 0x0004;
pub const POP_TRANSFORM: u16 = 0x0005;
pub const PUSH_ALPHA: u16 = 0x0006;
pub const POP_ALPHA: u16 = 0x0007;
pub const FILL_RECT: u16 = 0x0010;
pub const FILL_ROUND_RECT: u16 = 0x0011;
pub const FILL_CIRCLE: u16 = 0x0012;
pub const FILL_CONVEX_POLYGON: u16 = 0x0013;
pub const FILL_LINEAR_GRADIENT: u16 = 0x0014;
pub const STROKE_LINE: u16 = 0x0020;
pub const STROKE_ROUND_RECT: u16 = 0x0021;
pub const STROKE_ARC: u16 = 0x0022;
pub const DRAW_IMAGE: u16 = 0x0030;
pub const DRAW_IMAGE_PIXELS: u16 = 0x0031;
pub const DRAW_TEXT: u16 = 0x0040;
pub const DRAW_TEXT_RUNS: u16 = 0x0041;
pub const DRAW_SHADOW_ROUND_RECT: u16 = 0x0050;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DrawListHeader {
    pub magic: u32,
    pub version: u32,
    pub logical_w: f32,
    pub logical_h: f32,
    pub command_count: u32,
    pub payload_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DrawCommandHeader {
    pub opcode: u16,
    pub flags: u16,
    pub payload_len: u32,
}

const _: () = {
    use std::mem::{align_of, offset_of, size_of};

    assert!(size_of::<DrawListHeader>() == 24);
    assert!(align_of::<DrawListHeader>() == 4);
    assert!(offset_of!(DrawListHeader, magic) == 0);
    assert!(offset_of!(DrawListHeader, version) == 4);
    assert!(offset_of!(DrawListHeader, logical_w) == 8);
    assert!(offset_of!(DrawListHeader, logical_h) == 12);
    assert!(offset_of!(DrawListHeader, command_count) == 16);
    assert!(offset_of!(DrawListHeader, payload_len) == 20);
    assert!(size_of::<DrawCommandHeader>() == 8);
    assert!(align_of::<DrawCommandHeader>() == 4);
    assert!(offset_of!(DrawCommandHeader, opcode) == 0);
    assert!(offset_of!(DrawCommandHeader, flags) == 2);
    assert!(offset_of!(DrawCommandHeader, payload_len) == 4);
};
