use std::collections::HashMap;

use skia_safe::{Font, FontStyle as SkFontStyle};

use super::{FONT_MGR, FontManager, get_custom_typeface, text_paint};
use crate::painter::Painter;
use crate::types::{Rect, Rgba};

#[derive(Clone, Copy)]
pub struct PluginTextMetrics {
    pub width: f32,
    pub height: f32,
    pub ascent: f32,
    pub descent: f32,
}

#[derive(Clone, Copy)]
pub struct PluginTextParams<'a> {
    pub painter: Painter<'a>,
    pub rect: Rect,
    pub size: f32,
    pub italic: bool,
    pub family: &'a str,
    pub align: u8,
    pub wrap: bool,
    pub ellipsis: bool,
}

#[derive(Clone, Copy)]
pub struct PluginTextRun<'a> {
    pub text: &'a str,
    pub weight: u16,
    pub color: Rgba,
}

#[derive(Clone, Copy)]
struct Glyph {
    character: char,
    weight: u16,
    color: Rgba,
    width: f32,
}

impl FontManager {
    fn plugin_font(&self, size: f32, weight: u16, italic: bool, family: &str) -> Option<Font> {
        let slant = if italic {
            skia_safe::font_style::Slant::Italic
        } else {
            skia_safe::font_style::Slant::Upright
        };
        let style = SkFontStyle::new(i32::from(weight).into(), 5.into(), slant);
        let typeface = if family.is_empty() {
            get_custom_typeface()
        } else {
            None
        }
        .or_else(|| {
            FONT_MGR.with(|manager| {
                (!family.is_empty())
                    .then(|| manager.match_family_style(family, style))
                    .flatten()
                    .or_else(|| manager.match_family_style("Microsoft YaHei", style))
                    .or_else(|| manager.match_family_style("Segoe UI", style))
                    .or_else(|| manager.legacy_make_typeface(None, style))
            })
        })?;
        let mut font = Font::from_typeface(typeface.clone(), size);
        font.set_subpixel(true);
        if weight >= 600 && *typeface.font_style().weight() < 600 {
            font.set_embolden(true);
        }
        if italic && typeface.font_style().slant() == skia_safe::font_style::Slant::Upright {
            font.set_skew_x(-0.25);
        }
        Some(font)
    }

    pub fn measure_plugin_text(
        &self,
        text: &str,
        size: f32,
        weight: u16,
        italic: bool,
        family: &str,
    ) -> Option<PluginTextMetrics> {
        let font = self.plugin_font(size, weight, italic, family)?;
        let (width, _) = font.measure_str(text, None);
        let (_, metrics) = font.metrics();
        Some(PluginTextMetrics {
            width,
            height: metrics.descent - metrics.ascent,
            ascent: -metrics.ascent,
            descent: metrics.descent,
        })
    }

    pub fn plugin_font_family(&self, index: u32) -> Option<String> {
        FONT_MGR.with(|manager| {
            ((index as usize) < manager.count_families())
                .then(|| manager.family_name(index as usize))
        })
    }

    pub fn draw_plugin_text(
        &self,
        params: PluginTextParams<'_>,
        text: &str,
        weight: u16,
        color: Rgba,
    ) -> bool {
        self.draw_plugin_runs(
            params,
            &[PluginTextRun {
                text,
                weight,
                color,
            }],
        )
    }

    pub fn draw_plugin_runs(
        &self,
        params: PluginTextParams<'_>,
        runs: &[PluginTextRun<'_>],
    ) -> bool {
        if params.rect.width() <= 0.0 || params.rect.height() <= 0.0 {
            return true;
        }
        let Some(base_font) = self.plugin_font(params.size, 400, params.italic, params.family)
        else {
            return false;
        };
        let (_, metrics) = base_font.metrics();
        let line_height = (metrics.descent - metrics.ascent).max(params.size * 1.1);
        let max_lines = ((params.rect.height() / line_height).ceil() as usize).clamp(1, 256);
        let mut fonts = HashMap::new();
        fonts.insert(400_u16, base_font);
        let mut lines: Vec<Vec<Glyph>> = vec![Vec::new()];
        let mut widths = vec![0.0_f32];
        let mut truncated = false;
        'runs: for run in runs {
            if let std::collections::hash_map::Entry::Vacant(entry) = fonts.entry(run.weight) {
                let Some(font) =
                    self.plugin_font(params.size, run.weight, params.italic, params.family)
                else {
                    return false;
                };
                entry.insert(font);
            }
            let font = &fonts[&run.weight];
            for character in run.text.chars() {
                if character == '\n' {
                    if lines.len() >= max_lines {
                        truncated = true;
                        break 'runs;
                    }
                    lines.push(Vec::new());
                    widths.push(0.0);
                    continue;
                }
                let mut buffer = [0_u8; 4];
                let width = font.measure_str(character.encode_utf8(&mut buffer), None).0;
                let index = lines.len() - 1;
                if widths[index] + width > params.rect.width() && !lines[index].is_empty() {
                    if !params.wrap || lines.len() >= max_lines {
                        truncated = true;
                        break 'runs;
                    }
                    lines.push(Vec::new());
                    widths.push(0.0);
                }
                let index = lines.len() - 1;
                widths[index] += width;
                lines[index].push(Glyph {
                    character,
                    weight: run.weight,
                    color: run.color,
                    width,
                });
            }
        }
        if truncated && params.ellipsis {
            let index = lines.len() - 1;
            let weight = lines[index].last().map_or(400, |glyph| glyph.weight);
            let color = lines[index].last().map_or(Rgba::WHITE, |glyph| glyph.color);
            let ellipsis_width = fonts[&weight].measure_str("…", None).0;
            while widths[index] + ellipsis_width > params.rect.width() {
                let Some(glyph) = lines[index].pop() else {
                    break;
                };
                widths[index] -= glyph.width;
            }
            if ellipsis_width <= params.rect.width() {
                lines[index].push(Glyph {
                    character: '…',
                    weight,
                    color,
                    width: ellipsis_width,
                });
                widths[index] += ellipsis_width;
            }
        }
        params.painter.save();
        params.painter.clip_rect(params.rect);
        for (index, line) in lines.iter().enumerate() {
            let mut x = match params.align {
                1 => params.rect.left + (params.rect.width() - widths[index]) * 0.5,
                2 => params.rect.right - widths[index],
                _ => params.rect.left,
            };
            let baseline = params.rect.top - metrics.ascent + index as f32 * line_height;
            let mut start = 0;
            while start < line.len() {
                let weight = line[start].weight;
                let color = line[start].color;
                let mut end = start + 1;
                while end < line.len() && line[end].weight == weight && line[end].color == color {
                    end += 1;
                }
                let value: String = line[start..end]
                    .iter()
                    .map(|glyph| glyph.character)
                    .collect();
                let font = &fonts[&weight];
                params.painter.canvas().draw_str(
                    &value,
                    (x, baseline),
                    font,
                    &text_paint(color, None),
                );
                x += font.measure_str(&value, None).0;
                start = end;
            }
        }
        params.painter.restore();
        true
    }
}
