use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::panic::{self, PanicHookInfo};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;
use windows::Win32::UI::WindowsAndMessaging::{MESSAGEBOX_STYLE, MessageBoxW};
use windows::core::PCWSTR;

const LOG_DIR: &str = ".winisland/logs";
const LOG_FILE: &str = "winisland.log";
const CRASH_FLAG: &str = ".winisland/.crash_flag";
const MAX_LOG_SIZE: u64 = 1_024_000; // 1MB
const ERROR_MESSAGE_BOX_STYLE: MESSAGEBOX_STYLE = MESSAGEBOX_STYLE(0x0000_0010);

struct FileLogger {
    file: Mutex<File>,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs();
        // Format: [2026-06-07 20:15:00] [INFO] target - message
        let msg = format!(
            "[{}] [{}] {} - {}\n",
            format_timestamp(secs),
            record.level(),
            record.target(),
            record.args()
        );
        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(msg.as_bytes());
            let _ = file.flush();
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

fn format_timestamp(secs: u64) -> String {
    let secs = secs as i64;
    let days = secs / 86400;
    let rem = secs % 86400;
    let hours = rem / 3600;
    let rem = rem % 3600;
    let minutes = rem / 60;
    let seconds = rem % 60;

    // Compute year/month/day from Unix epoch (2026-06-07 is just a reference
    // for the algorithm; this works for any date).
    let mut y = 1970i64;
    let mut d = days;
    loop {
        let yd = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
            366
        } else {
            365
        };
        if d < yd {
            break;
        }
        d -= yd;
        y += 1;
    }
    let months_days = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    for (i, &md) in months_days.iter().enumerate() {
        if d < md {
            m = i;
            break;
        }
        d -= md;
    }

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        y,
        m + 1,
        d + 1,
        hours,
        minutes,
        seconds
    )
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn log_dir() -> PathBuf {
    let mut path = home_dir();
    path.push(LOG_DIR);
    let _ = fs::create_dir_all(&path);
    path
}

fn log_file_path() -> PathBuf {
    let mut path = log_dir();
    path.push(LOG_FILE);
    path
}

fn roll_if_needed(path: &PathBuf) {
    if let Ok(meta) = fs::metadata(path)
        && meta.len() > MAX_LOG_SIZE
    {
        let mut old = path.clone();
        old.set_extension("old.log");
        let _ = fs::rename(path, old);
    }
}

fn crash_flag_path() -> PathBuf {
    let mut path = home_dir();
    path.push(CRASH_FLAG);
    path
}

pub fn check_crash_flag() {
    let flag = crash_flag_path();
    if flag.exists() {
        log::warn!("Previous session crashed; delaying startup by 1s for GPU recovery");
        let _ = fs::remove_file(&flag);
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn write_crash_report(panic_info: &PanicHookInfo) {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let ts = format_timestamp(now.as_secs());

    let msg = panic_info
        .payload()
        .downcast_ref::<&str>()
        .map(std::string::ToString::to_string)
        .or_else(|| panic_info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "Unknown panic".into());

    let location = panic_info
        .location()
        .map(|l| format!("{}:{}", l.file(), l.line()))
        .unwrap_or_else(|| "unknown".into());

    let report = format!(
        r#"---- WinIsland Crash Report ----
Time: {ts}
Version: 1.0.0
Thread: main

// The crash happened at
Location: {location}

// Reason
{msg}

// Logs
See ~/.winisland/logs/winisland.log for recent activity.
"#,
    );

    // Try writing to log directory first
    let mut path = log_dir();
    path.push(format!("crash-{ts}.txt"));

    if write_report_to(&path, &report).is_ok() {
        show_message_box(
            "WinIsland Crash",
            "Crash report saved. Logs will be written on next startup.",
        );
        return;
    }

    // Fallback: write to Desktop
    let msg_text = format!("WinIsland crashed at {location}\n\nReason: {msg}");
    if let Some(desktop) = get_desktop_path() {
        let mut desktop_path = desktop;
        desktop_path.push(format!("WinIsland-crash-{ts}.txt"));
        if write_report_to(&desktop_path, &report).is_ok() {
            show_message_box(
                "WinIsland Crash",
                &format!("Crash report saved to:\n{}", desktop_path.display()),
            );
            return;
        }
    }

    // Final fallback: show message box with crash info
    show_message_box("WinIsland Crash", &msg_text);

    // Create crash flag for delayed startup next time
    let flag = crash_flag_path();
    let _ = fs::write(&flag, "");
}

fn show_message_box(title: &str, text: &str) {
    let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            ERROR_MESSAGE_BOX_STYLE,
        );
    }
}

fn write_report_to(path: &std::path::Path, report: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = fs::File::create(path)?;
    file.write_all(report.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

fn get_desktop_path() -> Option<std::path::PathBuf> {
    if let Ok(path) = std::env::var("USERPROFILE") {
        let mut buf = std::path::PathBuf::from(path);
        buf.push("Desktop");
        if buf.exists() {
            return Some(buf);
        }
    }
    None
}

fn panic_hook(info: &PanicHookInfo) {
    write_crash_report(info);
}

pub fn init() -> Result<(), SetLoggerError> {
    let path = log_file_path();
    roll_if_needed(&path);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .expect("Failed to open log file");

    let logger = FileLogger {
        file: Mutex::new(file),
    };

    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(LevelFilter::Info);

    panic::set_hook(Box::new(panic_hook));

    log::info!("Logger initialized");
    Ok(())
}
