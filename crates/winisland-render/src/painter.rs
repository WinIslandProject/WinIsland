use skia_safe::{Canvas, ClipOp, image_filters};

use crate::convert::{
    filled, stroked, to_skia_cap, to_skia_gradient, to_skia_join, to_skia_point, to_skia_rect,
    to_skia_rrect, to_skia_sampling, to_skia_tile_mode,
};
use crate::image::Image;
use crate::path::Path;
use crate::types::{
    Angle, BlurSpec, GradientStop, ImageFit, ImageOptions, LayerSpec, Point, Radius, Rect, Rgba,
    SrcConstraint, StrokeCap, StrokeJoin, TileMode, Vec2,
};

#[derive(Clone, Copy)]
pub struct Painter<'a> {
    pub(crate) canvas: &'a Canvas,
}

impl<'a> Painter<'a> {
    pub(crate) fn canvas(&self) -> &'a Canvas {
        self.canvas
    }

    pub fn save(&self) -> usize {
        self.canvas.save()
    }

    pub fn restore(&self) {
        self.canvas.restore();
    }

    pub fn restore_to(&self, count: usize) {
        self.canvas.restore_to_count(count);
    }

    pub fn translate(&self, delta: Vec2) {
        self.canvas.translate((delta.x, delta.y));
    }

    pub fn scale(&self, factor: Vec2) {
        self.canvas.scale((factor.x, factor.y));
    }

    pub fn rotate_degrees(&self, degrees: f32) {
        self.canvas.rotate(degrees, None);
    }

    pub fn skew(&self, delta: Vec2) {
        self.canvas.skew((delta.x, delta.y));
    }

    pub fn reset_matrix(&self) {
        self.canvas.reset_matrix();
    }

    pub fn clip_rect(&self, rect: Rect) {
        self.canvas
            .clip_rect(to_skia_rect(rect), ClipOp::Intersect, true);
    }

    pub fn clip_rect_with_anti_alias(&self, rect: Rect, anti_alias: bool) {
        self.canvas
            .clip_rect(to_skia_rect(rect), ClipOp::Intersect, anti_alias);
    }

    pub fn clip_round_rect(&self, rect: Rect, radius: Radius) {
        self.canvas
            .clip_rrect(to_skia_rrect(rect, radius), ClipOp::Intersect, true);
    }

    pub fn clip_path(&self, path: &Path) {
        self.canvas
            .clip_path(path.as_skia(), ClipOp::Intersect, true);
    }

    pub fn clip_path_difference(&self, path: &Path) {
        self.canvas
            .clip_path(path.as_skia(), ClipOp::Difference, true);
    }

    pub fn fill_rect(&self, rect: Rect, color: Rgba) {
        self.canvas.draw_rect(to_skia_rect(rect), &filled(color));
    }

    pub fn stroke_rect(&self, rect: Rect, width: f32, color: Rgba) {
        self.canvas
            .draw_rect(to_skia_rect(rect), &stroked(width, color));
    }

    pub fn fill_round_rect(&self, rect: Rect, radius: Radius, color: Rgba) {
        self.canvas
            .draw_rrect(to_skia_rrect(rect, radius), &filled(color));
    }

    pub fn stroke_round_rect(&self, rect: Rect, radius: Radius, width: f32, color: Rgba) {
        self.canvas
            .draw_rrect(to_skia_rrect(rect, radius), &stroked(width, color));
    }

    pub fn fill_circle(&self, center: Point, radius: f32, color: Rgba) {
        self.canvas
            .draw_circle(to_skia_point(center), radius, &filled(color));
    }

    pub fn stroke_circle(&self, center: Point, radius: f32, width: f32, color: Rgba) {
        self.canvas
            .draw_circle(to_skia_point(center), radius, &stroked(width, color));
    }

    pub fn fill_oval(&self, rect: Rect, color: Rgba) {
        self.canvas.draw_oval(to_skia_rect(rect), &filled(color));
    }

    pub fn stroke_oval(&self, rect: Rect, width: f32, color: Rgba) {
        self.canvas
            .draw_oval(to_skia_rect(rect), &stroked(width, color));
    }

    pub fn stroke_line(&self, from: Point, to: Point, width: f32, color: Rgba, cap: StrokeCap) {
        let mut paint = stroked(width, color);
        paint.set_stroke_cap(to_skia_cap(cap));
        self.canvas
            .draw_line(to_skia_point(from), to_skia_point(to), &paint);
    }

    pub fn fill_path(&self, path: &Path, color: Rgba) {
        self.canvas.draw_path(path.as_skia(), &filled(color));
    }

    pub fn stroke_path(
        &self,
        path: &Path,
        width: f32,
        color: Rgba,
        cap: StrokeCap,
        join: StrokeJoin,
    ) {
        let mut paint = stroked(width, color);
        paint.set_stroke_cap(to_skia_cap(cap));
        paint.set_stroke_join(to_skia_join(join));
        self.canvas.draw_path(path.as_skia(), &paint);
    }

    pub fn stroke_arc(
        &self,
        rect: Rect,
        start: Angle,
        sweep: Angle,
        width: f32,
        color: Rgba,
        cap: StrokeCap,
    ) {
        let mut paint = stroked(width, color);
        paint.set_stroke_cap(to_skia_cap(cap));
        self.canvas.draw_arc(
            to_skia_rect(rect),
            start.as_degrees() - 90.0,
            sweep.as_degrees(),
            false,
            &paint,
        );
    }

    pub fn fill_rect_with_gradient(
        &self,
        rect: Rect,
        from: Point,
        to: Point,
        stops: &[GradientStop],
        tile: TileMode,
    ) {
        let Some(paint) = gradient_paint(from, to, stops, tile) else {
            return;
        };
        self.canvas.draw_rect(to_skia_rect(rect), &paint);
    }

    pub fn fill_round_rect_with_gradient(
        &self,
        rect: Rect,
        radius: Radius,
        from: Point,
        to: Point,
        stops: &[GradientStop],
        tile: TileMode,
    ) {
        let Some(paint) = gradient_paint(from, to, stops, tile) else {
            return;
        };
        self.canvas.draw_rrect(to_skia_rrect(rect, radius), &paint);
    }

    pub fn fill_path_with_gradient(
        &self,
        path: &Path,
        from: Point,
        to: Point,
        stops: &[GradientStop],
        tile: TileMode,
    ) -> bool {
        let Some(paint) = gradient_paint(from, to, stops, tile) else {
            return false;
        };
        self.canvas.draw_path(path.as_skia(), &paint);
        true
    }

    pub fn axis_scale(&self) -> f32 {
        let matrix = self.canvas.local_to_device_as_3x3();
        matrix.scale_x().abs().max(matrix.scale_y().abs())
    }

    pub fn surface_size(&self) -> (i32, i32) {
        let info = self.canvas.image_info();
        (info.width(), info.height())
    }

    pub fn fill_path_blurred(&self, path: &Path, color: Rgba, blur: BlurSpec) {
        let mut paint = filled(color);
        if let Some(filter) =
            image_filters::blur(blur.sigma, blur.tile.map(to_skia_tile_mode), None, None)
        {
            paint.set_image_filter(filter);
        }
        self.canvas.draw_path(path.as_skia(), &paint);
    }

    pub fn draw_image_at(&self, image: &Image, position: Point) {
        self.canvas
            .draw_image(image.as_skia(), to_skia_point(position), None);
    }

    pub fn plugin_canvas_handle_v1(&self) -> *mut std::ffi::c_void {
        self.canvas as *const Canvas as *mut std::ffi::c_void
    }

    pub fn draw_image(&self, image: &Image, dst: Rect, options: &ImageOptions) {
        let (width, height) = image.dimensions();
        if width <= 0 || height <= 0 {
            return;
        }
        let full = Rect::from_xywh(0.0, 0.0, width as f32, height as f32);
        let source = options.src.unwrap_or(full);
        let (source, dst) = match options.fit {
            ImageFit::Fill => (options.src, dst),
            ImageFit::Cover => (Some(cover_source(source, dst)), dst),
            ImageFit::Contain => {
                let scaled = contain_dst(source, dst);
                (options.src, scaled)
            }
        };
        let mut paint = skia_safe::Paint::default();
        paint.set_anti_alias(options.anti_alias);
        if let Some(alpha) = options.alpha_f {
            paint.set_alpha_f(alpha);
        } else {
            paint.set_alpha(options.alpha);
        }
        let sampling = to_skia_sampling(options.sampling);
        let source = source.map(to_skia_rect);
        match options.constraint {
            SrcConstraint::Fast => {
                self.canvas.draw_image_rect_with_sampling_options(
                    image.as_skia(),
                    source
                        .as_ref()
                        .map(|rect| (rect, skia_safe::canvas::SrcRectConstraint::Fast)),
                    to_skia_rect(dst),
                    sampling,
                    &paint,
                );
            }
            SrcConstraint::Strict => {
                self.canvas.draw_image_rect_with_sampling_options(
                    image.as_skia(),
                    source
                        .as_ref()
                        .map(|rect| (rect, skia_safe::canvas::SrcRectConstraint::Strict)),
                    to_skia_rect(dst),
                    sampling,
                    &paint,
                );
            }
        }
    }

    pub fn begin_layer(&self, spec: LayerSpec) {
        match spec {
            LayerSpec::Blur(BlurSpec { sigma, tile }) => {
                let mut paint = skia_safe::Paint::default();
                if let Some(filter) =
                    image_filters::blur(sigma, tile.map(to_skia_tile_mode), None, None)
                {
                    paint.set_image_filter(filter);
                }
                self.canvas
                    .save_layer(&skia_safe::canvas::SaveLayerRec::default().paint(&paint));
            }
        }
    }

    pub fn end_layer(&self) {
        self.canvas.restore();
    }
}

fn gradient_paint(
    from: Point,
    to: Point,
    stops: &[GradientStop],
    tile: TileMode,
) -> Option<skia_safe::Paint> {
    let shader = to_skia_gradient(from, to, stops, tile)?;
    let mut paint = skia_safe::Paint::default();
    paint.set_anti_alias(true);
    paint.set_shader(shader);
    Some(paint)
}

fn cover_source(source: Rect, dst: Rect) -> Rect {
    if source.width() <= 0.0 || source.height() <= 0.0 || dst.width() <= 0.0 || dst.height() <= 0.0
    {
        return source;
    }
    let source_aspect = source.width() / source.height();
    let dst_aspect = dst.width() / dst.height();
    if source_aspect > dst_aspect {
        let crop = source.height() * dst_aspect;
        let offset = (source.width() - crop) / 2.0;
        Rect::from_xywh(source.left + offset, source.top, crop, source.height())
    } else {
        let crop = source.width() / dst_aspect;
        let offset = (source.height() - crop) / 2.0;
        Rect::from_xywh(source.left, source.top + offset, source.width(), crop)
    }
}

fn contain_dst(source: Rect, dst: Rect) -> Rect {
    if source.width() <= 0.0 || source.height() <= 0.0 || dst.width() <= 0.0 || dst.height() <= 0.0
    {
        return dst;
    }
    let scale = (dst.width() / source.width()).min(dst.height() / source.height());
    let width = source.width() * scale;
    let height = source.height() * scale;
    Rect::from_xywh(
        dst.center_x() - width / 2.0,
        dst.center_y() - height / 2.0,
        width,
        height,
    )
}
