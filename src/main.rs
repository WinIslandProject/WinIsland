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
use std::mem::ManuallyDrop;
use windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
use windows::Win32::Foundation::{CloseHandle, GetLastError};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{MSG, WM_DWMCOMPOSITIONCHANGED};
use windows::core::w;
use winit::event_loop::EventLoop;
use winit::platform::windows::EventLoopBuilderExtWindows;

fn main() {
    let _ = logger::init();
    log::info!("WinIsland v{} starting", env!("CARGO_PKG_VERSION"));

    let config = core::persistence::load_config();
    let _ = utils::autostart::set_autostart(config.auto_start);
    logger::check_crash_flag();
    init_i18n(&config.language);

    let args: Vec<String> = env::args().collect();
    let is_restart = args.iter().any(|arg| arg == "--restart");
    log::info!("Args: {args:?}");
    log::info!(
        "Config: style={:?}, scale={}, lang={}",
        config.island_style,
        config.global_scale,
        config.language
    );

    if is_restart {
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    let _single_mutex = {
        let start = std::time::Instant::now();
        loop {
            // SAFETY: CreateMutexW creates a named mutex for single-instance lock.
            // The name is a static string literal. On success with no ERROR_ALREADY_EXISTS,
            // the handle is kept in ManuallyDrop to hold the lock for the process lifetime.
            // On ERROR_ALREADY_EXISTS, the handle is closed and we retry or exit.
            unsafe {
                let h = CreateMutexW(None, true, w!("Local\\WinIsland_SingleInstance_Mutex"));
                match h {
                    Ok(handle) => {
                        if GetLastError() != ERROR_ALREADY_EXISTS {
                            break ManuallyDrop::new(handle);
                        }
                        let _ = CloseHandle(handle);
                    }
                    Err(_) => {
                        if !is_restart {
                            return;
                        }
                    }
                }
            }
            if !is_restart || start.elapsed() > std::time::Duration::from_secs(10) {
                if is_restart {
                    let own_pid = std::process::id();
                    if let Ok(output) = std::process::Command::new("powershell")
                        .args([
                            "-NoProfile",
                            "-Command",
                            &format!(
                                "Get-Process WinIsland -ErrorAction SilentlyContinue | Where-Object {{$_.Id -ne {own_pid}}} | Stop-Process -Force"
                            ),
                        ])
                        .output()
                        && output.status.success()
                    {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        continue;
                    }
                }
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
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
