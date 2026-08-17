/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Compiler builtins from libgcc / compiler-rt, and the C++ ABI's allocation
//! operators.
//!
//! The ARM cores these apps target have no integer divide instruction, so the
//! compiler turns every `/` and `%` on an integer into a call to one of the
//! helpers below. That makes them unusually load-bearing: an app missing them
//! does not fail at some optional feature, it fails at arithmetic. A survey of
//! 1501 apps found `__udivsi3` alone blocking fourteen.
//!
//! Resources:
//! - The [Run-time ABI for the ARM Architecture](https://github.com/ARM-software/abi-aa/blob/main/rtabi32/rtabi32.rst)
//!   defines the `__aeabi_*` spellings and their register conventions.

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{GuestUSize, MutPtr, MutVoidPtr};
use crate::Environment;

/// Division by zero is undefined in C, and the real helpers do not check for it
/// either — on these cores it traps or returns garbage depending on the build.
/// Returning zero keeps the emulator alive and is no less defined than what the
/// hardware would have done, which matters because the caller is often a
/// third-party library doing arithmetic on data tapHLE handed it.
fn divide_or_zero<T: Default>(divisor_is_zero: bool, compute: impl FnOnce() -> T) -> T {
    if divisor_is_zero {
        log_once!("Warning: integer division by zero in guest code, returning 0");
        return T::default();
    }
    compute()
}

fn __udivsi3(_env: &mut Environment, numerator: u32, denominator: u32) -> u32 {
    divide_or_zero(denominator == 0, || numerator / denominator)
}

fn __umodsi3(_env: &mut Environment, numerator: u32, denominator: u32) -> u32 {
    divide_or_zero(denominator == 0, || numerator % denominator)
}

/// Signed division truncates towards zero in C, which is what Rust's `/` does,
/// so this is a direct mapping — except for `INT_MIN / -1`, whose true quotient
/// is not representable and which would panic in Rust where C leaves it
/// undefined. Wrapping matches what the hardware produces.
fn __divsi3(_env: &mut Environment, numerator: i32, denominator: i32) -> i32 {
    divide_or_zero(denominator == 0, || numerator.wrapping_div(denominator))
}

fn __modsi3(_env: &mut Environment, numerator: i32, denominator: i32) -> i32 {
    divide_or_zero(denominator == 0, || numerator.wrapping_rem(denominator))
}

fn __udivdi3(_env: &mut Environment, numerator: u64, denominator: u64) -> u64 {
    divide_or_zero(denominator == 0, || numerator / denominator)
}

fn __umoddi3(_env: &mut Environment, numerator: u64, denominator: u64) -> u64 {
    divide_or_zero(denominator == 0, || numerator % denominator)
}

fn __divdi3(_env: &mut Environment, numerator: i64, denominator: i64) -> i64 {
    divide_or_zero(denominator == 0, || numerator.wrapping_div(denominator))
}

fn __moddi3(_env: &mut Environment, numerator: i64, denominator: i64) -> i64 {
    divide_or_zero(denominator == 0, || numerator.wrapping_rem(denominator))
}

// Conversion between 64-bit integers and floating point.
//
// VFP converts between a float and a *32-bit* integer in one instruction, but
// it has no 64-bit form, so every widening or narrowing conversion involving a
// `long long` becomes a call to one of the eight helpers below. Managed
// runtimes reach them constantly, because a language whose integer type is
// 64-bit does this on ordinary arithmetic rather than at some unusual boundary.
//
// These take the *base* AAPCS convention — operands and results in the core
// registers, not VFP — even though surrounding iOS code is AAPCS-VFP. That
// is not an assumption: at a `___fixdfdi` call site an app moves its double out
// of a VFP register with `vmov r0, r1, d16` immediately before branching, and
// reads a `___floatdidf` result straight back out of `r0`/`r1`. tapHLE's
// `GuestArg`/`GuestRet` for `f32` and `f64` already marshal through the core
// registers, so declaring the float types here is correct as written.
//
// Only the legacy libgcc spellings are exported. The ARM run-time ABI's
// `__aeabi_l2d`/`__aeabi_d2lz` family is the same set of operations under
// different names, but the Apple toolchain does not emit those, so nothing
// would resolve against them.

fn __floatdidf(_env: &mut Environment, a: i64) -> f64 {
    a as f64
}

fn __floatdisf(_env: &mut Environment, a: i64) -> f32 {
    a as f32
}

fn __floatundidf(_env: &mut Environment, a: u64) -> f64 {
    a as f64
}

fn __floatundisf(_env: &mut Environment, a: u64) -> f32 {
    a as f32
}

/// Truncates towards zero, which is what C's float-to-integer conversion
/// requires and what Rust's `as` does.
///
/// Out-of-range inputs saturate. C leaves them undefined, and compiler-rt
/// saturates too, so this matches the real helper for every finite value. The
/// one divergence is NaN: compiler-rt's range check happens to fall through to
/// the saturation branch and yields `INT64_MAX`/`INT64_MIN`, where Rust yields
/// zero. Either way the caller has already lost — a NaN reaching an integer
/// conversion is a bug in the guest, not a behaviour tapHLE should reproduce
/// bit-for-bit — and returning a defined value beats trapping.
fn __fixdfdi(_env: &mut Environment, a: f64) -> i64 {
    a as i64
}

fn __fixsfdi(_env: &mut Environment, a: f32) -> i64 {
    a as i64
}

/// As [__fixdfdi], and a negative input saturates to zero, matching
/// compiler-rt.
fn __fixunsdfdi(_env: &mut Environment, a: f64) -> u64 {
    a as u64
}

fn __fixunssfdi(_env: &mut Environment, a: f32) -> u64 {
    a as u64
}

// The ARM run-time ABI spellings the compiler emits directly.

fn __aeabi_uidiv(env: &mut Environment, numerator: u32, denominator: u32) -> u32 {
    __udivsi3(env, numerator, denominator)
}

fn __aeabi_idiv(env: &mut Environment, numerator: i32, denominator: i32) -> i32 {
    __divsi3(env, numerator, denominator)
}

/// `__aeabi_uidivmod` returns the quotient in r0 and the remainder in r1. On
/// little-endian AAPCS a 64-bit return occupies exactly that pair, low half
/// first, so packing the remainder into the high half places each where the
/// caller expects it.
fn __aeabi_uidivmod(env: &mut Environment, numerator: u32, denominator: u32) -> u64 {
    let quotient = __udivsi3(env, numerator, denominator);
    let remainder = __umodsi3(env, numerator, denominator);
    ((remainder as u64) << 32) | quotient as u64
}

fn __aeabi_idivmod(env: &mut Environment, numerator: i32, denominator: i32) -> u64 {
    let quotient = __divsi3(env, numerator, denominator) as u32;
    let remainder = __modsi3(env, numerator, denominator) as u32;
    ((remainder as u64) << 32) | quotient as u64
}

// compiler-rt's own divide-and-remainder helpers. They compute what
// `__aeabi_uidivmod` and `__aeabi_idivmod` above compute, and differ only in
// how the second result gets back: these return the quotient and write the
// remainder through a pointer, where the ARM run-time ABI spellings return
// both in a register pair. Both exist in the wild because an app can be built
// against either runtime, and a static library inside it against the other, so
// implementing one spelling does not cover the other.

fn __udivmodsi4(
    env: &mut Environment,
    numerator: u32,
    denominator: u32,
    remainder_out: MutPtr<u32>,
) -> u32 {
    let remainder = __umodsi3(env, numerator, denominator);
    env.mem.write(remainder_out, remainder);
    __udivsi3(env, numerator, denominator)
}

fn __divmodsi4(
    env: &mut Environment,
    numerator: i32,
    denominator: i32,
    remainder_out: MutPtr<i32>,
) -> i32 {
    let remainder = __modsi3(env, numerator, denominator);
    env.mem.write(remainder_out, remainder);
    __divsi3(env, numerator, denominator)
}

// The C++ ABI's allocation operators. C++ code in these apps is usually a game
// engine or a third-party SDK, and it allocates before it does anything else.
//
// The throwing forms are documented to throw std::bad_alloc rather than return
// null, and the nothrow forms to return null. tapHLE's allocator does not fail,
// so the distinction never arises and both simply allocate.

fn _Znwm(env: &mut Environment, size: GuestUSize) -> MutVoidPtr {
    // C++ requires that `new` with a zero size still return a distinct,
    // non-null pointer.
    env.mem.alloc(size.max(1))
}

fn _Znam(env: &mut Environment, size: GuestUSize) -> MutVoidPtr {
    _Znwm(env, size)
}

fn _ZdlPv(env: &mut Environment, ptr: MutVoidPtr) {
    if !ptr.is_null() {
        env.mem.free(ptr);
    }
}

fn _ZdaPv(env: &mut Environment, ptr: MutVoidPtr) {
    _ZdlPv(env, ptr)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(__udivsi3(_, _)),
    export_c_func!(__umodsi3(_, _)),
    export_c_func!(__divsi3(_, _)),
    export_c_func!(__modsi3(_, _)),
    export_c_func!(__udivdi3(_, _)),
    export_c_func!(__umoddi3(_, _)),
    export_c_func!(__divdi3(_, _)),
    export_c_func!(__moddi3(_, _)),
    export_c_func!(__floatdidf(_)),
    export_c_func!(__floatdisf(_)),
    export_c_func!(__floatundidf(_)),
    export_c_func!(__floatundisf(_)),
    export_c_func!(__fixdfdi(_)),
    export_c_func!(__fixsfdi(_)),
    export_c_func!(__fixunsdfdi(_)),
    export_c_func!(__fixunssfdi(_)),
    export_c_func!(__aeabi_uidiv(_, _)),
    export_c_func!(__aeabi_idiv(_, _)),
    export_c_func!(__aeabi_uidivmod(_, _)),
    export_c_func!(__aeabi_idivmod(_, _)),
    export_c_func!(__udivmodsi4(_, _, _)),
    export_c_func!(__divmodsi4(_, _, _)),
    export_c_func!(_Znwm(_)),
    export_c_func!(_Znam(_)),
    export_c_func!(_ZdlPv(_)),
    export_c_func!(_ZdaPv(_)),
];

#[cfg(test)]
mod tests {
    use crate::abi::{GuestArg, GuestRet};

    /// The conversion helpers are declared with `f32`/`f64` in their signatures
    /// on the strength of one fact about the guest: it hands these particular
    /// functions their floating-point operands in the core registers rather
    /// than in VFP, low word first. Everything else in an iOS binary is
    /// AAPCS-VFP, so that is the surprising half, and it is the half a future
    /// change to float marshalling could quietly invert — leaving the helpers
    /// reading whatever happened to be in `r0`/`r1` and returning numbers the
    /// guest never looks at. Pin it here, next to the callers that depend on
    /// it.
    #[test]
    fn double_is_marshalled_through_the_core_register_pair() {
        assert_eq!(<f64 as GuestArg>::REG_COUNT, 2);

        let mut regs = [0u32; 2];
        <f64 as GuestRet>::to_regs(1.0f64, &mut regs);
        assert_eq!(regs, [0x0000_0000, 0x3ff0_0000]);
        assert_eq!(<f64 as GuestArg>::from_regs(&regs), 1.0f64);
    }

    /// A single-precision result occupies `r0` alone, which is how a
    /// `___floatdisf` caller reads it.
    #[test]
    fn float_is_marshalled_through_one_core_register() {
        assert_eq!(<f32 as GuestArg>::REG_COUNT, 1);

        let mut regs = [0u32; 1];
        <f32 as GuestRet>::to_regs(1.0f32, &mut regs);
        assert_eq!(regs, [0x3f80_0000]);
        assert_eq!(<f32 as GuestArg>::from_regs(&regs), 1.0f32);
    }
}
