use super::{draw_widget_rounded_background, draw_widget_text_centered};
use std::cell::RefCell;
use winisland_core::i18n::tr;
use winisland_render::{Painter, Rect, Rgba};

struct CalendarText {
    year: u16,
    month: u16,
    day: u16,
    weekday: u16,
    month_text: String,
    day_text: String,
    weekday_text: String,
}

thread_local! {
    static CALENDAR_TEXT: RefCell<CalendarText> = const { RefCell::new(CalendarText {
        year: u16::MAX,
        month: u16::MAX,
        day: u16::MAX,
        weekday: u16::MAX,
        month_text: String::new(),
        day_text: String::new(),
        weekday_text: String::new(),
    }) };
}

pub(crate) fn clear_calendar_text_cache() {
    CALENDAR_TEXT.with(|cell| {
        cell.borrow_mut().year = u16::MAX;
    });
}

fn weekday_name(day: u16) -> String {
    match day {
        0 => tr("widget_calendar_sun"),
        1 => tr("widget_calendar_mon"),
        2 => tr("widget_calendar_tue"),
        3 => tr("widget_calendar_wed"),
        4 => tr("widget_calendar_thu"),
        5 => tr("widget_calendar_fri"),
        _ => tr("widget_calendar_sat"),
    }
}

fn month_name(month: u16) -> String {
    match month {
        1 => tr("widget_calendar_month_1"),
        2 => tr("widget_calendar_month_2"),
        3 => tr("widget_calendar_month_3"),
        4 => tr("widget_calendar_month_4"),
        5 => tr("widget_calendar_month_5"),
        6 => tr("widget_calendar_month_6"),
        7 => tr("widget_calendar_month_7"),
        8 => tr("widget_calendar_month_8"),
        9 => tr("widget_calendar_month_9"),
        10 => tr("widget_calendar_month_10"),
        11 => tr("widget_calendar_month_11"),
        _ => tr("widget_calendar_month_12"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_calendar_widget(
    painter: Painter<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale: f32,
    alpha: u8,
    text_color: Rgba,
) {
    // SAFETY: GetLocalTime writes a SYSTEMTIME value and has no preconditions.
    let local_time = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };

    draw_widget_rounded_background(painter, x, y, w, h, scale, alpha);

    let month_color = text_color.with_alpha((alpha as f32 * 0.78) as u8);
    let day_color = text_color.with_alpha(alpha);
    let weekday_color = text_color.with_alpha((alpha as f32 * 0.62) as u8);
    CALENDAR_TEXT.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.year != local_time.wYear
            || cache.month != local_time.wMonth
            || cache.day != local_time.wDay
            || cache.weekday != local_time.wDayOfWeek
        {
            cache.year = local_time.wYear;
            cache.month = local_time.wMonth;
            cache.day = local_time.wDay;
            cache.weekday = local_time.wDayOfWeek;
            cache.month_text = month_name(local_time.wMonth);
            cache.day_text = local_time.wDay.to_string();
            cache.weekday_text = weekday_name(local_time.wDayOfWeek);
        }

        draw_widget_text_centered(
            painter,
            &cache.month_text,
            Rect::from_xywh(x, y + h * 0.08, w, h * 0.18),
            (h * 0.14).clamp(9.0 * scale, 15.0 * scale),
            true,
            month_color,
        );

        let day_size = (h * 0.53).min(w * 0.70).max(26.0 * scale);
        draw_widget_text_centered(
            painter,
            &cache.day_text,
            Rect::from_xywh(x, y + h * 0.27, w, h * 0.48),
            day_size,
            true,
            day_color,
        );

        draw_widget_text_centered(
            painter,
            &cache.weekday_text,
            Rect::from_xywh(x, y + h * 0.78, w, h * 0.14),
            (h * 0.12).clamp(8.0 * scale, 12.0 * scale),
            false,
            weekday_color,
        );
    });
}
