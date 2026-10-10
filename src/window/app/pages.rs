use std::time::{Duration, Instant};

use crate::ui::expanded::pager::{self, ExpandedPage, PageAvailability, PagerHit, available_pages};
use crate::utils::mouse::is_point_in_continuous_rounded_rect;
use winisland_platform::MouseWheelDelta;
use winisland_render::{Point, Rect};

use super::{App, IslandLayout};

const WHEEL_COOLDOWN: Duration = Duration::from_millis(260);
const WHEEL_PIXEL_THRESHOLD: f32 = 40.0;
const DIAL_PIXEL_STEP: f32 = 20.0;

impl App {
    pub(super) fn expanded_pages(&self) -> Vec<ExpandedPage> {
        let calendar = self.plugin_host.as_ref().and_then(|host| {
            host.surface_id(
                crate::plugin::calendar::ID,
                crate::plugin::calendar::PAGE_KEY,
            )
        });
        let visible_order: Vec<_> = self
            .config
            .expanded_page_order
            .iter()
            .copied()
            .filter(|kind| !self.config.hidden_expanded_pages.contains(kind))
            .collect();
        let mut pages = available_pages(
            &visible_order,
            &PageAvailability {
                music: self.music_page_available,
                calendar,
            },
        );
        if let Some(host) = &self.plugin_host {
            pages.extend(
                host.surfaces()
                    .into_iter()
                    .filter(|(_, spec)| spec.kind == winisland_plugin_api::SURFACE_PAGE)
                    .filter(|(id, _)| Some(*id) != calendar)
                    .map(|(id, _)| ExpandedPage::Plugin(id)),
            );
        }
        if pages.is_empty() {
            pages.push(ExpandedPage::Widgets);
        }
        pages
    }

    fn page_index(&self, page: ExpandedPage) -> Option<usize> {
        self.expanded_pages()
            .iter()
            .position(|candidate| *candidate == page)
    }

    fn page_distance(&self, page: ExpandedPage) -> Option<f32> {
        self.page_index(page)
            .map(|index| (index as f32 - self.springs.view.value).abs())
    }

    pub(super) fn page_translation(&self, page: ExpandedPage) -> f32 {
        self.page_index(page).map_or(0.0, |index| {
            (index as f32 - self.springs.view.value) * self.springs.w.value
        })
    }

    pub(super) fn page_visible(&self, page: ExpandedPage) -> bool {
        self.page_distance(page)
            .is_some_and(|distance| distance < 1.0)
    }

    pub(super) fn page_focused(&self, page: ExpandedPage) -> bool {
        self.page_distance(page)
            .is_some_and(|distance| distance < 0.5)
    }

    pub(super) fn target_page_position(&self) -> f32 {
        self.page_index(self.current_page).unwrap_or(0) as f32
    }

    pub(super) fn reset_page(&mut self) {
        self.current_page = self
            .expanded_pages()
            .first()
            .copied()
            .unwrap_or(ExpandedPage::Widgets);
    }

    pub(super) fn snap_to_current_page(&mut self) {
        let pages = self.expanded_pages();
        if !pages.contains(&self.current_page) {
            self.current_page = pages.first().copied().unwrap_or(ExpandedPage::Widgets);
        }
        if pages.is_empty() {
            self.expanded = false;
        }
        self.springs.view.value = self.target_page_position();
        self.springs.view.velocity = 0.0;
    }

    fn go_to_page_index(&mut self, index: usize) -> bool {
        match self.expanded_pages().get(index) {
            Some(page) if *page != self.current_page => {
                self.current_page = *page;
                true
            }
            _ => false,
        }
    }

    fn step_page(&mut self, step: i32) -> bool {
        let count = self.expanded_pages().len();
        if count < 2 {
            return false;
        }
        let current = self.page_index(self.current_page).unwrap_or(0) as i32;
        let next = (current + step).clamp(0, count as i32 - 1) as usize;
        self.go_to_page_index(next)
    }

    pub(super) fn set_music_page_available(&mut self, available: bool) {
        if available == self.music_page_available {
            return;
        }
        let previous = self.expanded_pages();
        let view = self.springs.view.value;
        let shown_index = (view.round().max(0.0) as usize).min(previous.len().saturating_sub(1));
        let shown = previous.get(shown_index).copied();
        self.music_page_available = available;
        let pages = self.expanded_pages();
        if pages.is_empty() {
            self.expanded = false;
        }
        match shown.and_then(|page| pages.iter().position(|candidate| *candidate == page)) {
            Some(index) => self.springs.view.value = index as f32 + (view - shown_index as f32),
            None => {
                self.springs.view.value = shown_index.min(pages.len().saturating_sub(1)) as f32;
                self.springs.view.velocity = 0.0;
            }
        }
        if !pages.contains(&self.current_page) {
            self.current_page = shown
                .filter(|page| pages.contains(page))
                .or_else(|| pages.first().copied())
                .unwrap_or(ExpandedPage::Widgets);
        }
    }

    fn island_rect(&self, layout: &IslandLayout) -> Rect {
        Rect::from_xywh(
            layout.current_island_x as f32,
            layout.current_island_y as f32,
            self.springs.w.value,
            self.springs.h.value,
        )
    }

    pub(super) fn pager_contains(&self, rel_x: i32, rel_y: i32, layout: &IslandLayout) -> bool {
        self.expanded
            && pager::contains(
                self.island_rect(layout),
                self.expanded_pages().len(),
                self.config.expanded_scale,
                layout.dock_bottom,
                Point::new(rel_x as f32, rel_y as f32),
            )
    }

    pub(super) fn handle_pager_press(
        &mut self,
        rel_x: i32,
        rel_y: i32,
        layout: &IslandLayout,
    ) -> bool {
        if !self.expanded {
            return false;
        }
        let Some(hit) = pager::hit_test(
            self.island_rect(layout),
            self.expanded_pages().len(),
            self.springs.view.value,
            self.config.expanded_scale,
            layout.dock_bottom,
            Point::new(rel_x as f32, rel_y as f32),
        ) else {
            return false;
        };
        match hit {
            PagerHit::Page(index) => {
                self.go_to_page_index(index);
            }
            PagerHit::Close => {
                self.expanded = false;
                self.reset_page();
            }
        }
        true
    }

    pub(super) fn pager_hover_animating(&self) -> bool {
        [&self.close_hover, &self.bar_hover]
            .into_iter()
            .any(|spring| spring.velocity != 0.0 || (spring.value != 0.0 && spring.value != 1.0))
    }

    pub(super) fn update_pager_hover(
        &mut self,
        window: &crate::platform::WindowRef,
        rel_x: i32,
        rel_y: i32,
        layout: &IslandLayout,
        interaction_allowed: bool,
        dt: f32,
    ) {
        let hit = (interaction_allowed && self.expanded)
            .then(|| {
                pager::hit_test(
                    self.island_rect(layout),
                    self.expanded_pages().len(),
                    self.springs.view.value,
                    self.config.expanded_scale,
                    layout.dock_bottom,
                    Point::new(rel_x as f32, rel_y as f32),
                )
            })
            .flatten();
        let close_target = if hit == Some(PagerHit::Close) {
            1.0
        } else {
            0.0
        };
        let bar_target = if matches!(hit, Some(PagerHit::Page(_))) {
            1.0
        } else {
            0.0
        };
        let before = (self.close_hover.value, self.bar_hover.value);
        for (spring, target) in [
            (&mut self.close_hover, close_target),
            (&mut self.bar_hover, bar_target),
        ] {
            spring.update_dt(target, 0.22, 0.62, dt);
            spring.settle(target, 0.002, 0.001);
        }
        let timer_hover =
            (interaction_allowed && self.expanded && self.page_focused(ExpandedPage::Timer))
                .then(|| {
                    crate::ui::expanded::timer_view::hit_test(
                        layout.offset_x as f32 + self.page_translation(ExpandedPage::Timer),
                        layout.island_y as f32,
                        self.springs.w.value,
                        self.springs.h.value,
                        self.config.expanded_scale,
                        Point::new(rel_x as f32, rel_y as f32),
                    )
                })
                .flatten();
        let timer_changed = crate::ui::expanded::timer_view::set_hover(timer_hover);
        if (self.close_hover.value, self.bar_hover.value) != before || timer_changed {
            window.request_redraw();
        }
    }

    pub(super) fn handle_mouse_wheel(&mut self, delta: MouseWheelDelta, px: i32, py: i32) -> bool {
        if !self.expanded {
            return false;
        }
        let rel_x = px - self.geom.win_x;
        let rel_y = py - self.geom.win_y;
        let layout = self.compute_island_layout();
        let over_island = is_point_in_continuous_rounded_rect(
            rel_x as f64,
            rel_y as f64,
            layout.current_island_x,
            layout.current_island_y,
            self.springs.w.value as f64,
            self.springs.h.value as f64,
            self.springs.r.value as f64,
        );
        if !over_island && !self.pager_contains(rel_x, rel_y, &layout) {
            return false;
        }
        let dominant = |x: f32, y: f32| if x.abs() > y.abs() { x } else { -y };
        let over_timer_dial = self.page_focused(ExpandedPage::Timer)
            && crate::ui::expanded::timer_view::hit_test(
                layout.offset_x as f32 + self.page_translation(ExpandedPage::Timer),
                layout.island_y as f32,
                self.springs.w.value,
                self.springs.h.value,
                self.config.expanded_scale,
                Point::new(rel_x as f32, rel_y as f32),
            ) == Some(crate::ui::expanded::timer_view::TimerAction::Dial);
        if over_timer_dial {
            let steps = match delta {
                MouseWheelDelta::Lines { x, y } => -dominant(x, y).signum() as i32,
                MouseWheelDelta::Pixels { x, y } => {
                    self.wheel_accumulator += dominant(x as f32, y as f32);
                    let steps = (self.wheel_accumulator / DIAL_PIXEL_STEP).trunc();
                    self.wheel_accumulator -= steps * DIAL_PIXEL_STEP;
                    -steps as i32
                }
            };
            self.idle_timer = Instant::now();
            return crate::ui::expanded::timer_view::adjust_minutes(steps);
        }
        let step = match delta {
            MouseWheelDelta::Lines { x, y } => {
                self.wheel_accumulator = 0.0;
                dominant(x, y).signum()
            }
            MouseWheelDelta::Pixels { x, y } => {
                self.wheel_accumulator += dominant(x as f32, y as f32);
                if self.wheel_accumulator.abs() < WHEEL_PIXEL_THRESHOLD {
                    return false;
                }
                let step = self.wheel_accumulator.signum();
                self.wheel_accumulator = 0.0;
                step
            }
        };
        if step == 0.0 {
            return false;
        }
        let now = Instant::now();
        if self
            .last_wheel_page_at
            .is_some_and(|last| now.saturating_duration_since(last) < WHEEL_COOLDOWN)
        {
            return false;
        }
        self.last_wheel_page_at = Some(now);
        self.idle_timer = now;
        self.step_page(step as i32)
    }
}
