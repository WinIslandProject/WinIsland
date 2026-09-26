use winisland_plugin_api::draw::v2 as wire;
use winisland_render::text::FontManager;
use winisland_render::{GradientStop, Image, ImageFit, Path, PathBuilder, Point, Rect, Rgba, Vec2};

use super::validate::ValidatedDrawList;

pub struct PreparedFrame {
    pub logical_width: f32,
    pub logical_height: f32,
    pub(crate) commands: Vec<DrawOp>,
}

pub(crate) enum DrawOp {
    PushClipRect(Rect),
    PushClipRoundRect(Rect, f32),
    PopClip,
    PushTransform([f32; 6]),
    PopTransform,
    PushAlpha(u8),
    PopAlpha,
    FillRect(Rect, Rgba),
    FillRoundRect(Rect, f32, Rgba),
    FillCircle(Point, f32, Rgba),
    FillPolygon(Path, Rgba),
    FillGradient(Rect, Point, Point, Vec<GradientStop>),
    StrokeLine(Point, Point, f32, Rgba),
    StrokeRoundRect(Rect, f32, f32, Rgba),
    StrokeArc(Rect, f32, f32, f32, Rgba),
    Image(Image, Rect, ImageFit, u8),
    Text(String, Rect, TextLayout, Rgba),
    TextRuns(Vec<TextRun>, Rect, TextLayout),
    Shadow(Path, f32, Rgba),
}

pub(crate) struct TextLayout {
    pub size: f32,
    pub italic: bool,
    pub align: u8,
    pub wrap: bool,
    pub ellipsis: bool,
    pub family: String,
    pub weight: u16,
}

pub(crate) struct TextRun {
    pub text: String,
    pub color: Rgba,
    pub weight: u16,
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], &'static str> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or("draw payload length overflow")?;
        let part = self
            .bytes
            .get(self.offset..end)
            .ok_or("truncated validated draw payload")?;
        self.offset = end;
        Ok(part)
    }

    fn u8(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, &'static str> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().map_err(|_| "truncated u16")?,
        ))
    }

    fn u32(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| "truncated u32")?,
        ))
    }

    fn u64(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| "truncated u64")?,
        ))
    }

    fn f32(&mut self) -> Result<f32, &'static str> {
        Ok(f32::from_bits(self.u32()?))
    }

    fn rect(&mut self) -> Result<Rect, &'static str> {
        Ok(Rect::from_xywh(
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
        ))
    }

    fn point(&mut self) -> Result<Point, &'static str> {
        Ok(Point::new(self.f32()?, self.f32()?))
    }

    fn color(&mut self) -> Result<Rgba, &'static str> {
        let color = self.u32()?;
        Ok(Rgba::from_argb(
            (color >> 24) as u8,
            (color >> 16) as u8,
            (color >> 8) as u8,
            color as u8,
        ))
    }

    fn fit(&mut self) -> Result<ImageFit, &'static str> {
        Ok(match self.u8()? {
            0 => ImageFit::Contain,
            1 => ImageFit::Cover,
            2 => ImageFit::Fill,
            _ => return Err("invalid validated image fit"),
        })
    }

    fn utf8(&mut self, len: usize) -> Result<String, &'static str> {
        String::from_utf8(self.take(len)?.to_vec()).map_err(|_| "invalid validated UTF-8")
    }

    fn text(&mut self) -> Result<String, &'static str> {
        let len = self.u32()? as usize;
        self.utf8(len)
    }

    fn family(&mut self) -> Result<String, &'static str> {
        let len = self.u16()? as usize;
        self.utf8(len)
    }

    fn layout(&mut self, runs: bool) -> Result<TextLayout, &'static str> {
        let size = self.f32()?;
        let weight = if runs { 400 } else { self.u16()? };
        let italic = self.u8()? != 0;
        let align = self.u8()?;
        let wrap = self.u8()? != 0;
        let ellipsis = self.u8()? != 0;
        let family = self.family()?;
        Ok(TextLayout {
            size,
            weight,
            italic,
            align,
            wrap,
            ellipsis,
            family,
        })
    }

    fn finish(&self) -> Result<(), &'static str> {
        (self.offset == self.bytes.len())
            .then_some(())
            .ok_or("validated payload has trailing bytes")
    }
}

pub fn prepare(
    list: &ValidatedDrawList<'_>,
    mut image: impl FnMut(u64) -> Option<Image>,
) -> Result<PreparedFrame, &'static str> {
    let mut commands = Vec::with_capacity(list.commands.len());
    for command in &list.commands {
        let mut reader = Reader::new(command.payload);
        let operation = match command.opcode {
            wire::PUSH_CLIP_RECT => DrawOp::PushClipRect(reader.rect()?),
            wire::PUSH_CLIP_ROUND_RECT => DrawOp::PushClipRoundRect(reader.rect()?, reader.f32()?),
            wire::POP_CLIP => DrawOp::PopClip,
            wire::PUSH_TRANSFORM => {
                let mut matrix = [0.0; 6];
                for value in &mut matrix {
                    *value = reader.f32()?;
                }
                DrawOp::PushTransform(matrix)
            }
            wire::POP_TRANSFORM => DrawOp::PopTransform,
            wire::PUSH_ALPHA => DrawOp::PushAlpha(reader.u8()?),
            wire::POP_ALPHA => DrawOp::PopAlpha,
            wire::FILL_RECT => DrawOp::FillRect(reader.rect()?, reader.color()?),
            wire::FILL_ROUND_RECT => {
                DrawOp::FillRoundRect(reader.rect()?, reader.f32()?, reader.color()?)
            }
            wire::FILL_CIRCLE => {
                DrawOp::FillCircle(reader.point()?, reader.f32()?, reader.color()?)
            }
            wire::FILL_CONVEX_POLYGON => {
                let count = reader.u16()?;
                let mut builder = PathBuilder::default();
                for index in 0..count {
                    let point = reader.point()?;
                    if index == 0 {
                        builder.move_to(point);
                    } else {
                        builder.line_to(point);
                    }
                }
                builder.close();
                DrawOp::FillPolygon(builder.detach(), reader.color()?)
            }
            wire::FILL_LINEAR_GRADIENT => {
                let rect = reader.rect()?;
                let angle = reader.f32()?.to_radians();
                let count = reader.u8()?;
                let mut stops = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    stops.push(GradientStop {
                        offset: reader.f32()?,
                        color: reader.color()?,
                    });
                }
                let direction = Vec2::new(angle.sin(), -angle.cos());
                let half =
                    (direction.x.abs() * rect.width() + direction.y.abs() * rect.height()) * 0.5;
                let center = Point::new(rect.center_x(), rect.center_y());
                let from = Point::new(center.x - direction.x * half, center.y - direction.y * half);
                let to = Point::new(center.x + direction.x * half, center.y + direction.y * half);
                DrawOp::FillGradient(rect, from, to, stops)
            }
            wire::STROKE_LINE => {
                let from = reader.point()?;
                let to = reader.point()?;
                DrawOp::StrokeLine(from, to, reader.f32()?, reader.color()?)
            }
            wire::STROKE_ROUND_RECT => DrawOp::StrokeRoundRect(
                reader.rect()?,
                reader.f32()?,
                reader.f32()?,
                reader.color()?,
            ),
            wire::STROKE_ARC => DrawOp::StrokeArc(
                reader.rect()?,
                reader.f32()?,
                reader.f32()?,
                reader.f32()?,
                reader.color()?,
            ),
            wire::DRAW_IMAGE => {
                let id = reader.u64()?;
                let image = image(id).ok_or("validated image is no longer available")?;
                let rect = reader.rect()?;
                let fit = reader.fit()?;
                DrawOp::Image(image, rect, fit, reader.u8()?)
            }
            wire::DRAW_IMAGE_PIXELS => {
                let rect = reader.rect()?;
                let fit = reader.fit()?;
                let alpha = reader.u8()?;
                let width = reader.u32()?;
                let height = reader.u32()?;
                let len = reader.u32()? as usize;
                let rgba = reader.take(len)?;
                let image = Image::from_rgba8(width as i32, height as i32, rgba)
                    .ok_or("inline image could not be decoded")?;
                DrawOp::Image(image, rect, fit, alpha)
            }
            wire::DRAW_TEXT => {
                let rect = reader.rect()?;
                let text = reader.text()?;
                let layout = reader.layout(false)?;
                let color = reader.color()?;
                FontManager::global()
                    .measure_plugin_text(
                        "",
                        layout.size,
                        layout.weight,
                        layout.italic,
                        &layout.family,
                    )
                    .ok_or("text font is unavailable")?;
                DrawOp::Text(text, rect, layout, color)
            }
            wire::DRAW_TEXT_RUNS => {
                let rect = reader.rect()?;
                let layout = reader.layout(true)?;
                let count = reader.u8()?;
                let mut runs = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let text = reader.text()?;
                    let color = reader.color()?;
                    let weight = reader.u16()?;
                    FontManager::global()
                        .measure_plugin_text("", layout.size, weight, layout.italic, &layout.family)
                        .ok_or("text run font is unavailable")?;
                    runs.push(TextRun {
                        text,
                        color,
                        weight,
                    });
                }
                DrawOp::TextRuns(runs, rect, layout)
            }
            wire::DRAW_SHADOW_ROUND_RECT => {
                let rect = reader.rect()?;
                let radius = reader.f32()?;
                let sigma = reader.f32()?;
                let offset = Vec2::new(reader.f32()?, reader.f32()?);
                let color = reader.color()?;
                DrawOp::Shadow(
                    Path::continuous_rounded_rect(rect.offset(offset), radius),
                    sigma,
                    color,
                )
            }
            _ => return Err("unknown validated opcode"),
        };
        reader.finish()?;
        commands.push(operation);
    }
    Ok(PreparedFrame {
        logical_width: list.logical_width,
        logical_height: list.logical_height,
        commands,
    })
}
