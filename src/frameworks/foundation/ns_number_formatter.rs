/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSNumberFormatter`.
//!
//! Only the decimal styles are modelled, in the POSIX locale: a game or an SDK
//! embedded in one uses this to turn a score or an identifier into a string,
//! not to typeset currency for a specific region. A style this does not model
//! falls back to the plain decimal rendering and says so once, rather than
//! silently producing something that looks localised but is not.
//!
//! Resources:
//! - Apple's [Data Formatting Guide](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/DataFormatting/DataFormatting.html)

use crate::frameworks::foundation::{ns_string, NSInteger, NSUInteger};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};

pub type NSNumberFormatterStyle = NSUInteger;
pub const NSNumberFormatterNoStyle: NSNumberFormatterStyle = 0;
pub const NSNumberFormatterDecimalStyle: NSNumberFormatterStyle = 1;
// The rest of the enum. `setNumberStyle:` accepts these and warns that it
// renders them as plain decimal, so nothing here references them by name; they
// are written out because a partial enum invites someone to guess the values.
#[allow(dead_code)]
pub const NSNumberFormatterCurrencyStyle: NSNumberFormatterStyle = 2;
#[allow(dead_code)]
pub const NSNumberFormatterPercentStyle: NSNumberFormatterStyle = 3;
#[allow(dead_code)]
pub const NSNumberFormatterScientificStyle: NSNumberFormatterStyle = 4;
#[allow(dead_code)]
pub const NSNumberFormatterSpellOutStyle: NSNumberFormatterStyle = 5;

pub type NSNumberFormatterPadPosition = NSUInteger;
pub const NSNumberFormatterPadBeforePrefix: NSNumberFormatterPadPosition = 0;

struct NSNumberFormatterHostObject {
    style: NSNumberFormatterStyle,
    minimum_fraction_digits: NSUInteger,
    maximum_fraction_digits: NSUInteger,
    uses_grouping_separator: bool,
    /// Stored but not used when formatting; the currency style is not modelled.
    currency_code: Option<String>,
    /// Stored but not used when formatting; the currency style is not modelled.
    currency_symbol: Option<String>,
    /// Affixes, which *are* applied by -stringFromNumber:.
    positive_prefix: Option<String>,
    positive_suffix: Option<String>,
    negative_prefix: Option<String>,
    negative_suffix: Option<String>,
    grouping_separator: Option<String>,
    decimal_separator: Option<String>,
    /// The character standing in for "negative". Reaches the output: it is the
    /// default negative prefix, used when the app has not set one explicitly.
    minus_sign: Option<String>,
    /// Stored and reported back, but it does not reach the output — none of the
    /// styles modelled here ever shows a plus, so applying it would add a sign
    /// the app did not ask for.
    plus_sign: Option<String>,
    /// `NSNumber*` or nil. Applied by -stringFromNumber:, as documented.
    multiplier: id,
    /// ICU pattern strings. Stored and reported back, but not interpreted:
    /// see the note on -setFormat:.
    positive_format: Option<String>,
    negative_format: Option<String>,
    padding_position: NSUInteger,
    generates_decimal_numbers: bool,
}
impl HostObject for NSNumberFormatterHostObject {}

/// Insert `,` every three digits, left of the decimal point.
fn group(integer_part: &str) -> String {
    let (sign, digits) = match integer_part.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", integer_part),
    };
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{sign}{grouped}")
}

/// Read an optional configuration string back out as an autoreleased NSString.
fn optional_string(
    env: &mut crate::Environment,
    formatter: id,
    get: impl Fn(&NSNumberFormatterHostObject) -> Option<String>,
) -> id {
    let Some(value) = get(env.objc.borrow::<NSNumberFormatterHostObject>(formatter)) else {
        return nil;
    };
    let string = ns_string::from_rust_string(env, value);
    autorelease(env, string)
}

/// Convert an incoming NSString configuration value, treating nil as unset.
fn optional_rust_string(env: &mut crate::Environment, value: id) -> Option<String> {
    if value == nil {
        None
    } else {
        Some(ns_string::to_rust_string(env, value).to_string())
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSNumberFormatter: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSNumberFormatterHostObject {
        style: NSNumberFormatterNoStyle,
        minimum_fraction_digits: 0,
        // NSNumberFormatterNoStyle rounds to an integer; the decimal style's
        // documented default is 3.
        maximum_fraction_digits: 0,
        uses_grouping_separator: false,
        currency_code: None,
        currency_symbol: None,
        positive_prefix: None,
        positive_suffix: None,
        negative_prefix: None,
        negative_suffix: None,
        grouping_separator: None,
        decimal_separator: None,
        minus_sign: None,
        plus_sign: None,
        multiplier: nil,
        positive_format: None,
        negative_format: None,
        padding_position: NSNumberFormatterPadBeforePrefix,
        generates_decimal_numbers: false,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (NSNumberFormatterStyle)numberStyle {
    env.objc.borrow::<NSNumberFormatterHostObject>(this).style
}
- (())setNumberStyle:(NSNumberFormatterStyle)style {
    if style != NSNumberFormatterNoStyle && style != NSNumberFormatterDecimalStyle {
        log_once!("TODO: NSNumberFormatter only models the no-style and decimal styles; others render as plain decimal");
    }
    let host_object = env.objc.borrow_mut::<NSNumberFormatterHostObject>(this);
    host_object.style = style;
    if style == NSNumberFormatterDecimalStyle {
        host_object.maximum_fraction_digits = 3;
        host_object.uses_grouping_separator = true;
    } else {
        host_object.maximum_fraction_digits = 0;
        host_object.uses_grouping_separator = false;
    }
}

- (NSUInteger)minimumFractionDigits {
    env.objc.borrow::<NSNumberFormatterHostObject>(this).minimum_fraction_digits
}
- (())setMinimumFractionDigits:(NSUInteger)digits {
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).minimum_fraction_digits = digits;
}

- (NSUInteger)maximumFractionDigits {
    env.objc.borrow::<NSNumberFormatterHostObject>(this).maximum_fraction_digits
}
- (())setMaximumFractionDigits:(NSUInteger)digits {
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).maximum_fraction_digits = digits;
}

- (bool)usesGroupingSeparator {
    env.objc.borrow::<NSNumberFormatterHostObject>(this).uses_grouping_separator
}
- (())setUsesGroupingSeparator:(bool)uses {
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).uses_grouping_separator = uses;
}

// Accepted so that a formatter configured the usual way behaves; tapHLE models
// only the POSIX locale, so there is nothing to switch.
- (())setLocale:(id)_locale {
    log_once!("TODO: NSNumberFormatter ignores -setLocale:, formatting is always POSIX");
}
- (())setFormatterBehavior:(NSInteger)_behavior {
}

// Currency configuration is stored so a caller reads back what it wrote, but
// the currency style itself is not modelled, so it does not reach the output.
- (id)currencyCode {
    optional_string(env, this, |host_object| host_object.currency_code.clone())
}
- (())setCurrencyCode:(id)code { // NSString*
    log_once!("TODO: NSNumberFormatter stores the currency code but does not format currency");
    let code = optional_rust_string(env, code);
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).currency_code = code;
}

- (id)currencySymbol {
    optional_string(env, this, |host_object| host_object.currency_symbol.clone())
}
- (())setCurrencySymbol:(id)symbol { // NSString*
    let symbol = optional_rust_string(env, symbol);
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).currency_symbol = symbol;
}

// Affixes and separators. Unlike the currency settings these do reach the
// output, which is why an app can rely on them.
- (id)positivePrefix {
    optional_string(env, this, |h| h.positive_prefix.clone())
}
- (())setPositivePrefix:(id)value { // NSString*
    let value = optional_rust_string(env, value);
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).positive_prefix = value;
}
- (id)positiveSuffix {
    optional_string(env, this, |h| h.positive_suffix.clone())
}
- (())setPositiveSuffix:(id)value { // NSString*
    let value = optional_rust_string(env, value);
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).positive_suffix = value;
}
- (id)negativePrefix {
    optional_string(env, this, |h| h.negative_prefix.clone())
}
- (())setNegativePrefix:(id)value { // NSString*
    let value = optional_rust_string(env, value);
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).negative_prefix = value;
}
- (id)negativeSuffix {
    optional_string(env, this, |h| h.negative_suffix.clone())
}
- (())setNegativeSuffix:(id)value { // NSString*
    let value = optional_rust_string(env, value);
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).negative_suffix = value;
}
- (id)minusSign {
    optional_string(env, this, |h| h.minus_sign.clone())
}
- (())setMinusSign:(id)value { // NSString*
    let value = optional_rust_string(env, value);
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).minus_sign = value;
}
- (id)plusSign {
    optional_string(env, this, |h| h.plus_sign.clone())
}
- (())setPlusSign:(id)value { // NSString*
    let value = optional_rust_string(env, value);
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).plus_sign = value;
}
- (id)groupingSeparator {
    optional_string(env, this, |h| h.grouping_separator.clone())
}
- (())setGroupingSeparator:(id)value { // NSString*
    let value = optional_rust_string(env, value);
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).grouping_separator = value;
}
- (id)decimalSeparator {
    optional_string(env, this, |h| h.decimal_separator.clone())
}
- (())setDecimalSeparator:(id)value { // NSString*
    let value = optional_rust_string(env, value);
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).decimal_separator = value;
}

// The value is multiplied by this before being formatted, which is how the
// percent style is built out of the decimal one.
- (id)multiplier {
    env.objc.borrow::<NSNumberFormatterHostObject>(this).multiplier
}
- (())setMultiplier:(id)multiplier { // NSNumber*
    retain(env, multiplier);
    let host_object = env.objc.borrow_mut::<NSNumberFormatterHostObject>(this);
    let old = std::mem::replace(&mut host_object.multiplier, multiplier);
    release(env, old);
}

// The ICU pattern properties. These are stored and reported back so a caller
// reads what it wrote, but they are not interpreted: implementing the pattern
// language would be a much larger job than the numeric formatting this class
// actually does here, and pretending to honour a pattern would be worse than
// saying plainly that it is ignored. The affix and fraction-digit properties
// above cover what these patterns are usually set up to express.
- (id)positiveFormat {
    optional_string(env, this, |h| h.positive_format.clone())
}
- (())setPositiveFormat:(id)value { // NSString*
    log_once!("TODO: NSNumberFormatter stores format patterns but does not interpret them");
    let value = optional_rust_string(env, value);
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).positive_format = value;
}
- (id)negativeFormat {
    optional_string(env, this, |h| h.negative_format.clone())
}
- (())setNegativeFormat:(id)value { // NSString*
    log_once!("TODO: NSNumberFormatter stores format patterns but does not interpret them");
    let value = optional_rust_string(env, value);
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).negative_format = value;
}
// -format is the older name for the positive pattern.
- (id)format {
    msg![env; this positiveFormat]
}
- (())setFormat:(id)value { // NSString*
    () = msg![env; this setPositiveFormat:value];
}

// Padding only has an effect together with a format width, which nothing has
// been observed to set, so this is stored and reported back only.
- (NSNumberFormatterPadPosition)paddingPosition {
    env.objc.borrow::<NSNumberFormatterHostObject>(this).padding_position
}
- (())setPaddingPosition:(NSNumberFormatterPadPosition)position {
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).padding_position = position;
}

// tapHLE has no NSDecimalNumber, so -numberFromString: always answers an
// NSNumber. Say so once rather than silently ignoring the request.
- (bool)generatesDecimalNumbers {
    env.objc.borrow::<NSNumberFormatterHostObject>(this).generates_decimal_numbers
}
- (())setGeneratesDecimalNumbers:(bool)generates {
    if generates {
        log_once!("TODO: NSNumberFormatter always returns NSNumber; setGeneratesDecimalNumbers:YES has no effect");
    }
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).generates_decimal_numbers = generates;
}

- (id)stringFromNumber:(id)number { // NSNumber*
    if number == nil {
        return nil;
    }
    let (
        minimum_fraction_digits,
        maximum_fraction_digits,
        uses_grouping_separator,
        positive_prefix,
        positive_suffix,
        negative_prefix,
        negative_suffix,
        grouping_separator,
        decimal_separator,
        minus_sign,
    ) = {
        let host_object = env.objc.borrow::<NSNumberFormatterHostObject>(this);
        (
            host_object.minimum_fraction_digits,
            host_object.maximum_fraction_digits,
            host_object.uses_grouping_separator,
            host_object.positive_prefix.clone(),
            host_object.positive_suffix.clone(),
            host_object.negative_prefix.clone(),
            host_object.negative_suffix.clone(),
            host_object.grouping_separator.clone(),
            host_object.decimal_separator.clone(),
            host_object.minus_sign.clone(),
        )
    };

    let mut value: f64 = msg![env; number doubleValue];
    let multiplier = env.objc.borrow::<NSNumberFormatterHostObject>(this).multiplier;
    if multiplier != nil {
        let multiplier: f64 = msg![env; multiplier doubleValue];
        value *= multiplier;
    }
    // Render with the most fraction digits allowed, then trim back to the
    // minimum, which is what "maximum/minimum fraction digits" means.
    let mut string = format!("{:.*}", maximum_fraction_digits as usize, value);
    if maximum_fraction_digits > minimum_fraction_digits && string.contains('.') {
        while string.ends_with('0')
            && string.split('.').nth(1).map_or(0, |f| f.len()) as NSUInteger
                > minimum_fraction_digits
        {
            string.pop();
        }
        if string.ends_with('.') {
            string.pop();
        }
    }

    if uses_grouping_separator {
        string = match string.split_once('.') {
            Some((integer_part, fraction)) => format!("{}.{}", group(integer_part), fraction),
            None => group(&string),
        };
    }

    if let Some(separator) = grouping_separator.as_deref() {
        string = string.replace(',', separator);
    }
    if let Some(separator) = decimal_separator.as_deref() {
        string = string.replace('.', separator);
    }

    // The negative affixes replace the leading "-" that formatting produced;
    // the positive ones simply wrap. This matches how the affix properties are
    // documented to work.
    string = if let Some(rest) = string.strip_prefix('-') {
        format!(
            "{}{}{}",
            negative_prefix
                .as_deref()
                .unwrap_or(minus_sign.as_deref().unwrap_or("-")),
            rest,
            negative_suffix.as_deref().unwrap_or("")
        )
    } else {
        format!(
            "{}{}{}",
            positive_prefix.as_deref().unwrap_or(""),
            string,
            positive_suffix.as_deref().unwrap_or("")
        )
    };

    let string = ns_string::from_rust_string(env, string);
    autorelease(env, string)
}

- (id)numberFromString:(id)string { // NSString*
    if string == nil {
        return nil;
    }
    let text = ns_string::to_rust_string(env, string);
    // Grouping separators are decoration, not part of the value.
    let text: String = text.chars().filter(|&c| c != ',').collect();
    let text = text.trim();
    // An integer stays an integer, so that -intValue on the result is exact.
    if let Ok(value) = text.parse::<i32>() {
        return msg_class![env; NSNumber numberWithInt:value];
    }
    match text.parse::<f64>() {
        Ok(value) => msg_class![env; NSNumber numberWithDouble:value],
        // Documented: nil when the string cannot be read as a number.
        Err(_) => nil,
    }
}

@end

};

#[cfg(test)]
mod tests {
    use super::group;

    #[test]
    fn grouping_inserts_separators_every_three_digits() {
        assert_eq!(group("1"), "1");
        assert_eq!(group("123"), "123");
        assert_eq!(group("1234"), "1,234");
        assert_eq!(group("1234567"), "1,234,567");
    }

    #[test]
    fn grouping_keeps_the_sign_outside() {
        assert_eq!(group("-1234"), "-1,234");
    }
}
