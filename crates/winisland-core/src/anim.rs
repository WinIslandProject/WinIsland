use std::collections::HashMap;

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
