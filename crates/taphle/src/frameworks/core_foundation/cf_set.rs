/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFSet` and `CFMutableSet`.
//!
//! These are toll-free bridged to `NSSet` and `NSMutableSet` in Apple's
//! implementation. Here they are the same types, as with `CFArray`.

use super::cf_allocator::{kCFAllocatorDefault, CFAllocatorRef};
use super::cf_string::CFStringRef;
use super::cf_type::{CFEqual, CFHash};
use super::{CFHashCode, CFIndex, CFRelease, CFRetain};
use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::{
    export_c_func, ConstantExports, Dyld, FunctionExports, HostConstant, HostFunction,
};
use crate::frameworks::foundation::NSUInteger;
use crate::mem::{ConstPtr, ConstVoidPtr, Mem, MutVoidPtr, SafeRead};
use crate::objc::{id, msg, msg_class};
use crate::Environment;

#[allow(dead_code)]
pub type CFSetRef = super::CFTypeRef;
pub type CFMutableSetRef = super::CFTypeRef;

/// The layout of `CFSetCallBacks` in guest memory.
#[repr(C, packed)]
pub struct CFSetCallBacks {
    pub version: CFIndex,         // version
    pub retain: GuestFunction,    // const void *(*retain)(CFAllocatorRef, const void *value)
    pub release: GuestFunction,   // void (*release)(CFAllocatorRef alloc, const void *value)
    pub copy_desc: GuestFunction, // CFStringRef (*copyDescription)(const void *value)
    pub equal: GuestFunction,     // Boolean (*equal)(const void *value1, const void *value2)
    pub hash: GuestFunction,      // CFHashCode (*hash)(const void *value)
}
unsafe impl SafeRead for CFSetCallBacks {}

fn CFSetCreateMutable(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    capacity: CFIndex,
    callbacks: ConstPtr<CFSetCallBacks>,
) -> CFMutableSetRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented

    // The capacity is a hint. Core Foundation documents 0 as "no limit" and
    // does not enforce a non-zero value; this set grows as needed either way.
    if capacity != 0 {
        log_dbg!(
            "CFSetCreateMutable() capacity hint {} ignored; the set grows as needed",
            capacity
        );
    }

    // A NULL callbacks pointer, or one whose retain callback is NULL, means the
    // set does not retain its values and they need not be objects at all. This
    // set is backed by `NSMutableSet`, which retains and which sends `hash` and
    // `isEqual:` to its members, so a set of non-objects would not work — say
    // so rather than crashing somewhere later inside a message send.
    if !callbacks.is_null() {
        let retain = env.mem.read(callbacks).retain;
        if retain.to_ptr().is_null() {
            log!("Warning: CFSetCreateMutable() was given callbacks that do not retain. tapHLE's set retains its values and treats them as objects; a set of plain pointers will not behave.");
        }
    } else {
        log!("Warning: CFSetCreateMutable() was given no callbacks. tapHLE's set retains its values and treats them as objects; a set of plain pointers will not behave.");
    }

    msg_class![env; _tapHLE_NSMutableSet new]
}

fn CFSetGetCount(env: &mut Environment, set: CFSetRef) -> CFIndex {
    let count: NSUInteger = msg![env; set count];
    count.try_into().unwrap()
}

fn CFSetContainsValue(env: &mut Environment, set: CFSetRef, value: ConstVoidPtr) -> bool {
    let value: id = value.cast().cast_mut();
    msg![env; set containsObject:value]
}

fn CFSetAddValue(env: &mut Environment, set: CFMutableSetRef, value: ConstVoidPtr) {
    let value: id = value.cast().cast_mut();
    msg![env; set addObject:value]
}

fn CFSetRemoveValue(env: &mut Environment, set: CFMutableSetRef, value: ConstVoidPtr) {
    let value: id = value.cast().cast_mut();
    msg![env; set removeObject:value]
}

fn CFSetRemoveAllValues(env: &mut Environment, set: CFMutableSetRef) {
    msg![env; set removeAllObjects]
}

/// `void CFSetApplyFunction(CFSetRef, CFSetApplierFunction, void *context)`,
/// where the applier is `void (*)(const void *value, void *context)`.
fn CFSetApplyFunction(
    env: &mut Environment,
    set: CFSetRef,
    applier: GuestFunction,
    context: MutVoidPtr,
) {
    // The members are collected first: an applier is allowed to look at the
    // set, and iterating it while sending messages that could touch it is how
    // an ordinary traversal turns into a borrow of something that moved.
    let objects: id = msg![env; set allObjects];
    let count: NSUInteger = msg![env; objects count];
    for i in 0..count {
        let object: id = msg![env; objects objectAtIndex:i];
        let _: () = applier.call_from_host(env, (object.cast_void().cast_const(), context));
    }
}

// Default CFSet callbacks, matching `kCFTypeSetCallBacks`. tapHLE does not call
// these itself — the set retains its members through `NSMutableSet` — but the
// guest may read the struct or call them directly, so they behave as CF-type
// retain/release/equal/hash.
fn _tapHLE_CFSet_retain(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    value: ConstVoidPtr,
) -> ConstVoidPtr {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    CFRetain(env, value.cast_mut().cast()).cast_const().cast()
}
fn _tapHLE_CFSet_release(env: &mut Environment, allocator: CFAllocatorRef, value: ConstVoidPtr) {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    CFRelease(env, value.cast_mut().cast());
}
fn _tapHLE_CFSet_copyDescription(_env: &mut Environment, _value: ConstVoidPtr) -> CFStringRef {
    todo!()
}
fn _tapHLE_CFSet_equal(env: &mut Environment, value1: ConstVoidPtr, value2: ConstVoidPtr) -> bool {
    CFEqual(env, value1.cast_mut().cast(), value2.cast_mut().cast())
}
fn _tapHLE_CFSet_hash(env: &mut Environment, value: ConstVoidPtr) -> CFHashCode {
    CFHash(env, value.cast_mut().cast())
}

fn create_default_callbacks(mem: &mut Mem, dyld: &mut Dyld) -> CFSetCallBacks {
    let retain_hf: HostFunction = &(_tapHLE_CFSet_retain as fn(&mut Environment, _, _) -> _);
    let retain = dyld.create_guest_function(mem, "__tapHLE_CFSet_retain", retain_hf);

    let release_hf: HostFunction = &(_tapHLE_CFSet_release as fn(&mut Environment, _, _));
    let release = dyld.create_guest_function(mem, "__tapHLE_CFSet_release", release_hf);

    let copy_desc_hf: HostFunction =
        &(_tapHLE_CFSet_copyDescription as fn(&mut Environment, _) -> _);
    let copy_desc = dyld.create_guest_function(mem, "__tapHLE_CFSet_copyDescription", copy_desc_hf);

    let equal_hf: HostFunction = &(_tapHLE_CFSet_equal as fn(&mut Environment, _, _) -> _);
    let equal = dyld.create_guest_function(mem, "__tapHLE_CFSet_equal", equal_hf);

    let hash_hf: HostFunction = &(_tapHLE_CFSet_hash as fn(&mut Environment, _) -> _);
    let hash = dyld.create_guest_function(mem, "__tapHLE_CFSet_hash", hash_hf);

    CFSetCallBacks {
        version: 0, // always 0
        retain,
        release,
        copy_desc,
        equal,
        hash,
    }
}

pub const CONSTANTS: ConstantExports = &[(
    "_kCFTypeSetCallBacks",
    HostConstant::Custom(|env| {
        let callbacks = create_default_callbacks(&mut env.mem, &mut env.dyld);
        env.mem.alloc_and_write(callbacks).cast_void().cast_const()
    }),
)];

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CFSetCreateMutable(_, _, _)),
    export_c_func!(CFSetGetCount(_)),
    export_c_func!(CFSetContainsValue(_, _)),
    export_c_func!(CFSetAddValue(_, _)),
    export_c_func!(CFSetRemoveValue(_, _)),
    export_c_func!(CFSetRemoveAllValues(_)),
    export_c_func!(CFSetApplyFunction(_, _, _)),
];
