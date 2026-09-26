#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod core;
mod icons;
mod platform;
mod plugin;
mod ui;
mod utils;
mod window;
use crate::utils::logger;
use crate::window::app::App;
use std::env;
use std::time::{Duration, Instant};
use winisland_core::i18n::{init_i18n, set_system_locale_provider};
use winisland_platform::InstanceLock;

const RESTART_ARG: &str = "--restart";
const RESTART_LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const INSTANCE_RETRY_INTERVAL: Duration = Duration::from_millis(200);
const TERMINATION_GRACE_PERIOD: Duration = Duration::from_millis(500);

#[used]
#[unsafe(export_name = "RTSSHooksCompatibility")]
pub static RTSS_HOOKS_COMPATIBILITY: u32 = 0;

fn main() {
    let _ = logger::init();
    log::info!("WinIsland v{} starting", env!("CARGO_PKG_VERSION"));

    let config = core::persistence::load_config();
    if let Err(error) = platform::shell().set_autostart(config.auto_start) {
        platform::update_capabilities(|caps| caps.autostart = false);
        log::warn!("Autostart is unavailable: {error}");
    }
    set_system_locale_provider(platform::system_locale);
    winisland_core::lyrics::set_simplify_hook(platform::to_simplified);
    init_i18n(&config.language);

    let args: Vec<String> = env::args().collect();
    let restart_requested = args.iter().any(|arg| arg == RESTART_ARG);
    log::info!("Args: {args:?}");
    log::info!(
        "Config: style={:?}, compact_scale={}, expanded_scale={}, lang={}",
        config.island_style,
        config.compact_scale,
        config.expanded_scale,
        config.language
    );

    let Some(_instance_mutex) = acquire_instance_mutex(restart_requested) else {
        return;
    };
    logger::check_crash_flag();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let _guard = runtime.enter();

    let capability_probe = std::thread::Builder::new()
        .name("winisland-capability-probe".to_string())
        .spawn(|| {
            let probed = winisland_platform_windows::WindowsPlatform::probe_capabilities();
            platform::update_capabilities(|capabilities| {
                capabilities.toast_events &= probed.toast_events;
                capabilities.media_session &= probed.media_session;
                capabilities.audio_loopback &= probed.audio_loopback;
                capabilities.volume_control &= probed.volume_control;
                capabilities.brightness_control &= probed.brightness_control;
                capabilities.autostart &= probed.autostart;
            });
            log::info!("Platform capabilities: {:?}", platform::capabilities());
            platform::wake();
        })
        .ok();

    utils::updater::start_update_checker();

    let mut app = App::default();
    platform::window().run(&mut app).unwrap();
    if let Some(probe) = capability_probe {
        let _ = probe.join();
    }
    log::info!("Application event loop exited, shutting down");
    logger::flush();
}

fn acquire_instance_mutex(restart_requested: bool) -> Option<Box<dyn InstanceLock>> {
    let started = Instant::now();
    let mut terminated_stale_instance = false;
    loop {
        match platform::shell().acquire_single_instance("Local\\WinIsland_SingleInstance_Mutex") {
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
