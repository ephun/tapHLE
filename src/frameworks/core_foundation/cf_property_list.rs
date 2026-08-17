/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFPropertyList`.
//!
//! Implemented on top of Foundation, like the rest of Core Foundation here.

use super::cf_allocator::CFAllocatorRef;
use super::{CFIndex, CFOptionFlags, CFTypeRef};
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::foundation::NSUInteger;
use crate::objc::{id, msg, msg_class, nil, retain};
use crate::Environment;

type CFPropertyListRef = CFTypeRef;

/// `CFPropertyListMutabilityOptions`.
type CFPropertyListMutabilityOptions = CFOptionFlags;
const kCFPropertyListImmutable: CFPropertyListMutabilityOptions = 0;
const kCFPropertyListMutableContainers: CFPropertyListMutabilityOptions = 1;
const kCFPropertyListMutableContainersAndLeaves: CFPropertyListMutabilityOptions = 2;

/// Copy a property list, recursing into its containers.
///
/// The point of the call is the recursion: `-copy` on a dictionary is shallow,
/// so the copy shares its values with the original and mutating one shows up in
/// the other. Apps use this to take a template out of a bundled plist and then
/// edit the copy, which is exactly the case a shallow copy corrupts.
///
/// A game with data-driven scenes does that per scene, reading a schema once
/// and deep-copying it for each element it builds.
fn deep_copy(env: &mut Environment, value: id, mutability: CFPropertyListMutabilityOptions) -> id {
    if value == nil {
        return nil;
    }

    let dictionary_class = env.objc.get_known_class("NSDictionary", &mut env.mem);
    let array_class = env.objc.get_known_class("NSArray", &mut env.mem);
    let class = msg![env; value class];

    if env.objc.class_is_subclass_of(class, dictionary_class) {
        let new: id = msg_class![env; NSMutableDictionary new];
        let keys: id = msg![env; value allKeys];
        let count: NSUInteger = msg![env; keys count];
        for i in 0..count {
            let key: id = msg![env; keys objectAtIndex:i];
            let old: id = msg![env; value objectForKey:key];
            let copied = deep_copy(env, old, mutability);
            // The key is copied by the dictionary itself, as Foundation
            // documents; only the value needs deep-copying.
            () = msg![env; new setObject:copied forKey:key];
            crate::objc::release(env, copied);
        }
        return finish_container(env, new, mutability);
    }

    if env.objc.class_is_subclass_of(class, array_class) {
        let new: id = msg_class![env; NSMutableArray new];
        let count: NSUInteger = msg![env; value count];
        for i in 0..count {
            let old: id = msg![env; value objectAtIndex:i];
            let copied = deep_copy(env, old, mutability);
            () = msg![env; new addObject:copied];
            crate::objc::release(env, copied);
        }
        return finish_container(env, new, mutability);
    }

    // A leaf: string, number, data, date, boolean. Only the deepest mutability
    // option asks for these to be copied at all, and the rest are immutable
    // value types where sharing is indistinguishable from copying.
    if mutability == kCFPropertyListMutableContainersAndLeaves {
        let copied: id = msg![env; value mutableCopy];
        if copied != nil {
            return copied;
        }
        // Not everything that can appear in a property list is NSMutableCopying
        // — a number is not — so fall through rather than losing the value.
    }
    retain(env, value)
}

/// Freeze a freshly built container if the caller did not ask for a mutable
/// one.
fn finish_container(
    env: &mut Environment,
    container: id,
    mutability: CFPropertyListMutabilityOptions,
) -> id {
    if mutability == kCFPropertyListImmutable {
        let immutable: id = msg![env; container copy];
        crate::objc::release(env, container);
        immutable
    } else {
        container
    }
}

fn CFPropertyListCreateDeepCopy(
    env: &mut Environment,
    _allocator: CFAllocatorRef,
    property_list: CFPropertyListRef,
    mutability_option: CFPropertyListMutabilityOptions,
) -> CFPropertyListRef {
    assert!(
        mutability_option == kCFPropertyListImmutable
            || mutability_option == kCFPropertyListMutableContainers
            || mutability_option == kCFPropertyListMutableContainersAndLeaves
    );
    let copy = deep_copy(env, property_list, mutability_option);
    log_dbg!(
        "CFPropertyListCreateDeepCopy({:?}, {}) -> {:?}",
        property_list,
        mutability_option,
        copy
    );
    copy
}

/// Whether a value is something a property list may contain.
///
/// tapHLE's property list support is Foundation's, so this answers for the
/// same set of classes that `NSPropertyListSerialization` handles.
fn CFPropertyListIsValid(
    env: &mut Environment,
    property_list: CFPropertyListRef,
    _format: CFIndex,
) -> bool {
    if property_list == nil {
        return false;
    }
    let class = msg![env; property_list class];
    for name in [
        "NSDictionary",
        "NSArray",
        "NSString",
        "NSNumber",
        "NSData",
        "NSDate",
    ] {
        let known = env.objc.get_known_class(name, &mut env.mem);
        if env.objc.class_is_subclass_of(class, known) {
            return true;
        }
    }
    false
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CFPropertyListCreateDeepCopy(_, _, _)),
    export_c_func!(CFPropertyListIsValid(_, _)),
];
