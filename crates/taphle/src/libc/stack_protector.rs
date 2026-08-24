/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Compiler stack-protector runtime support.

use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::mem::ConstVoidPtr;
use crate::Environment;

/// The guest only requires a stable, non-zero process-wide canary. The value
/// itself is not security-sensitive here because guest code cannot escape the
/// emulator's address space.
fn stack_chk_guard(env: &mut Environment) -> ConstVoidPtr {
    env.mem
        .alloc_and_write(0x9e37_79b9_u32)
        .cast_void()
        .cast_const()
}

fn __stack_chk_fail(_env: &mut Environment) {
    panic!("Guest stack protector detected stack corruption");
}

pub const CONSTANTS: ConstantExports =
    &[("___stack_chk_guard", HostConstant::Custom(stack_chk_guard))];

pub const FUNCTIONS: FunctionExports = &[export_c_func!(__stack_chk_fail())];
