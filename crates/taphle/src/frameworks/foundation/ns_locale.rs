/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSLocale`.

use super::{ns_array, ns_string};
use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::core_foundation::cf_locale::{
    kCFLocaleCountryCode, kCFLocaleIdentifier, kCFLocaleLanguageCode,
};
use crate::objc::{
    autorelease, id, msg, nil, objc_classes, release, retain, ClassExports, HostObject, NSZonePtr,
};
use crate::window::{get_preferred_country_codes, get_preferred_language_codes};
use crate::Environment;

const NSLocaleCountryCode: &str = "NSLocaleCountryCode";
const NSLocaleIdentifier: &str = "NSLocaleIdentifier";
const NSLocaleLanguageCode: &str = "NSLocaleLanguageCode";

/// Read by apps choosing between metric and imperial units. tapHLE reports the
/// host region, and an unbound key is a null pointer the app dereferences.
const NSLocaleUsesMetricSystem: &str = "NSLocaleUsesMetricSystem";
const NSLocaleMeasurementSystem: &str = "NSLocaleMeasurementSystem";
const NSLocaleDecimalSeparator: &str = "NSLocaleDecimalSeparator";
const NSLocaleGroupingSeparator: &str = "NSLocaleGroupingSeparator";
const NSLocaleCurrencySymbol: &str = "NSLocaleCurrencySymbol";
const NSLocaleCurrencyCode: &str = "NSLocaleCurrencyCode";

pub const CONSTANTS: ConstantExports = &[
    (
        "_NSLocaleUsesMetricSystem",
        HostConstant::NSString(NSLocaleUsesMetricSystem),
    ),
    (
        "_NSLocaleMeasurementSystem",
        HostConstant::NSString(NSLocaleMeasurementSystem),
    ),
    (
        "_NSLocaleDecimalSeparator",
        HostConstant::NSString(NSLocaleDecimalSeparator),
    ),
    (
        "_NSLocaleGroupingSeparator",
        HostConstant::NSString(NSLocaleGroupingSeparator),
    ),
    (
        "_NSLocaleCurrencySymbol",
        HostConstant::NSString(NSLocaleCurrencySymbol),
    ),
    (
        "_NSLocaleCurrencyCode",
        HostConstant::NSString(NSLocaleCurrencyCode),
    ),
    (
        "_NSLocaleCountryCode",
        HostConstant::NSString(NSLocaleCountryCode),
    ),
    (
        "_NSLocaleIdentifier",
        HostConstant::NSString(NSLocaleIdentifier),
    ),
    (
        "_NSLocaleLanguageCode",
        HostConstant::NSString(NSLocaleLanguageCode),
    ),
];

#[derive(Default)]
pub struct State {
    current_locale: Option<id>,
    system_locale: Option<id>,
    preferred_languages: Option<id>,
}
impl State {
    fn get(env: &mut Environment) -> &mut State {
        &mut env.framework_state.foundation.ns_locale
    }
}

/// Use `msg_class![env; NSLocale preferredLanguages]` rather than calling this
/// directly, because it may be slow and there is no caching.
fn get_preferred_languages(env: &mut Environment) -> Vec<String> {
    let options = env.options.as_ref();
    if let Some(ref preferred_languages) = options.preferred_languages {
        log!("The app requested your preferred languages. {:?} will reported based on your --preferred-languages= option.", preferred_languages);
        return preferred_languages.clone();
    }

    let languages = get_preferred_language_codes(env);
    if languages.is_empty() {
        let lang = "en".to_string();
        log!("The app requested your preferred languages. No information could be retrieved, so {:?} (English) will be reported.", lang);
        vec![lang]
    } else {
        log!("The app requested your preferred languages. {:?} will be reported based on your system language preferences.", languages);
        languages
    }
}

fn get_preferred_countries(env: &mut Environment) -> Vec<String> {
    let countries = get_preferred_country_codes(env);
    if countries.is_empty() {
        let country = "US".to_string();
        log!("The app requested your current locale. No country information could be retrieved, so {:?} will be reported.", country);
        vec![country]
    } else {
        log!("The app requested your current locale. {:?} will be reported based on your system region settings.", countries);
        countries
    }
}

struct NSLocaleHostObject {
    /// `NSString *`
    country_code: id,
    /// `NSString *`
    language_code: id,
}
impl HostObject for NSLocaleHostObject {}

/// Split a locale identifier into its language and optional region.
///
/// Identifiers look like `language[_Script][_REGION]` and use either `_` or `-`
/// as the separator: `en`, `en_US`, `en-US`, `zh_Hans_CN`. The region is the
/// two-letter component that is not the language, which is what distinguishes
/// it from a script subtag like `Hans`.
fn parse_locale_identifier(identifier: &str) -> (String, Option<String>) {
    let mut parts = identifier.split(['_', '-']).filter(|part| !part.is_empty());
    let language = parts.next().unwrap_or("en").to_lowercase();
    // A region subtag is two letters or three digits; a script is four letters.
    let region = parts
        .find(|part| {
            (part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()))
                || (part.len() == 3 && part.chars().all(|c| c.is_ascii_digit()))
        })
        .map(|part| part.to_uppercase());
    (language, region)
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSLocale: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSLocaleHostObject {
        country_code: nil,
        language_code: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

// The documentation isn't clear about what the format of the strings should be,
// but Super Monkey Ball does `isEqualToString:` against "fr", "es", "de", "it"
// and "ja", and its locale detection works properly, so presumably they do not
// usually have region suffixes.
+ (id)preferredLanguages {
    if let Some(existing) = State::get(env).preferred_languages {
        existing
    } else {
        let langs = get_preferred_languages(env);
        let lang_ns_strings = langs.into_iter().map(|lang| ns_string::from_rust_string(env, lang)).collect();
        let new = ns_array::from_vec(env, lang_ns_strings);
        State::get(env).preferred_languages = Some(new);
        new
    }
}

+ (id)currentLocale {
    if let Some(locale) = State::get(env).current_locale {
        locale
    } else {
        let countries = get_preferred_countries(env);
        let country_code = ns_string::from_rust_string(env, countries[0].clone());
        let languages = get_preferred_languages(env);
        let language_code = ns_string::from_rust_string(env, languages[0].clone());
        let host_object = NSLocaleHostObject {
            country_code,
            language_code,
        };
        let new_locale = env.objc.alloc_object(
            this,
            Box::new(host_object),
            &mut env.mem
        );
        State::get(env).current_locale = Some(new_locale);
        new_locale
    }
}
+ (id)autoupdatingCurrentLocale {
    // TODO: autoupdating part
    msg![env; this currentLocale]
}

+ (id)systemLocale {
    if let Some(locale) = State::get(env).system_locale {
        locale
    } else {
        let host_object = NSLocaleHostObject {
            // Was confirmed on the iOS Simulator
            country_code: nil,
            language_code: nil,
        };
        let new_locale = env.objc.alloc_object(
            this,
            Box::new(host_object),
            &mut env.mem
        );
        State::get(env).system_locale = Some(new_locale);
        new_locale
    }
}

// TODO: constructors, more accessors

- (id)initWithLocaleIdentifier:(id)string { // NSString *
    let str = ns_string::to_rust_string(env, string).to_string();
    log_dbg!("[(NSLocale *){:?} initWithLocaleIdentifier:'{}']", this, str);

    // Identifiers are `language[_Script][_REGION]`, with either separator, and
    // the language alone is valid: "en", "en_US", "en-US", "zh_Hans_CN" are all
    // real. Insisting on exactly two lowercase letters rejected most of them
    // and killed seventeen apps in a 1501-app survey over a locale name.
    let (language, region) = parse_locale_identifier(&str);

    let language = ns_string::from_rust_string(env, language);
    let host_object = env.objc.borrow_mut::<NSLocaleHostObject>(this);
    let old_language = std::mem::replace(&mut host_object.language_code, language);
    release(env, old_language);

    if let Some(region) = region {
        let region = ns_string::from_rust_string(env, region);
        let host_object = env.objc.borrow_mut::<NSLocaleHostObject>(this);
        let old_country = std::mem::replace(&mut host_object.country_code, region);
        release(env, old_country);
    }
    this
}

- (())dealloc {
    let &NSLocaleHostObject { country_code, language_code } = env.objc.borrow::<NSLocaleHostObject>(this);
    release(env, country_code);
    release(env, language_code);
    env.objc.dealloc_object(this, &mut env.mem)
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

- (id)localeIdentifier {
    let locale_id_key = ns_string::get_static_str(env, NSLocaleIdentifier);
    msg![env; this objectForKey:locale_id_key]
}

- (id)objectForKey:(id)key {
    let key_str: &str = &ns_string::to_rust_string(env, key);
    match key_str {
        // Note: this is not the cleanest separation between NS and CF parts
        // But it does work on the iOS Simulator
        // TODO: Define NSLocaleCountryCode _as_ kCFLocaleCountryCode
        NSLocaleCountryCode | kCFLocaleCountryCode => {
            let &NSLocaleHostObject { country_code, .. } = env.objc.borrow(this);
            country_code
        },
        // TODO: Define NSLocaleLanguageCode _as_ kCFLocaleLanguageCode
        NSLocaleLanguageCode | kCFLocaleLanguageCode => {
            let &NSLocaleHostObject { language_code, .. } = env.objc.borrow(this);
            language_code
        },
        // TODO: Define NSLocaleIdentifier _as_ kCFLocaleIdentifier
        NSLocaleIdentifier | kCFLocaleIdentifier => {
            let &NSLocaleHostObject { country_code, language_code } = env.objc.borrow(this);
            // A locale may legitimately carry only a language: "en" is a
            // complete identifier, and demanding a region rejected it.
            let language = if language_code == nil {
                String::new()
            } else {
                ns_string::to_rust_string(env, language_code).to_string()
            };
            let locale_id_str = if country_code == nil {
                language
            } else {
                format!("{}_{}", language, ns_string::to_rust_string(env, country_code))
            };
            let res = ns_string::from_rust_string(env, locale_id_str);
            autorelease(env, res)
        },
        // Foundation answers nil for a key a locale does not carry, and apps
        // check the result. Aborting turned a supported question with a
        // negative answer into a dead app.
        _ => {
            log!("TODO: [(NSLocale *){:?} objectForKey:] for an unimplemented key; returning nil", this);
            nil
        }
    }
}

@end

};

#[cfg(test)]
mod tests {
    use super::parse_locale_identifier;
    use super::{HostConstant, NSLocaleLanguageCode, CONSTANTS};

    #[test]
    fn exports_language_code_key() {
        let (_, constant) = CONSTANTS
            .iter()
            .find(|(name, _)| *name == "_NSLocaleLanguageCode")
            .expect("NSLocaleLanguageCode export is missing");

        match constant {
            HostConstant::NSString(value) => assert_eq!(*value, NSLocaleLanguageCode),
            _ => panic!("NSLocaleLanguageCode must be an NSString constant"),
        }
    }

    #[test]
    fn locale_identifier_forms_that_apps_actually_use() {
        // A bare language is a complete identifier.
        assert_eq!(parse_locale_identifier("en"), ("en".into(), None));
        // Both separators appear in the wild.
        assert_eq!(
            parse_locale_identifier("en_US"),
            ("en".into(), Some("US".into()))
        );
        assert_eq!(
            parse_locale_identifier("en-GB"),
            ("en".into(), Some("GB".into()))
        );
        // Case is normalised rather than rejected.
        assert_eq!(
            parse_locale_identifier("EN_us"),
            ("en".into(), Some("US".into()))
        );
        // A script subtag is four letters and must not be mistaken for a
        // region; the region follows it.
        assert_eq!(
            parse_locale_identifier("zh_Hans_CN"),
            ("zh".into(), Some("CN".into()))
        );
        // A script with no region yields no region rather than "Hans".
        assert_eq!(parse_locale_identifier("zh_Hant"), ("zh".into(), None));
        // UN M.49 numeric regions are three digits.
        assert_eq!(
            parse_locale_identifier("es_419"),
            ("es".into(), Some("419".into()))
        );
        // Degenerate input must not panic; the language falls back rather than
        // the caller losing its locale.
        assert_eq!(parse_locale_identifier(""), ("en".into(), None));
        assert_eq!(parse_locale_identifier("_"), ("en".into(), None));
    }
}
