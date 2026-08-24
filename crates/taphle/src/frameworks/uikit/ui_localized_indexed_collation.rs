/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UILocalizedIndexedCollation`.
//!
//! The A–Z strip down the side of an indexed table view, and the rule for which
//! section a given object belongs in.
//!
//! Real UIKit takes its sections from the user's locale, so a Japanese device
//! gets kana and a Greek one gets the Greek alphabet. tapHLE has no locale
//! data, so this is **the English collation, unconditionally**: A–Z followed by
//! `#`. For an app running in English that is exactly right; for one running in
//! another script the section titles will be Latin letters and objects that
//! start with a non-Latin character all land in `#`.
//!
//! Sorting compares whole strings rather than performing a real collation, so
//! accented characters sort by code point rather than next to their unaccented
//! forms — "Ångström" after "Zulu" instead of alongside "Angstrom".
//!
//! Resources:
//! - Apple's [UILocalizedIndexedCollation](https://developer.apple.com/documentation/uikit/uilocalizedindexedcollation)

use crate::frameworks::foundation::{ns_array, ns_string, NSInteger, NSUInteger};
use crate::objc::{
    autorelease, id, msg, msg_send, nil, objc_classes, ClassExports, HostObject, SEL,
};
use crate::Environment;

#[derive(Default)]
pub struct State {
    current: Option<id>,
}

struct UILocalizedIndexedCollationHostObject;
impl HostObject for UILocalizedIndexedCollationHostObject {}

/// A–Z then `#`, which is where anything not starting with a Latin letter goes.
const SECTION_TITLES: &[&str] = &[
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z", "#",
];

/// The section a string belongs to, by its first character.
fn section_for_string(text: &str) -> NSInteger {
    let Some(first) = text.trim_start().chars().next() else {
        // An empty string sorts into "#", as it has no letter to file under.
        return (SECTION_TITLES.len() - 1) as NSInteger;
    };
    let upper = first.to_ascii_uppercase();
    if upper.is_ascii_alphabetic() {
        (upper as u8 - b'A') as NSInteger
    } else {
        (SECTION_TITLES.len() - 1) as NSInteger
    }
}

/// Ask an object for the string it should be filed under.
fn collation_string(env: &mut Environment, object: id, selector: SEL) -> String {
    let string: id = msg_send(env, (object, selector));
    if string == nil {
        String::new()
    } else {
        ns_string::to_rust_string(env, string).to_string()
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UILocalizedIndexedCollation: NSObject

// A singleton, as the name says. It holds no state — the collation is fixed —
// but the identity matters because callers compare it.
+ (id)currentCollation {
    if let Some(current) = env.framework_state.uikit.ui_localized_indexed_collation.current {
        return current;
    }
    let host_object = Box::new(UILocalizedIndexedCollationHostObject);
    let new = env.objc.alloc_object(this, host_object, &mut env.mem);
    env.framework_state.uikit.ui_localized_indexed_collation.current = Some(new);
    new
}

- (id)sectionTitles {
    let titles: Vec<id> = SECTION_TITLES
        .iter()
        .map(|title| ns_string::get_static_str(env, title))
        .collect();
    let array = ns_array::from_vec(env, titles);
    autorelease(env, array)
}

// The strip down the side of the table. Identical to the section titles here;
// UIKit distinguishes them only for collations that group several titles under
// one index entry, which the English one does not.
- (id)sectionIndexTitles {
    msg![env; this sectionTitles]
}

- (NSInteger)sectionForSectionIndexTitleAtIndex:(NSInteger)index {
    index
}

- (NSInteger)sectionForObject:(id)object
      collationStringSelector:(SEL)selector {
    if object == nil {
        return (SECTION_TITLES.len() - 1) as NSInteger;
    }
    let text = collation_string(env, object, selector);
    section_for_string(&text)
}

// Sorts the whole array by the collation string, which is what UIKit does —
// the caller then splits it into sections itself using -sectionForObject:.
- (id)sortedArrayFromArray:(id)array // NSArray*
   collationStringSelector:(SEL)selector {
    let count: NSUInteger = msg![env; array count];
    let mut keyed: Vec<(NSInteger, String, id)> = Vec::with_capacity(count as usize);
    for i in 0..count {
        let object: id = msg![env; array objectAtIndex:i];
        let text = collation_string(env, object, selector);
        keyed.push((section_for_string(&text), text.to_uppercase(), object));
    }
    // By section first so the result is already grouped, then alphabetically
    // within each section.
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let sorted: Vec<id> = keyed.into_iter().map(|(_, _, object)| object).collect();
    let new = ns_array::from_vec(env, sorted);
    autorelease(env, new)
}

@end

};
