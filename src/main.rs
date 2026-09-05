#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod core;
mod icons;
mod plugin;
mod ui;
mod utils;
mod window;
use crate::core::i18n::init_i18n;
use crate::utils::logger;
use crate::window::app::App;
use std::env;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{MSG, WM_DWMCOMPOSITIONCHANGED};
use windows::core::w;
use winit::event_loop::EventLoop;
use winit::platform::windows::EventLoopBuilderExtWindows;

const RESTART_ARG: &str = "--restart";
const RESTART_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const INSTANCE_RETRY_INTERVAL: Duration = Duration::from_millis(200);
const TERMINATION_GRACE_PERIOD: Duration = Duration::from_millis(500);

fn main() {
    let _ = logger::init();
    log::info!("WinIsland v{} starting", env!("CARGO_PKG_VERSION"));

    let config = core::persistence::load_config();
    let _ = utils::autostart::set_autostart(config.auto_start);
    logger::check_crash_flag();
    init_i18n(&config.language);

    let args: Vec<String> = env::args().collect();
    let restart_requested = args.iter().any(|arg| arg == RESTART_ARG);
    log::info!("Args: {args:?}");
    log::info!(
        "Config: style={:?}, scale={}, lang={}",
        config.island_style,
        config.global_scale,
        config.language
    );

    let Some(_instance_mutex) = acquire_instance_mutex(restart_requested) else {
        return;
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let _guard = runtime.enter();

    utils::updater::start_update_checker();

    let mut event_loop_builder = EventLoop::builder();
    event_loop_builder.with_msg_hook(|message| {
        if !message.is_null() {
            // SAFETY: winit invokes the hook with a valid pointer to the MSG currently being
            // dispatched, and the pointer is only read during this synchronous callback.
            let message = unsafe { &*message.cast::<MSG>() };
            if message.message == WM_DWMCOMPOSITIONCHANGED {
                window::d3d::signal_dwm_composition_changed();
            }
        }
        false
    });
    let event_loop = event_loop_builder.build().unwrap();
    utils::event_loop::set_proxy(event_loop.create_proxy());
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
    log::info!("Application event loop exited, shutting down");
    logger::flush();
}

fn acquire_instance_mutex(restart_requested: bool) -> Option<HANDLE> {
    let started = Instant::now();
    let mut terminated_stale_instance = false;
    loop {
        match try_create_instance_mutex() {
            Ok(Some(handle)) => return Some(handle),
            Ok(None) => {}
            Err(error) => {
                log::error!("Failed to create the single-instance mutex: {error}");
                return None;
            }
        }
        if !restart_requested {
            return None;
        }
        if started.elapsed() <= RESTART_LOCK_TIMEOUT {
            std::thread::sleep(INSTANCE_RETRY_INTERVAL);
            continue;
        }
        if terminated_stale_instance || !terminate_stale_instances() {
            return None;
        }
        terminated_stale_instance = true;
        std::thread::sleep(TERMINATION_GRACE_PERIOD);
    }
}

fn try_create_instance_mutex() -> windows::core::Result<Option<HANDLE>> {
    // SAFETY: The mutex name is a static string. A handle returned for an existing mutex is
    // closed immediately; a newly created handle remains open for the process lifetime.
    unsafe {
        let handle = CreateMutexW(None, true, w!("Local\\WinIsland_SingleInstance_Mutex"))?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            Ok(None)
        } else {
            Ok(Some(handle))
        }
    }
}

fn terminate_stale_instances() -> bool {
    let own_pid = std::process::id();
    std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Get-Process WinIsland -ErrorAction SilentlyContinue | Where-Object {{$_.Id -ne {own_pid}}} | Stop-Process -Force"
            ),
        ])
        .output()
        .is_ok_and(|output| output.status.success())
}
