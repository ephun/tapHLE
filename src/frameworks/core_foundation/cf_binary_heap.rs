/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFBinaryHeap`.
//!
//! A collection that answers one question quickly: which of the values in it is
//! the smallest? That is what a route search wants — the next place to look at
//! is always the cheapest one found so far — and it is why a game with any kind
//! of pathfinding reaches for this.
//!
//! There is no Foundation type to bridge to, so this is its own object, and the
//! values are kept in order rather than in a heap: an ordered sequence answers
//! "the smallest" by looking at the front, and the position for a new value is
//! found by halving the range rather than by walking it. That costs the same
//! number of comparisons as sifting through a heap would, and comparisons are
//! the expensive part here because each one is a call into the app's own code.

use super::cf_allocator::{kCFAllocatorDefault, CFAllocatorRef};
use super::{CFComparisonResult, CFIndex, CFTypeRef};
use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstPtr, ConstVoidPtr, MutPtr, MutVoidPtr, SafeRead};
use crate::objc::{objc_classes, ClassExports, HostObject};
use crate::Environment;

pub type CFBinaryHeapRef = CFTypeRef;

/// The layout of `CFBinaryHeapCallBacks` in guest memory.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct CFBinaryHeapCallBacks {
    pub version: CFIndex,
    /// `const void *(*retain)(CFAllocatorRef allocator, const void *ptr)`
    pub retain: GuestFunction,
    /// `void (*release)(CFAllocatorRef allocator, const void *ptr)`
    pub release: GuestFunction,
    /// `CFStringRef (*copyDescription)(const void *ptr)`
    pub copy_description: GuestFunction,
    /// `CFComparisonResult (*compare)(const void *ptr1, const void *ptr2,
    /// void *context)`
    pub compare: GuestFunction,
}
unsafe impl SafeRead for CFBinaryHeapCallBacks {}

/// The layout of `CFBinaryHeapCompareContext` in guest memory.
#[repr(C, packed)]
pub struct CFBinaryHeapCompareContext {
    pub version: CFIndex,
    pub info: MutVoidPtr,
    pub retain: GuestFunction,
    pub release: GuestFunction,
    pub copy_description: GuestFunction,
}
unsafe impl SafeRead for CFBinaryHeapCompareContext {}

struct CFBinaryHeapHostObject {
    /// Smallest first, so the minimum is always the front.
    values: Vec<ConstVoidPtr>,
    callbacks: Option<CFBinaryHeapCallBacks>,
    /// The `info` field of the compare context, handed to every comparison.
    compare_context: MutVoidPtr,
}
impl HostObject for CFBinaryHeapHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// CFBinaryHeap has no Foundation counterpart, but callers release it with
// CFRelease, which is a message send here.
@implementation _tapHLE_CFBinaryHeap: NSObject

- (())dealloc {
    remove_all_values(env, this);
    env.objc.dealloc_object(this, &mut env.mem);
}

@end

};

/// Ask the app which of two values is the smaller.
///
/// With no comparison callback the values are ordered by the pointers
/// themselves, which is what a heap of plain numbers wants and is the only
/// order available when the app has not described one.
fn compare_values(
    env: &mut Environment,
    heap: CFBinaryHeapRef,
    left: ConstVoidPtr,
    right: ConstVoidPtr,
) -> CFComparisonResult {
    let host_object = env.objc.borrow::<CFBinaryHeapHostObject>(heap);
    let compare = host_object.callbacks.map(|callbacks| callbacks.compare);
    let context = host_object.compare_context;

    match compare {
        Some(compare) if !compare.to_ptr().is_null() => {
            compare.call_from_host(env, (left, right, context))
        }
        _ => left.to_bits().cmp(&right.to_bits()) as CFComparisonResult,
    }
}

fn retain_value(env: &mut Environment, heap: CFBinaryHeapRef, value: ConstVoidPtr) -> ConstVoidPtr {
    let retain = env
        .objc
        .borrow::<CFBinaryHeapHostObject>(heap)
        .callbacks
        .map(|callbacks| callbacks.retain);
    match retain {
        Some(retain) if !retain.to_ptr().is_null() => {
            retain.call_from_host(env, (kCFAllocatorDefault, value))
        }
        _ => value,
    }
}

fn release_value(env: &mut Environment, heap: CFBinaryHeapRef, value: ConstVoidPtr) {
    let release = env
        .objc
        .borrow::<CFBinaryHeapHostObject>(heap)
        .callbacks
        .map(|callbacks| callbacks.release);
    if let Some(release) = release {
        if !release.to_ptr().is_null() {
            () = release.call_from_host(env, (kCFAllocatorDefault, value));
        }
    }
}

fn remove_all_values(env: &mut Environment, heap: CFBinaryHeapRef) {
    // Taken out of the object before anything is released, because releasing a
    // value runs the app's code, and that code is allowed to look at the heap.
    let values = std::mem::take(&mut env.objc.borrow_mut::<CFBinaryHeapHostObject>(heap).values);
    for value in values {
        release_value(env, heap, value);
    }
}

fn CFBinaryHeapCreate(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    capacity: CFIndex,
    callbacks: ConstPtr<CFBinaryHeapCallBacks>,
    compare_context: ConstPtr<CFBinaryHeapCompareContext>,
) -> CFBinaryHeapRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented

    // The capacity is a hint in Core Foundation too; this grows as needed.
    if capacity != 0 {
        log_dbg!(
            "CFBinaryHeapCreate() capacity hint {} ignored; the heap grows as needed",
            capacity
        );
    }

    // Core Foundation copies both structures, so the app is free to reuse or
    // free its own after this returns.
    let callbacks = if callbacks.is_null() {
        None
    } else {
        Some(env.mem.read(callbacks))
    };
    let compare_context = if compare_context.is_null() {
        MutVoidPtr::null()
    } else {
        env.mem.read(compare_context).info
    };

    let host_object = Box::new(CFBinaryHeapHostObject {
        values: Vec::new(),
        callbacks,
        compare_context,
    });
    let class = env
        .objc
        .get_known_class("_tapHLE_CFBinaryHeap", &mut env.mem);
    env.objc.alloc_object(class, host_object, &mut env.mem)
}

fn CFBinaryHeapAddValue(env: &mut Environment, heap: CFBinaryHeapRef, value: ConstVoidPtr) {
    let value = retain_value(env, heap, value);

    // The position is found by halving the range, and the values are re-read
    // each time: a comparison is the app's own code and may add to or empty the
    // heap while it runs.
    let mut low: usize = 0;
    let mut high: usize = env.objc.borrow::<CFBinaryHeapHostObject>(heap).values.len();
    while low < high {
        let middle = low + (high - low) / 2;
        let Some(&existing) = env
            .objc
            .borrow::<CFBinaryHeapHostObject>(heap)
            .values
            .get(middle)
        else {
            break;
        };
        if compare_values(env, heap, value, existing) < 0 {
            high = middle;
        } else {
            low = middle + 1;
        }
        high = high.min(env.objc.borrow::<CFBinaryHeapHostObject>(heap).values.len());
        low = low.min(high);
    }

    let values = &mut env.objc.borrow_mut::<CFBinaryHeapHostObject>(heap).values;
    let at = low.min(values.len());
    values.insert(at, value);
}

fn CFBinaryHeapGetCount(env: &mut Environment, heap: CFBinaryHeapRef) -> CFIndex {
    env.objc
        .borrow::<CFBinaryHeapHostObject>(heap)
        .values
        .len()
        .try_into()
        .unwrap()
}

fn CFBinaryHeapGetCountOfValue(
    env: &mut Environment,
    heap: CFBinaryHeapRef,
    value: ConstVoidPtr,
) -> CFIndex {
    let values = env
        .objc
        .borrow::<CFBinaryHeapHostObject>(heap)
        .values
        .clone();
    let mut count = 0;
    for existing in values {
        if compare_values(env, heap, existing, value) == 0 {
            count += 1;
        }
    }
    count
}

fn CFBinaryHeapContainsValue(
    env: &mut Environment,
    heap: CFBinaryHeapRef,
    value: ConstVoidPtr,
) -> bool {
    CFBinaryHeapGetCountOfValue(env, heap, value) > 0
}

fn CFBinaryHeapGetMinimum(env: &mut Environment, heap: CFBinaryHeapRef) -> ConstVoidPtr {
    // Core Foundation's own behaviour on an empty heap is undefined; a null
    // pointer is at least an answer the caller can recognise.
    env.objc
        .borrow::<CFBinaryHeapHostObject>(heap)
        .values
        .first()
        .copied()
        .unwrap_or(ConstVoidPtr::null())
}

fn CFBinaryHeapGetMinimumIfPresent(
    env: &mut Environment,
    heap: CFBinaryHeapRef,
    value: MutPtr<ConstVoidPtr>,
) -> bool {
    let minimum = env
        .objc
        .borrow::<CFBinaryHeapHostObject>(heap)
        .values
        .first()
        .copied();
    let Some(minimum) = minimum else {
        return false;
    };
    if !value.is_null() {
        env.mem.write(value, minimum);
    }
    true
}

fn CFBinaryHeapRemoveMinimumValue(env: &mut Environment, heap: CFBinaryHeapRef) {
    let values = &mut env.objc.borrow_mut::<CFBinaryHeapHostObject>(heap).values;
    if values.is_empty() {
        return;
    }
    let minimum = values.remove(0);
    release_value(env, heap, minimum);
}

fn CFBinaryHeapRemoveAllValues(env: &mut Environment, heap: CFBinaryHeapRef) {
    remove_all_values(env, heap);
}

fn CFBinaryHeapGetValues(
    env: &mut Environment,
    heap: CFBinaryHeapRef,
    values: MutPtr<ConstVoidPtr>,
) {
    if values.is_null() {
        return;
    }
    let stored = env
        .objc
        .borrow::<CFBinaryHeapHostObject>(heap)
        .values
        .clone();
    for (index, value) in stored.into_iter().enumerate() {
        env.mem.write(values + index.try_into().unwrap(), value);
    }
}

fn CFBinaryHeapApplyFunction(
    env: &mut Environment,
    heap: CFBinaryHeapRef,
    applier: GuestFunction,
    context: MutVoidPtr,
) {
    // Collected first: an applier is allowed to look at the heap, and may add
    // to it.
    let values = env
        .objc
        .borrow::<CFBinaryHeapHostObject>(heap)
        .values
        .clone();
    for value in values {
        () = applier.call_from_host(env, (value, context));
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CFBinaryHeapCreate(_, _, _, _)),
    export_c_func!(CFBinaryHeapAddValue(_, _)),
    export_c_func!(CFBinaryHeapGetCount(_)),
    export_c_func!(CFBinaryHeapGetCountOfValue(_, _)),
    export_c_func!(CFBinaryHeapContainsValue(_, _)),
    export_c_func!(CFBinaryHeapGetMinimum(_)),
    export_c_func!(CFBinaryHeapGetMinimumIfPresent(_, _)),
    export_c_func!(CFBinaryHeapRemoveMinimumValue(_)),
    export_c_func!(CFBinaryHeapRemoveAllValues(_)),
    export_c_func!(CFBinaryHeapGetValues(_, _)),
    export_c_func!(CFBinaryHeapApplyFunction(_, _, _)),
];
