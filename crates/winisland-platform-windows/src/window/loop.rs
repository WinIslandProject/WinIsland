use std::cell::{Cell, RefCell};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::UI::WindowsAndMessaging::{MSG, WM_DWMCOMPOSITIONCHANGED};
use winisland_platform::{
    AppHandler, DisplayProvider, InputState, Key, MouseButton, MouseWheelDelta, PlatformError,
    PlatformEvent, Theme, TouchPhase, WindowId, WindowPoint, WindowPosition, WindowSize,
};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{
    ElementState, InnerSizeWriter, MouseButton as WinitMouseButton, MouseScrollDelta,
    TouchPhase as WinitTouchPhase, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::platform::windows::EventLoopBuilderExtWindows;
use winit::window::{Theme as WinitTheme, WindowId as WinitWindowId};

use crate::display::WindowsDisplay;

static PROXY: OnceLock<EventLoopProxy<()>> = OnceLock::new();
static WAKE_PENDING: AtomicBool = AtomicBool::new(false);
static COMPOSITION_CHANGED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static ACTIVE_EVENT_LOOP: Cell<*const ActiveEventLoop> = const { Cell::new(std::ptr::null()) };
    static DPI_WRITER: RefCell<Option<(WindowId, InnerSizeWriter)>> = const { RefCell::new(None) };
}

pub fn wake() {
    let Some(proxy) = PROXY.get() else {
        return;
    };
    if !WAKE_PENDING.swap(true, Ordering::AcqRel) && proxy.send_event(()).is_err() {
        WAKE_PENDING.store(false, Ordering::Release);
    }
}

pub(super) fn run(handler: &mut dyn AppHandler) -> Result<(), PlatformError> {
    let mut builder = EventLoop::<()>::builder();
    builder.with_msg_hook(|message| {
        if !message.is_null() {
            // SAFETY: winit supplies a valid MSG pointer for this synchronous callback.
            let message = unsafe { &*message.cast::<MSG>() };
            if message.message == WM_DWMCOMPOSITIONCHANGED {
                COMPOSITION_CHANGED.store(true, Ordering::Release);
                wake();
            }
        }
        false
    });
    let event_loop = builder.build().map_err(PlatformError::backend)?;
    let _ = PROXY.set(event_loop.create_proxy());
    event_loop
        .run_app(&mut EventAdapter { handler })
        .map_err(PlatformError::backend)
}

pub(super) fn exit() {
    let _ = with_active_event_loop(ActiveEventLoop::exit);
}

pub(super) fn with_active_event_loop<T>(f: impl FnOnce(&ActiveEventLoop) -> T) -> Option<T> {
    ACTIVE_EVENT_LOOP.with(|slot| {
        let event_loop = slot.get();
        if event_loop.is_null() {
            None
        } else {
            // SAFETY: The pointer is installed only for the duration of a winit callback on this thread.
            Some(f(unsafe { &*event_loop }))
        }
    })
}

pub(super) fn request_dpi_inner_size(id: WindowId, size: WindowSize) -> Option<bool> {
    DPI_WRITER.with(|slot| {
        let mut active = slot.borrow_mut();
        let (active_id, writer) = active.as_mut()?;
        if *active_id != id {
            return None;
        }
        Some(
            writer
                .request_inner_size(PhysicalSize::new(size.width, size.height))
                .is_ok(),
        )
    })
}

fn scope_active_event_loop<T>(event_loop: &ActiveEventLoop, f: impl FnOnce() -> T) -> T {
    ACTIVE_EVENT_LOOP.with(|slot| {
        let previous = slot.replace(event_loop);
        struct Restore<'a> {
            slot: &'a Cell<*const ActiveEventLoop>,
            previous: *const ActiveEventLoop,
        }
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.slot.set(self.previous);
            }
        }
        let _restore = Restore { slot, previous };
        f()
    })
}

fn scope_dpi_writer<T>(id: WindowId, writer: InnerSizeWriter, f: impl FnOnce() -> T) -> T {
    DPI_WRITER.with(|slot| {
        let previous = slot.replace(Some((id, writer)));
        struct Restore<'a> {
            slot: &'a RefCell<Option<(WindowId, InnerSizeWriter)>>,
            previous: Option<(WindowId, InnerSizeWriter)>,
        }
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                let _ = self.slot.replace(self.previous.take());
            }
        }
        let _restore = Restore { slot, previous };
        f()
    })
}

struct EventAdapter<'a> {
    handler: &'a mut dyn AppHandler,
}

impl EventAdapter<'_> {
    fn composition_changed(&mut self) {
        if COMPOSITION_CHANGED.swap(false, Ordering::AcqRel) {
            self.handler.on_event(PlatformEvent::CompositionChanged);
        }
    }
}

impl ApplicationHandler<()> for EventAdapter<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        scope_active_event_loop(event_loop, || {
            self.handler.on_event(PlatformEvent::Resumed);
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WinitWindowId,
        event: WindowEvent,
    ) {
        scope_active_event_loop(event_loop, || match event {
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer,
            } => scope_dpi_writer(WindowId(u64::from(id)), inner_size_writer, || {
                self.handler.on_event(PlatformEvent::ScaleFactorChanged {
                    id: WindowId(u64::from(id)),
                    scale: scale_factor,
                });
            }),
            other => {
                if let Some(event) = platform_event(id, other) {
                    self.handler.on_event(event);
                }
            }
        });
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: ()) {
        scope_active_event_loop(event_loop, || {
            WAKE_PENDING.store(false, Ordering::Release);
            self.composition_changed();
            self.handler.on_event(PlatformEvent::Wake);
        });
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        scope_active_event_loop(event_loop, || {
            self.composition_changed();
            let control_flow = match self.handler.on_about_to_wait() {
                Some(deadline) => ControlFlow::WaitUntil(deadline),
                None => ControlFlow::Wait,
            };
            event_loop.set_control_flow(control_flow);
        });
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        scope_active_event_loop(event_loop, || {
            self.handler.on_event(PlatformEvent::Exiting);
            self.handler.on_exit();
        });
    }
}

fn platform_event(id: WinitWindowId, event: WindowEvent) -> Option<PlatformEvent> {
    let id = WindowId(u64::from(id));
    let event = match event {
        WindowEvent::CloseRequested => PlatformEvent::CloseRequested { id },
        WindowEvent::Destroyed => PlatformEvent::Destroyed { id },
        WindowEvent::Resized(size) => PlatformEvent::Resized {
            id,
            size: WindowSize {
                width: size.width,
                height: size.height,
            },
        },
        WindowEvent::Moved(position) => PlatformEvent::Moved {
            id,
            position: WindowPosition {
                x: position.x,
                y: position.y,
            },
        },
        WindowEvent::ThemeChanged(theme) => PlatformEvent::ThemeChanged {
            id,
            theme: match theme {
                WinitTheme::Light => Theme::Light,
                WinitTheme::Dark => Theme::Dark,
            },
        },
        WindowEvent::RedrawRequested => PlatformEvent::RedrawRequested { id },
        WindowEvent::Focused(focused) => PlatformEvent::Focused { id, focused },
        WindowEvent::CursorMoved { position, .. } => PlatformEvent::CursorMoved {
            id,
            position: WindowPoint {
                x: position.x,
                y: position.y,
            },
        },
        WindowEvent::CursorLeft { .. } => PlatformEvent::CursorLeft { id },
        WindowEvent::MouseInput { state, button, .. } => PlatformEvent::MouseInput {
            id,
            button: match button {
                WinitMouseButton::Left => MouseButton::Left,
                WinitMouseButton::Right => MouseButton::Right,
                _ => MouseButton::Other,
            },
            state: input_state(state),
            position: WindowsDisplay
                .cursor_position()
                .ok()
                .flatten()
                .unwrap_or_default(),
        },
        WindowEvent::MouseWheel { delta, .. } => PlatformEvent::MouseWheel {
            id,
            delta: match delta {
                MouseScrollDelta::LineDelta(x, y) => MouseWheelDelta::Lines { x, y },
                MouseScrollDelta::PixelDelta(position) => MouseWheelDelta::Pixels {
                    x: position.x,
                    y: position.y,
                },
            },
        },
        WindowEvent::KeyboardInput { event, .. } => PlatformEvent::KeyInput {
            id,
            key: match event.logical_key {
                WinitKey::Named(NamedKey::Backspace) => Key::Backspace,
                WinitKey::Named(NamedKey::Enter) => Key::Enter,
                WinitKey::Named(NamedKey::Escape) => Key::Escape,
                WinitKey::Named(NamedKey::ArrowLeft) => Key::ArrowLeft,
                WinitKey::Named(NamedKey::ArrowRight) => Key::ArrowRight,
                WinitKey::Character(value) => Key::Character(value.to_string()),
                _ => Key::Other,
            },
            state: input_state(event.state),
        },
        WindowEvent::Touch(touch) => PlatformEvent::Touch {
            id,
            touch_id: touch.id,
            phase: match touch.phase {
                WinitTouchPhase::Started => TouchPhase::Started,
                WinitTouchPhase::Moved => TouchPhase::Moved,
                WinitTouchPhase::Ended => TouchPhase::Ended,
                WinitTouchPhase::Cancelled => TouchPhase::Cancelled,
            },
            position: WindowPoint {
                x: touch.location.x,
                y: touch.location.y,
            },
        },
        WindowEvent::DroppedFile(path) => PlatformEvent::DroppedFile { id, path },
        _ => return None,
    };
    Some(event)
}

fn input_state(state: ElementState) -> InputState {
    match state {
        ElementState::Pressed => InputState::Pressed,
        ElementState::Released => InputState::Released,
    }
}
