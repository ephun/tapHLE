/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `unwind.h` — the setjmp/longjmp flavour of the Itanium unwinder.
//!
//! ARM iPhone OS binaries are built with SjLj exception handling rather than
//! table-driven unwinding, so the compiler emits a call to
//! `_Unwind_SjLj_Register` on entry to *any* function containing a `try` block
//! or a cleanup, and `_Unwind_SjLj_Unregister` on the way out. The registration
//! maintains a per-thread linked list of function contexts that a later throw
//! would walk.
//!
//! That registration happens whether or not anything is ever thrown, which is
//! why this matters so much in practice: a survey of 1501 apps found
//! `_Unwind_SjLj_Register` to be the second most common reason an app died
//! before reaching its own code, in 87 of them. Almost none of those were
//! throwing — they were merely *able* to.
//!
//! So the bookkeeping is implemented and throwing is not. Maintaining the list
//! is exact and cheap; unwinding through it needs the personality routine and
//! the LSDA of every frame, which is a much larger piece of work. An app that
//! actually throws is told so plainly rather than being allowed to continue on
//! a corrupted context chain.
//!
//! **This is the fallback, not the usual path.** An app that uses exceptions
//! links `/usr/lib/libgcc_s.1.dylib`, which tapHLE bundles and which contains
//! the real unwinder — raising included. Where that dylib is loaded, its
//! definitions bind in preference to these; see `guest_definition_wins` in
//! [crate::dyld] for why the whole family has to come from one place. What is
//! left here is for the apps that import the registration calls from libSystem
//! without bringing libgcc, which is the majority of the 87 above.
//!
//! Resources:
//! - libgcc's `unwind-sjlj.c`, which defines the context layout and the
//!   register/unregister contract.

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{MutPtr, MutVoidPtr, SafeRead};
use crate::{Environment, ThreadId};
use std::collections::HashMap;

/// The head of each thread's function-context chain.
///
/// Per-thread rather than global because the chain describes one call stack.
/// Sharing it between threads would splice unrelated stacks together, and the
/// corruption would only surface later, at a throw, far from the cause.
#[derive(Default)]
pub struct State {
    contexts: HashMap<ThreadId, MutVoidPtr>,
}

/// `struct SjLj_Function_Context`. Only the first field is needed here: the
/// rest (call site, saved registers, personality routine, LSDA) is read by the
/// unwinder during a throw, which is not implemented.
#[repr(C, packed)]
struct SjLj_Function_Context {
    prev: MutVoidPtr,
}
unsafe impl SafeRead for SjLj_Function_Context {}

fn _Unwind_SjLj_Register(env: &mut Environment, fc: MutPtr<SjLj_Function_Context>) {
    let thread = env.current_thread;
    let previous = env
        .libc_state
        .unwind
        .contexts
        .get(&thread)
        .copied()
        .unwrap_or(MutVoidPtr::null());
    env.mem.write(fc, SjLj_Function_Context { prev: previous });
    env.libc_state.unwind.contexts.insert(thread, fc.cast());
}

fn _Unwind_SjLj_Unregister(env: &mut Environment, fc: MutPtr<SjLj_Function_Context>) {
    let thread = env.current_thread;
    // Restore the chain from the context being retired rather than assuming it
    // is the head. They should be the same, but a longjmp out of a guarded
    // region can retire several frames at once, and following the stored `prev`
    // is what the real unwinder does.
    let previous = env.mem.read(fc).prev;
    env.libc_state.unwind.contexts.insert(thread, previous);
}

/// Throwing. The context chain is maintained, but walking it requires each
/// frame's personality routine and language-specific data, which tapHLE does
/// not interpret.
///
/// This is deliberately fatal and says so. Returning instead would hand control
/// back to code that has just been told its exception was handled, with the
/// catch block never entered — the app would carry on with its invariants
/// broken, and fail later somewhere unrelated.
fn _Unwind_SjLj_RaiseException(_env: &mut Environment, exception: MutVoidPtr) -> i32 {
    unimplemented!(
        "The app threw an exception ({:?}), but SjLj unwinding is not implemented \
         here, and this app did not bring the libgcc that implements it. \
         Registration and unregistration of function contexts are, so this app got \
         further than it used to; catching is the remaining work.",
        exception
    );
}

fn _Unwind_SjLj_Resume(_env: &mut Environment, exception: MutVoidPtr) {
    unimplemented!(
        "The app resumed unwinding an exception ({:?}), which SjLj unwinding does \
         not implement.",
        exception
    );
}

fn _Unwind_SjLj_ForcedUnwind(
    _env: &mut Environment,
    exception: MutVoidPtr,
    _stop: MutVoidPtr,
    _stop_argument: MutVoidPtr,
) -> i32 {
    unimplemented!(
        "The app forced an unwind ({:?}), which SjLj unwinding does not implement.",
        exception
    );
}

/// Releasing an exception object that was never raised is harmless, and is
/// reached on ordinary paths where a prepared exception is discarded.
fn _Unwind_DeleteException(_env: &mut Environment, _exception: MutVoidPtr) {}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(_Unwind_SjLj_Register(_)),
    export_c_func!(_Unwind_SjLj_Unregister(_)),
    export_c_func!(_Unwind_SjLj_RaiseException(_)),
    export_c_func!(_Unwind_SjLj_Resume(_)),
    export_c_func!(_Unwind_SjLj_ForcedUnwind(_, _, _)),
    export_c_func!(_Unwind_DeleteException(_)),
];
