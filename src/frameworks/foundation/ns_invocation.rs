/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSInvocation`.

use crate::abi::{extend_stack_for_args, write_next_arg, GuestArg, GuestRet};
use crate::cpu::Cpu;
use crate::frameworks::foundation::ns_method_signature::type_encoding_size;
use crate::frameworks::foundation::{NSInteger, NSUInteger};
use crate::libc::string::strdup;
use crate::mem::{ConstPtr, MutPtr, MutVoidPtr};
use crate::msg;
use crate::objc::{
    autorelease, id, nil, objc_classes, objc_msgSend, release, retain, ClassExports, HostObject,
    SEL,
};

struct NSInvocationHostObject {
    /// `NSMethodSignature *`
    sig: id,
    /// Argument type strings resolved from `sig` at creation time
    argument_types: Vec<String>,
    /// Return type string resolved from `sig` at creation time.
    return_type: String,
    /// Owned buffer holding the return value, allocated when there is one to
    /// hold. `None` for a void method, or before the first `invoke`.
    return_value: Option<MutVoidPtr>,
    target: id,
    selector: Option<SEL>,
    /// Per-slot owned buffer for each argument.
    /// Option denotes if argument was set with `setArgument:atIndex:`
    arguments: Vec<Option<MutVoidPtr>>,
    arguments_retained: bool,
    /// Objects retained by `retainArguments`
    retained_objects: Vec<id>,
    /// C string copies made by `retainArguments`
    copied_strings: Vec<MutPtr<u8>>,
}
impl HostObject for NSInvocationHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSInvocation: NSObject

+ (id)invocationWithMethodSignature:(id)sig { // NSMethodSignature *
    retain(env, sig);
    let num_of_args: NSUInteger = msg![env; sig numberOfArguments];
    let mut argument_types: Vec<String> = Vec::with_capacity(num_of_args as usize);
    for i in 0..num_of_args {
        let type_ptr: ConstPtr<u8> = msg![env; sig getArgumentTypeAtIndex:i];
        argument_types.push(env.mem.cstr_at_utf8(type_ptr).unwrap().to_string());
    }
    let return_type_ptr: ConstPtr<u8> = msg![env; sig methodReturnType];
    let return_type = env.mem.cstr_at_utf8(return_type_ptr).unwrap().to_string();
    let host_object = Box::new(NSInvocationHostObject {
        sig,
        argument_types,
        return_type,
        return_value: None,
        target: nil,
        selector: None,
        arguments: vec![None; num_of_args as usize],
        arguments_retained: false,
        retained_objects: Vec::new(),
        copied_strings: Vec::new(),
    });
    let res = env.objc.alloc_object(this, host_object, &mut env.mem);
    autorelease(env, res)
}

// The read side of the target/selector pair. An app that hands an invocation
// around commonly asks what it will call before deciding whether to invoke it.
- (id)target {
    env.objc.borrow::<NSInvocationHostObject>(this).target
}

- (SEL)selector {
    // A selector was always set before this is asked for in practice; a nil
    // selector has no representation in the SEL type, so an unset one is an
    // app-visible programming error rather than something to paper over.
    env.objc
        .borrow::<NSInvocationHostObject>(this)
        .selector
        .expect("-[NSInvocation selector] before -setSelector:")
}

- (id)methodSignature {
    env.objc.borrow::<NSInvocationHostObject>(this).sig
}

- (())setTarget:(id)target {
    let old_target = env.objc.borrow::<NSInvocationHostObject>(this).target;
    let arguments_retained = env.objc.borrow::<NSInvocationHostObject>(this).arguments_retained;
    env.objc.borrow_mut::<NSInvocationHostObject>(this).target = target;
    if arguments_retained {
        retain(env, target);
        release(env, old_target);
    }
}

- (())setSelector:(SEL)selector {
    // Reassignment is legal: an invocation can be reconfigured and invoked
    // again. The arguments are indexed by the method signature rather than by
    // the selector, so nothing else has to change here.
    env.objc.borrow_mut::<NSInvocationHostObject>(this).selector = Some(selector);
}

- (())retainArguments {
    // TODO: handle return val
    // TODO: copy blocks
    assert!(!env.objc.borrow::<NSInvocationHostObject>(this).arguments_retained); // TODO

    let target = env.objc.borrow::<NSInvocationHostObject>(this).target;
    retain(env, target);

    let mut retained_objects: Vec<id> = Vec::new();
    let mut copied_strings: Vec<MutPtr<u8>> = Vec::new();

    // Skip index 0 (self) and 1 (SEL): handled via target/selector fields.
    let num_of_args = env.objc.borrow::<NSInvocationHostObject>(this).argument_types.len();
    for i in 2..num_of_args {
        let host = env.objc.borrow::<NSInvocationHostObject>(this);
        let Some(arg_loc) = host.arguments[i] else { continue };
        match host.argument_types[i].as_str() {
            "@" => {
                let obj: id = env.mem.read(arg_loc.cast().cast_const());
                retain(env, obj);
                retained_objects.push(obj);
            }
            "*" => {
                let str: MutPtr<u8> = env.mem.read(arg_loc.cast().cast_const());
                let str_copy = strdup(env, str.cast_const());
                env.mem.write(arg_loc.cast(), str_copy);
                copied_strings.push(str_copy);
            }
            _ => {}
        }
    }

    let host = env.objc.borrow_mut::<NSInvocationHostObject>(this);
    host.retained_objects = retained_objects;
    host.copied_strings = copied_strings;
    host.arguments_retained = true;
}

- (())setArgument:(MutVoidPtr)arg_loc
          atIndex:(NSInteger)idx {
    let &NSInvocationHostObject {
        ref arguments,
        arguments_retained,
        ..
    } = env.objc.borrow::<NSInvocationHostObject>(this);

    // 0 and 1 are reserved for `self` and `_cmd`
    // TODO: can they be set too?
    assert!(1 < idx && idx < arguments.len() as NSInteger);

    if let Some(prev_arg) = arguments[idx as usize] {
        env.mem.free(prev_arg.cast());
    }

    let argument_types: &Vec<String> = env.objc.borrow::<NSInvocationHostObject>(this).argument_types.as_ref();
    let arg_type = argument_types.get(idx as usize).unwrap();
    let new: MutVoidPtr = match arg_type.as_str() {
        "f" => {
            let arg_loc: MutPtr<f32> = arg_loc.cast();
            let arg = env.mem.read(arg_loc);
            env.mem.alloc_and_write(arg).cast()
        }
        "@" => {
            assert!(!arguments_retained); // TODO
            let arg_loc: MutPtr<id> = arg_loc.cast();
            let arg = env.mem.read(arg_loc);
            env.mem.alloc_and_write(arg).cast()
        }
        ":" => {
            let arg_loc: MutPtr<SEL> = arg_loc.cast();
            let arg = env.mem.read(arg_loc);
            env.mem.alloc_and_write(arg).cast()
        }
        // Every one-word scalar. They are stored identically; the encoding
        // only matters when the value is read back out, and the caller that
        // set it is the one that knows. Listed in full rather than grown one
        // crash at a time.
        "c" | "C" | "B" | "s" | "S" | "i" | "I" | "l" | "L" => {
            let arg_loc: MutPtr<u32> = arg_loc.cast();
            let arg = env.mem.read(arg_loc);
            env.mem.alloc_and_write(arg).cast()
        }
        "q" | "Q" | "d" => {
            let arg_loc: MutPtr<u64> = arg_loc.cast();
            let arg = env.mem.read(arg_loc);
            env.mem.alloc_and_write(arg).cast()
        }
        "*" => {
            assert!(!arguments_retained); // TODO
            let arg_loc: MutPtr<MutPtr<u8>> = arg_loc.cast();
            let arg = env.mem.read(arg_loc);
            env.mem.alloc_and_write(arg).cast()
        }
        // pointer cases
        _ if arg_type.starts_with('^') => {
            let arg_loc: MutPtr<MutVoidPtr> = arg_loc.cast();
            let arg = env.mem.read(arg_loc);
            env.mem.alloc_and_write(arg).cast()
        }
        _ => unimplemented!("unhandled argument type {arg_type}"),
    };

    env.objc.borrow_mut::<NSInvocationHostObject>(this).arguments[idx as usize] = Some(new);
}

- (())invokeWithTarget:(id)target {
    () = msg![env; this setTarget:target];
    () = msg![env; this invoke];
}

- (())invoke {
    // An argument that was never set is passed as zero, and that is not a
    // caller error: the real runtime allocates the invocation's argument
    // buffer zero-filled, so a caller that sets only the arguments it cares
    // about gets nil or 0 for the rest. Requiring every slot to be set turned
    // that ordinary pattern into an abort during launch.

    // `call_from_host` re-use
    // TODO: retval_ptr
    // TODO: cross check against frame length from NSMethodSignature
    let mut reg_count = 0;
    let argument_types: &Vec<String> = env.objc.borrow::<NSInvocationHostObject>(this).argument_types.as_ref();
    for arg_type in argument_types.iter() {
        // TODO: refactor and simplify
        reg_count += match arg_type.as_str() {
            "@" => <id as GuestArg>::REG_COUNT,
            ":" => <SEL as GuestArg>::REG_COUNT,
            "f" => <f32 as GuestArg>::REG_COUNT,
            "c" | "C" | "B" | "s" | "S" | "i" | "I" | "l" | "L" => {
                <u32 as GuestArg>::REG_COUNT
            }
            "q" | "Q" | "d" => <u64 as GuestArg>::REG_COUNT,
            "*" => <MutPtr<u8> as GuestArg>::REG_COUNT,
            // pointer cases
            _ if arg_type.starts_with('^') => <MutVoidPtr as GuestArg>::REG_COUNT,
            _ => unimplemented!("reg_count for {arg_type}")
        }
    }
    let regs = env.cpu.regs_mut();
    let old_sp = extend_stack_for_args(
        reg_count,
        regs,
    );

    let arguments: &Vec<Option<MutVoidPtr>> = env.objc.borrow::<NSInvocationHostObject>(this).arguments.as_ref();
    let mut reg_offset = 0;
    for i in 0..arguments.len() {
        // TODO: do not handle target and sel as special cases
        if i == 0 {
            assert!(argument_types[i] == "@");
            // target
            let target = env.objc.borrow::<NSInvocationHostObject>(this).target;
            let regs = env.cpu.regs_mut();
            write_next_arg::<id>(&mut reg_offset, regs, &mut env.mem, target);
            continue;
        }
        if i == 1 {
            assert!(argument_types[i] == ":");
            // selector
            let selector = env.objc.borrow::<NSInvocationHostObject>(this).selector.unwrap();
            let regs = env.cpu.regs_mut();
            write_next_arg::<SEL>(&mut reg_offset, regs, &mut env.mem, selector);
            continue;
        }
        let arg_slot = arguments[i];
        let arg_type = argument_types[i].as_str();
        // TODO: refactor and simplify
        match arg_type {
            "@" => {
                let arg_val = arg_slot.map_or(nil, |slot| env.mem.read(slot.cast().cast_const()));
                let regs = env.cpu.regs_mut();
                write_next_arg::<id>(&mut reg_offset, regs, &mut env.mem, arg_val);
            }
            ":" => {
                let arg_val = arg_slot
                    .map_or_else(SEL::null, |slot| env.mem.read(slot.cast().cast_const()));
                let regs = env.cpu.regs_mut();
                write_next_arg::<SEL>(&mut reg_offset, regs, &mut env.mem, arg_val);
            }
            "f" => {
                let arg_val = arg_slot.map_or(0f32, |slot| env.mem.read(slot.cast().cast_const()));
                let regs = env.cpu.regs_mut();
                write_next_arg::<f32>(&mut reg_offset, regs, &mut env.mem, arg_val);
            },
            "c" | "C" | "B" | "s" | "S" | "i" | "I" | "l" | "L" => {
                let arg_val = arg_slot.map_or(0u32, |slot| env.mem.read(slot.cast().cast_const()));
                let regs = env.cpu.regs_mut();
                write_next_arg::<u32>(&mut reg_offset, regs, &mut env.mem, arg_val);
            }
            "q" | "Q" | "d" => {
                let arg_val = arg_slot.map_or(0u64, |slot| env.mem.read(slot.cast().cast_const()));
                let regs = env.cpu.regs_mut();
                write_next_arg::<u64>(&mut reg_offset, regs, &mut env.mem, arg_val);
            }
            "*" => {
                let arg_val = arg_slot
                    .map_or(MutPtr::null(), |slot| env.mem.read(slot.cast().cast_const()));
                let regs = env.cpu.regs_mut();
                write_next_arg::<MutPtr<u8>>(&mut reg_offset, regs, &mut env.mem, arg_val);
            }
            // pointer cases
            _ if arg_type.starts_with('^') => {
                let arg_val = arg_slot
                    .map_or(MutVoidPtr::null(), |slot| env.mem.read(slot.cast().cast_const()));
                let regs = env.cpu.regs_mut();
                write_next_arg::<MutVoidPtr>(&mut reg_offset, regs, &mut env.mem, arg_val);
            }
            _ => unimplemented!("write_next_arg for {arg_type}")
        }
    }

    // actual invocation
    let &NSInvocationHostObject { target, selector, .. } = env.objc.borrow::<NSInvocationHostObject>(this);
    objc_msgSend(env, target, selector.unwrap());

    // Capture the return value before anything else runs and clobbers the
    // registers. `objc_msgSend` above is a complete synchronous call — a guest
    // implementation runs to its `bx lr` inside it — so the result is sitting
    // in the return registers right now and nowhere else.
    let return_type = env.objc.borrow::<NSInvocationHostObject>(this).return_type.clone();
    let size = type_encoding_size(&return_type);
    if size != 0 {
        // Reuse the buffer across repeat invocations; the size cannot change,
        // because it comes from the signature this invocation was built with.
        let buffer = match env.objc.borrow::<NSInvocationHostObject>(this).return_value {
            Some(buffer) => buffer,
            None => {
                let buffer = env.mem.alloc(size);
                env.objc.borrow_mut::<NSInvocationHostObject>(this).return_value = Some(buffer);
                buffer
            }
        };
        let regs = env.cpu.regs();
        match (size, return_type.as_bytes()[0]) {
            (8, b'd') => env.mem.write(buffer.cast(), <f64 as GuestRet>::from_regs(regs)),
            (8, _) => env.mem.write(buffer.cast(), <u64 as GuestRet>::from_regs(regs)),
            (4, b'f') => env.mem.write(buffer.cast(), <f32 as GuestRet>::from_regs(regs)),
            (4, _) => env.mem.write(buffer.cast(), <u32 as GuestRet>::from_regs(regs)),
            // A narrow integer is returned in the low bits of r0, widened by
            // the callee; truncating is how the caller would read it too.
            (2, _) => env.mem.write(buffer.cast(), <u32 as GuestRet>::from_regs(regs) as u16),
            (_, _) => env.mem.write(buffer.cast(), <u32 as GuestRet>::from_regs(regs) as u8),
        }

        // The stack has to go back before the retain below, which is itself a
        // message send and would otherwise run on the extended frame.
        env.cpu.regs_mut()[Cpu::SP] = old_sp;

        // -retainArguments covers the return value as well, and this is the
        // only moment at which there is one to retain.
        let &NSInvocationHostObject { arguments_retained, .. } =
            env.objc.borrow::<NSInvocationHostObject>(this);
        if arguments_retained && return_type.starts_with('@') {
            let object: id = env.mem.read(buffer.cast().cast_const());
            retain(env, object);
            env.objc.borrow_mut::<NSInvocationHostObject>(this).retained_objects.push(object);
        }
    } else {
        env.cpu.regs_mut()[Cpu::SP] = old_sp;
    }
}

// Copy the return value out. The caller supplies a buffer of at least
// -methodReturnLength bytes, as with -getArgument:atIndex:.
//
// Reading before invoking, or from a void method, leaves the buffer alone
// rather than filling it with zeroes: the real runtime returns whatever the
// uninitialised invocation held, and zeroing would turn "not run yet" into a
// convincing nil.
- (())getReturnValue:(MutVoidPtr)buffer {
    if buffer.is_null() {
        return;
    }
    let &NSInvocationHostObject { ref return_type, return_value, .. } =
        env.objc.borrow::<NSInvocationHostObject>(this);
    let size = type_encoding_size(return_type);
    let Some(source) = return_value else {
        log!("-[NSInvocation getReturnValue:] before -invoke, leaving the buffer unwritten");
        return;
    };
    let bytes = env.mem.bytes_at(source.cast().cast_const(), size).to_vec();
    env.mem.bytes_at_mut(buffer.cast(), size).copy_from_slice(&bytes);
}

// Set the return value directly, which is what a forwarding target does after
// handling a message itself instead of invoking it.
- (())setReturnValue:(MutVoidPtr)buffer {
    if buffer.is_null() {
        return;
    }
    let return_type = env.objc.borrow::<NSInvocationHostObject>(this).return_type.clone();
    let size = type_encoding_size(&return_type);
    if size == 0 {
        return;
    }
    let destination = match env.objc.borrow::<NSInvocationHostObject>(this).return_value {
        Some(destination) => destination,
        None => {
            let destination = env.mem.alloc(size);
            env.objc.borrow_mut::<NSInvocationHostObject>(this).return_value = Some(destination);
            destination
        }
    };
    let bytes = env.mem.bytes_at(buffer.cast().cast_const(), size).to_vec();
    env.mem.bytes_at_mut(destination.cast(), size).copy_from_slice(&bytes);
}

- (NSUInteger)methodReturnLength {
    let return_type = &env.objc.borrow::<NSInvocationHostObject>(this).return_type;
    type_encoding_size(return_type)
}

- (())dealloc {
    let &NSInvocationHostObject { sig, target, arguments_retained, .. } = env.objc.borrow::<NSInvocationHostObject>(this);
    release(env, sig);
    if arguments_retained {
        release(env, target);
        let retained_objects = std::mem::take(
            &mut env.objc.borrow_mut::<NSInvocationHostObject>(this).retained_objects
        );
        for obj in retained_objects {
            release(env, obj);
        }
        let copied_strings = std::mem::take(
            &mut env.objc.borrow_mut::<NSInvocationHostObject>(this).copied_strings
        );
        for s in copied_strings {
            env.mem.free(s.cast());
        }
    } else {
        assert!(env.objc.borrow::<NSInvocationHostObject>(this).retained_objects.is_empty());
        assert!(env.objc.borrow::<NSInvocationHostObject>(this).copied_strings.is_empty());
    }
    for ptr in env.objc.borrow::<NSInvocationHostObject>(this).arguments.iter().flatten() {
        env.mem.free(ptr.cast());
    }
    if let Some(return_value) = env.objc.borrow::<NSInvocationHostObject>(this).return_value {
        env.mem.free(return_value.cast());
    }
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

};
