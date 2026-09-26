use winisland_platform::HitRegion;
use winit::window::Window;

pub(super) fn set_hit_regions(window: &Window, regions: &[HitRegion]) {
    let enabled = regions
        .iter()
        .any(|region| matches!(region, HitRegion::WholeWindow(true)));
    let _ = window.set_cursor_hittest(enabled);
}
