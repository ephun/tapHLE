/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Turning instants and durations into the short strings the interface shows.
//!
//! Timestamps are stored as seconds or milliseconds since the Unix epoch,
//! which is what [std::time::SystemTime] gives without any dependency. The
//! standard library has no way to ask what the local time zone is, so the
//! offset is read from the operating system once, on Windows, where a real
//! clock time in a log matters most. Elsewhere the offset is zero and times
//! read as UTC; the interface labels them, and reading a time zone on those
//! platforms is left for later.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds to add to UTC to get the local wall clock, as of program start.
///
/// It is read once rather than per timestamp. A machine that crosses a
/// daylight-saving boundary while the frontend is open will be an hour out
/// until it is restarted, which is not worth a per-call system call.
pub fn local_offset_seconds() -> i64 {
    static OFFSET: OnceLock<i64> = OnceLock::new();
    *OFFSET.get_or_init(read_local_offset_seconds)
}

#[cfg(windows)]
fn read_local_offset_seconds() -> i64 {
    // SYSTEMTIME and these two calls have been stable since Windows NT. They
    // are declared here rather than pulled from a binding crate because this
    // is the only Windows API the frontend needs.
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct SystemTimeFields {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    extern "system" {
        fn GetSystemTime(out: *mut SystemTimeFields);
        fn GetLocalTime(out: *mut SystemTimeFields);
    }

    fn as_seconds(t: SystemTimeFields) -> i64 {
        let days = days_from_civil(t.year as i64, t.month as u32, t.day as u32);
        days * 86_400 + t.hour as i64 * 3600 + t.minute as i64 * 60 + t.second as i64
    }

    let (utc, local) = unsafe {
        let mut utc = SystemTimeFields::default();
        let mut local = SystemTimeFields::default();
        // Read UTC either side of the local reading so that a clock tick
        // between the two calls cannot be mistaken for a one-second offset.
        GetSystemTime(&mut utc);
        GetLocalTime(&mut local);
        (utc, local)
    };
    let difference = as_seconds(local) - as_seconds(utc);
    // Every real time zone is a whole number of minutes from UTC, so rounding
    // to the nearest minute removes the sub-second skew between the calls.
    (difference as f64 / 60.0).round() as i64 * 60
}

#[cfg(not(windows))]
fn read_local_offset_seconds() -> i64 {
    0
}

/// Whether displayed times are UTC rather than local, so the interface can
/// say so instead of quietly showing the wrong hour.
pub fn times_are_utc() -> bool {
    local_offset_seconds() == 0 && !cfg!(windows)
}

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn now_seconds() -> u64 {
    now_millis() / 1000
}

/// Days from the Unix epoch to a proleptic Gregorian date, and back.
///
/// These are Howard Hinnant's `days_from_civil` and `civil_from_days`, which
/// are the standard way to do this without a calendar library. They are exact
/// for every year the frontend will ever see.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = month as i64;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day as i64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = (month_prime + if month_prime < 10 { 3 } else { -9 }) as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Split a Unix timestamp into local calendar and clock fields.
fn local_parts(seconds: u64) -> (i64, u32, u32, u32, u32, u32) {
    let shifted = seconds as i64 + local_offset_seconds();
    let days = shifted.div_euclid(86_400);
    let time_of_day = shifted.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    (
        year,
        month,
        day,
        (time_of_day / 3600) as u32,
        (time_of_day % 3600 / 60) as u32,
        (time_of_day % 60) as u32,
    )
}

/// `HH:MM:SS.mmm`, the time column of the log panel.
pub fn format_clock(millis: u64) -> String {
    let (_, _, _, hour, minute, second) = local_parts(millis / 1000);
    format!("{hour:02}:{minute:02}:{second:02}.{:03}", millis % 1000)
}

/// `YYYY-MM-DD HH:MM`, for anywhere an exact instant is wanted.
pub fn format_datetime(seconds: u64) -> String {
    let (year, month, day, hour, minute, _) = local_parts(seconds);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

/// `YYYY-MM-DD`, for file names and report drafts.
pub fn format_date(seconds: u64) -> String {
    let (year, month, day, _, _, _) = local_parts(seconds);
    format!("{year:04}-{month:02}-{day:02}")
}

/// A file-name-safe stamp, used when suggesting a name for an exported log.
pub fn format_file_stamp(seconds: u64) -> String {
    let (year, month, day, hour, minute, second) = local_parts(seconds);
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
}

/// How long ago something happened, in the terms a person would use.
///
/// The details panel shows this rather than a bare date because "yesterday"
/// answers the question "have I played this recently" without arithmetic. The
/// exact date is still available as a tooltip.
pub fn format_relative(seconds: u64, now: u64) -> String {
    if seconds > now {
        return format_datetime(seconds);
    }
    let elapsed = now - seconds;
    match elapsed {
        0..=59 => "just now".to_string(),
        60..=3599 => {
            let minutes = elapsed / 60;
            format!("{minutes} minute{} ago", plural(minutes))
        }
        3600..=86_399 => {
            let hours = elapsed / 3600;
            format!("{hours} hour{} ago", plural(hours))
        }
        86_400..=604_799 => {
            let days = elapsed / 86_400;
            format!("{days} day{} ago", plural(days))
        }
        _ => format_date(seconds),
    }
}

/// A played-time total, e.g. `2 h 14 min`.
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds} sec");
    }
    let hours = seconds / 3600;
    let minutes = seconds % 3600 / 60;
    if hours == 0 {
        format!("{minutes} min")
    } else {
        format!("{hours} h {minutes:02} min")
    }
}

fn plural(count: u64) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::{civil_from_days, days_from_civil, format_duration, format_relative, plural};

    /// The two calendar conversions have to be exact inverses; every
    /// displayed date depends on it, and an off-by-one would only show up as
    /// a subtly wrong date months later.
    #[test]
    fn the_calendar_conversions_are_inverses() {
        for &(year, month, day) in &[
            (1970, 1, 1),
            (1999, 12, 31),
            (2000, 2, 29),
            (2026, 8, 13),
            (2100, 3, 1),
        ] {
            let days = days_from_civil(year, month, day);
            assert_eq!(civil_from_days(days), (year, month, day));
        }
    }

    /// The epoch is day zero. This pins the origin, which the round trip
    /// above would not catch if both directions were shifted equally.
    #[test]
    fn the_epoch_is_day_zero() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn relative_times_read_as_prose() {
        let now = 1_000_000;
        assert_eq!(format_relative(now, now), "just now");
        assert_eq!(format_relative(now - 60, now), "1 minute ago");
        assert_eq!(format_relative(now - 7200, now), "2 hours ago");
        assert_eq!(format_relative(now - 86_400, now), "1 day ago");
    }

    /// A timestamp in the future is a clock change or an edited file, not a
    /// negative duration. It must not panic on the subtraction.
    #[test]
    fn a_future_timestamp_falls_back_to_an_exact_time() {
        let text = format_relative(2_000_000, 1_000_000);
        assert!(text.contains('-'), "expected a date, got {text:?}");
    }

    #[test]
    fn durations_are_readable() {
        assert_eq!(format_duration(5), "5 sec");
        assert_eq!(format_duration(90), "1 min");
        assert_eq!(format_duration(8100), "2 h 15 min");
    }

    #[test]
    fn one_is_singular() {
        assert_eq!(plural(1), "");
        assert_eq!(plural(0), "s");
        assert_eq!(plural(2), "s");
    }
}
