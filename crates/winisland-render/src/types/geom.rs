/// 平面上的一个点，单位逻辑像素。
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl From<(f32, f32)> for Point {
    fn from((x, y): (f32, f32)) -> Self {
        Self { x, y }
    }
}

/// 位移或尺寸增量，单位逻辑像素。
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl From<(f32, f32)> for Vec2 {
    fn from((x, y): (f32, f32)) -> Self {
        Self { x, y }
    }
}

/// 轴对齐矩形，字段与 `skia_safe::Rect` 同名同义（`left`/`top` 为左上角，单位逻辑像素）。
///
/// 不变量：不要求 `left <= right`；`is_empty`/`contains` 与 Skia 的行为逐条一致。
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub const fn from_ltrb(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn from_xywh(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        }
    }

    pub const fn width(&self) -> f32 {
        self.right - self.left
    }

    pub const fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub const fn center_x(&self) -> f32 {
        (self.left + self.right) * 0.5
    }

    pub const fn center_y(&self) -> f32 {
        (self.top + self.bottom) * 0.5
    }

    /// 与 `SkRect::contains(SkPoint)` 相同：左闭右开、上闭下开。
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.left && point.x < self.right && point.y >= self.top && point.y < self.bottom
    }

    pub fn contains_rect(&self, other: Rect) -> bool {
        other.left >= self.left
            && other.right <= self.right
            && other.top >= self.top
            && other.bottom <= self.bottom
    }

    pub fn is_empty(&self) -> bool {
        !(self.left < self.right && self.top < self.bottom)
    }

    pub fn is_finite(&self) -> bool {
        self.left.is_finite()
            && self.top.is_finite()
            && self.right.is_finite()
            && self.bottom.is_finite()
    }

    /// 平移后返回新矩形。
    pub fn offset(self, delta: Vec2) -> Self {
        Self {
            left: self.left + delta.x,
            top: self.top + delta.y,
            right: self.right + delta.x,
            bottom: self.bottom + delta.y,
        }
    }

    /// 四边同时内缩/外扩 `delta` 后返回新矩形。
    pub fn inset(self, delta: f32) -> Self {
        Self {
            left: self.left + delta,
            top: self.top + delta,
            right: self.right - delta,
            bottom: self.bottom - delta,
        }
    }

    /// 交集；无交集时返回 `None`（与 `SkRect::intersect` 失败时的行为一致）。
    pub fn intersect(self, other: Rect) -> Option<Self> {
        let result = Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (!result.is_empty()).then_some(result)
    }
}

/// 圆角半径。四角各自可不同，且每角是 `(x, y)` 半轴对，以表达椭圆角。
///
/// 不变量：不做夹取；半径超过半边长时的缩放行为由后端（Skia `RRect`）决定，
/// 预夹取会改变观感，因此禁止在构造时夹取。
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Radius {
    pub top_left: Vec2,
    pub top_right: Vec2,
    pub bottom_right: Vec2,
    pub bottom_left: Vec2,
}

impl Radius {
    pub const ZERO: Self = Self {
        top_left: Vec2::ZERO,
        top_right: Vec2::ZERO,
        bottom_right: Vec2::ZERO,
        bottom_left: Vec2::ZERO,
    };

    /// 四角相同的圆角，`x == y == radius`。
    pub const fn uniform(radius: f32) -> Self {
        Self::uniform_vec(Vec2::new(radius, radius))
    }

    /// 四角相同的椭圆角，`x` 与 `y` 半轴分别给定。
    pub const fn uniform_vec(radius: Vec2) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub fn is_uniform(self) -> bool {
        self.top_left == self.top_right
            && self.top_left == self.bottom_right
            && self.top_left == self.bottom_left
    }
}

/// 角度，单位为度。
///
/// 约定：**0° 指向 12 点钟方向，顺时针为正**。Skia 的 `draw_arc` 以 3 点钟为 0°，
/// 换算 `skia_degrees = as_degrees() - 90.0`。该偏移只适用于弧线起始/扫过角，
/// **不适用于** `skia_safe::Canvas::rotate`（那是普通旋转，单位同样是度，但无偏移）。
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug, Default)]
pub struct Angle(f32);

impl Angle {
    pub const ZERO: Self = Self(0.0);

    pub const fn from_degrees(degrees: f32) -> Self {
        Self(degrees)
    }

    pub const fn as_degrees(self) -> f32 {
        self.0
    }
}
