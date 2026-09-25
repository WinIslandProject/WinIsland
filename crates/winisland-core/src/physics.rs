pub struct Spring {
    pub value: f32,
    pub velocity: f32,
}

impl Spring {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
        }
    }
    pub fn update_dt(&mut self, target: f32, stiffness: f32, damping: f32, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let force = (target - self.value) * stiffness * dt;
        self.velocity = (self.velocity + force) * damping.powf(dt);
        self.value += self.velocity * dt;
        if !self.value.is_finite() {
            self.value = target;
            self.velocity = 0.0;
        }
        if !self.velocity.is_finite() {
            self.velocity = 0.0;
        }
    }

    pub fn redirect_velocity_towards(&mut self, target: f32) {
        const MOMENTUM_RETENTION: f32 = 0.35;
        const MAX_DISTANCE_RATIO: f32 = 0.2;

        let distance = target - self.value;
        if distance.abs() <= f32::EPSILON {
            self.velocity = 0.0;
            return;
        }

        let speed = if self.velocity * distance < 0.0 {
            self.velocity.abs() * MOMENTUM_RETENTION
        } else {
            self.velocity.abs()
        };
        self.velocity = speed
            .min(distance.abs() * MAX_DISTANCE_RATIO)
            .copysign(distance);
    }
}
