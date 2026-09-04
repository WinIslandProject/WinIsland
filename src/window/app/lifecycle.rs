use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::WindowId;

use super::App;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.on_resumed(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self
            .settings
            .as_ref()
            .and_then(super::super::settings::SettingsApp::window_id)
            == Some(id)
        {
            if let (Some(settings), Some(renderer)) =
                (self.settings.as_mut(), self.renderer.as_mut())
            {
                settings.handle_window_event(event_loop, event, renderer);
            }
            if let Some(error) = self
                .renderer
                .as_mut()
                .and_then(crate::window::d3d::D3DRenderer::take_failure)
            {
                self.invalidate_renderer(&error, Instant::now());
            }
            self.handle_plugin_settings_request(event_loop);
            if self
                .settings
                .as_ref()
                .is_some_and(super::super::settings::SettingsApp::close_requested)
            {
                self.close_settings();
            }
            return;
        }
        self.on_window_event(event_loop, id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.on_about_to_wait(event_loop);
        if let Some(settings_deadline) = self
            .settings
            .as_mut()
            .and_then(super::super::settings::SettingsApp::update)
        {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                self.next_frame_deadline.min(settings_deadline),
            ));
        }
        self.handle_plugin_settings_request(event_loop);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {
        crate::utils::event_loop::acknowledge_wake();
        self.next_frame_deadline = Instant::now();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
