use std::path::PathBuf;
use std::time::Instant;

use crate::{
    InputState, Key, MouseButton, MouseWheelDelta, Point, Theme, TouchPhase, WindowId, WindowPoint,
    WindowPosition, WindowSize,
};

/// Owned event values delivered synchronously on the event-loop thread.
///
/// The handler may call `WindowSystem` during delivery. Events do not report failures; a
/// `Destroyed` ID may already be invalid, and absence of an optional OS value is represented
/// by a query result instead. The values remain usable after the callback returns.
#[derive(Clone, Debug)]
pub enum PlatformEvent {
    /// Loop resumed; windows may be created during this callback.
    Resumed,
    /// Close was requested for a live window; it remains owned until explicitly destroyed.
    CloseRequested { id: WindowId },
    /// A window was destroyed; its ID may no longer resolve.
    Destroyed { id: WindowId },
    /// Inner size changed in physical pixels; zero is valid while minimized.
    Resized { id: WindowId, size: WindowSize },
    /// Outer position changed in physical screen pixels.
    Moved {
        id: WindowId,
        position: WindowPosition,
    },
    /// Scale changed; request a replacement inner size before this callback ends.
    ScaleFactorChanged { id: WindowId, scale: f64 },
    /// A known light or dark system theme was reported for this window.
    ThemeChanged { id: WindowId, theme: Theme },
    /// A live window requested rendering; the callback may schedule another redraw.
    RedrawRequested { id: WindowId },
    /// Focus state changed for this window.
    Focused { id: WindowId, focused: bool },
    /// Pointer moved within the window in local physical pixels.
    CursorMoved { id: WindowId, position: WindowPoint },
    /// Pointer left the window; no position is available.
    CursorLeft { id: WindowId },
    /// Button changed; position is global physical pixels or zero if unavailable.
    MouseInput {
        id: WindowId,
        button: MouseButton,
        state: InputState,
        position: Point,
    },
    /// Wheel moved in the delta's line or physical pixel units.
    MouseWheel {
        id: WindowId,
        delta: MouseWheelDelta,
    },
    /// Logical key changed; unsupported keys appear as `Key::Other`.
    KeyInput {
        id: WindowId,
        key: Key,
        state: InputState,
    },
    /// Contact changed at a local physical pixel position; ID lasts for that contact.
    Touch {
        id: WindowId,
        touch_id: u64,
        phase: TouchPhase,
        position: WindowPoint,
    },
    /// A file was dropped; the owned path remains valid after delivery.
    DroppedFile { id: WindowId, path: PathBuf },
    /// Desktop composition changed; consumers may recreate rendering resources now.
    CompositionChanged,
    /// A deduplicated external wake was delivered; the pending flag is already cleared.
    Wake,
    /// Loop is exiting; this is delivered immediately before `on_exit`.
    Exiting,
}

/// Receives normalized events on the event-loop thread while `WindowSystem::run` is active.
///
/// Callbacks are synchronous and may call window methods, but are not invoked recursively by the
/// adapter. No callback runs if loop setup fails. The handler remains borrowed until `run`
/// returns; callback failures have no result channel and must be handled by the application.
pub trait AppHandler {
    /// Handles one owned event before the next loop phase; no result is returned.
    fn on_event(&mut self, event: PlatformEvent);
    /// Returns the next wake deadline, or `None` to wait indefinitely for an event.
    fn on_about_to_wait(&mut self) -> Option<Instant>;
    /// Performs final cleanup once after `Exiting` and before `run` returns.
    fn on_exit(&mut self);
}
