use skia_safe::{Path as SkPath, PathBuilder as SkPathBuilder};

use crate::types::{Point, Rect};

/// 矢量路径句柄。构造方式为 `from_svg` 或 `PathBuilder`。
#[derive(Clone, Default)]
pub struct Path {
    inner: SkPath,
}

impl Path {
    /// 解析 SVG path 数据（`d` 属性的语法），失败返回 `None`。
    pub fn from_svg(svg: &str) -> Option<Self> {
        SkPath::from_svg(svg).map(|inner| Self { inner })
    }

    /// 路径包围盒，单位逻辑像素。
    pub fn bounds(&self) -> Rect {
        let bounds = self.inner.bounds();
        Rect::from_ltrb(bounds.left, bounds.top, bounds.right, bounds.bottom)
    }

    /// 路径是否不含任何图元。
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub(crate) fn from_skia(inner: SkPath) -> Self {
        Self { inner }
    }
}

/// 路径构建器，能力对齐 `skia_safe::PathBuilder` 中本项目实际使用到的部分
/// （`move_to` / `line_to` / `cubic_to` / `conic_to` / `close` / `add_rect`）。
#[derive(Default)]
pub struct PathBuilder {
    inner: SkPathBuilder,
}

impl PathBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_to(&mut self, point: Point) -> &mut Self {
        self.inner.move_to((point.x, point.y));
        self
    }

    pub fn line_to(&mut self, point: Point) -> &mut Self {
        self.inner.line_to((point.x, point.y));
        self
    }

    pub fn cubic_to(&mut self, control1: Point, control2: Point, end: Point) -> &mut Self {
        self.inner.cubic_to(
            (control1.x, control1.y),
            (control2.x, control2.y),
            (end.x, end.y),
        );
        self
    }

    /// 有理二次曲线；`weight` 为 conic 权重。
    pub fn conic_to(&mut self, control: Point, end: Point, weight: f32) -> &mut Self {
        self.inner
            .conic_to((control.x, control.y), (end.x, end.y), weight);
        self
    }

    pub fn close(&mut self) -> &mut Self {
        self.inner.close();
        self
    }

    /// 追加一个矩形子路径（顺时针）。
    pub fn add_rect(&mut self, rect: Rect) -> &mut Self {
        self.inner.add_rect(
            skia_safe::Rect::from_ltrb(rect.left, rect.top, rect.right, rect.bottom),
            None,
            None,
        );
        self
    }

    /// 取出累积的路径并把构建器重置为空。
    pub fn detach(&mut self) -> Path {
        Path::from_skia(self.inner.detach())
    }
}
