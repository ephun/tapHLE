/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSCalendar` and `NSDateComponents`.
//!
//! This is a pragmatic implementation covering the Gregorian calendar in GMT,
//! which is what the CoreFoundation date helpers this reuses assume. It is
//! enough for apps that read the components of a date (year, month, day, hour,
//! …) or build a date from components — e.g. daily-reward and timestamp logic.
//! Locales, time zones and non-Gregorian calendars are not modelled.

use super::{ns_string, NSInteger, NSTimeInterval, NSUInteger};
use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::core_foundation::time::SECS_FROM_UNIX_TO_APPLE_EPOCHS;
use crate::libc::time::{calendar_date_to_timestamp, time_t, timestamp_to_calendar_date, tm};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, ClassExports, HostObject,
    NSZonePtr,
};
use crate::Environment;

/// `NSUndefinedDateComponent` — the value an unset component reads back as.
/// This is `NSIntegerMax`, which is `i32::MAX` on the 32-bit guest ABI.
const NS_UNDEFINED_DATE_COMPONENT: NSInteger = NSInteger::MAX;

/// Calendar identifiers. Their values are the CoreFoundation identifier
/// strings, which is what `-initWithCalendarIdentifier:` above already compares
/// against.
///
/// Only the Gregorian calendar is actually implemented, so an app asking for
/// one of the others gets Gregorian behaviour. Exporting them anyway is still
/// strictly better than leaving them unbound: an unbound one is a null pointer
/// that the app dereferences without checking, and a survey of 1300 apps found
/// twelve — Bad Piggies, Pitfall!, Organ Trail and others — dying on exactly
/// that read of `NSBuddhistCalendar` before reaching any of their own code.
const NSGregorianCalendar: &str = "gregorian";
const NSBuddhistCalendar: &str = "buddhist";
const NSChineseCalendar: &str = "chinese";
const NSHebrewCalendar: &str = "hebrew";
const NSIslamicCalendar: &str = "islamic";
const NSIslamicCivilCalendar: &str = "islamic-civil";
const NSJapaneseCalendar: &str = "japanese";
const NSRepublicOfChinaCalendar: &str = "roc";
const NSPersianCalendar: &str = "persian";
const NSIndianCalendar: &str = "indian";
const NSISO8601Calendar: &str = "iso8601";

pub const CONSTANTS: ConstantExports = &[
    (
        "_NSGregorianCalendar",
        HostConstant::NSString(NSGregorianCalendar),
    ),
    (
        "_NSBuddhistCalendar",
        HostConstant::NSString(NSBuddhistCalendar),
    ),
    (
        "_NSChineseCalendar",
        HostConstant::NSString(NSChineseCalendar),
    ),
    (
        "_NSHebrewCalendar",
        HostConstant::NSString(NSHebrewCalendar),
    ),
    (
        "_NSIslamicCalendar",
        HostConstant::NSString(NSIslamicCalendar),
    ),
    (
        "_NSIslamicCivilCalendar",
        HostConstant::NSString(NSIslamicCivilCalendar),
    ),
    (
        "_NSJapaneseCalendar",
        HostConstant::NSString(NSJapaneseCalendar),
    ),
    (
        "_NSRepublicOfChinaCalendar",
        HostConstant::NSString(NSRepublicOfChinaCalendar),
    ),
    (
        "_NSPersianCalendar",
        HostConstant::NSString(NSPersianCalendar),
    ),
    (
        "_NSIndianCalendar",
        HostConstant::NSString(NSIndianCalendar),
    ),
    (
        "_NSISO8601Calendar",
        HostConstant::NSString(NSISO8601Calendar),
    ),
];

// Pre-iOS-8 `NSCalendarUnit` flags (the same bits as `kCFCalendarUnit*`), which
// is what an iPhone OS 3.2 app uses.
const NS_ERA_UNIT: NSUInteger = 1 << 1;
const NS_YEAR_UNIT: NSUInteger = 1 << 2;
const NS_MONTH_UNIT: NSUInteger = 1 << 3;
const NS_DAY_UNIT: NSUInteger = 1 << 4;
const NS_HOUR_UNIT: NSUInteger = 1 << 5;
const NS_MINUTE_UNIT: NSUInteger = 1 << 6;
const NS_SECOND_UNIT: NSUInteger = 1 << 7;
const NS_WEEK_UNIT: NSUInteger = 1 << 8;
const NS_WEEKDAY_UNIT: NSUInteger = 1 << 9;

#[derive(Default)]
struct NSDateComponentsHostObject {
    era: Option<NSInteger>,
    year: Option<NSInteger>,
    month: Option<NSInteger>,
    day: Option<NSInteger>,
    hour: Option<NSInteger>,
    minute: Option<NSInteger>,
    second: Option<NSInteger>,
    week: Option<NSInteger>,
    weekday: Option<NSInteger>,
}
impl HostObject for NSDateComponentsHostObject {}

struct NSCalendarHostObject {
    /// The calendar identifier string (e.g. "gregorian"). Only Gregorian
    /// behaviour is implemented, but the identifier is stored so
    /// -calendarIdentifier round-trips.
    identifier: id,
}
impl HostObject for NSCalendarHostObject {}

/// Convert a date's components into a `struct tm` (Gregorian, GMT).
fn tm_from_date(env: &mut Environment, date: id) -> tm {
    let since_reference: NSTimeInterval = msg![env; date timeIntervalSinceReferenceDate];
    let unix_secs = since_reference + SECS_FROM_UNIX_TO_APPLE_EPOCHS as f64;
    timestamp_to_calendar_date(unix_secs as time_t)
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSDateComponents: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<NSDateComponentsHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)copyWithZone:(NSZonePtr)_zone {
    let &NSDateComponentsHostObject {
        era, year, month, day, hour, minute, second, week, weekday,
    } = env.objc.borrow(this);
    let new: id = msg_class![env; NSDateComponents alloc];
    *env.objc.borrow_mut(new) = NSDateComponentsHostObject {
        era, year, month, day, hour, minute, second, week, weekday,
    };
    new
}

- (NSInteger)era { component_or_undefined(env.objc.borrow::<NSDateComponentsHostObject>(this).era) }
- (())setEra:(NSInteger)v { env.objc.borrow_mut::<NSDateComponentsHostObject>(this).era = Some(v); }
- (NSInteger)year { component_or_undefined(env.objc.borrow::<NSDateComponentsHostObject>(this).year) }
- (())setYear:(NSInteger)v { env.objc.borrow_mut::<NSDateComponentsHostObject>(this).year = Some(v); }
- (NSInteger)month { component_or_undefined(env.objc.borrow::<NSDateComponentsHostObject>(this).month) }
- (())setMonth:(NSInteger)v { env.objc.borrow_mut::<NSDateComponentsHostObject>(this).month = Some(v); }
- (NSInteger)day { component_or_undefined(env.objc.borrow::<NSDateComponentsHostObject>(this).day) }
- (())setDay:(NSInteger)v { env.objc.borrow_mut::<NSDateComponentsHostObject>(this).day = Some(v); }
- (NSInteger)hour { component_or_undefined(env.objc.borrow::<NSDateComponentsHostObject>(this).hour) }
- (())setHour:(NSInteger)v { env.objc.borrow_mut::<NSDateComponentsHostObject>(this).hour = Some(v); }
- (NSInteger)minute { component_or_undefined(env.objc.borrow::<NSDateComponentsHostObject>(this).minute) }
- (())setMinute:(NSInteger)v { env.objc.borrow_mut::<NSDateComponentsHostObject>(this).minute = Some(v); }
- (NSInteger)second { component_or_undefined(env.objc.borrow::<NSDateComponentsHostObject>(this).second) }
- (())setSecond:(NSInteger)v { env.objc.borrow_mut::<NSDateComponentsHostObject>(this).second = Some(v); }
- (NSInteger)week { component_or_undefined(env.objc.borrow::<NSDateComponentsHostObject>(this).week) }
- (())setWeek:(NSInteger)v { env.objc.borrow_mut::<NSDateComponentsHostObject>(this).week = Some(v); }
- (NSInteger)weekday { component_or_undefined(env.objc.borrow::<NSDateComponentsHostObject>(this).weekday) }
- (())setWeekday:(NSInteger)v { env.objc.borrow_mut::<NSDateComponentsHostObject>(this).weekday = Some(v); }

@end

@implementation NSCalendar: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSCalendarHostObject { identifier: nil });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)currentCalendar {
    // We only implement the Gregorian calendar.
    let ident: id = ns_string::get_static_str(env, "gregorian");
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCalendarIdentifier:ident];
    autorelease(env, new)
}

+ (id)autoupdatingCurrentCalendar {
    msg_class![env; NSCalendar currentCalendar]
}

- (id)initWithCalendarIdentifier:(id)identifier { // NSString*
    if identifier != nil {
        let name = ns_string::to_rust_string(env, identifier);
        if name != NSGregorianCalendar {
            log!(
                "TODO: [NSCalendar initWithCalendarIdentifier:{:?}] - only the Gregorian calendar is implemented, so this will behave as Gregorian",
                name
            );
        }
    }
    let identifier: id = msg![env; identifier copy];
    env.objc.borrow_mut::<NSCalendarHostObject>(this).identifier = identifier;
    this
}

- (id)calendarIdentifier {
    env.objc.borrow::<NSCalendarHostObject>(this).identifier
}

- (())dealloc {
    let identifier = env.objc.borrow::<NSCalendarHostObject>(this).identifier;
    release(env, identifier);
    env.objc.dealloc_object(this, &mut env.mem)
}

// Time zone and locale are accepted and ignored: this calendar is always
// Gregorian in GMT.
- (())setTimeZone:(id)_time_zone {}
- (id)timeZone { nil }
- (())setLocale:(id)_locale {}
- (id)locale { nil }
- (())setFirstWeekday:(NSUInteger)_weekday {}
- (NSUInteger)firstWeekday { 1 }

- (id)components:(NSUInteger)unit_flags
        fromDate:(id)date { // NSDate*
    let tm = tm_from_date(env, date);
    let components: id = msg_class![env; NSDateComponents alloc];
    let components: id = msg![env; components init];
    if unit_flags & NS_ERA_UNIT != 0 {
        () = msg![env; components setEra:1i32];
    }
    if unit_flags & NS_YEAR_UNIT != 0 {
        () = msg![env; components setYear:(1900 + tm.tm_year)];
    }
    if unit_flags & NS_MONTH_UNIT != 0 {
        () = msg![env; components setMonth:(tm.tm_mon + 1)];
    }
    if unit_flags & NS_DAY_UNIT != 0 {
        () = msg![env; components setDay:(tm.tm_mday)];
    }
    if unit_flags & NS_HOUR_UNIT != 0 {
        () = msg![env; components setHour:(tm.tm_hour)];
    }
    if unit_flags & NS_MINUTE_UNIT != 0 {
        () = msg![env; components setMinute:(tm.tm_min)];
    }
    if unit_flags & NS_SECOND_UNIT != 0 {
        () = msg![env; components setSecond:(tm.tm_sec)];
    }
    if unit_flags & NS_WEEKDAY_UNIT != 0 {
        // NSCalendar weekdays are 1-based with Sunday = 1; tm_wday is 0-based
        // with Sunday = 0.
        () = msg![env; components setWeekday:(tm.tm_wday + 1)];
    }
    let _ = NS_WEEK_UNIT; // week-of-year not modelled
    autorelease(env, components)
}

// The difference between two dates, broken into the requested units.
//
// Units are filled largest-first, each taking as much of the remaining span as
// it can and leaving the rest to the next one, which is what makes "1 month and
// 3 days" come out of a single call. Only the requested units are set; the rest
// stay undefined, as Apple's does.
//
// Years and months are done as calendar arithmetic rather than by dividing
// elapsed seconds, because they are not fixed-length: from 31 January, one
// month is 28 days, and the answer has to agree with what a calendar says. The
// remaining units are fixed-length and are divided out of the leftover seconds.
//
// A `toDate` earlier than `fromDate` gives negative components, computed by
// swapping the two and negating, so the magnitudes match the forward direction
// exactly.
//
// `options` selects wrapping arithmetic, which nothing here needs and which is
// ignored; a caller passing NSWrapCalendarComponents gets the unwrapped answer.
- (id)components:(NSUInteger)unit_flags
        fromDate:(id)from_date // NSDate*
          toDate:(id)to_date // NSDate*
         options:(NSUInteger)_options {
    let from_interval: NSTimeInterval = msg![env; from_date timeIntervalSinceReferenceDate];
    let to_interval: NSTimeInterval = msg![env; to_date timeIntervalSinceReferenceDate];
    let negative = to_interval < from_interval;
    let (earlier, later) = if negative { (to_date, from_date) } else { (from_date, to_date) };

    let mut base = tm_from_date(env, earlier);
    let end = tm_from_date(env, later);
    let end_secs = calendar_date_to_timestamp(end);

    let mut years = 0;
    if unit_flags & NS_YEAR_UNIT != 0 {
        years = whole_units_between(base, end, 12) / 12;
        base = add_months(base, years * 12);
    }
    let mut months = 0;
    if unit_flags & NS_MONTH_UNIT != 0 {
        months = whole_units_between(base, end, 1);
        base = add_months(base, months);
    }

    let mut remaining = i64::from(end_secs) - i64::from(calendar_date_to_timestamp(base));
    let mut take = |unit: NSUInteger, seconds_per: i64| -> NSInteger {
        if unit_flags & unit == 0 {
            return 0;
        }
        let count = remaining / seconds_per;
        remaining -= count * seconds_per;
        count as NSInteger
    };
    let weeks = take(NS_WEEK_UNIT, 7 * 24 * 60 * 60);
    let days = take(NS_DAY_UNIT, 24 * 60 * 60);
    let hours = take(NS_HOUR_UNIT, 60 * 60);
    let minutes = take(NS_MINUTE_UNIT, 60);
    let seconds = take(NS_SECOND_UNIT, 1);

    let sign = if negative { -1 } else { 1 };
    let components: id = msg_class![env; NSDateComponents alloc];
    let components: id = msg![env; components init];
    if unit_flags & NS_YEAR_UNIT != 0 {
        () = msg![env; components setYear:(sign * years as NSInteger)];
    }
    if unit_flags & NS_MONTH_UNIT != 0 {
        () = msg![env; components setMonth:(sign * months as NSInteger)];
    }
    if unit_flags & NS_WEEK_UNIT != 0 {
        () = msg![env; components setWeek:(sign * weeks)];
    }
    if unit_flags & NS_DAY_UNIT != 0 {
        () = msg![env; components setDay:(sign * days)];
    }
    if unit_flags & NS_HOUR_UNIT != 0 {
        () = msg![env; components setHour:(sign * hours)];
    }
    if unit_flags & NS_MINUTE_UNIT != 0 {
        () = msg![env; components setMinute:(sign * minutes)];
    }
    if unit_flags & NS_SECOND_UNIT != 0 {
        () = msg![env; components setSecond:(sign * seconds)];
    }
    autorelease(env, components)
}

- (id)dateFromComponents:(id)components { // NSDateComponents*
    let year: NSInteger = msg![env; components year];
    let month: NSInteger = msg![env; components month];
    let day: NSInteger = msg![env; components day];
    let hour: NSInteger = msg![env; components hour];
    let minute: NSInteger = msg![env; components minute];
    let second: NSInteger = msg![env; components second];

    // Undefined components default to the start of their range, as NSCalendar
    // does (year 1, month 1, day 1, and 0 for the time fields).
    let defined = |v: NSInteger, default: NSInteger| if v == NS_UNDEFINED_DATE_COMPONENT { default } else { v };
    let tm_value = tm::from(
        defined(year, 1) as u16,
        defined(month, 1) as u8,
        defined(day, 1) as u8,
        defined(hour, 0) as u8,
        defined(minute, 0) as u8,
        defined(second, 0) as u8,
    );
    let unix_secs = calendar_date_to_timestamp(tm_value);
    let since_reference = unix_secs as NSTimeInterval - SECS_FROM_UNIX_TO_APPLE_EPOCHS as f64;
    msg_class![env; NSDate dateWithTimeIntervalSinceReferenceDate:since_reference]
}

@end

};

/// Days in a Gregorian month. `month` is 1-based.
fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => unreachable!("month {month} out of range"),
    }
}

/// Move a date forward by whole months, keeping the time of day.
///
/// The day of month is clamped to the target month's length, so 31 January plus
/// one month is 28 February rather than an overflow into March. That is what
/// NSCalendar does and what makes month arithmetic reversible in the common
/// case.
fn add_months(date: tm, months: i64) -> tm {
    let total = (1900 + date.tm_year) as i64 * 12 + date.tm_mon as i64 + months;
    let year = total.div_euclid(12) as i32;
    let month = total.rem_euclid(12) as i32 + 1;
    let day = date.tm_mday.min(days_in_month(year, month));
    tm::from(
        year as u16,
        month as u8,
        day as u8,
        date.tm_hour as u8,
        date.tm_min as u8,
        date.tm_sec as u8,
    )
}

/// How many whole `step`-month intervals fit between two dates.
///
/// "Whole" is the point: the count is reduced until adding it to `from` no
/// longer overshoots `to`, so a difference of one month less a second reports
/// zero months rather than one.
fn whole_units_between(from: tm, to: tm, step: i64) -> i64 {
    let to_secs = calendar_date_to_timestamp(to);
    let months = ((1900 + to.tm_year) as i64 * 12 + to.tm_mon as i64)
        - ((1900 + from.tm_year) as i64 * 12 + from.tm_mon as i64);
    let mut count = (months / step) * step;
    while count > 0 && calendar_date_to_timestamp(add_months(from, count)) > to_secs {
        count -= step;
    }
    count
}

/// Map an optional component to the guest-visible value, using
/// `NSUndefinedDateComponent` for an unset one.
fn component_or_undefined(value: Option<NSInteger>) -> NSInteger {
    value.unwrap_or(NS_UNDEFINED_DATE_COMPONENT)
}
