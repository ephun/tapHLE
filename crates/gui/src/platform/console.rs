/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Printing to the terminal that started the frontend, when there was one.
//!
//! On Windows a program is built for either the console subsystem or the
//! windowed one, and it is not a runtime choice. The frontend is windowed,
//! because a library window that opens a black console box beside itself
//! looks broken. The cost is that it has no standard output at all: run it
//! from a prompt and `--version` prints into nothing.
//!
//! Reattaching to the parent's console is the usual way out. When the
//! frontend was started from a terminal it borrows that terminal; when it was
//! double-clicked there is no console to borrow and nothing changes.
//!
//! The one wart is the shell's own: because the program is windowed, the
//! prompt returns immediately rather than waiting, so output arrives after the
//! next prompt has been drawn. That is a property of the subsystem rather
//! than something this can fix, and it is why `tapHLE` — the emulator, and the
//! binary every script and compatibility report drives — stays a console
//! program instead.

/// Borrow the terminal that started this program, if there was one.
///
/// Returns whether there was.
#[cfg(windows)]
pub fn attach_to_parent() -> bool {
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::System::Console::{
        AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
    };

    // Fails when there is no parent console, which is the double-clicked case
    // and not an error.
    if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } == 0 {
        return false;
    }

    // Attaching gives the process a console; it does not point the standard
    // streams at it, because they were bound to nothing when the program
    // started. Rust's `println!` writes to whatever handle it is given at
    // first use, so this has to happen before anything prints.
    for std_handle in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        let handle = unsafe { GetStdHandle(std_handle) };
        if handle.is_null() || handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            continue;
        }
        // Leaked deliberately: the console lives as long as the process, and
        // closing the File would close the handle the runtime is about to
        // write through.
        std::mem::forget(unsafe { std::fs::File::from_raw_handle(handle as _) });
    }
    true
}

/// Nothing to do anywhere else: every other platform gives a program its
/// standard streams whether or not anyone is reading them.
#[cfg(not(windows))]
pub fn attach_to_parent() -> bool {
    // Standard error is already going wherever it should.
    true
}
