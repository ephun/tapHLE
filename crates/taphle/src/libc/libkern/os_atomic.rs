/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `libkern/OSAtomic.h`
//!
//! Atomic operations.
//!
//! Right now tapHLE is a single host thread application.
//! Thus, the execution of host functions couldn't be interrupted
//! by other threads. So we consider host functions to be atomic!

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::mem::{MutPtr, MutVoidPtr};
use crate::Environment;

fn OSAtomicAdd32(env: &mut Environment, amount: i32, value_ptr: MutPtr<i32>) -> i32 {
    OSAtomicAdd32Barrier(env, amount, value_ptr)
}

fn OSAtomicAdd32Barrier(env: &mut Environment, the_amount: i32, the_value: MutPtr<i32>) -> i32 {
    let curr = env.mem.read(the_value);
    let new = curr + the_amount;
    env.mem.write(the_value, new);
    new
}

fn OSAtomicCompareAndSwap32(
    env: &mut Environment,
    old_value: i32,
    new_value: i32,
    the_value: MutPtr<i32>,
) -> bool {
    OSAtomicCompareAndSwap32Barrier(env, old_value, new_value, the_value)
}

fn OSAtomicCompareAndSwapIntBarrier(
    env: &mut Environment,
    old_value: i32,
    new_value: i32,
    the_value: MutPtr<i32>,
) -> bool {
    OSAtomicCompareAndSwap32Barrier(env, old_value, new_value, the_value)
}

fn OSAtomicCompareAndSwap32Barrier(
    env: &mut Environment,
    old_value: i32,
    new_value: i32,
    the_value: MutPtr<i32>,
) -> bool {
    if old_value == env.mem.read(the_value) {
        env.mem.write(the_value, new_value);
        true
    } else {
        false
    }
}

fn OSAtomicCompareAndSwapPtr(
    env: &mut Environment,
    old_value: MutVoidPtr,
    new_value: MutVoidPtr,
    the_value: MutPtr<MutVoidPtr>,
) -> bool {
    OSAtomicCompareAndSwapPtrBarrier(env, old_value, new_value, the_value)
}

fn OSAtomicCompareAndSwapPtrBarrier(
    env: &mut Environment,
    old_value: MutVoidPtr,
    new_value: MutVoidPtr,
    the_value: MutPtr<MutVoidPtr>,
) -> bool {
    if old_value == env.mem.read(the_value) {
        env.mem.write(the_value, new_value);
        true
    } else {
        false
    }
}

fn OSMemoryBarrier(_env: &mut Environment) {
    // no-op
}

/// `OSSpinLock` is a bare `int32_t`, zero meaning unlocked.
#[allow(non_camel_case_types)]
type OSSpinLock = i32;

/// Spin locks guard very short critical sections, and the whole design assumes
/// the holder is running on another core and will release almost immediately.
/// tapHLE's guest threads are cooperatively scheduled, so that assumption does
/// not hold: a thread that spun here would never yield, and the holder could
/// never run to release it — busy-waiting would be a guaranteed hang where the
/// real thing merely burns a few cycles.
///
/// So the lock is recorded but never waited on. In practice these sections are
/// short enough, and the scheduler coarse enough, that contention does not
/// arise; the log below exists so that if it ever does, it is visible rather
/// than silently mis-serialised.
fn OSSpinLockLock(env: &mut Environment, lock: MutPtr<OSSpinLock>) {
    if env.mem.read(lock) != 0 {
        log_once!(
            "Warning: OSSpinLockLock() on a lock that is already held. tapHLE does not \
             block here, so this critical section is not actually serialised."
        );
    }
    env.mem.write(lock, 1);
}

fn OSSpinLockUnlock(env: &mut Environment, lock: MutPtr<OSSpinLock>) {
    env.mem.write(lock, 0);
}

fn OSSpinLockTry(env: &mut Environment, lock: MutPtr<OSSpinLock>) -> bool {
    if env.mem.read(lock) != 0 {
        return false;
    }
    env.mem.write(lock, 1);
    true
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(OSSpinLockLock(_)),
    export_c_func!(OSSpinLockUnlock(_)),
    export_c_func!(OSSpinLockTry(_)),
    export_c_func!(OSAtomicAdd32(_, _)),
    export_c_func!(OSAtomicAdd32Barrier(_, _)),
    export_c_func!(OSAtomicCompareAndSwap32(_, _, _)),
    export_c_func!(OSAtomicCompareAndSwapIntBarrier(_, _, _)),
    export_c_func!(OSAtomicCompareAndSwap32Barrier(_, _, _)),
    export_c_func!(OSAtomicCompareAndSwapPtr(_, _, _)),
    export_c_func!(OSAtomicCompareAndSwapPtrBarrier(_, _, _)),
    export_c_func!(OSMemoryBarrier()),
];
