use std::cell::OnceCell;
use std::sync::Arc;

use windows::{
    System::DispatcherQueueController,
    UI::Composition::Desktop::DesktopWindowTarget,
    UI::Composition::{
        CompositionGeometricClip, CompositionRoundedRectangleGeometry, Compositor, ContainerVisual,
        SpriteVisual,
    },
    Win32::{
        Foundation::HWND,
        System::WinRT::{
            Composition::ICompositorDesktopInterop, CreateDispatcherQueueController,
            DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
        },
        UI::WindowsAndMessaging::{
            SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
            SetWindowPos,
        },
    },
    core::Interface,
};
use windows_numerics::{Vector2, Vector3};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

const HOST_BACKDROP_INSET: f32 = 1.0;

struct BackdropCompositionContext {
    _dispatcher_queue: DispatcherQueueController,
    compositor: Compositor,
}

pub(crate) struct HostBackdrop {
    _window: Arc<Window>,
    main_hwnd: HWND,
    backdrop_hwnd: HWND,
    target: DesktopWindowTarget,
    _root: ContainerVisual,
    visual: SpriteVisual,
    _clip: CompositionGeometricClip,
    geometry: CompositionRoundedRectangleGeometry,
}

pub(crate) struct HostBackdropParams {
    pub(crate) enabled: bool,
    pub(crate) screen_x: f32,
    pub(crate) screen_y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) radius: f32,
}

thread_local! {
    static BACKDROP_COMPOSITION: OnceCell<Option<BackdropCompositionContext>> = const { OnceCell::new() };
}

impl HostBackdrop {
    pub(crate) fn new(main_window: &Window, backdrop_window: &Arc<Window>) -> Result<Self, String> {
        let compositor = backdrop_compositor()
            .ok_or_else(|| "Windows host backdrop compositor is unavailable".to_string())?;
        let main_hwnd = window_hwnd(main_window)?;
        let backdrop_hwnd = window_hwnd(backdrop_window)?;
        if !crate::utils::win32::enable_host_backdrop(backdrop_hwnd) {
            return Err("DWM host backdrop support could not be enabled".to_string());
        }
        let interop: ICompositorDesktopInterop = compositor
            .cast()
            .map_err(|error| format!("ICompositorDesktopInterop is unavailable: {error}"))?;
        let target = unsafe {
            // SAFETY: backdrop_hwnd belongs to the companion window and has no other composition tree.
            interop.CreateDesktopWindowTarget(backdrop_hwnd, false)
        }
        .map_err(|error| format!("CreateDesktopWindowTarget failed: {error}"))?;
        let root = compositor
            .CreateContainerVisual()
            .map_err(|error| format!("CreateContainerVisual failed: {error}"))?;
        let visual = compositor
            .CreateSpriteVisual()
            .map_err(|error| format!("CreateSpriteVisual failed: {error}"))?;
        let geometry = compositor
            .CreateRoundedRectangleGeometry()
            .map_err(|error| format!("CreateRoundedRectangleGeometry failed: {error}"))?;
        let clip = compositor
            .CreateGeometricClipWithGeometry(&geometry)
            .map_err(|error| format!("CreateGeometricClipWithGeometry failed: {error}"))?;
        let brush = compositor
            .CreateHostBackdropBrush()
            .map_err(|error| format!("CreateHostBackdropBrush failed: {error}"))?;
        visual
            .SetBrush(&brush)
            .map_err(|error| format!("Host backdrop brush assignment failed: {error}"))?;
        visual
            .SetClip(&clip)
            .map_err(|error| format!("Host backdrop clip assignment failed: {error}"))?;
        visual
            .SetIsVisible(false)
            .map_err(|error| format!("Host backdrop visibility setup failed: {error}"))?;
        root.Children()
            .and_then(|children| children.InsertAtTop(&visual))
            .map_err(|error| format!("Host backdrop visual insertion failed: {error}"))?;
        target
            .SetRoot(&root)
            .map_err(|error| format!("Host backdrop root assignment failed: {error}"))?;
        Ok(Self {
            _window: backdrop_window.clone(),
            main_hwnd,
            backdrop_hwnd,
            target,
            _root: root,
            visual,
            _clip: clip,
            geometry,
        })
    }

    pub(crate) fn update(&self, params: HostBackdropParams) -> windows::core::Result<()> {
        let enabled = params.enabled && params.width > 0.0 && params.height > 0.0;
        if !enabled {
            self.hide();
            return Ok(());
        }

        let window_width = params.width.ceil().max(1.0) as i32;
        let window_height = params.height.ceil().max(1.0) as i32;
        let width = (params.width - HOST_BACKDROP_INSET * 2.0).max(0.0);
        let height = (params.height - HOST_BACKDROP_INSET * 2.0).max(0.0);
        let radius = (params.radius - HOST_BACKDROP_INSET).max(0.0);
        self.visual.SetOffset(Vector3 {
            X: HOST_BACKDROP_INSET,
            Y: HOST_BACKDROP_INSET,
            Z: 0.0,
        })?;
        self.visual.SetSize(Vector2 {
            X: width,
            Y: height,
        })?;
        self.geometry.SetSize(Vector2 {
            X: width,
            Y: height,
        })?;
        self.geometry.SetCornerRadius(Vector2 {
            X: radius,
            Y: radius,
        })?;
        self.visual.SetIsVisible(true)?;
        unsafe {
            // SAFETY: Both HWND values belong to live windows on this thread. Placing the backdrop
            // immediately behind the owned foreground window preserves z-order without activation.
            SetWindowPos(
                self.backdrop_hwnd,
                Some(self.main_hwnd),
                params.screen_x.floor() as i32,
                params.screen_y.floor() as i32,
                window_width,
                window_height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )?;
        }
        Ok(())
    }

    pub(crate) fn hide(&self) {
        let _ = self.visual.SetIsVisible(false);
        unsafe {
            // SAFETY: The backdrop HWND remains owned by `_window`; this only hides it.
            let _ = SetWindowPos(
                self.backdrop_hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_HIDEWINDOW | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
        }
    }
}

impl Drop for HostBackdrop {
    fn drop(&mut self) {
        self.hide();
        let _ = self.target.Close();
    }
}

fn backdrop_compositor() -> Option<Compositor> {
    BACKDROP_COMPOSITION.with(|cell| {
        cell.get_or_init(|| match create_backdrop_composition_context() {
            Ok(context) => Some(context),
            Err(error) => {
                log::warn!("Windows host backdrop initialization failed: {error}");
                None
            }
        })
        .as_ref()
        .map(|context| context.compositor.clone())
    })
}

fn create_backdrop_composition_context() -> Result<BackdropCompositionContext, String> {
    let options = DispatcherQueueOptions {
        dwSize: size_of::<DispatcherQueueOptions>() as u32,
        threadType: DQTYPE_THREAD_CURRENT,
        apartmentType: DQTAT_COM_NONE,
    };
    let dispatcher_queue = unsafe {
        // SAFETY: winit initializes the main thread's STA before renderer creation. The options
        // attach a dispatcher queue to that current thread without changing its COM apartment.
        CreateDispatcherQueueController(options)
    }
    .map_err(|error| format!("CreateDispatcherQueueController failed: {error}"))?;
    let compositor = Compositor::new().map_err(|error| format!("Compositor failed: {error}"))?;
    Ok(BackdropCompositionContext {
        _dispatcher_queue: dispatcher_queue,
        compositor,
    })
}

fn window_hwnd(window: &Window) -> Result<HWND, String> {
    let handle = window
        .window_handle()
        .map_err(|error| format!("Window handle unavailable: {error}"))?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as _)),
        _ => Err("Host backdrop requires a Win32 window".to_string()),
    }
}
