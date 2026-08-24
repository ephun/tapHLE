/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Starting other programs without a console window appearing.
//!
//! A windowed program on Windows has no console, so every child process it
//! starts is given a fresh one unless told otherwise — which is exactly the
//! black window that is not supposed to appear when an app is launched from
//! the frontend. `CREATE_NO_WINDOW` suppresses it. The child's output still
//! arrives through the pipes; only the window is gone.
//!
//! Nothing is needed on other platforms, where a child inherits the parent's
//! standard streams and no window is created.

use std::process::Command;

/// Windows process creation flag: give the child no console at all.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn without_console(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Ask the desktop to open a file, folder or URL with whatever handles it.
///
/// Used for "Open app data location" and for opening a compatibility entry in
/// a browser. Each platform has its own one-line answer for this, so it is not
/// worth a dependency.
pub fn open_in_desktop(target: &str) -> Result<(), String> {
    let mut command;
    #[cfg(windows)]
    {
        // `start` is a shell builtin, and its first quoted argument is taken
        // as the window title, hence the empty one.
        command = Command::new("cmd");
        command.args(["/C", "start", "", target]);
    }
    #[cfg(target_os = "macos")]
    {
        command = Command::new("open");
        command.arg(target);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        command = Command::new("xdg-open");
        command.arg(target);
    }
    without_console(&mut command)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open {target}: {e}"))
}
