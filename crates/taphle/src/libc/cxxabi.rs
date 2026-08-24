/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `cxxabi.h`
//!
//! Resources:
//! - [Itanium C++ ABI specification](https://itanium-cxx-abi.github.io/cxx-abi/abi.html#dso-dtor-runtime-api)

use crate::abi::GuestFunction;
use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{MutPtr, MutVoidPtr};
use crate::Environment;

fn __cxa_atexit(
    _env: &mut Environment,
    func: GuestFunction, // void (*func)(void *)
    p: MutVoidPtr,
    d: MutVoidPtr,
) -> i32 {
    // TODO: when this is implemented, make sure it's properly compatible with
    // C atexit.
    log!(
        "TODO: __cxa_atexit({:?}, {:?}, {:?}) (unimplemented)",
        func,
        p,
        d
    );
    0 // success
}

fn __cxa_finalize(_env: &mut Environment, d: MutVoidPtr) {
    log!("TODO: __cxa_finalize({:?}) (unimplemented)", d);
}

/// The guards around a function-local `static` with a non-trivial constructor.
/// The first thread through runs the initialiser; the rest wait.
///
/// tapHLE's guest threads are cooperatively scheduled and an initialiser cannot
/// yield partway, so the race the guard exists to prevent cannot occur here.
/// That reduces it to its other job, which is real and required: running the
/// initialiser exactly once. The guard's first byte records that.
fn __cxa_guard_acquire(env: &mut Environment, guard: MutPtr<u8>) -> i32 {
    if env.mem.read(guard) != 0 {
        return 0; // already initialised, do not run it again
    }
    1 // caller should run the initialiser, then call release
}

fn __cxa_guard_release(env: &mut Environment, guard: MutPtr<u8>) {
    env.mem.write(guard, 1);
}

fn __cxa_guard_abort(_env: &mut Environment, _guard: MutPtr<u8>) {
    // The initialiser threw. Leaving the guard clear is correct: the next
    // attempt should try again.
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(__cxa_atexit(_, _, _)),
    export_c_func!(__cxa_guard_acquire(_)),
    export_c_func!(__cxa_guard_release(_)),
    export_c_func!(__cxa_guard_abort(_)),
    export_c_func!(__cxa_finalize(_)),
];
