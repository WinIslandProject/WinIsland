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

/// 语义绘制接口：把本项目自有的值类型翻译成后端调用。
///
/// 线程要求：与所属 `Renderer` 相同，只在渲染线程使用。
/// 生命周期：借用一帧的绘制作用域，不可保存到帧之外。
/// 失败语义：单次绘制不返回错误；后端失败由 `Renderer` 在帧结束时统一记录
/// （"失败即整体不可用"）。
/// 可重入性：方法只借用 `&self`，与后端画布一致；**不维护自己的状态栈**，
/// 因此外部直接操作画布不会让它失同步。
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

    /// 回退到指定保存层级；直接转发后端的 `restore_to_count`。
    pub fn restore_to(&self, count: usize) {
        self.canvas.restore_to_count(count);
    }

    pub fn translate(&self, delta: Vec2) {
        self.canvas.translate((delta.x, delta.y));
    }

    pub fn scale(&self, factor: Vec2) {
        self.canvas.scale((factor.x, factor.y));
    }

    /// 角度单位为**度**，且**不加 12 点钟偏移**（与 `stroke_arc` 的约定不同）。
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

    pub fn clip_round_rect(&self, rect: Rect, radius: Radius) {
        self.canvas
            .clip_rrect(to_skia_rrect(rect, radius), ClipOp::Intersect, true);
    }

    pub fn clip_path(&self, path: &Path) {
        self.canvas
            .clip_path(path.as_skia(), ClipOp::Intersect, true);
    }

    /// 从当前裁剪区中挖去路径（阴影绘制使用）。
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

    /// 画一段圆弧（不画扇形）。`start`/`sweep` 用 12 点钟约定，换算在后端完成。
    pub fn stroke_arc(&self, rect: Rect, start: Angle, sweep: Angle, width: f32, color: Rgba) {
        self.canvas.draw_arc(
            to_skia_rect(rect),
            start.as_degrees() - 90.0,
            sweep.as_degrees(),
            false,
            &stroked(width, color),
        );
    }

    /// 用线性渐变填充矩形。`from`/`to` 为渐变的两个端点（逻辑像素）。
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

    /// 用线性渐变填充圆角矩形（频谱柱共用同一组端点与色标）。
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

    pub fn draw_image(&self, image: &Image, dst: Rect, options: &ImageOptions) {
        let (width, height) = image.dimensions();
        if width <= 0 || height <= 0 {
            return;
        }
        let full = Rect::from_xywh(0.0, 0.0, width as f32, height as f32);
        let source = options.src.unwrap_or(full);
        let (source, dst) = match options.fit {
            ImageFit::Fill => (source, dst),
            ImageFit::Cover => (cover_source(source, dst), dst),
            ImageFit::Contain => {
                let scaled = contain_dst(source, dst);
                (source, scaled)
            }
        };
        let mut paint = skia_safe::Paint::default();
        paint.set_anti_alias(true);
        let sampling = to_skia_sampling(options.sampling);
        match options.constraint {
            SrcConstraint::Fast => {
                self.canvas.draw_image_rect_with_sampling_options(
                    image.as_skia(),
                    Some((
                        &to_skia_rect(source),
                        skia_safe::canvas::SrcRectConstraint::Fast,
                    )),
                    to_skia_rect(dst),
                    sampling,
                    &paint,
                );
            }
            SrcConstraint::Strict => {
                self.canvas.draw_image_rect_with_sampling_options(
                    image.as_skia(),
                    Some((
                        &to_skia_rect(source),
                        skia_safe::canvas::SrcRectConstraint::Strict,
                    )),
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
