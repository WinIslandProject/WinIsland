use skia_safe::{
    Color, Color4f, FilterMode, MipmapMode, Paint, RRect, Rect as SkRect, SamplingOptions,
    TileMode as SkTileMode, gradient, paint,
};

use crate::types::{
    GradientStop, Point, Radius, Rect, Rgba, Sampling, StrokeCap, StrokeJoin, TileMode,
};

pub(crate) fn to_skia_color(color: Rgba) -> Color {
    Color::from_argb(color.a(), color.r(), color.g(), color.b())
}

pub(crate) fn to_skia_rect(rect: Rect) -> SkRect {
    SkRect::from_ltrb(rect.left, rect.top, rect.right, rect.bottom)
}

pub(crate) fn to_skia_point(point: Point) -> skia_safe::Point {
    skia_safe::Point::new(point.x, point.y)
}

pub(crate) fn to_skia_cap(cap: StrokeCap) -> paint::Cap {
    match cap {
        StrokeCap::Butt => paint::Cap::Butt,
        StrokeCap::Round => paint::Cap::Round,
        StrokeCap::Square => paint::Cap::Square,
    }
}

pub(crate) fn to_skia_join(join: StrokeJoin) -> paint::Join {
    match join {
        StrokeJoin::Miter => paint::Join::Miter,
        StrokeJoin::Round => paint::Join::Round,
        StrokeJoin::Bevel => paint::Join::Bevel,
    }
}

pub(crate) fn to_skia_tile_mode(tile: TileMode) -> SkTileMode {
    match tile {
        TileMode::Clamp => SkTileMode::Clamp,
        TileMode::Mirror => SkTileMode::Mirror,
        TileMode::Repeat => SkTileMode::Repeat,
        TileMode::Decal => SkTileMode::Decal,
    }
}

pub(crate) fn to_skia_sampling(sampling: Sampling) -> SamplingOptions {
    match sampling {
        Sampling::Default => SamplingOptions::default(),
        Sampling::LinearNone => SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
        Sampling::LinearLinear => SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
    }
}

pub(crate) fn to_skia_rrect(rect: Rect, radius: Radius) -> RRect {
    let sk_rect = to_skia_rect(rect);
    if radius.is_uniform() {
        return RRect::new_rect_xy(sk_rect, radius.top_left.x, radius.top_left.y);
    }
    let radii = [
        skia_safe::Vector::new(radius.top_left.x, radius.top_left.y),
        skia_safe::Vector::new(radius.top_right.x, radius.top_right.y),
        skia_safe::Vector::new(radius.bottom_right.x, radius.bottom_right.y),
        skia_safe::Vector::new(radius.bottom_left.x, radius.bottom_left.y),
    ];
    RRect::new_rect_radii(sk_rect, &radii)
}

pub(crate) fn filled(color: Rgba) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(paint::Style::Fill);
    paint.set_color(to_skia_color(color));
    paint
}

pub(crate) fn stroked(width: f32, color: Rgba) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(paint::Style::Stroke);
    paint.set_stroke_width(width);
    paint.set_color(to_skia_color(color));
    paint
}

pub(crate) fn to_skia_gradient(
    from: Point,
    to: Point,
    stops: &[GradientStop],
    tile: TileMode,
) -> Option<skia_safe::Shader> {
    if stops.is_empty() {
        return None;
    }
    let colors = stops
        .iter()
        .map(|stop| Color4f::from(to_skia_color(stop.color)))
        .collect::<Vec<_>>();
    let evenly_spaced = stops.iter().enumerate().all(|(index, stop)| {
        let expected = index as f32 / (stops.len() - 1).max(1) as f32;
        (stop.offset - expected).abs() <= f32::EPSILON
    });
    let positions =
        (!evenly_spaced).then(|| stops.iter().map(|stop| stop.offset).collect::<Vec<_>>());
    let tile = to_skia_tile_mode(tile);
    let colors = match positions.as_deref() {
        Some(positions) => gradient::Colors::new(&colors, Some(positions), tile, None),
        None => gradient::Colors::new_evenly_spaced(&colors, tile, None),
    };
    let gradient = gradient::Gradient::new(colors, gradient::Interpolation::default());
    gradient::shaders::linear_gradient((to_skia_point(from), to_skia_point(to)), &gradient, None)
}
