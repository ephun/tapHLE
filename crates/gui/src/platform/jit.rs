/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
//! Coordinate device JIT while the frontend still owns the event loop.

use std::ffi::{c_char, CStr};

extern "C" {
    fn tapHLE_ios_jit_begin() -> i32;
    fn tapHLE_ios_jit_poll() -> i32;
    fn tapHLE_ios_jit_cancel();
    fn tapHLE_ios_jit_error() -> *const c_char;
}

fn result(status: i32) -> Result<bool, String> {
    match status {
        1 => Ok(true),
        0 => Ok(false),
        _ => Err(unsafe { CStr::from_ptr(tapHLE_ios_jit_error()) }
            .to_string_lossy()
            .into_owned()),
    }
}

pub fn begin() -> Result<bool, String> {
    result(unsafe { tapHLE_ios_jit_begin() })
}

pub fn poll() -> Result<bool, String> {
    result(unsafe { tapHLE_ios_jit_poll() })
}

pub fn cancel() {
    unsafe { tapHLE_ios_jit_cancel() }
}
