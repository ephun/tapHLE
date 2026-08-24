/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Runtime support for Objective-C fast enumeration (`for (x in y)`).
//!
//! The loop the compiler emits around a `for...in` body is not just a call to
//! `countByEnumeratingWithState:objects:count:`. It also reads the word at the
//! state's `mutationsPtr` before the first batch and compares it on every
//! batch, and calls [objc_enumerationMutation] when it changes. That is the
//! check that turns a collection modified mid-loop into a diagnostic instead of
//! a walk off the end of a stale buffer.
//!
//! `objc_enumerationMutation` is the single most widely imported symbol tapHLE
//! did not provide: 1100 of the 1192 distinct apps in the import-demand
//! catalogue reference it, because every binary containing one `for...in` loop
//! does. It is reached only when a mutation is detected, so most of those apps
//! never call it — but the ones that do were ended by tapHLE rather than told
//! what they had done.
//!
//! **tapHLE's own collections do not currently detect mutation.**
//! [crate::frameworks::foundation::ns_enumerator::fast_enumeration_helper]
//! points `mutationsPtr` at the enumerated object itself, whose first word is
//! its `isa` and therefore never changes during a loop. So for an `NSArray` or
//! `NSDictionary` the check silently always passes. A guest class that
//! implements fast enumeration over its own storage — which collection wrappers
//! do — supplies a real counter, and those are the callers that get here.

use super::{id, msg, nil, Class};
use crate::dyld::{export_c_func, FunctionExports};
use crate::Environment;

/// `objc_enumerationMutation` — a collection was modified while being
/// enumerated.
///
/// On a device this raises `NSGenericException` and the app almost always dies.
/// Here it logs and returns, which is the same choice
/// [crate::frameworks::foundation::ns_exception] already makes for every other
/// exception: native Objective-C exception delivery is not implemented, so
/// raising would mean terminating, and terminating on a diagnostic the app
/// might have caught is worse than continuing.
///
/// Continuing is not free and the log line says so: the enumeration proceeds
/// over a batch that no longer matches the collection, so the loop may see a
/// removed element or miss an added one. That is a wrong result rather than a
/// crash, and it is visible in the log next to the class that caused it.
fn objc_enumerationMutation(env: &mut Environment, collection: id) {
    let class_name = describe(env, collection);
    log!(
        "{} was mutated while being enumerated. tapHLE cannot raise \
         NSGenericException here, so the loop continues over a stale batch and \
         may see a removed element or miss an added one.",
        class_name
    );
}

/// The collection's class name for the log line, without risking a message send
/// to something that is not an object.
///
/// `description` would be more informative and is what the real exception
/// message uses, but this is called from a broken enumeration: the collection
/// is mid-mutation, and asking it to describe itself could re-enter the very
/// code that went wrong.
fn describe(env: &mut Environment, collection: id) -> String {
    if collection == nil {
        return "A nil collection".to_string();
    }
    let class: Class = msg![env; collection class];
    if class == nil {
        return format!("The object at {:?}", collection);
    }
    format!(
        "An instance of {} at {:?}",
        env.objc.get_class_name(class),
        collection
    )
}

pub(super) const FUNCTIONS: FunctionExports = &[export_c_func!(objc_enumerationMutation(_))];
