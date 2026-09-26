use crate::draw::v2 as wire;
use crate::types::v2::ImageId;

#[derive(Clone, Copy)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Rgba(u32);

impl Rgba {
    pub const WHITE: Self = Self(0xffff_ffff);

    pub const fn from_argb(value: u32) -> Self {
        Self(value)
    }

    pub const fn argb(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy)]
pub struct Deg12(pub f32);

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum ImageFit {
    Contain = 0,
    Cover = 1,
    Fill = 2,
}

#[derive(Clone)]
pub struct TextStyle {
    pub size: f32,
    pub weight: u16,
    pub italic: bool,
    pub align: u8,
    pub wrap: bool,
    pub ellipsis: bool,
    pub family: String,
}

impl TextStyle {
    pub fn default_at(size: f32) -> Self {
        Self {
            size,
            weight: 400,
            italic: false,
            align: 0,
            wrap: false,
            ellipsis: false,
            family: String::new(),
        }
    }

    pub fn bold(mut self) -> Self {
        self.weight = 700;
        self
    }
}

#[derive(Clone, Copy)]
pub struct TextRun<'a> {
    pub text: &'a str,
    pub color: Rgba,
    pub weight: u16,
}

#[derive(Clone, Copy)]
pub struct GradientStop {
    pub position: f32,
    pub color: Rgba,
}

pub struct DrawListBuilder {
    bytes: Vec<u8>,
    command_count: u32,
}

pub struct DrawList<'a> {
    bytes: &'a [u8],
}

impl DrawList<'_> {
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes
    }
}

impl DrawListBuilder {
    pub fn new(logical: Size) -> Self {
        let mut bytes = Vec::with_capacity(256);
        bytes.extend_from_slice(&wire::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&wire::VERSION.to_le_bytes());
        bytes.extend_from_slice(&logical.width.to_le_bytes());
        bytes.extend_from_slice(&logical.height.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        Self {
            bytes,
            command_count: 0,
        }
    }

    pub fn finish(&self) -> DrawList<'_> {
        DrawList { bytes: &self.bytes }
    }

    fn command(&mut self, opcode: u16, payload: &[u8]) -> &mut Self {
        let len = u32::try_from(payload.len()).unwrap_or(u32::MAX);
        self.bytes.extend_from_slice(&opcode.to_le_bytes());
        self.bytes.extend_from_slice(&0_u16.to_le_bytes());
        self.bytes.extend_from_slice(&len.to_le_bytes());
        self.bytes.extend_from_slice(payload);
        self.command_count = self.command_count.saturating_add(1);
        self.bytes[16..20].copy_from_slice(&self.command_count.to_le_bytes());
        let total = u32::try_from(self.bytes.len() - 24).unwrap_or(u32::MAX);
        self.bytes[20..24].copy_from_slice(&total.to_le_bytes());
        self
    }

    pub fn clip_rect(&mut self, rect: Rect) -> &mut Self {
        self.command(wire::PUSH_CLIP_RECT, &rect_bytes(rect))
    }

    pub fn clip_round_rect(&mut self, rect: Rect, radius: f32) -> &mut Self {
        let mut payload = rect_bytes(rect);
        push_f32(&mut payload, radius);
        self.command(wire::PUSH_CLIP_ROUND_RECT, &payload)
    }

    pub fn pop_clip(&mut self) -> &mut Self {
        self.command(wire::POP_CLIP, &[])
    }

    pub fn transform(&mut self, affine: [f32; 6]) -> &mut Self {
        let mut payload = Vec::with_capacity(24);
        for value in affine {
            push_f32(&mut payload, value);
        }
        self.command(wire::PUSH_TRANSFORM, &payload)
    }

    pub fn pop_transform(&mut self) -> &mut Self {
        self.command(wire::POP_TRANSFORM, &[])
    }

    pub fn alpha(&mut self, value: u8) -> &mut Self {
        self.command(wire::PUSH_ALPHA, &[value])
    }

    pub fn pop_alpha(&mut self) -> &mut Self {
        self.command(wire::POP_ALPHA, &[])
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Rgba) -> &mut Self {
        let mut payload = rect_bytes(rect);
        push_u32(&mut payload, color.argb());
        self.command(wire::FILL_RECT, &payload)
    }

    pub fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: Rgba) -> &mut Self {
        let mut payload = rect_bytes(rect);
        push_f32(&mut payload, radius);
        push_u32(&mut payload, color.argb());
        self.command(wire::FILL_ROUND_RECT, &payload)
    }

    pub fn fill_circle(&mut self, x: f32, y: f32, radius: f32, color: Rgba) -> &mut Self {
        let mut payload = Vec::with_capacity(16);
        push_f32(&mut payload, x);
        push_f32(&mut payload, y);
        push_f32(&mut payload, radius);
        push_u32(&mut payload, color.argb());
        self.command(wire::FILL_CIRCLE, &payload)
    }

    pub fn fill_convex_polygon(&mut self, points: &[(f32, f32)], color: Rgba) -> &mut Self {
        let mut payload = Vec::with_capacity(6 + points.len() * 8);
        push_u16(&mut payload, points.len().min(u16::MAX as usize) as u16);
        for &(x, y) in points {
            push_f32(&mut payload, x);
            push_f32(&mut payload, y);
        }
        push_u32(&mut payload, color.argb());
        self.command(wire::FILL_CONVEX_POLYGON, &payload)
    }

    pub fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        angle: f32,
        stops: &[GradientStop],
    ) -> &mut Self {
        let mut payload = rect_bytes(rect);
        push_f32(&mut payload, angle);
        payload.push(stops.len().min(u8::MAX as usize) as u8);
        for stop in stops {
            push_f32(&mut payload, stop.position);
            push_u32(&mut payload, stop.color.argb());
        }
        self.command(wire::FILL_LINEAR_GRADIENT, &payload)
    }

    pub fn stroke_line(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        width: f32,
        color: Rgba,
    ) -> &mut Self {
        let mut payload = Vec::with_capacity(24);
        for value in [from.0, from.1, to.0, to.1, width] {
            push_f32(&mut payload, value);
        }
        push_u32(&mut payload, color.argb());
        self.command(wire::STROKE_LINE, &payload)
    }

    pub fn stroke_round_rect(
        &mut self,
        rect: Rect,
        radius: f32,
        width: f32,
        color: Rgba,
    ) -> &mut Self {
        let mut payload = rect_bytes(rect);
        push_f32(&mut payload, radius);
        push_f32(&mut payload, width);
        push_u32(&mut payload, color.argb());
        self.command(wire::STROKE_ROUND_RECT, &payload)
    }

    pub fn stroke_arc(
        &mut self,
        rect: Rect,
        start: Deg12,
        sweep: Deg12,
        width: f32,
        color: Rgba,
    ) -> &mut Self {
        let mut payload = rect_bytes(rect);
        for value in [start.0, sweep.0, width] {
            push_f32(&mut payload, value);
        }
        push_u32(&mut payload, color.argb());
        self.command(wire::STROKE_ARC, &payload)
    }

    pub fn image(&mut self, id: ImageId, rect: Rect, fit: ImageFit) -> &mut Self {
        let mut payload = Vec::with_capacity(26);
        payload.extend_from_slice(&id.get().to_le_bytes());
        payload.extend_from_slice(&rect_bytes(rect));
        payload.extend_from_slice(&[fit as u8, 255]);
        self.command(wire::DRAW_IMAGE, &payload)
    }

    pub fn image_pixels(
        &mut self,
        rgba: &[u8],
        dimensions: (u32, u32),
        rect: Rect,
        fit: ImageFit,
    ) -> &mut Self {
        let mut payload = rect_bytes(rect);
        payload.extend_from_slice(&[fit as u8, 255]);
        push_u32(&mut payload, dimensions.0);
        push_u32(&mut payload, dimensions.1);
        push_u32(&mut payload, rgba.len().min(u32::MAX as usize) as u32);
        payload.extend_from_slice(rgba);
        self.command(wire::DRAW_IMAGE_PIXELS, &payload)
    }

    pub fn text(&mut self, value: &str, rect: Rect, style: &TextStyle, color: Rgba) -> &mut Self {
        let mut payload = rect_bytes(rect);
        push_u32(&mut payload, value.len().min(u32::MAX as usize) as u32);
        payload.extend_from_slice(value.as_bytes());
        push_f32(&mut payload, style.size);
        push_u16(&mut payload, style.weight);
        push_text_flags(&mut payload, style);
        push_family(&mut payload, &style.family);
        push_u32(&mut payload, color.argb());
        self.command(wire::DRAW_TEXT, &payload)
    }

    pub fn text_runs(&mut self, runs: &[TextRun<'_>], rect: Rect, style: &TextStyle) -> &mut Self {
        let mut payload = rect_bytes(rect);
        push_f32(&mut payload, style.size);
        push_text_flags(&mut payload, style);
        push_family(&mut payload, &style.family);
        payload.push(runs.len().min(u8::MAX as usize) as u8);
        for run in runs {
            push_u32(&mut payload, run.text.len().min(u32::MAX as usize) as u32);
            payload.extend_from_slice(run.text.as_bytes());
            push_u32(&mut payload, run.color.argb());
            push_u16(&mut payload, run.weight);
        }
        self.command(wire::DRAW_TEXT_RUNS, &payload)
    }

    pub fn shadow_round_rect(
        &mut self,
        rect: Rect,
        radius: f32,
        sigma: f32,
        offset: (f32, f32),
        color: Rgba,
    ) -> &mut Self {
        let mut payload = rect_bytes(rect);
        for value in [radius, sigma, offset.0, offset.1] {
            push_f32(&mut payload, value);
        }
        push_u32(&mut payload, color.argb());
        self.command(wire::DRAW_SHADOW_ROUND_RECT, &payload)
    }
}

fn rect_bytes(rect: Rect) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16);
    for value in [rect.x, rect.y, rect.width, rect.height] {
        push_f32(&mut bytes, value);
    }
    bytes
}

fn push_text_flags(bytes: &mut Vec<u8>, style: &TextStyle) {
    bytes.extend_from_slice(&[
        u8::from(style.italic),
        style.align,
        u8::from(style.wrap),
        u8::from(style.ellipsis),
    ]);
}

fn push_family(bytes: &mut Vec<u8>, family: &str) {
    let mut len = family.len().min(wire::MAX_FAMILY_BYTES);
    while !family.is_char_boundary(len) {
        len -= 1;
    }
    push_u16(bytes, len as u16);
    bytes.extend_from_slice(&family.as_bytes()[..len]);
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
