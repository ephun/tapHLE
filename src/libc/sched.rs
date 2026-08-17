/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `sched.h`.

use crate::dyld::{export_c_func, FunctionExports};
use crate::Environment;

fn sched_yield(env: &mut Environment) -> i32 {
    log_dbg!(
        "TODO: thread {} requested processor yield, ignoring",
        env.current_thread
    );
    0 // success
}

/// The priority range Darwin reports, for every policy it supports.
///
/// tapHLE runs guest threads on host threads it does not prioritise, so these
/// are answers rather than promises. They matter anyway: a thread pool asks for
/// the range and then computes its own priorities inside it, and a wrong range
/// produces values the app itself later rejects. Returning Darwin's real
/// numbers keeps that arithmetic in the range the app was written against.
const SCHED_PRIORITY_MIN: i32 = 15;
const SCHED_PRIORITY_MAX: i32 = 47;

fn sched_get_priority_min(_env: &mut Environment, _policy: i32) -> i32 {
    SCHED_PRIORITY_MIN
}

fn sched_get_priority_max(_env: &mut Environment, _policy: i32) -> i32 {
    SCHED_PRIORITY_MAX
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(sched_yield()),
    export_c_func!(sched_get_priority_min(_)),
    export_c_func!(sched_get_priority_max(_)),
];
