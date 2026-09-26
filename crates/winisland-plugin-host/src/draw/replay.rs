use std::time::{Duration, Instant};

use winisland_render::text::{FontManager, PluginTextParams, PluginTextRun};
use winisland_render::{
    Angle, BlurSpec, ImageOptions, Painter, Radius, Rect, Rgba, StrokeCap, TileMode,
};

use super::decode::DrawOp;
pub use super::decode::{PreparedFrame, prepare};

enum CanvasState {
    ClipRect(Rect),
    ClipRoundRect(Rect, f32),
    Transform([f32; 6]),
}

impl CanvasState {
    fn apply(&self, painter: Painter<'_>) {
        painter.save();
        match self {
            Self::ClipRect(rect) => painter.clip_rect(*rect),
            Self::ClipRoundRect(rect, radius) => {
                painter.clip_round_rect(*rect, Radius::uniform(*radius))
            }
            Self::Transform(matrix) => painter.concat_affine(*matrix),
        }
    }
}

fn pop_canvas_state(states: &mut Vec<CanvasState>, clip: bool, painter: Painter<'_>, base: usize) {
    let index = states.iter().rposition(|state| {
        if clip {
            matches!(
                state,
                CanvasState::ClipRect(_) | CanvasState::ClipRoundRect(_, _)
            )
        } else {
            matches!(state, CanvasState::Transform(_))
        }
    });
    let Some(index) = index else { return };
    let was_last = index + 1 == states.len();
    states.remove(index);
    if was_last {
        painter.restore();
    } else {
        painter.restore_to(base);
        for state in states.iter() {
            state.apply(painter);
        }
    }
}

fn color_with_alpha(color: Rgba, alpha: f32) -> Rgba {
    color.with_alpha(((color.a() as f32) * alpha).round().clamp(0.0, 255.0) as u8)
}

fn text_params<'a>(
    painter: Painter<'a>,
    rect: Rect,
    layout: &'a super::decode::TextLayout,
) -> PluginTextParams<'a> {
    PluginTextParams {
        painter,
        rect,
        size: layout.size,
        italic: layout.italic,
        family: &layout.family,
        align: layout.align,
        wrap: layout.wrap,
        ellipsis: layout.ellipsis,
    }
}

pub fn replay(frame: &PreparedFrame, painter: Painter<'_>, base_alpha: u8) -> Duration {
    let started = Instant::now();
    let base = painter.save();
    let mut canvas_states = Vec::new();
    let mut alpha_stack = vec![base_alpha as f32 / 255.0];
    for command in &frame.commands {
        let alpha = *alpha_stack.last().unwrap_or(&1.0);
        match command {
            DrawOp::PushClipRect(rect) => {
                let state = CanvasState::ClipRect(*rect);
                state.apply(painter);
                canvas_states.push(state);
            }
            DrawOp::PushClipRoundRect(rect, radius) => {
                let state = CanvasState::ClipRoundRect(*rect, *radius);
                state.apply(painter);
                canvas_states.push(state);
            }
            DrawOp::PopClip => pop_canvas_state(&mut canvas_states, true, painter, base),
            DrawOp::PushTransform(matrix) => {
                let state = CanvasState::Transform(*matrix);
                state.apply(painter);
                canvas_states.push(state);
            }
            DrawOp::PopTransform => pop_canvas_state(&mut canvas_states, false, painter, base),
            DrawOp::PushAlpha(value) => alpha_stack.push(alpha * (*value as f32 / 255.0)),
            DrawOp::PopAlpha => {
                alpha_stack.pop();
            }
            DrawOp::FillRect(rect, color) => {
                painter.fill_rect(*rect, color_with_alpha(*color, alpha))
            }
            DrawOp::FillRoundRect(rect, radius, color) => painter.fill_round_rect(
                *rect,
                Radius::uniform(*radius),
                color_with_alpha(*color, alpha),
            ),
            DrawOp::FillCircle(center, radius, color) => {
                painter.fill_circle(*center, *radius, color_with_alpha(*color, alpha));
            }
            DrawOp::FillPolygon(path, color) => {
                painter.fill_path(path, color_with_alpha(*color, alpha))
            }
            DrawOp::FillGradient(rect, from, to, stops) => {
                let stops = stops
                    .iter()
                    .map(|stop| winisland_render::GradientStop {
                        offset: stop.offset,
                        color: color_with_alpha(stop.color, alpha),
                    })
                    .collect::<Vec<_>>();
                painter.fill_rect_with_gradient(*rect, *from, *to, &stops, TileMode::Clamp);
            }
            DrawOp::StrokeLine(from, to, width, color) => painter.stroke_line(
                *from,
                *to,
                *width,
                color_with_alpha(*color, alpha),
                StrokeCap::Butt,
            ),
            DrawOp::StrokeRoundRect(rect, radius, width, color) => painter.stroke_round_rect(
                *rect,
                Radius::uniform(*radius),
                *width,
                color_with_alpha(*color, alpha),
            ),
            DrawOp::StrokeArc(rect, start, sweep, width, color) => painter.stroke_arc(
                *rect,
                Angle::from_degrees(*start),
                Angle::from_degrees(*sweep),
                *width,
                color_with_alpha(*color, alpha),
                StrokeCap::Butt,
            ),
            DrawOp::Image(image, rect, fit, image_alpha) => {
                let effective_alpha =
                    ((*image_alpha as f32) * alpha).round().clamp(0.0, 255.0) as u8;
                let options = ImageOptions::default()
                    .with_fit(*fit)
                    .with_alpha(effective_alpha);
                painter.draw_image(image, *rect, &options);
            }
            DrawOp::Text(value, rect, layout, color) => {
                let _ = FontManager::global().draw_plugin_text(
                    text_params(painter, *rect, layout),
                    value,
                    layout.weight,
                    color_with_alpha(*color, alpha),
                );
            }
            DrawOp::TextRuns(runs, rect, layout) => {
                let runs = runs
                    .iter()
                    .map(|run| PluginTextRun {
                        text: &run.text,
                        weight: run.weight,
                        color: color_with_alpha(run.color, alpha),
                    })
                    .collect::<Vec<_>>();
                let _ = FontManager::global()
                    .draw_plugin_runs(text_params(painter, *rect, layout), &runs);
            }
            DrawOp::Shadow(path, sigma, color) => painter.fill_path_blurred(
                path,
                color_with_alpha(*color, alpha),
                BlurSpec::uniform(*sigma),
            ),
        }
    }
    painter.restore_to(base);
    started.elapsed()
}
