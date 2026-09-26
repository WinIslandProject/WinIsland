pub(crate) fn expanded_island_radius(w: f32, h: f32, scale: f32) -> f32 {
    (48.0 * scale).min(w / 2.0).min(h / 2.0).max(0.0)
}
