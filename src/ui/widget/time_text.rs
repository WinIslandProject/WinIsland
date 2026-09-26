use std::cell::RefCell;
use std::time::Duration;

struct TimeText {
    hour: u16,
    minute: u16,
    value: String,
}

thread_local! {
    static TIME_TEXT: RefCell<TimeText> = const {
        RefCell::new(TimeText {
            hour: u16::MAX,
            minute: u16::MAX,
            value: String::new(),
        })
    };
}

pub(crate) fn with_current_time_text<T>(draw: impl FnOnce(&str) -> T) -> T {
    let local_time = crate::platform::shell().local_datetime();
    TIME_TEXT.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.hour != local_time.hour || cache.minute != local_time.minute {
            cache.hour = local_time.hour;
            cache.minute = local_time.minute;
            cache.value = format!("{:02}:{:02}", local_time.hour, local_time.minute);
        }
        draw(&cache.value)
    })
}

pub(crate) fn until_next_minute() -> Duration {
    let local_time = crate::platform::shell().local_datetime();
    let elapsed_ms = u64::from(local_time.second) * 1_000 + u64::from(local_time.millisecond);
    Duration::from_millis(60_000_u64.saturating_sub(elapsed_ms).max(1))
}
