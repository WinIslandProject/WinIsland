use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::panic::{self, PanicHookInfo};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::WindowsAndMessaging::{MESSAGEBOX_STYLE, MessageBoxW};
use windows::core::PCWSTR;

const LOG_DIR: &str = ".winisland/logs";
const LOG_FILE: &str = "winisland.log";
const CRASH_FLAG: &str = ".winisland/.crash_flag";
const MAX_LOG_SIZE: u64 = 1_024_000; // 1MB
const ERROR_MESSAGE_BOX_STYLE: MESSAGEBOX_STYLE = MESSAGEBOX_STYLE(0x0000_0010);

struct FileLogger {
    state: Mutex<LogFile>,
}

struct LogFile {
    path: PathBuf,
    writer: Option<BufWriter<File>>,
    bytes_written: u64,
}

impl LogFile {
    fn open(path: PathBuf) -> std::io::Result<Self> {
        let _ = roll_if_needed(&path);
        let bytes_written = fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let writer = open_log_writer(&path)?;
        Ok(Self {
            path,
            writer: Some(writer),
            bytes_written,
        })
    }

    fn write(&mut self, message: &[u8]) -> std::io::Result<()> {
        if self.bytes_written.saturating_add(message.len() as u64) > MAX_LOG_SIZE {
            self.rotate()?;
        }
        let Some(writer) = self.writer.as_mut() else {
            return Err(std::io::Error::other("log writer is unavailable"));
        };
        writer.write_all(message)?;
        self.bytes_written = self.bytes_written.saturating_add(message.len() as u64);
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let Some(writer) = self.writer.as_mut() else {
            return Err(std::io::Error::other("log writer is unavailable"));
        };
        writer.flush()
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        let Some(mut writer) = self.writer.take() else {
            return Err(std::io::Error::other("log writer is unavailable"));
        };
        if let Err(error) = writer.flush() {
            self.writer = Some(writer);
            return Err(error);
        }
        drop(writer);
        if let Err(error) = fs::rename(&self.path, next_archive_path(&self.path)) {
            self.writer = open_log_writer(&self.path).ok();
            self.bytes_written = fs::metadata(&self.path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            return Err(error);
        }
        self.writer = Some(open_log_writer(&self.path)?);
        self.bytes_written = 0;
        Ok(())
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let msg = format!(
            "[{}] [{}] {} - {}\n",
            timestamp(),
            record.level(),
            record.target(),
            record.args()
        );
        if let Ok(mut state) = self.state.lock()
            && state.write(msg.as_bytes()).is_ok()
            && record.level() <= Level::Warn
        {
            let _ = state.flush();
        }
    }

    fn flush(&self) {
        if let Ok(mut state) = self.state.lock() {
            let _ = state.flush();
        }
    }
}

fn timestamp() -> String {
    let time = local_time();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond
    )
}

fn file_timestamp() -> String {
    let time = local_time();
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond
    )
}

fn local_time() -> SYSTEMTIME {
    // SAFETY: GetLocalTime returns a fully initialized SYSTEMTIME value without input pointers.
    unsafe { GetLocalTime() }
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

fn open_log_writer(path: &Path) -> std::io::Result<BufWriter<File>> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map(BufWriter::new)
}

fn roll_if_needed(path: &Path) -> std::io::Result<()> {
    if let Ok(meta) = fs::metadata(path)
        && meta.len() > MAX_LOG_SIZE
    {
        fs::rename(path, next_archive_path(path))?;
    }
    Ok(())
}

fn next_archive_path(path: &Path) -> PathBuf {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let timestamp = file_timestamp();
    let mut index = 0;
    loop {
        let suffix = (index > 0).then(|| format!("-{index}"));
        let archive = directory.join(format!(
            "winisland-{timestamp}{}.log",
            suffix.unwrap_or_default()
        ));
        if !archive.exists() {
            return archive;
        }
        index += 1;
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

pub fn flush() {
    log::logger().flush();
}

fn write_crash_report(panic_info: &PanicHookInfo) {
    let ts = timestamp();
    let file_ts = file_timestamp();

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
Version: {}
Thread: main

// The crash happened at
Location: {location}

// Reason
{msg}

// Logs
See ~/.winisland/logs/winisland.log for recent activity.
"#,
        env!("CARGO_PKG_VERSION"),
    );

    // Try writing to log directory first
    let mut path = log_dir();
    path.push(format!("crash-{file_ts}.txt"));

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
        desktop_path.push(format!("WinIsland-crash-{file_ts}.txt"));
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
}

fn show_message_box(title: &str, text: &str) {
    let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: both UTF-16 buffers are NUL-terminated and remain valid for the synchronous call.
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
    log::logger().flush();
    let _ = fs::write(crash_flag_path(), "");
    write_crash_report(info);
}

pub fn init() -> Result<(), SetLoggerError> {
    let path = log_file_path();
    let state = LogFile::open(path).expect("Failed to open log file");

    let logger = FileLogger {
        state: Mutex::new(state),
    };

    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(LevelFilter::Info);

    panic::set_hook(Box::new(panic_hook));

    log::info!("Logger initialized");
    Ok(())
}
