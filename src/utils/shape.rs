// Copyright 2013 The Flutter Authors. All rights reserved.
// Rounded-superellipse geometry adapted from Flutter under the BSD-3-Clause license.
// See resources/licenses/flutter.txt.

use std::cell::RefCell;
use std::collections::VecDeque;

use skia_safe::{Path, PathBuilder, Rect};

const PATH_CACHE_CAPACITY: usize = 64;
type CurvePoint = (f64, f64);

struct Octant {
    points: [CurvePoint; 8],
    weights: [f32; 2],
}

fn interpolate(table: &[(f64, f64)], position: f64) -> (f64, f64) {
    let position = position.clamp(0.0, (table.len() - 1) as f64);
    let index = (position.floor() as usize).min(table.len() - 2);
    let fraction = position - index as f64;
    (
        table[index].0 + (table[index + 1].0 - table[index].0) * fraction,
        table[index].1 + (table[index + 1].1 - table[index].1) * fraction,
    )
}

fn superellipse_parameters(ratio: f64) -> (f64, f64) {
    const PARAMETERS: [(f64, f64); 11] = [
        (2.0, 1.13276676),
        (2.18349805, 1.20311921),
        (2.33888662, 1.28698796),
        (2.48660575, 1.36351941),
        (2.62226596, 1.44717976),
        (2.75148990, 1.53385819),
        (3.36298265, 1.98288283),
        (4.08649929, 2.23811846),
        (4.85481134, 2.47563463),
        (5.62945551, 2.72948597),
        (6.43023796, 2.98020421),
    ];
    let (exponent, factor) = if ratio > 5.0 {
        (
            6.43023796 + 1.559599389 * (ratio - 5.0),
            2.98020421 + 0.522807185 * (ratio - 5.0),
        )
    } else {
        let position = if ratio < 2.5 {
            (ratio - 2.0) * 10.0
        } else {
            5.0 + (ratio - 2.5) * 2.0
        };
        interpolate(&PARAMETERS, position)
    };
    (exponent, 1.0 - 1.0 / factor)
}

fn tangent_intersection(a: CurvePoint, slope_a: f64, b: CurvePoint, slope_b: f64) -> CurvePoint {
    if (slope_a - slope_b).abs() < 1e-5 {
        return ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    }
    let x = (slope_a * a.0 - slope_b * b.0 + b.1 - a.1) / (slope_a - slope_b);
    (x, slope_a * (x - a.0) + a.1)
}

impl Octant {
    fn new(a: f64, radius: f64) -> Self {
        const BEZIER_FACTORS: [(f64, f64); 13] = [
            (0.7078, 8.3194),
            (0.7895, 2.4523),
            (0.8379, 1.8528),
            (0.8701, 1.6891),
            (0.8932, 1.5806),
            (0.9107, 1.5043),
            (0.9244, 1.4470),
            (0.9355, 1.4037),
            (0.9448, 1.3701),
            (0.9526, 1.3431),
            (0.9594, 1.3212),
            (0.9653, 1.3032),
            (0.9705, 1.2880),
        ];
        let (n, x_ratio) = superellipse_parameters(2.0 * a / radius);
        let start = (0.0, a);
        let join = (x_ratio * a, (1.0 - x_ratio.powf(n)).powf(1.0 / n) * a);
        let gap = (1.0 - std::f64::consts::FRAC_1_SQRT_2) * radius;
        let end = (a - gap, a - gap);
        let slope = (join.0 / join.1).powf(n - 1.0);
        let d = (join.0 - slope * join.1) / (1.0 - slope);
        let circle_radius = (a - d - gap) * std::f64::consts::SQRT_2;
        let delta = (end.0 - join.0, end.1 - join.1);
        let chord = delta.0.hypot(delta.1);
        let distance = (circle_radius * circle_radius - chord * chord / 4.0)
            .max(0.0)
            .sqrt();
        let center = (
            (join.0 + end.0) / 2.0 + delta.1 / chord * distance,
            (join.1 + end.1) / 2.0 - delta.0 / chord * distance,
        );
        let v0 = (join.0 - center.0, join.1 - center.1);
        let v1 = (end.0 - center.0, end.1 - center.1);
        let angle = (v1.0 * v0.1 - v1.1 * v0.0).atan2(v1.0 * v0.0 + v1.1 * v0.1);
        let factor = (angle / 4.0).tan() * 4.0 / 3.0;
        let c1 = (join.0 + v0.1 * factor, join.1 - v0.0 * factor);
        let c2 = (end.0 - v1.1 * factor, end.1 + v1.0 * factor);
        let limited_n = n.min(14.0);
        let (weight1, weight2) = interpolate(&BEZIER_FACTORS, limited_n - 2.0);
        let root_n = limited_n.sqrt();
        let y_mid = (root_n + join.1 / a) / (root_n + 1.0);
        let midpoint = ((1.0 - y_mid.powf(n)).powf(1.0 / n) * a, y_mid * a);
        let mid_slope = -(midpoint.0 / midpoint.1).powf(n - 1.0);
        Self {
            points: [
                start,
                tangent_intersection(start, 0.0, midpoint, mid_slope),
                midpoint,
                tangent_intersection(midpoint, mid_slope, join, -slope),
                join,
                c1,
                c2,
                end,
            ],
            weights: [(weight1 * root_n) as f32, (weight2 * x_ratio) as f32],
        }
    }

    fn append(
        &self,
        builder: &mut PathBuilder,
        map: impl Fn(CurvePoint) -> (f32, f32),
        reverse: bool,
    ) {
        let [start, c1, midpoint, c2, join, arc1, arc2, end] = self.points;
        if reverse {
            builder.cubic_to(map(arc2), map(arc1), map(join));
            builder.conic_to(map(c2), map(midpoint), self.weights[1]);
            builder.conic_to(map(c1), map(start), self.weights[0]);
        } else {
            builder.conic_to(map(c1), map(midpoint), self.weights[0]);
            builder.conic_to(map(c2), map(join), self.weights[1]);
            builder.cubic_to(map(arc1), map(arc2), map(end));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PathKey {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    radius: u32,
}

struct CachedPath {
    key: PathKey,
    path: Path,
}

thread_local! {
    static PATH_CACHE: RefCell<VecDeque<CachedPath>> = const { RefCell::new(VecDeque::new()) };
}

pub(crate) fn expanded_island_radius(w: f32, h: f32, scale: f32) -> f32 {
    (48.0 * scale).min(w / 2.0).min(h / 2.0).max(0.0)
}

pub(crate) fn continuous_rounded_rect_path(rect: Rect, radius: f32) -> Path {
    if !rect.is_finite() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return Path::default();
    }

    let radius = radius
        .max(0.0)
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0);
    let key = PathKey {
        left: rect.left().to_bits(),
        top: rect.top().to_bits(),
        right: rect.right().to_bits(),
        bottom: rect.bottom().to_bits(),
        radius: radius.to_bits(),
    };
    if let Some(path) = PATH_CACHE.with(|cache| {
        cache
            .borrow()
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.path.clone())
    }) {
        return path;
    }

    let mut builder = PathBuilder::new();
    if radius == 0.0 {
        builder.add_rect(rect, None, None);
    } else {
        let half_w = rect.width() as f64 / 2.0;
        let half_h = rect.height() as f64 / 2.0;
        let top = Octant::new(half_w, radius as f64);
        let right = Octant::new(half_h, radius as f64);
        let difference = half_w - half_h;
        builder.move_to((rect.center_x(), rect.bottom()));
        for (sx, sy, reverse) in [
            (1.0, 1.0, false),
            (1.0, -1.0, true),
            (-1.0, -1.0, false),
            (-1.0, 1.0, true),
        ] {
            let top_point = |p: CurvePoint| {
                (
                    rect.center_x() + (sx * p.0) as f32,
                    rect.center_y() + (sy * (p.1 - difference)) as f32,
                )
            };
            let right_point = |p: CurvePoint| {
                (
                    rect.center_x() + (sx * (p.1 + difference)) as f32,
                    rect.center_y() + (sy * p.0) as f32,
                )
            };
            if reverse {
                right.append(&mut builder, right_point, false);
                top.append(&mut builder, top_point, true);
            } else {
                top.append(&mut builder, top_point, false);
                right.append(&mut builder, right_point, true);
            }
        }
        builder.close();
    }

    let path = builder.detach();
    PATH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.push_front(CachedPath {
            key,
            path: path.clone(),
        });
        cache.truncate(PATH_CACHE_CAPACITY);
    });
    path
}
