/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Handling of Objective-C messaging (`objc_msgSend` and friends).
//!
//! Resources:
//! - Apple's [Objective-C Runtime Programming Guide](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/ObjCRuntimeGuide/Articles/ocrtHowMessagingWorks.html)
//! - [Apple's documentation of `objc_msgSend`](https://developer.apple.com/documentation/objectivec/1456712-objc_msgsend)
//! - Mike Ash's [objc_msgSend's New Prototype](https://www.mikeash.com/pyblog/objc_msgsends-new-prototype.html)
//! - Peter Steinberger's [Calling Super at Runtime in Swift](https://steipete.com/posts/calling-super-at-runtime/) explains `objc_msgSendSuper2`

use super::objects::CxxLifecycle;
use super::{id, nil, Class, ClassHostObject, ObjC, IMP, SEL};
use crate::abi::{write_next_arg, CallFromHost, GuestRet};
use crate::environment::ThreadId;
use crate::libc::pthread::cond::{
    pthread_cond_broadcast, pthread_cond_destroy, pthread_cond_init, pthread_cond_t,
    pthread_cond_wait,
};
use crate::libc::pthread::mutex::{
    pthread_mutex_destroy, pthread_mutex_init, pthread_mutex_lock, pthread_mutex_t,
    pthread_mutex_unlock,
};
use crate::mem::{guest_size_of, ConstPtr, MutPtr, MutVoidPtr, Ptr, SafeRead};
use crate::objc::classes::InitializationStatus;
use crate::Environment;
use std::any::TypeId;
use std::sync::LazyLock;

/// Call an Objective-C++ lifecycle method on one exact class implementation.
/// Normal message dispatch is deliberately bypassed because every class-local
/// implementation in the hierarchy must run.
fn call_cxx_method(env: &mut Environment, object: id, selector: SEL, imp: IMP) -> id {
    let saved_regs = *env.cpu.regs();
    let result = match imp {
        IMP::Host(host_imp) => {
            // There are currently no host lifecycle methods, but handling them
            // keeps future host classes and guest categories unsurprising.
            let mut reg_offset = 0;
            write_next_arg(&mut reg_offset, env.cpu.regs_mut(), &mut env.mem, object);
            write_next_arg(&mut reg_offset, env.cpu.regs_mut(), &mut env.mem, selector);
            host_imp.call_from_guest(env);
            let result: id = Ptr::from_bits(env.cpu.regs()[0]);
            result
        }
        IMP::Guest(guest_imp) => guest_imp.call_from_host(env, (object, selector)),
    };
    env.cpu.regs_mut().copy_from_slice(&saved_regs);
    result
}

fn class_chain_from_class(objc: &ObjC, mut class: Class) -> Vec<Class> {
    let mut classes = Vec::new();
    while class != nil {
        classes.push(class);
        class = objc.borrow::<ClassHostObject>(class).superclass;
    }
    classes
}

fn class_chain(env: &Environment, object: id) -> Vec<Class> {
    class_chain_from_class(&env.objc, ObjC::read_isa(object, &env.mem))
}

fn uninherited_cxx_imp(env: &Environment, class: Class, selector: SEL) -> Option<IMP> {
    env.objc
        .borrow::<ClassHostObject>(class)
        .methods
        .get(&selector)
        .copied()
}

fn call_cxx_destructors(
    env: &mut Environment,
    object: id,
    classes_subclass_first: impl IntoIterator<Item = Class>,
) {
    let Some(selector) = env.objc.lookup_selector(".cxx_destruct") else {
        return;
    };
    for class in classes_subclass_first {
        let Some(imp) = uninherited_cxx_imp(env, class, selector) else {
            continue;
        };
        log_dbg!(
            "Calling .cxx_destruct for class {} ({:?}) on {:?}",
            env.objc.get_class_name(class),
            class,
            object
        );
        let _ = call_cxx_method(env, object, selector, imp);
    }
}

/// Finish construction of a newly allocated object returned by a host method.
/// A nil constructor result means failure: already-constructed superclasses
/// are unwound and the allocation is discarded, matching objc4.
fn maybe_construct_object(env: &mut Environment, object: id) -> id {
    let Some(entry) = env.objc.objects.get_mut(&object) else {
        return object;
    };
    if entry.cxx_lifecycle != CxxLifecycle::Allocated {
        return object;
    }
    entry.cxx_lifecycle = CxxLifecycle::Constructing;

    let mut classes = class_chain(env, object);
    classes.reverse();

    if let Some(selector) = env.objc.lookup_selector(".cxx_construct") {
        for (index, &class) in classes.iter().enumerate() {
            let Some(imp) = uninherited_cxx_imp(env, class, selector) else {
                continue;
            };
            log_dbg!(
                "Calling .cxx_construct for class {} ({:?}) on {:?}",
                env.objc.get_class_name(class),
                class,
                object
            );
            if call_cxx_method(env, object, selector, imp) == nil {
                // The failing class itself was never fully constructed.
                call_cxx_destructors(env, object, classes[..index].iter().rev().copied());
                env.objc.objects.get_mut(&object).unwrap().cxx_lifecycle = CxxLifecycle::Destructed;
                env.objc
                    .discard_failed_cxx_construction(object, &mut env.mem);
                return nil;
            }
        }
    }

    env.objc.objects.get_mut(&object).unwrap().cxx_lifecycle = CxxLifecycle::Constructed;
    object
}

/// Run destructors immediately before dispatch reaches the first host
/// deallocator. Guest `-dealloc` overrides therefore run first, while the
/// lifecycle state prevents host super-calls from running destructors twice.
fn maybe_destruct_object(env: &mut Environment, object: id) {
    let Some(entry) = env.objc.objects.get_mut(&object) else {
        return;
    };
    if entry.cxx_lifecycle != CxxLifecycle::Constructed {
        return;
    }
    entry.cxx_lifecycle = CxxLifecycle::Destructing;

    let classes = class_chain(env, object);
    call_cxx_destructors(env, object, classes);
    if let Some(entry) = env.objc.objects.get_mut(&object) {
        entry.cxx_lifecycle = CxxLifecycle::Destructed;
    }
}

pub(super) struct ThreadInitializer {
    mutex: MutPtr<pthread_mutex_t>,
    cond: MutPtr<pthread_cond_t>,
    tid: ThreadId,
    waiters: u32,
}

fn maybe_initialize_class(env: &mut Environment, receiver: id) {
    let Some(class_host_object) = env.objc.get_host_object(receiver) else {
        // Compiler-created objects such as stack and global block literals do
        // not have a HostObjectEntry. Their isa still names a real class, so
        // initialize that class before dispatching the first message.
        let class = ObjC::read_isa(receiver, &env.mem);
        if class == nil {
            // Except when it does not. A global block literal whose
            // `_NSConcreteGlobalBlock` import was never bound still reads as a
            // nil isa here, and a `super` send reaches this point without the
            // receiver's own isa having been looked at at all. Sending
            // +initialize is bookkeeping ahead of dispatch, so there is simply
            // nothing to initialize; the dispatch that follows uses the class
            // the caller already resolved, and reports its own failure by name
            // if there is one.
            log_dbg!("No class to initialize for {:?}, its isa is nil", receiver);
            return;
        }
        maybe_initialize_class(env, class);
        return;
    };
    let Some(&super::ClassHostObject {
        superclass,
        is_metaclass,
        is_initialized,
        ..
    }) = class_host_object.as_any().downcast_ref()
    else {
        // If it's here, there's one of two cases:
        //
        // 1: The receiver is an instance. The class should then have already
        // called +initialize since you need to call +alloc to create an
        // instance (this also needs to be upheld for instances created with
        // class_createInstance(), whenever we implement that)
        //
        // 2: The reciever is a fake/unimplemented class. There's no reason to
        // send +initialize to those, so we don't bother.
        return;
    };

    if is_metaclass || is_initialized == InitializationStatus::Initialized {
        // On the offchance that this is a metaclass, we don't need to send
        // +initialize to it. We also don't need to send it if the class is
        // already initialized.
        return;
    }

    // This class is not initialized, but there might be classes above it in the
    // hierarchy that also need to be checked, so check those first.
    if !superclass.is_null() {
        maybe_initialize_class(env, superclass);
    }

    if is_initialized == InitializationStatus::Initializing {
        env.objc
            .initializer_threads
            .get_mut(&receiver)
            .unwrap()
            .waiters += 1;
        let ThreadInitializer {
            mutex, cond, tid, ..
        } = *env.objc.initializer_threads.get(&receiver).unwrap();

        // The current thread is already initializing, so let it call other
        // messages while it does so.
        if tid == env.current_thread {
            return;
        }

        // We are waiting for another thread to initialize, wait for it to
        // broadcast that it has finished.
        pthread_mutex_lock(env, mutex);
        loop {
            let class_host_object = env.objc.get_host_object(receiver).unwrap();
            let &super::ClassHostObject { is_initialized, .. } =
                class_host_object.as_any().downcast_ref().unwrap();
            if is_initialized == InitializationStatus::Initialized {
                break;
            }
            pthread_cond_wait(env, cond, mutex);
        }
        pthread_mutex_unlock(env, mutex);

        let ThreadInitializer {
            ref mut waiters, ..
        } = *env.objc.initializer_threads.get_mut(&receiver).unwrap();
        *waiters -= 1;
        if *waiters == 0 {
            // We're the last waiter for this initialize, so clean up state on
            // the way out.
            pthread_cond_destroy(env, cond);
            pthread_mutex_destroy(env, mutex);
            env.objc.initializer_threads.remove(&receiver);
        }
    } else {
        log_dbg!(
            "Initializing {:?} on thread {}",
            env.objc.try_get_class_name(receiver),
            env.current_thread
        );
        let regs = *env.cpu.regs();

        let mutex = env.mem.alloc(guest_size_of::<pthread_mutex_t>()).cast();
        let cond = env.mem.alloc(guest_size_of::<pthread_cond_t>()).cast();
        pthread_mutex_init(env, mutex, ConstPtr::null());
        pthread_cond_init(env, cond, ConstPtr::null());
        env.objc.initializer_threads.insert(
            receiver,
            ThreadInitializer {
                mutex,
                cond,
                tid: env.current_thread,
                waiters: 0,
            },
        );

        let super::ClassHostObject { is_initialized, .. } = env.objc.borrow_mut(receiver);
        *is_initialized = InitializationStatus::Initializing;
        () = msg![env; receiver initialize];
        let super::ClassHostObject { is_initialized, .. } = env.objc.borrow_mut(receiver);
        *is_initialized = InitializationStatus::Initialized;
        env.cpu.regs_mut().copy_from_slice(&regs);
        log_dbg!(
            "Done initializing {:?} on thread {}",
            env.objc.try_get_class_name(receiver),
            env.current_thread
        );
        if env.objc.initializer_threads.get(&receiver).unwrap().waiters == 0 {
            // Nobody ended up waiting for this initializer, so we can just
            // destroy it.
            pthread_cond_destroy(env, cond);
            pthread_mutex_destroy(env, mutex);
            env.objc.initializer_threads.remove(&receiver);
        } else {
            pthread_mutex_lock(env, mutex);
            pthread_cond_broadcast(env, cond);
            pthread_mutex_unlock(env, mutex);
        }
    }
}

/// The core implementation of `objc_msgSend`, the main function of Objective-C.
///
/// Note that while only two parameters (usually receiver and selector) are
/// defined by the wrappers over this function, a call to an `objc_msgSend`
/// variant may have additional arguments to be forwarded (or rather, left
/// untouched) by `objc_msgSend` when it tail-calls the method implementation it
/// looks up. This is invisible to the Rust type system; we're relying on
/// [crate::abi::CallFromGuest] here.
///
/// Similarly, the return value of `objc_msgSend` is whatever value is returned
/// by the method implementation. We are relying on CallFromGuest not
/// overwriting it.
/// Name of an environment variable holding a comma-separated list of selector
/// names to trace, e.g. `touchesBegan:withEvent:,setAppMode:`.
///
/// Every matching dispatch logs the receiving class and the selector. This is a
/// bounded alternative to enabling `log_dbg` for this module, which logs every
/// message send and is far too noisy to use on a running game. The special
/// value `all` traces everything, which is only useful for a very short window.
pub const TRACE_SELECTORS_ENV_VAR: &str = "TAPHLE_TRACE_SELECTORS";

/// Log a dispatch when its selector was named in [TRACE_SELECTORS_ENV_VAR].
fn trace_selector(env: &mut Environment, receiver: id, selector: SEL, super2: Option<Class>) {
    static TRACED: LazyLock<Vec<String>> = LazyLock::new(|| {
        std::env::var(TRACE_SELECTORS_ENV_VAR)
            .unwrap_or_default()
            .split(',')
            .map(|entry| entry.trim().to_string())
            .filter(|entry| !entry.is_empty())
            .collect()
    });
    if TRACED.is_empty() {
        return;
    }

    let selector_str = selector.as_str(&env.mem);
    if !TRACED.iter().any(|t| t == "all" || t == selector_str) {
        return;
    }
    let selector_str = selector_str.to_string();

    // Name the receiver by class where possible. A nil receiver and a freed
    // object both have no readable class, and both are worth seeing: they are
    // the usual reason a traced message appears to do nothing.
    let class_name = if receiver == nil {
        "nil".to_string()
    } else {
        let class = super2.unwrap_or_else(|| ObjC::read_isa(receiver, &env.mem));
        match env.objc.try_get_class_name(class) {
            Some(name) => name.to_string(),
            None => "<no class>".to_string(),
        }
    };
    log!("trace: [{} ({:?}) {}]", class_name, receiver, selector_str);
}

#[allow(non_snake_case)]
fn objc_msgSend_inner(
    env: &mut Environment,
    receiver: id,
    selector: SEL,
    super2: Option<Class>,
    tolerate_type_mismatch: bool,
    skip_initialize: bool,
) {
    log_dbg!(
        "Dispatching {} for {:?}",
        selector.as_str(&env.mem),
        receiver
    );
    trace_selector(env, receiver, selector, super2);
    let message_type_info = env.objc.message_type_info.take();

    if receiver == nil {
        // https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/ObjectiveC/Chapters/ocObjectsClasses.html#//apple_ref/doc/uid/TP30001163-CH11-SW7
        log_dbg!("[nil {}]", selector.as_str(&env.mem));
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }

    let orig_class = super2.unwrap_or_else(|| ObjC::read_isa(receiver, &env.mem));
    if orig_class == nil {
        // A nil isa on a non-nil receiver means a freed object is being
        // messaged, typically after an over-release.
        //
        // On Apple's runtime this is undefined behaviour that very often does
        // nothing visible: the memory has not been reused yet, or the object
        // was a constant string or other immortal whose release did nothing in
        // the first place. So apps ship with this bug and work, and killing
        // them here reports a real defect at the least useful possible moment —
        // far from the over-release that caused it, with nothing on the stack
        // that identifies the culprit. It was the second largest tapHLE-side
        // crash in a survey of 1501 apps.
        //
        // Treat it as a message to nil, which is what the guest's own code is
        // written to tolerate. The warning is not rate-limited, because how
        // often this happens is itself the signal.
        let selector_str = selector.as_str(&env.mem).to_string();
        log!(
            "Warning: sending {:?} to {:?}, which has already been deallocated. \
             It was over-released elsewhere; treating this as a message to nil.",
            selector_str,
            receiver
        );
        // Same answer as the nil receiver above: zeroed return registers.
        env.cpu.regs_mut()[0..2].fill(0);
        return;
    }
    if !skip_initialize {
        maybe_initialize_class(env, receiver);
    }

    // Traverse the chain of superclasses to find the method implementation.

    let mut class = orig_class;
    loop {
        if class == nil {
            assert!(class != orig_class);

            let class_host_object = env.objc.get_host_object(orig_class).unwrap();
            let &super::ClassHostObject {
                ref name,
                is_metaclass,
                ..
            } = class_host_object.as_any().downcast_ref().unwrap();

            env.dump_current_guest_state();
            panic!(
                "{} {:?} ({}class \"{}\", {:?}){} does not respond to selector \"{}\"!",
                if is_metaclass { "Class" } else { "Object" },
                receiver,
                if is_metaclass { "meta" } else { "" },
                name,
                orig_class,
                if super2.is_some() {
                    "'s superclass"
                } else {
                    ""
                },
                selector.as_str(&env.mem),
            );
        }

        let Some(host_object) = env.objc.get_host_object(class) else {
            // `class` is a pointer that is not any object tapHLE knows, let
            // alone a class, so there is no method table to search. This is the
            // same situation as the nil isa handled above and it arrives the
            // same way: the receiver is not a live object. The case seen in the
            // survey is `objc_setProperty` releasing whatever the property's
            // ivar happened to hold before its first assignment.
            //
            // Apple's runtime would follow the garbage pointer, and mostly get
            // away with it - which is why apps ship like this and work. Give
            // the same answer as a message to nil so the app carries on, and
            // say so plainly, because the count of these is the signal.
            let selector_str = selector.as_str(&env.mem).to_string();
            log!(
                "Warning: sending {:?} to {:?}, whose {} {:?} is not a class \
                 tapHLE knows; treating this as a message to nil.",
                selector_str,
                receiver,
                if class == orig_class {
                    "class"
                } else {
                    "superclass"
                },
                class
            );
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        };

        if let Some(&super::ClassHostObject {
            superclass,
            ref methods,
            ref name,
            ..
        }) = host_object.as_any().downcast_ref()
        {
            // Skip method lookup on first iteration if this is the super-call
            // variant of objc_msgSend (look up the superclass first)
            if super2.is_some() && class == orig_class {
                class = superclass;
                continue;
            }

            if let Some(&imp) = methods.get(&selector) {
                log_dbg!("Found method on: {}", name);
                match imp {
                    IMP::Host(host_imp) => {
                        // TODO: do type checks when calling GuestIMPs too.
                        // That requires using Objective-C type strings,
                        // rather than Rust types, and should probably
                        // warn rather than panicking,
                        // because apps might rely on type punning.
                        if let Some((sent_type_id, sent_type_desc)) = message_type_info {
                            let (expected_type_id, expected_type_desc) = host_imp.type_info();
                            if sent_type_id != expected_type_id {
                                let msg = format!(
                                    "\
Type mismatch when sending message {} to {:?}!
- Message has type: {:?} / {}
- Method expects type: {:?} / {}",
                                    selector.as_str(&env.mem),
                                    receiver,
                                    sent_type_id,
                                    sent_type_desc,
                                    expected_type_id,
                                    expected_type_desc
                                );
                                // Never fatal. Objective-C dispatch does no
                                // type checking whatsoever — objc_msgSend moves
                                // registers and the callee interprets them — so
                                // a mismatch is something the real runtime
                                // permits and apps genuinely rely on, whether
                                // by type punning or by declaring a method with
                                // a slightly different signature than the one
                                // tapHLE implements. Fourteen apps in a survey
                                // of 1501 died here.
                                //
                                // It is still worth saying. A mismatch means
                                // the arguments are being interpreted
                                // differently than the caller intended, and if
                                // something misbehaves shortly afterwards this
                                // is the first thing to suspect.
                                // `tolerate_type_mismatch` now selects silence
                                // rather than survival: the call sites that
                                // pass it know the mismatch is expected and
                                // would only produce noise.
                                if !tolerate_type_mismatch {
                                    log!("Warning: {}", msg);
                                }
                            }
                        }
                        if selector.as_str(&env.mem) == "dealloc" {
                            maybe_destruct_object(env, receiver);
                        }
                        host_imp.call_from_guest(env);

                        // All normal guest objects are allocated by a host
                        // implementation of +allocWithZone:. Run any C++ ivar
                        // constructors before that object is returned to its
                        // caller. Host-only classes have no such methods and
                        // simply transition to the constructed state.
                        let returned_object: id = Ptr::from_bits(env.cpu.regs()[0]);
                        let constructed_object = maybe_construct_object(env, returned_object);
                        env.cpu.regs_mut()[0] = constructed_object.to_bits();
                    }
                    // We can't create a new stack frame, because that would
                    // interfere with pass-through of stack arguments.
                    IMP::Guest(guest_imp) => guest_imp.call_without_pushing_stack_frame(env),
                }
                return;
            } else {
                class = superclass;
            }
        } else if let Some(&super::UnimplementedClass {
            ref name,
            is_metaclass,
        }) = host_object.as_any().downcast_ref()
        {
            panic!(
                "Class \"{}\" ({:?}) is unimplemented. Call to {} method \"{}\".",
                name,
                class,
                if is_metaclass { "class" } else { "instance" },
                selector.as_str(&env.mem),
            );
        } else if let Some(&super::FakeClass {
            ref name,
            is_metaclass,
        }) = host_object.as_any().downcast_ref()
        {
            log!(
                "Call to faked class \"{}\" ({:?}) {} method \"{}\". Behaving as if message was sent to nil.",
                name,
                class,
                if is_metaclass { "class" } else { "instance" },
                selector.as_str(&env.mem),
            );
            env.cpu.regs_mut()[0..2].fill(0);
            return;
        } else {
            panic!(
                "Item {class:?} in superclass chain of object {receiver:?}'s class {orig_class:?} has an unexpected host object type."
            );
        }
    }
}

/// Standard variant of `objc_msgSend`. See [objc_msgSend_inner].
#[allow(non_snake_case)]
pub(crate) fn objc_msgSend(env: &mut Environment, receiver: id, selector: SEL) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ false,
        /* skip_initialize: */ false,
    )
}

#[allow(non_snake_case)]
pub(crate) fn _tapHLE_objc_msgSend_tolerant(env: &mut Environment, receiver: id, selector: SEL) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ true,
        /* skip_initialize: */ false,
    )
}

/// Variant of `objc_msgSend` that does not trigger `+initialize`.
#[allow(non_snake_case)]
pub(crate) fn _tapHLE_objc_msgSend_no_initialize(
    env: &mut Environment,
    receiver: id,
    selector: SEL,
) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ false,
        /* skip_initialize: */ true,
    )
}

/// Variant of `objc_msgSend` for methods that return a struct via a pointer.
/// See [objc_msgSend_inner].
///
/// The first parameter here is the pointer for the struct return. This is an
/// ABI detail that is usually hidden and handled behind-the-scenes by
/// [crate::abi], but `objc_msgSend` is a special case because of the
/// pass-through behaviour. Of course, the pass-through only works if the [IMP]
/// also has the pointer parameter. The caller therefore has to pick the
/// appropriate `objc_msgSend` variant depending on the method it wants to call.
pub(super) fn objc_msgSend_stret(
    env: &mut Environment,
    _stret: MutVoidPtr,
    receiver: id,
    selector: SEL,
) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ false,
        /* skip_initialize: */ false,
    )
}

#[allow(non_snake_case)]
pub(crate) fn _tapHLE_objc_msgSend_stret_tolerant(
    env: &mut Environment,
    _stret: MutVoidPtr,
    receiver: id,
    selector: SEL,
) {
    objc_msgSend_inner(
        env, receiver, selector, /* super2: */ None, /* tolerate_type_mismatch: */ true,
        /* skip_initialize: */ false,
    )
}

#[repr(C, packed)]
/// A pointer to this struct replaces the normal receiver parameter for
/// `objc_msgSendSuper2` and [msg_send_super2].
pub struct objc_super {
    pub receiver: id,
    /// If this is used with `objc_msgSendSuper` (not implemented here, TODO),
    /// this is a pointer to the superclass to look up the method on.
    /// If this is used with `objc_msgSendSuper2`, this is a pointer to a class
    /// and the superclass will be looked up from it.
    pub class: Class,
}
unsafe impl SafeRead for objc_super {}

/// Variant of `objc_msgSend` for supercalls. See [objc_msgSend_inner].
///
/// This variant has a weird ABI because it needs to receive an additional piece
/// of information (a class pointer), but it can't actually take this as an
/// extra parameter, because that would take one of the argument slots reserved
/// for arguments passed onto the method implementation. Hence the [objc_super]
/// pointer in place of the normal [id].
#[allow(non_snake_case)]
pub(super) fn objc_msgSendSuper2(
    env: &mut Environment,
    super_ptr: ConstPtr<objc_super>,
    selector: SEL,
) {
    let objc_super { receiver, class } = env.mem.read(super_ptr);

    // Rewrite first argument to match the normal ABI.
    crate::abi::write_next_arg(&mut 0, env.cpu.regs_mut(), &mut env.mem, receiver);

    objc_msgSend_inner(
        env,
        receiver,
        selector,
        /* super2: */ Some(class),
        /* tolerate_type_mismatch: */ false,
        /* skip_initialize: */ false,
    )
}

/// Structure-return variant of [objc_msgSendSuper2]. The hidden return buffer
/// occupies r0, so the real receiver has to replace the `objc_super` pointer
/// in r1 before the selected method implementation is tail-called.
#[allow(non_snake_case)]
pub(super) fn objc_msgSendSuper2_stret(
    env: &mut Environment,
    _stret: MutVoidPtr,
    super_ptr: ConstPtr<objc_super>,
    selector: SEL,
) {
    let objc_super { receiver, class } = env.mem.read(super_ptr);

    // Preserve the hidden structure-result pointer in r0 and rewrite r1,
    // which is the receiver slot for a stret method implementation.
    crate::abi::write_next_arg(&mut 1, env.cpu.regs_mut(), &mut env.mem, receiver);

    objc_msgSend_inner(
        env,
        receiver,
        selector,
        /* super2: */ Some(class),
        /* tolerate_type_mismatch: */ false,
        /* skip_initialize: */ false,
    )
}

/// Trait that assists with type-checking of [msg_send]'s arguments.
///
/// - Statically constrains the types of [msg_send]'s arguments so that the
///   first two are always [id] and [SEL].
/// - Provides the type ID to enable dynamic type checking of subsequent
///   arguments and the return type.
///
/// See `impl_HostIMP` for implementations. See also [MsgSendSuperSignature].
pub trait MsgSendSignature: 'static {
    /// Get the [TypeId] and a human-readable description for this signature.
    fn type_info() -> (TypeId, &'static str) {
        #[cfg(debug_assertions)]
        let type_name = std::any::type_name::<Self>();
        // Avoid wasting space on type names in release builds. At the time of
        // writing this saves about 36KB.
        #[cfg(not(debug_assertions))]
        let type_name = "[description unavailable in release builds]";
        (TypeId::of::<Self>(), type_name)
    }
}

/// Wrapper around [objc_msgSend] which, together with [msg], makes it easy to
/// send messages in host code. Warning: all types are inferred from the
/// call-site and they may not be checked, so be very sure you get them correct!
pub fn msg_send<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, id, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, id, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSignature,
    R: GuestRet,
{
    // Provide type info for dynamic type checking.
    env.objc.message_type_info = Some(<(R, P) as MsgSendSignature>::type_info());
    if R::SIZE_IN_MEM.is_some() {
        (objc_msgSend_stret as fn(&mut Environment, MutVoidPtr, id, SEL)).call_from_host(env, args)
    } else {
        (objc_msgSend as fn(&mut Environment, id, SEL)).call_from_host(env, args)
    }
}

pub fn msg_send_no_type_checking<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, id, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, id, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSignature,
    R: GuestRet,
{
    if R::SIZE_IN_MEM.is_some() {
        (_tapHLE_objc_msgSend_stret_tolerant as fn(&mut Environment, MutVoidPtr, id, SEL))
            .call_from_host(env, args)
    } else {
        (_tapHLE_objc_msgSend_tolerant as fn(&mut Environment, id, SEL)).call_from_host(env, args)
    }
}

/// Variant of [msg_send] which does not trigger `+initialize` on the receiver.
///
/// This is meant for sending `+load`: the Objective-C runtime guarantees that
/// `+load` runs before `+initialize`, so it must not go through the normal
/// [msg_send] path (which would call `maybe_initialize_class` first).
pub fn msg_send_no_initialize<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, id, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSignature,
    R: GuestRet,
{
    assert!(
        R::SIZE_IN_MEM.is_none(),
        "msg_send_no_initialize does not support struct returns"
    );
    // Provide type info for dynamic type checking.
    env.objc.message_type_info = Some(<(R, P) as MsgSendSignature>::type_info());
    (_tapHLE_objc_msgSend_no_initialize as fn(&mut Environment, id, SEL)).call_from_host(env, args)
}

/// Counterpart of [MsgSendSignature] for [msg_send_super2].
pub trait MsgSendSuperSignature: 'static {
    /// Signature with the [objc_super] pointer replaced by [id].
    type WithoutSuper: MsgSendSignature;
}

/// [msg_send] but for super-calls (calls [objc_msgSendSuper2]). You probably
/// want to use [msg_super] rather than calling this directly.
pub fn msg_send_super2<R, P>(env: &mut Environment, args: P) -> R
where
    fn(&mut Environment, ConstPtr<objc_super>, SEL): CallFromHost<R, P>,
    fn(&mut Environment, MutVoidPtr, ConstPtr<objc_super>, SEL): CallFromHost<R, P>,
    (R, P): MsgSendSuperSignature,
    R: GuestRet,
{
    // Provide type info for dynamic type checking.
    env.objc.message_type_info = Some(<(R, P) as MsgSendSuperSignature>::WithoutSuper::type_info());
    if R::SIZE_IN_MEM.is_some() {
        (objc_msgSendSuper2_stret as fn(&mut Environment, MutVoidPtr, ConstPtr<objc_super>, SEL))
            .call_from_host(env, args)
    } else {
        (objc_msgSendSuper2 as fn(&mut Environment, ConstPtr<objc_super>, SEL))
            .call_from_host(env, args)
    }
}

/// Macro for sending a message which imitates the Objective-C messaging syntax.
/// See [msg_send] for the underlying implementation. Warning: all types are
/// inferred from the call-site and they may not be checked, so be very sure you
/// get them correct!
///
/// ```ignore
/// msg![env; foo setBar:bar withQux:qux];
/// ```
///
/// desugars to:
///
/// ```ignore
/// {
///     let sel = env.objc.lookup_selector("setFoo:withBar").unwrap();
///     msg_send(env, (foo, sel, bar, qux))
/// }
/// ```
///
/// Note that argument values that aren't a bare single identifier like `foo`
/// need to be bracketed.
///
/// See also [msg_class], if you want to send a message to a class.
#[macro_export]
macro_rules! msg {
    [$env:expr; $receiver:tt $name:ident $(: $arg1:tt $($($namen:ident)?: $argn:tt)*)?] => {
        {
            let sel = $crate::objc::selector!($($arg1;)? $name $($(, $($namen)?)*)?);
            let sel = $env.objc.lookup_selector(sel)
                .expect("Unknown selector");
            let args = ($receiver, sel, $($arg1, $($argn),*)?);
            $crate::objc::msg_send($env, args)
        }
    }
}
pub use crate::msg; // #[macro_export] is weird...

/// Variant of [msg] for super-calls.
///
/// Unlike the other variants, this macro can only be used within
/// [crate::objc::objc_classes], because it relies on that macro defining a
/// constant containing the name of the current class.
///
/// ```ignore
/// msg_super![env; this init]
/// ```
///
/// desugars to something like this, if the current class is `SomeClass`:
///
/// ```ignore
/// {
///     let super_arg_ptr = push_to_stack(env, objc_super {
///         receiver: this,
///         class: env.objc.get_known_class("SomeClass", &mut env.mem),
///     });
///     let sel = env.objc.lookup_selector("init").unwrap();
///     let res = msg_send_super2(env, (super_arg_ptr, sel));
///     pop_from_stack::<objc_super>(env);
///     res
/// }
/// ```
#[macro_export]
macro_rules! msg_super {
    [$env:expr; $receiver:tt $name:ident $(: $arg1:tt $($($namen:ident)?: $argn:tt)*)?] => {
        {
            let class = $env.objc.get_known_class(
                _OBJC_CURRENT_CLASS,
                &mut $env.mem
            );
            let sel = $crate::objc::selector!($($arg1;)? $name $($(, $($namen)?)*)?);
            let sel = $env.objc.lookup_selector(sel)
                .expect("Unknown selector");

            let sp = &mut $env.cpu.regs_mut()[$crate::cpu::Cpu::SP];
            let old_sp = *sp;
            *sp -= $crate::mem::guest_size_of::<$crate::objc::objc_super>();
            let super_ptr = $crate::mem::Ptr::from_bits(*sp);
            $env.mem.write(super_ptr, $crate::objc::objc_super {
                receiver: $receiver,
                class,
            });

            let args = (super_ptr.cast_const(), sel, $($arg1, $($argn),*)?);
            let res = $crate::objc::msg_send_super2($env, args);

            $env.cpu.regs_mut()[$crate::cpu::Cpu::SP] = old_sp;

            res
        }
    }
}
pub use crate::msg_super; // #[macro_export] is weird...

/// Variant of [msg] for sending a message to a named class. Useful for calling
/// class methods, especially `new`.
///
/// ```ignore
/// msg_class![env; SomeClass alloc]
/// ```
///
/// desugars to:
///
/// ```ignore
/// msg![env; (env.objc.get_known_class("SomeClass", &mut env.mem)) alloc]
/// ```
#[macro_export]
macro_rules! msg_class {
    [$env:expr; $receiver_class:ident $name:ident $(: $arg1:tt $($($namen:ident)?: $argn:tt)*)?] => {
        {
            let class = $env.objc.get_known_class(
                stringify!($receiver_class),
                &mut $env.mem
            );
            $crate::objc::msg![$env; class $name $(: $arg1 $($($namen)?: $argn)*)?]
        }
    }
}
pub use crate::msg_class; // #[macro_export] is weird...

/// Shorthand for `let _: id = msg![env; object retain];`
pub fn retain(env: &mut Environment, object: id) -> id {
    if object == nil {
        // fast path
        return nil;
    }
    msg![env; object retain]
}

/// Shorthand for `() = msg![env; object release];`
pub fn release(env: &mut Environment, object: id) {
    if object == nil {
        // fast path
        return;
    }
    msg![env; object release]
}

/// Shorthand for `let _: id = msg![env; object autorelease];`
pub fn autorelease(env: &mut Environment, object: id) -> id {
    if object == nil {
        // fast path
        return nil;
    }
    msg![env; object autorelease]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_class(name: &str, superclass: Class) -> ClassHostObject {
        ClassHostObject {
            name: name.to_string(),
            is_metaclass: false,
            superclass,
            methods: HashMap::new(),
            guest_method_signatures: HashMap::new(),
            ivars: HashMap::new(),
            properties: Vec::new(),
            base_protocols: ConstPtr::null(),
            instance_start: 4,
            instance_size: 4,
            is_initialized: InitializationStatus::NotInitialized,
        }
    }

    #[test]
    fn cxx_class_chain_supports_opposite_constructor_and_destructor_order() {
        let mut objc = ObjC::new();
        let root: Class = Ptr::from_bits(0x1000);
        let middle: Class = Ptr::from_bits(0x2000);
        let leaf: Class = Ptr::from_bits(0x3000);
        objc.register_static_object(root, Box::new(test_class("Root", nil)));
        objc.register_static_object(middle, Box::new(test_class("Middle", root)));
        objc.register_static_object(leaf, Box::new(test_class("Leaf", middle)));

        let destructor_order = class_chain_from_class(&objc, leaf);
        assert_eq!(destructor_order, vec![leaf, middle, root]);

        let constructor_order: Vec<_> = destructor_order.into_iter().rev().collect();
        assert_eq!(constructor_order, vec![root, middle, leaf]);
    }
}
