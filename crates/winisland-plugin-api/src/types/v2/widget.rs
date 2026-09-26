pub const WIDGET_FLAG_SHOW_COMPACT: u32 = 1 << 0;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WidgetSpecV2 {
    pub struct_size: u32,
    pub span_cols: u32,
    pub span_rows: u32,
    pub flags: u32,
    pub title: [u8; 256],
    pub body: [u8; 512],
    pub key: [u8; 64],
    pub min_width: f32,
    pub min_height: f32,
}

impl Default for WidgetSpecV2 {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            span_cols: 2,
            span_rows: 1,
            flags: 0,
            title: [0; 256],
            body: [0; 512],
            key: [0; 64],
            min_width: 0.0,
            min_height: 0.0,
        }
    }
}
