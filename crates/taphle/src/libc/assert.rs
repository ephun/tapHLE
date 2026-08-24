/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `assert.h`
//!
//! Only the failure handler lives here. `assert()` itself is a macro that the
//! guest's compiler already expanded, so the one thing a binary can import is
//! the function the expansion calls when the condition is false.

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstPtr, Mem};
use crate::Environment;

/// The message Darwin's `assert()` prints before aborting.
///
/// Kept separate from the guest-memory reads so the formatting is testable, and
/// worded exactly as Darwin words it — an app's own log or crash report may be
/// compared against it.
fn assertion_message(func: Option<&str>, file: &str, line: i32, expr: &str) -> String {
    // Darwin omits the function clause entirely when the compiler could not
    // supply `__func__`, in which case it passes a null pointer.
    match func {
        Some(func) => {
            format!("Assertion failed: ({expr}), function {func}, file {file}, line {line}.")
        }
        None => format!("Assertion failed: ({expr}), file {file}, line {line}."),
    }
}

/// A string the assert handler was handed, which may be null or unreadable.
///
/// A failing assert is already a bad moment for the guest, so nothing here may
/// itself abort: a pointer into freed or wild memory is exactly what an app in
/// this state might pass, and losing the whole message because one of four
/// strings was unreadable would hide the assertion that actually fired.
fn describe(mem: &Mem, ptr: ConstPtr<u8>) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    match mem.cstr_at_utf8(ptr) {
        Ok(str) => Some(str.to_string()),
        Err(_) => Some(format!("<unreadable string at {:?}>", ptr)),
    }
}

/// `__assert_rtn`, the handler Darwin's `assert()` macro calls on failure.
///
/// This is the app asserting on itself, not tapHLE failing, and the distinction
/// matters when reading a log: the four arguments say exactly which line of the
/// game's own source gave up. Before this existed the app died on
/// "Call to unimplemented function ___assert_rtn" instead, which named tapHLE
/// and threw the assertion away.
///
/// It does not return, matching the real function's `__dead2`. The signature
/// still says `()` because that is what the guest ABI layer knows how to
/// describe; the panic is what enforces it.
fn __assert_rtn(
    env: &mut Environment,
    func: ConstPtr<u8>,
    file: ConstPtr<u8>,
    line: i32,
    expr: ConstPtr<u8>,
) {
    let message = assertion_message(
        describe(&env.mem, func).as_deref(),
        describe(&env.mem, file).as_deref().unwrap_or("<null>"),
        line,
        describe(&env.mem, expr).as_deref().unwrap_or("<null>"),
    );
    // Logged as well as panicked with, so it lands in the log next to whatever
    // the app printed on its way here.
    log!("{}", message);
    panic!(
        "{} This is the app's own assertion failing, not a tapHLE failure.",
        message
    );
}

pub const FUNCTIONS: FunctionExports = &[export_c_func!(__assert_rtn(_, _, _, _))];

#[cfg(test)]
mod tests {
    use super::assertion_message;

    #[test]
    fn assertion_message_matches_darwins_wording() {
        assert_eq!(
            assertion_message(Some("-[Foo bar]"), "Foo.m", 42, "x != NULL"),
            "Assertion failed: (x != NULL), function -[Foo bar], file Foo.m, line 42."
        );
    }

    #[test]
    fn assertion_message_drops_the_function_clause_when_there_is_none() {
        assert_eq!(
            assertion_message(None, "Foo.c", 7, "0"),
            "Assertion failed: (0), file Foo.c, line 7."
        );
    }
}
