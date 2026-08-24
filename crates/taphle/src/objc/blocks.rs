/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Apple Blocks runtime support.
//!
//! Blocks are Objective-C-compatible objects, but stack and global block
//! literals are emitted directly by guest code rather than allocated through
//! the Objective-C runtime. Their layout and helper calls are specified by
//! Clang's Apple Blocks ABI documentation.

use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::mem::{guest_size_of, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr, Ptr};
use crate::objc::{id, objc_classes, release, retain, ClassExports, NSZonePtr};
use crate::Environment;

const BLOCK_REFCOUNT_MASK: u32 = 0xffff;
const BLOCK_NEEDS_FREE: u32 = 1 << 24;
const BLOCK_HAS_COPY_DISPOSE: u32 = 1 << 25;
const BLOCK_IS_GLOBAL: u32 = 1 << 28;

const BLOCK_FIELD_IS_OBJECT: i32 = 3;
const BLOCK_FIELD_IS_BLOCK: i32 = 7;
const BLOCK_FIELD_IS_BYREF: i32 = 8;
const BLOCK_FIELD_IS_WEAK: i32 = 16;
const BLOCK_BYREF_CALLER: i32 = 128;
const BLOCK_FIELD_KIND_MASK: i32 = 0x0f;

const BLOCK_LITERAL_WORDS: GuestUSize = 5;
const BLOCK_DESCRIPTOR_SIZE_WORD: GuestUSize = 1;
const BLOCK_DESCRIPTOR_COPY_WORD: GuestUSize = 2;
const BLOCK_DESCRIPTOR_DISPOSE_WORD: GuestUSize = 3;

const BYREF_FORWARDING_WORD: GuestUSize = 1;
const BYREF_FLAGS_WORD: GuestUSize = 2;
const BYREF_SIZE_WORD: GuestUSize = 3;
const BYREF_KEEP_WORD: GuestUSize = 4;
const BYREF_DISPOSE_WORD: GuestUSize = 5;

fn word_ptr(ptr: ConstVoidPtr, offset: GuestUSize) -> MutPtr<u32> {
    ptr.cast_mut().cast::<u32>() + offset
}

/// The guest function that implements a block's body, read from the block
/// literal (word 3 of the layout). Per the Apple Blocks ABI, call the returned
/// function with the block literal pointer as the hidden first argument,
/// followed by the block's own declared arguments.
pub fn block_invoke_function(env: &Environment, block: id) -> GuestFunction {
    let block: ConstVoidPtr = block.cast().cast_const();
    env.mem
        .read(word_ptr(block, 3).cast::<GuestFunction>().cast_const())
}

fn block_flags(env: &Environment, block: ConstVoidPtr) -> u32 {
    env.mem.read(word_ptr(block, 1).cast_const())
}

fn set_block_flags(env: &mut Environment, block: MutVoidPtr, flags: u32) {
    env.mem.write(word_ptr(block.cast_const(), 1), flags);
}

fn increment_heap_refcount(flags: u32) -> u32 {
    let count = flags & BLOCK_REFCOUNT_MASK;
    assert!(
        count != BLOCK_REFCOUNT_MASK,
        "Block reference count overflow"
    );
    (flags & !BLOCK_REFCOUNT_MASK) | (count + 1)
}

fn decrement_heap_refcount(flags: u32) -> (u32, bool) {
    let count = flags & BLOCK_REFCOUNT_MASK;
    assert!(count > 0, "Block reference count underflow");
    let new_count = count - 1;
    ((flags & !BLOCK_REFCOUNT_MASK) | new_count, new_count == 0)
}

fn block_descriptor(env: &Environment, block: ConstVoidPtr) -> ConstVoidPtr {
    env.mem.read(
        word_ptr(block, BLOCK_LITERAL_WORDS - 1)
            .cast::<ConstVoidPtr>()
            .cast_const(),
    )
}

fn block_copy(env: &mut Environment, block: ConstVoidPtr) -> MutVoidPtr {
    if block.is_null() {
        return Ptr::null();
    }

    let flags = block_flags(env, block);
    if flags & BLOCK_IS_GLOBAL != 0 {
        return block.cast_mut();
    }
    if flags & BLOCK_NEEDS_FREE != 0 {
        set_block_flags(env, block.cast_mut(), increment_heap_refcount(flags));
        return block.cast_mut();
    }

    let descriptor = block_descriptor(env, block);
    assert!(!descriptor.is_null(), "Stack block has no descriptor");
    let size: GuestUSize = env
        .mem
        .read(word_ptr(descriptor, BLOCK_DESCRIPTOR_SIZE_WORD).cast_const());
    assert!(
        size >= BLOCK_LITERAL_WORDS * guest_size_of::<u32>(),
        "Stack block descriptor has invalid size {size}"
    );

    let heap_block = env.mem.alloc(size);
    env.mem.memmove(heap_block, block, size);
    let malloc_class = env
        .objc
        .get_known_class("_NSConcreteMallocBlock", &mut env.mem);
    env.mem.write(heap_block.cast::<id>(), malloc_class);
    set_block_flags(
        env,
        heap_block,
        (flags & !BLOCK_REFCOUNT_MASK) | BLOCK_NEEDS_FREE | 1,
    );

    if flags & BLOCK_HAS_COPY_DISPOSE != 0 {
        let copy_helper: GuestFunction = env.mem.read(
            word_ptr(descriptor, BLOCK_DESCRIPTOR_COPY_WORD)
                .cast::<GuestFunction>()
                .cast_const(),
        );
        assert!(copy_helper.addr_without_thumb_bit() != 0);
        () = copy_helper.call_from_host(env, (heap_block, block));
    }

    heap_block
}

fn block_release(env: &mut Environment, block: ConstVoidPtr) {
    if block.is_null() {
        return;
    }

    let flags = block_flags(env, block);
    if flags & BLOCK_NEEDS_FREE == 0 {
        return;
    }

    let (new_flags, should_free) = decrement_heap_refcount(flags);
    set_block_flags(env, block.cast_mut(), new_flags);
    if !should_free {
        return;
    }

    if flags & BLOCK_HAS_COPY_DISPOSE != 0 {
        let descriptor = block_descriptor(env, block);
        let dispose_helper: GuestFunction = env.mem.read(
            word_ptr(descriptor, BLOCK_DESCRIPTOR_DISPOSE_WORD)
                .cast::<GuestFunction>()
                .cast_const(),
        );
        assert!(dispose_helper.addr_without_thumb_bit() != 0);
        () = dispose_helper.call_from_host(env, (block,));
    }
    env.mem.free(block.cast_mut());
}

fn byref_copy(env: &mut Environment, source: ConstVoidPtr) -> MutVoidPtr {
    let forwarding: MutVoidPtr = env.mem.read(
        word_ptr(source, BYREF_FORWARDING_WORD)
            .cast::<MutVoidPtr>()
            .cast_const(),
    );
    let source = if forwarding.is_null() {
        source
    } else {
        forwarding.cast_const()
    };
    let flags: u32 = env
        .mem
        .read(word_ptr(source, BYREF_FLAGS_WORD).cast_const());
    if flags & BLOCK_NEEDS_FREE != 0 {
        env.mem.write(
            word_ptr(source, BYREF_FLAGS_WORD),
            increment_heap_refcount(flags),
        );
        return source.cast_mut();
    }

    let size: GuestUSize = env.mem.read(word_ptr(source, BYREF_SIZE_WORD).cast_const());
    assert!(
        size >= 4 * guest_size_of::<u32>(),
        "Block byref structure has invalid size {size}"
    );
    let heap_byref = env.mem.alloc(size);
    env.mem.memmove(heap_byref, source, size);
    env.mem.write(
        word_ptr(heap_byref.cast_const(), BYREF_FORWARDING_WORD).cast::<MutVoidPtr>(),
        heap_byref,
    );
    env.mem.write(
        word_ptr(source, BYREF_FORWARDING_WORD).cast::<MutVoidPtr>(),
        heap_byref,
    );
    env.mem.write(
        word_ptr(heap_byref.cast_const(), BYREF_FLAGS_WORD),
        // One reference belongs to the copied block and one represents the
        // original stack scope, which will dispose its reference later.
        (flags & !BLOCK_REFCOUNT_MASK) | BLOCK_NEEDS_FREE | 2,
    );

    if flags & BLOCK_HAS_COPY_DISPOSE != 0 {
        let keep_helper: GuestFunction = env.mem.read(
            word_ptr(source, BYREF_KEEP_WORD)
                .cast::<GuestFunction>()
                .cast_const(),
        );
        assert!(keep_helper.addr_without_thumb_bit() != 0);
        () = keep_helper.call_from_host(env, (heap_byref, source));
    }
    heap_byref
}

fn byref_release(env: &mut Environment, source: ConstVoidPtr) {
    if source.is_null() {
        return;
    }
    let forwarding: MutVoidPtr = env.mem.read(
        word_ptr(source, BYREF_FORWARDING_WORD)
            .cast::<MutVoidPtr>()
            .cast_const(),
    );
    let source = if forwarding.is_null() {
        source
    } else {
        forwarding.cast_const()
    };
    let flags: u32 = env
        .mem
        .read(word_ptr(source, BYREF_FLAGS_WORD).cast_const());
    if flags & BLOCK_NEEDS_FREE == 0 {
        return;
    }
    let (new_flags, should_free) = decrement_heap_refcount(flags);
    env.mem.write(word_ptr(source, BYREF_FLAGS_WORD), new_flags);
    if !should_free {
        return;
    }
    if flags & BLOCK_HAS_COPY_DISPOSE != 0 {
        let dispose_helper: GuestFunction = env.mem.read(
            word_ptr(source, BYREF_DISPOSE_WORD)
                .cast::<GuestFunction>()
                .cast_const(),
        );
        assert!(dispose_helper.addr_without_thumb_bit() != 0);
        () = dispose_helper.call_from_host(env, (source,));
    }
    env.mem.free(source.cast_mut());
}

#[allow(non_snake_case)]
fn _Block_copy(env: &mut Environment, block: ConstVoidPtr) -> MutVoidPtr {
    block_copy(env, block)
}

#[allow(non_snake_case)]
/// `objc_retainBlock` — what ARC emits when a block is stored somewhere that
/// outlives the current scope.
///
/// A block starts on the stack, so retaining one in place would keep a pointer
/// to a frame that is about to disappear. The runtime's answer is to copy it to
/// the heap instead, and copying an already-heap block is what retains it — so
/// this is `_Block_copy`, not a separate reference-count operation. Getting
/// that wrong would not fail here; it would fail later, when the stack frame
/// the block still pointed into was reused.
fn objc_retainBlock(env: &mut Environment, block: ConstVoidPtr) -> MutVoidPtr {
    _Block_copy(env, block)
}

fn _Block_release(env: &mut Environment, block: ConstVoidPtr) {
    block_release(env, block)
}

#[allow(non_snake_case)]
fn _Block_object_assign(
    env: &mut Environment,
    destination: MutPtr<ConstVoidPtr>,
    object: ConstVoidPtr,
    flags: i32,
) {
    if flags & BLOCK_BYREF_CALLER != 0 {
        // A byref keep helper is copying an object/block field inside the
        // already-migrated byref structure. The byref structure owns the
        // lifetime; this nested assignment must not retain or copy again.
        env.mem.write(destination, object);
        return;
    }

    let weak = flags & BLOCK_FIELD_IS_WEAK != 0;
    let copied = match flags & BLOCK_FIELD_KIND_MASK {
        BLOCK_FIELD_IS_OBJECT => {
            if !weak {
                retain(env, object.cast_mut().cast());
            }
            object.cast_mut()
        }
        BLOCK_FIELD_IS_BLOCK => block_copy(env, object),
        BLOCK_FIELD_IS_BYREF => byref_copy(env, object),
        kind => panic!("Unsupported _Block_object_assign field flags {flags:#x} (kind {kind})"),
    };
    env.mem.write(destination, copied.cast_const());
}

#[allow(non_snake_case)]
fn _Block_object_dispose(env: &mut Environment, object: ConstVoidPtr, flags: i32) {
    if flags & BLOCK_BYREF_CALLER != 0 {
        return;
    }

    let weak = flags & BLOCK_FIELD_IS_WEAK != 0;
    match flags & BLOCK_FIELD_KIND_MASK {
        BLOCK_FIELD_IS_OBJECT => {
            if !weak {
                release(env, object.cast_mut().cast());
            }
        }
        BLOCK_FIELD_IS_BLOCK => block_release(env, object),
        BLOCK_FIELD_IS_BYREF => byref_release(env, object),
        kind => panic!("Unsupported _Block_object_dispose field flags {flags:#x} (kind {kind})"),
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation _NSConcreteStackBlock: NSObject

- (id)copyWithZone:(NSZonePtr)_zone {
    block_copy(env, this.cast().cast_const()).cast()
}
- (id)retain {
    this
}
- (())release {
}

@end

@implementation _NSConcreteGlobalBlock: NSObject

- (id)copyWithZone:(NSZonePtr)_zone {
    this
}
- (id)retain {
    this
}
- (())release {
}

@end

@implementation _NSConcreteMallocBlock: NSObject

- (id)copyWithZone:(NSZonePtr)_zone {
    block_copy(env, this.cast().cast_const()).cast()
}
- (id)retain {
    block_copy(env, this.cast().cast_const()).cast()
}
- (())release {
    block_release(env, this.cast().cast_const())
}

@end
};

pub const CONSTANTS: ConstantExports = &[
    (
        "__NSConcreteStackBlock",
        HostConstant::Custom(|env| {
            env.objc
                .get_known_class("_NSConcreteStackBlock", &mut env.mem)
                .cast()
                .cast_const()
        }),
    ),
    (
        "__NSConcreteGlobalBlock",
        HostConstant::Custom(|env| {
            env.objc
                .get_known_class("_NSConcreteGlobalBlock", &mut env.mem)
                .cast()
                .cast_const()
        }),
    ),
    (
        "__NSConcreteMallocBlock",
        HostConstant::Custom(|env| {
            env.objc
                .get_known_class("_NSConcreteMallocBlock", &mut env.mem)
                .cast()
                .cast_const()
        }),
    ),
];

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(_Block_copy(_)),
    export_c_func!(objc_retainBlock(_)),
    export_c_func!(_Block_release(_)),
    export_c_func!(_Block_object_assign(_, _, _)),
    export_c_func!(_Block_object_dispose(_, _)),
];

#[cfg(test)]
mod tests {
    use super::{
        decrement_heap_refcount, increment_heap_refcount, BLOCK_HAS_COPY_DISPOSE, BLOCK_NEEDS_FREE,
    };

    #[test]
    fn block_refcount_preserves_abi_flags() {
        let flags = BLOCK_NEEDS_FREE | BLOCK_HAS_COPY_DISPOSE | 1;
        let flags = increment_heap_refcount(flags);
        assert_eq!(flags & 0xffff, 2);
        assert_ne!(flags & BLOCK_HAS_COPY_DISPOSE, 0);

        let (flags, should_free) = decrement_heap_refcount(flags);
        assert_eq!(flags & 0xffff, 1);
        assert!(!should_free);
        let (_, should_free) = decrement_heap_refcount(flags);
        assert!(should_free);
    }
}
