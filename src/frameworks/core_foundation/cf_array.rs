/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFArray` and `CFMutableArray`.
//!
//! These are toll-free bridged to `NSArray` and `NSMutableArray` in Apple's
//! implementation. Here they are the same types.

use super::cf_allocator::{kCFAllocatorDefault, CFAllocatorRef};
use super::cf_string::CFStringRef;
use super::cf_type::CFEqual;
use super::{kCFNotFound, CFIndex, CFRange, CFRelease, CFRetain};
use crate::abi::GuestFunction;
use crate::dyld::{
    export_c_func, ConstantExports, Dyld, FunctionExports, HostConstant, HostFunction,
};
use crate::frameworks::foundation::NSUInteger;
use crate::mem::{ConstPtr, ConstVoidPtr, Mem, SafeRead};
use crate::objc::{id, msg, msg_class};
use crate::Environment;

#[allow(dead_code)]
pub type CFArrayRef = super::CFTypeRef;
pub type CFMutableArrayRef = super::CFTypeRef;

/// The layout of `CFArrayCallBacks` in guest memory. This is the same shape as
/// `CFDictionaryValueCallBacks`.
#[repr(C, packed)]
pub struct CFArrayCallBacks {
    pub version: CFIndex,         // version
    pub retain: GuestFunction,    // const void *(*retain)(CFAllocatorRef, const void *value)
    pub release: GuestFunction,   // void (*release)(CFAllocatorRef alloc, const void *val)
    pub copy_desc: GuestFunction, // CFStringRef (*copy_desc)(const void *val)
    pub equal: GuestFunction,     // Boolean (*equal)(const void *val1, const void *val2)
}
unsafe impl SafeRead for CFArrayCallBacks {}

fn CFArrayCreateMutable(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    capacity: CFIndex,
    callbacks: ConstPtr<CFArrayCallBacks>,
) -> CFMutableArrayRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented

    // The capacity is a hint. Core Foundation documents it as the number of
    // values the array is expected to hold and does not enforce it, so an array
    // that simply grows is a faithful implementation of any capacity.
    log_dbg!("CFArrayCreateMutable() with capacity hint {}", capacity);

    // A NULL callbacks pointer means the array does not retain/release its
    // values (they need not even be objects). A callbacks struct with a
    // non-NULL retain callback (e.g. `kCFTypeArrayCallBacks`) means the array
    // retains its values as CF/Objective-C objects, which we model with the
    // ordinary retaining `_tapHLE_NSMutableArray`.
    let retaining = if callbacks.is_null() {
        false
    } else {
        let retain = env.mem.read(callbacks).retain;
        !retain.to_ptr().is_null()
    };

    if retaining {
        msg_class![env; _tapHLE_NSMutableArray new]
    } else {
        msg_class![env; _tapHLE_NSMutableArray_non_retaining new]
    }
}

/// The immutable counterpart, built from a C array of values. An app that has
/// its values to hand builds one of these rather than creating a mutable array
/// and appending; the result is an ordinary array here, because tapHLE has no
/// separate immutable representation to put it in.
fn CFArrayCreate(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    values: ConstPtr<ConstVoidPtr>,
    num_values: CFIndex,
    callbacks: ConstPtr<CFArrayCallBacks>,
) -> CFArrayRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented

    let array = CFArrayCreateMutable(env, allocator, num_values, callbacks);
    // A NULL values pointer with a count of zero is legal and means an empty
    // array; with a non-zero count it is a caller error, and reading from it
    // would be worse than saying so.
    assert!(num_values == 0 || !values.is_null());
    for i in 0..num_values {
        let value = env.mem.read(values + i.try_into().unwrap());
        CFArrayAppendValue(env, array, value);
    }
    array
}

fn CFArrayGetCount(env: &mut Environment, array: CFArrayRef) -> CFIndex {
    let count: NSUInteger = msg![env; array count];
    count.try_into().unwrap()
}

fn CFArrayGetValueAtIndex(env: &mut Environment, array: CFArrayRef, idx: CFIndex) -> ConstVoidPtr {
    let idx: NSUInteger = idx.try_into().unwrap();
    let value: id = msg![env; array objectAtIndex:idx];
    value.cast().cast_const()
}

fn CFArrayGetFirstIndexOfValue(
    env: &mut Environment,
    array: CFArrayRef,
    range: CFRange,
    value: ConstVoidPtr,
) -> CFIndex {
    let count: NSUInteger = msg![env; array count];
    let start = range.location;
    let end = range.location.checked_add(range.length).unwrap();
    assert!(start >= 0 && end >= 0 && (end as NSUInteger) <= count); // range must be valid

    let value: id = value.cast().cast_mut();
    for i in start..end {
        let idx: NSUInteger = i.try_into().unwrap();
        let curr: id = msg![env; array objectAtIndex:idx];
        // CF's default equal callback compares by pointer identity, then by the
        // value's equality. NSObject's isEqual: is pointer identity and
        // CF-typed subclasses (NSString, NSNumber, ...) override it, so this
        // mirrors both the CF semantics and the sibling `NSArray
        // indexOfObject:`.
        let equal: bool = msg![env; value isEqual:curr];
        if equal {
            return i;
        }
    }
    kCFNotFound
}

fn CFArrayAppendValue(env: &mut Environment, array: CFMutableArrayRef, value: ConstVoidPtr) {
    let value: id = value.cast().cast_mut();
    msg![env; array addObject:value]
}

fn CFArrayInsertValueAtIndex(
    env: &mut Environment,
    array: CFMutableArrayRef,
    idx: CFIndex,
    value: ConstVoidPtr,
) {
    let idx: NSUInteger = idx.try_into().unwrap();
    let value: id = value.cast().cast_mut();
    msg![env; array insertObject:value atIndex:idx]
}

fn CFArraySetValueAtIndex(
    env: &mut Environment,
    array: CFMutableArrayRef,
    idx: CFIndex,
    value: ConstVoidPtr,
) {
    let idx: NSUInteger = idx.try_into().unwrap();
    let value: id = value.cast().cast_mut();
    msg![env; array replaceObjectAtIndex:idx withObject:value]
}

fn CFArrayRemoveValueAtIndex(env: &mut Environment, array: CFMutableArrayRef, idx: CFIndex) {
    let idx: NSUInteger = idx.try_into().unwrap();
    msg![env; array removeObjectAtIndex:idx]
}

fn CFArrayRemoveAllValues(env: &mut Environment, array: CFMutableArrayRef) {
    msg![env; array removeAllObjects]
}

// Default CFArray callbacks, matching `kCFTypeArrayCallBacks`. tapHLE does not
// call these itself (retaining is handled by the chosen array class), but the
// guest may read the struct or invoke the callbacks directly, so they behave as
// CF-type retain/release/equal.
fn _tapHLE_CFArray_retain(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    value: ConstVoidPtr,
) -> ConstVoidPtr {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    CFRetain(env, value.cast_mut().cast()).cast_const().cast()
}
fn _tapHLE_CFArray_release(env: &mut Environment, allocator: CFAllocatorRef, value: ConstVoidPtr) {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    CFRelease(env, value.cast_mut().cast());
}
fn _tapHLE_CFArray_copyDescription(_env: &mut Environment, _value: ConstVoidPtr) -> CFStringRef {
    todo!()
}
fn _tapHLE_CFArray_equal(
    env: &mut Environment,
    value1: ConstVoidPtr,
    value2: ConstVoidPtr,
) -> bool {
    CFEqual(env, value1.cast_mut().cast(), value2.cast_mut().cast())
}

fn create_default_callbacks(mem: &mut Mem, dyld: &mut Dyld) -> CFArrayCallBacks {
    let retain_hf: HostFunction = &(_tapHLE_CFArray_retain as fn(&mut Environment, _, _) -> _);
    let retain = dyld.create_guest_function(mem, "__tapHLE_CFArray_retain", retain_hf);

    let release_hf: HostFunction = &(_tapHLE_CFArray_release as fn(&mut Environment, _, _));
    let release = dyld.create_guest_function(mem, "__tapHLE_CFArray_release", release_hf);

    let copy_desc_hf: HostFunction =
        &(_tapHLE_CFArray_copyDescription as fn(&mut Environment, _) -> _);
    let copy_desc =
        dyld.create_guest_function(mem, "__tapHLE_CFArray_copyDescription", copy_desc_hf);

    let equal_hf: HostFunction = &(_tapHLE_CFArray_equal as fn(&mut Environment, _, _) -> _);
    let equal = dyld.create_guest_function(mem, "__tapHLE_CFArray_equal", equal_hf);

    CFArrayCallBacks {
        version: 0, // always 0
        retain,
        release,
        copy_desc,
        equal,
    }
}

pub const CONSTANTS: ConstantExports = &[(
    "_kCFTypeArrayCallBacks",
    HostConstant::Custom(|env| {
        let callbacks = create_default_callbacks(&mut env.mem, &mut env.dyld);
        env.mem.alloc_and_write(callbacks).cast_void().cast_const()
    }),
)];

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CFArrayCreate(_, _, _, _)),
    export_c_func!(CFArrayCreateMutable(_, _, _)),
    export_c_func!(CFArrayGetCount(_)),
    export_c_func!(CFArrayGetValueAtIndex(_, _)),
    export_c_func!(CFArrayGetFirstIndexOfValue(_, _, _)),
    export_c_func!(CFArrayAppendValue(_, _)),
    export_c_func!(CFArrayInsertValueAtIndex(_, _, _)),
    export_c_func!(CFArraySetValueAtIndex(_, _, _)),
    export_c_func!(CFArrayRemoveValueAtIndex(_, _)),
    export_c_func!(CFArrayRemoveAllValues(_)),
];
