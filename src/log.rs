/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Logging and terminal output macros.

use std::fs::File;
use std::sync::LazyLock;

/// Get a handle to the log file. This is only for use by logging macros!
///
/// All the logging macros print to stderr or (on Android) logcat, but this
/// is not convenient for users who aren't accustomed to command-line tools or
/// who don't have access to ADB, so we also write to a log file.
pub fn get_log_file() -> &'static File {
    static LOG_FILE: LazyLock<File> = LazyLock::new(|| {
        File::create(crate::paths::user_data_base_path().join("tapHLE_log.txt")).unwrap()
    });

    &LOG_FILE
}

/// Prints a log message unconditionally. Use this for errors or warnings.
///
/// The message is prefixed with the module path, so it is clear where it comes
/// from.
macro_rules! log {
    ($($arg:tt)+) => {
        echo!("{}: {}", module_path!(), format_args!($($arg)+));
    }
}

/// Same as [log], but silently fails on panic instead of
/// panicking.
macro_rules! log_no_panic {
    ($($arg:tt)+) => {
        echo_no_panic!("{}: {}", module_path!(), format_args!($($arg)+));
    }
}

/// Like [log], but prints the message only if debugging is enabled for the
/// module where it is used. This can be used for verbose things only needed
/// when debugging.
macro_rules! log_dbg {
    ($($arg:tt)+) => {
        if $crate::log::debug_enabled_for(module_path!()) {
            log!($($arg)*);
        }
    }
}

/// Like [log], but messages only log once and cannot have formatting.
/// To be used for log messages that are known to spam the log file (like those
/// logged every frame).
macro_rules! log_once {
    ($msg:literal) => {{
        static LOG_ONCE: std::sync::Once = std::sync::Once::new();
        LOG_ONCE.call_once(|| {
            log!("{} [this log will only be shown once]", $msg);
        });
    }};
}

/// Print a message (with implicit newline). This should be used for all
/// tapHLE output that isn't coming from the app itself.
///
/// Prefer use [log] or [log_dbg] for errors and warnings during emulation.
macro_rules! echo {
    ($($arg:tt)+) => {
        {
            let formatted_str = format!($($arg)+);

            #[cfg(target_os = "android")]
            {
                sdl2::log::log(&formatted_str);
            }
            #[cfg(not(target_os = "android"))]
            eprintln!("{}", formatted_str);

            use std::io::Write;
            let mut log_file = $crate::log::get_log_file();
            let _ = log_file.write_all(formatted_str.as_bytes());
            let _ = log_file.write_all(b"\n");
        }
    };
    () => {
        {
            #[cfg(target_os = "android")]
            {
                sdl2::log::log("");
            }
            #[cfg(not(target_os = "android"))]
            eprintln!("");

            use std::io::Write;
            let _ = $crate::log::get_log_file().write_all(b"\n");
        }
    }
}

/// Same as [echo], but silently fails on panic instead of
/// panicking.
macro_rules! echo_no_panic {
    ($($arg:tt)*) => {
        {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                echo!($($arg)*);
            }));
        }
    }
}

/// Put modules to enable [log_dbg] for here, e.g. "tapHLE::mem" to see when
/// memory is allocated and freed.
///
/// This is the compile-time list. Prefer the `TAPHLE_LOG_MODULES` environment
/// variable (see [debug_enabled_for]) when investigating an app, so that a
/// trace question does not cost a rebuild.
pub const ENABLED_MODULES: &[&str] = &[];

/// Name of an environment variable holding a comma-separated list of module
/// paths to enable [log_dbg] output for, without recompiling.
///
/// Each entry is matched as a path prefix at a module boundary, so
/// `tapHLE::frameworks::uikit` enables every module beneath it, and the
/// special value `all` enables everything. Example:
///
/// ```text
/// TAPHLE_LOG_MODULES=tapHLE::frameworks::uikit::ui_touch,tapHLE::mem
/// ```
pub const LOG_MODULES_ENV_VAR: &str = "TAPHLE_LOG_MODULES";

/// Whether [log_dbg] output is enabled for `module`.
///
/// This is only for use by logging macros!
pub fn debug_enabled_for(module: &str) -> bool {
    static FROM_ENV: LazyLock<Vec<String>> = LazyLock::new(|| {
        std::env::var(LOG_MODULES_ENV_VAR)
            .unwrap_or_default()
            .split(',')
            .map(|entry| entry.trim().to_string())
            .filter(|entry| !entry.is_empty())
            .collect()
    });

    if ENABLED_MODULES.contains(&module) {
        return true;
    }
    FROM_ENV.iter().any(|entry| {
        entry == "all"
            || entry == module
            // A prefix only matches at a module boundary, so that "tapHLE::mem"
            // does not also enable a hypothetical "tapHLE::memcache".
            || (module.starts_with(entry.as_str())
                && module[entry.len()..].starts_with("::"))
    })
}

#[cfg(test)]
mod tests {

    /// [debug_enabled_for] reads the environment once, so exercise the
    /// matching rule directly rather than through the cached lookup.
    fn matches(entries: &[&str], module: &str) -> bool {
        entries.iter().any(|entry| {
            *entry == "all"
                || *entry == module
                || (module.starts_with(entry) && module[entry.len()..].starts_with("::"))
        })
    }

    #[test]
    fn exact_module_matches() {
        assert!(matches(&["tapHLE::mem"], "tapHLE::mem"));
    }

    #[test]
    fn parent_module_enables_children() {
        assert!(matches(
            &["tapHLE::frameworks::uikit"],
            "tapHLE::frameworks::uikit::ui_touch"
        ));
    }

    #[test]
    fn prefix_only_matches_at_a_module_boundary() {
        assert!(!matches(&["tapHLE::mem"], "tapHLE::memcache"));
    }

    #[test]
    fn all_enables_everything() {
        assert!(matches(&["all"], "tapHLE::anything::at::all"));
    }

    #[test]
    fn unrelated_module_does_not_match() {
        assert!(!matches(&["tapHLE::mem"], "tapHLE::cpu"));
    }
}
