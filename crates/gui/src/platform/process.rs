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

use std::path::Path;
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

/// Ask the platform to show a filesystem location.
pub fn open_path(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "ios")]
    {
        use std::ffi::CString;

        extern "C" {
            fn tapHLE_ios_export_folder(path: *const std::ffi::c_char) -> i32;
        }
        let display = path.display();
        let path = CString::new(path.as_os_str().as_encoded_bytes())
            .map_err(|_| format!("Could not open {display}: path contains a null byte"))?;
        return match unsafe { tapHLE_ios_export_folder(path.as_ptr()) } {
            1 => Ok(()),
            _ => Err(format!("Could not open {display}.")),
        };
    }

    #[cfg(not(target_os = "ios"))]
    {
        open_with_desktop(path.as_os_str(), &path.display().to_string())
    }
}

/// Ask the platform to open an external URL with its registered application.
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "ios")]
    {
        use std::ffi::CString;

        extern "C" {
            fn tapHLE_ios_open_url(url: *const std::ffi::c_char) -> i32;
        }
        let encoded = CString::new(url)
            .map_err(|_| format!("Could not open {url}: URL contains a null byte"))?;
        return match unsafe { tapHLE_ios_open_url(encoded.as_ptr()) } {
            1 => Ok(()),
            _ => Err(format!("Could not open {url}.")),
        };
    }

    #[cfg(not(target_os = "ios"))]
    {
        open_with_desktop(url.as_ref(), url)
    }
}

#[cfg(not(target_os = "ios"))]
fn open_with_desktop(target: &std::ffi::OsStr, display: &str) -> Result<(), String> {
    let mut command;
    #[cfg(windows)]
    {
        // `start` is a shell builtin, and its first quoted argument is taken
        // as the window title, hence the empty one.
        command = Command::new("cmd");
        command.args(["/C", "start", ""]);
        command.arg(target);
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
        .map_err(|e| format!("Could not open {display}: {e}"))
}
