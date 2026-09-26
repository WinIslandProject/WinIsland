use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

struct TimeText {
    hour: u16,
    minute: u16,
    use_12h: bool,
    value: String,
}

static USE_12H_FORMAT: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_12h_format(enabled: bool) {
    USE_12H_FORMAT.store(enabled, Ordering::Relaxed);
}

pub(crate) fn is_12h_format() -> bool {
    USE_12H_FORMAT.load(Ordering::Relaxed)
}

thread_local! {
    static TIME_TEXT: RefCell<TimeText> = const {
        RefCell::new(TimeText {
            hour: u16::MAX,
            minute: u16::MAX,
            use_12h: false,
            value: String::new(),
        })
    };
}

pub(crate) fn with_current_time_text<T>(draw: impl FnOnce(&str) -> T) -> T {
    // SAFETY: GetLocalTime returns a fully initialized SYSTEMTIME value.
    let local_time = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let use_12h = is_12h_format();
    TIME_TEXT.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.hour != local_time.wHour
            || cache.minute != local_time.wMinute
            || cache.use_12h != use_12h
        {
            cache.hour = local_time.wHour;
            cache.minute = local_time.wMinute;
            cache.use_12h = use_12h;
            if use_12h {
                let (hour_12, am_pm) = match local_time.wHour {
                    0 => (12, "AM"),
                    1..=11 => (local_time.wHour, "AM"),
                    12 => (12, "PM"),
                    _ => (local_time.wHour - 12, "PM"),
                };
                cache.value = format!("{hour_12:02}:{:02} {am_pm}", local_time.wMinute);
            } else {
                cache.value = format!("{:02}:{:02}", local_time.wHour, local_time.wMinute);
            }
        }
        draw(&cache.value)
    })
}

pub(crate) fn until_next_minute() -> Duration {
    // SAFETY: GetLocalTime returns a fully initialized SYSTEMTIME value.
    let local_time = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let elapsed_ms = u64::from(local_time.wSecond) * 1_000 + u64::from(local_time.wMilliseconds);
    Duration::from_millis(60_000_u64.saturating_sub(elapsed_ms).max(1))
}
