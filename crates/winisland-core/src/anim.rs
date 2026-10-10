use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub enum Curve {
    Ease,
    Rebound,
    EaseIn,
    EaseInRebound,
}

pub struct Tween {
    value: f32,
    from: f32,
    target: f32,
    started: Option<Instant>,
    duration: Duration,
    curve: Curve,
}

impl Tween {
    pub fn new(value: f32) -> Self {
        let value = if value.is_finite() { value } else { 0.0 };
        Self {
            value,
            from: value,
            target: value,
            started: None,
            duration: Duration::ZERO,
            curve: Curve::Ease,
        }
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn retarget(&mut self, target: f32, duration: Duration, curve: Curve, now: Instant) {
        if !target.is_finite()
            || self.target == target && (self.started.is_none() || self.duration <= duration)
        {
            return;
        }
        self.update(now);
        self.from = self.value;
        self.target = target;
        self.started = Some(now);
        self.duration = duration.max(Duration::from_millis(1));
        self.curve = curve;
    }

    pub fn update(&mut self, now: Instant) -> bool {
        let Some(started) = self.started else {
            return false;
        };
        let before = self.value;
        let time =
            now.saturating_duration_since(started).as_secs_f32() / self.duration.as_secs_f32();
        if time >= 1.0 {
            self.value = self.target;
            self.started = None;
        } else {
            let (first, second) = match self.curve {
                Curve::EaseIn | Curve::EaseInRebound => ((0.40, 0.12), (0.60, 1.0)),
                Curve::Ease => ((0.55, 0.0), (0.22, 1.0)),
                Curve::Rebound => ((0.58, 0.0), (0.78, 1.0)),
            };
            let motion_time = if matches!(self.curve, Curve::EaseInRebound) {
                if time <= 0.72 {
                    time * (0.885 / 0.72)
                } else {
                    0.885 + (time - 0.72) * (0.115 / 0.28)
                }
            } else {
                time
            };
            let travel_time = if matches!(self.curve, Curve::EaseInRebound) {
                let acceleration = ((motion_time - 0.45) / 0.55).clamp(0.0, 1.0);
                (motion_time + 0.04 * acceleration * acceleration * (3.0 - 2.0 * acceleration))
                    .min(1.0)
            } else {
                motion_time.clamp(0.0, 1.0)
            };
            let eased = cubic_bezier(travel_time, first, second);
            let progress = match self.curve {
                Curve::Ease | Curve::EaseIn => eased,
                Curve::EaseInRebound => {
                    let rebound = ((motion_time - 0.75) / 0.25).clamp(0.0, 1.0);
                    eased + 0.10 * (std::f32::consts::PI * rebound).sin().powi(2)
                }
                Curve::Rebound => {
                    let tail = eased - 1.0;
                    1.0 + 2.1 * tail.powi(3) + 1.1 * tail.powi(2)
                }
            };
            self.value = self.from + (self.target - self.from) * progress;
        }
        self.value != before
    }

    pub fn is_animating(&self) -> bool {
        self.started.is_some()
    }
}

fn cubic_bezier(time: f32, first: (f32, f32), second: (f32, f32)) -> f32 {
    if time <= 0.0 {
        return 0.0;
    }
    if time >= 1.0 {
        return 1.0;
    }
    let sample = |t: f32, p1: f32, p2: f32| {
        3.0 * (1.0 - t).powi(2) * t * p1 + 3.0 * (1.0 - t) * t * t * p2 + t.powi(3)
    };
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..14 {
        let mid = (low + high) * 0.5;
        if sample(mid, first.0, second.0) < time {
            low = mid
        } else {
            high = mid
        }
    }
    sample((low + high) * 0.5, first.1, second.1)
}

struct AnimValue {
    value: f32,
    target: f32,
    speed: f32,
}

pub struct AnimPool {
    values: HashMap<u64, AnimValue>,
    default_speed: f32,
}

impl Default for AnimPool {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimPool {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            default_speed: 0.15,
        }
    }

    pub fn set(&mut self, key: u64, target: f32) {
        let speed = self.default_speed;
        self.set_with_speed(key, target, speed);
    }

    pub fn set_with_speed(&mut self, key: u64, target: f32, speed: f32) {
        if let Some(v) = self.values.get_mut(&key) {
            v.target = target;
            v.speed = speed;
        } else {
            self.values.insert(
                key,
                AnimValue {
                    value: 0.0,
                    target,
                    speed,
                },
            );
        }
    }

    pub fn snap(&mut self, key: u64, value: f32) {
        let speed = self.default_speed;
        let entry = self.values.entry(key).or_insert(AnimValue {
            value,
            target: value,
            speed,
        });
        entry.value = value;
        entry.target = value;
    }

    pub fn try_get(&self, key: u64) -> Option<f32> {
        self.values.get(&key).map(|v| v.value)
    }

    pub fn get(&self, key: u64) -> f32 {
        self.values.get(&key).map(|v| v.value).unwrap_or(0.0)
    }

    pub fn tick(&mut self) -> bool {
        let mut changed = false;
        for v in self.values.values_mut() {
            let diff = v.target - v.value;
            if diff.abs() > 0.005 {
                v.value += diff * v.speed;
                changed = true;
            } else if (v.value - v.target).abs() > f32::EPSILON {
                v.value = v.target;
                changed = true;
            }
        }
        changed
    }

    pub fn is_animating(&self) -> bool {
        for v in self.values.values() {
            if (v.target - v.value).abs() > 0.005 {
                return true;
            }
        }
        false
    }
}
