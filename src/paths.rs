/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Paths for host files used by tapHLE: settings, fonts, etc.
//!
//! There are three categories of files:
//!
//! * Resources bundled with tapHLE that neither tapHLE nor the user should
//!   modify: [DYLIBS_DIR], [FONTS_DIR], [DEFAULT_OPTIONS_FILE]. Depending on
//!   the platform these may or may not be ordinary files, and must be accessed
//!   through [ResourceFile].
//! * Files the user is expected to modify, but not tapHLE: [APPS_DIR],
//!   [USER_OPTIONS_FILE]. These are ordinary files and are found in
//!   [user_data_base_path].
//! * Files that tapHLE will create and modify, and the user may modify if
//!   they want to: [SANDBOX_DIR]. These are ordinary files and are found in
//!   [user_data_base_path].
//!
//! See also [crate::fs], which provides a virtual filesystem for the guest app
//! and defines path types.

use std::borrow::Cow;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

/// Name of the directory containing ARMv6 dynamic libraries bundled with
/// tapHLE.
pub const DYLIBS_DIR: &str = "tapHLE_dylibs";

/// Name of the directory containing fonts bundled with tapHLE.
pub const FONTS_DIR: &str = "tapHLE_fonts";

/// Name of the file containing tapHLE's default options for various apps.
pub const DEFAULT_OPTIONS_FILE: &str = "tapHLE_default_options.txt";

/// macOS-only: If tapHLE is located in a .app bundle, return the path of the
/// Resources directory. If tapHLE is not located in a .app bundle, return
/// [None].
#[allow(dead_code)]
fn get_macos_bundled_resources_path() -> Option<PathBuf> {
    if std::env::consts::OS != "macos" {
        return None;
    }
    let base_path = PathBuf::from(sdl2::filesystem::base_path().ok()?);
    if base_path.file_name().is_some_and(|p| p == "Resources") {
        Some(base_path)
    } else {
        None
    }
}

/// Abstraction over a platform-specific type for accessing a resource bundled
/// with tapHLE.
pub struct ResourceFile {
    #[cfg(target_os = "android")]
    file: sdl2::rwops::RWops<'static>,
    #[cfg(not(target_os = "android"))]
    file: std::fs::File,
}
impl ResourceFile {
    pub fn open(path: &str) -> Result<Self, String> {
        Ok(Self {
            // On Android, these resources are included as "assets" within the
            // APK. We access them via SDL2's wrapper of Android's assets API.
            #[cfg(target_os = "android")]
            file: sdl2::rwops::RWops::from_file(path, "r")?,

            // On other OSes, resources are accessed as ordinary files.
            #[cfg(not(target_os = "android"))]
            file: {
                let base_path = get_macos_bundled_resources_path();
                // When not in a bundle, look in the current directory.
                let path = base_path.as_deref().unwrap_or(Path::new(".")).join(path);
                std::fs::File::open(path).map_err(|e| e.to_string())?
            },
        })
    }
    pub fn get(&mut self) -> &mut (impl Read + Seek) {
        &mut self.file
    }

    /// When the resource was last modified, in seconds since the Unix epoch,
    /// where the platform can say. On Android the resource is an asset inside
    /// the APK rather than a file on disk, and there is nothing to ask.
    pub fn modified(path: &str) -> Option<i64> {
        #[cfg(target_os = "android")]
        {
            let _ = path;
            None
        }
        #[cfg(not(target_os = "android"))]
        {
            let base_path = get_macos_bundled_resources_path();
            let path = base_path.as_deref().unwrap_or(Path::new(".")).join(path);
            let modified = std::fs::metadata(path).ok()?.modified().ok()?;
            modified
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs()
                .try_into()
                .ok()
        }
    }
}
impl std::fmt::Debug for ResourceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "ResourceFile")
    }
}

/// Whether various resources are in user-accessible files. If they aren't,
/// tapHLE has to be able to display their license terms.
pub const RESOURCES_ARE_EXTERNAL_FILES: bool = cfg!(not(target_os = "android"));

/// Name of the directory where the user can put apps if they want them to
/// appear in the tapHLE-gui library.
pub const APPS_DIR: &str = "tapHLE_apps";

/// Name of the file intended for the user's own options.
pub const USER_OPTIONS_FILE: &str = "tapHLE_options.txt";

/// Name of the directory where tapHLE will store sandboxed app data, e.g.
/// the `Documents` directory.
pub const SANDBOX_DIR: &str = "tapHLE_sandbox";

/// Get a platform-specific base path needed for accessing tapHLE's
/// user-modifiable files. This is empty on platforms other than Android.
pub fn user_data_base_path() -> Cow<'static, Path> {
    #[cfg(target_os = "android")]
    unsafe {
        // This is an exception to the rule that SDL2 should only be used
        // directly from src/window.rs. This is just too distant from windowing
        // to belong there.

        // Android storage has evolved in a quite messy fashion. Both "internal
        // storage" and "external storage" (aka the "SD card") are likely to be
        // internal on a modern device, as absurd as that might sound. SDL2 has
        // APIs to get paths for both. We use the "external storage" because
        // it's more likely to be user-accessible.
        extern "C" {
            fn SDL_AndroidGetExternalStoragePath() -> *const std::ffi::c_char;
        }
        let path = SDL_AndroidGetExternalStoragePath();
        if path.is_null() {
            log!("Couldn't get Android external storage path!");
            panic!();
        }
        Cow::from(Path::new(std::ffi::CStr::from_ptr(path).to_str().unwrap()))
    }
    #[cfg(not(target_os = "android"))]
    {
        // When tapHLE is run from a .app bundle on macOS, the user might not
        // be able to control the current directory, so user data needs to go in
        // a standard location.
        if get_macos_bundled_resources_path().is_some() {
            return Cow::from(PathBuf::from(
                sdl2::filesystem::pref_path("org.taphle", "tapHLE").unwrap(),
            ));
        }
        Cow::from(Path::new("."))
    }
}

/// Get a URI that can be used to open a file manager or similar for the path
/// that [user_data_base_path] represents.
pub fn url_for_opening_user_data_dir() -> Result<String, String> {
    if std::env::consts::OS == "android" {
        // See DocumentsProvider.kt, app/build.gradle and AndroidManifest.xml
        let brand = crate::branding();
        Ok(format!(
            "content://org.taphle.android{}{}.provider/root/root",
            if brand.is_empty() { "" } else { "." },
            brand.to_lowercase()
        ))
    } else {
        let path = user_data_base_path()
            .join(".")
            .canonicalize()
            .map_err(|e| format!("Can't canonicalize path to user data directory: {e}"))?;
        let path = path
            .to_str()
            .ok_or_else(|| "User data directory path is not UTF-8".to_string())?;
        // std::fs::canonicalize() on Windows uses the extended-length path
        // syntax, but Windows Explorer doesn't understand it.
        let path = if std::env::consts::OS == "windows" {
            path.strip_prefix("\\\\?\\").unwrap_or(path)
        } else {
            path
        };
        Ok(format!("file://{path}"))
    }
}

/// Only meaningful on certain OSes: create the user data directory if it
/// doesn't exist, and populate it with templates or README files. (On other
/// platforms these are simply bundled with tapHLE in a ZIP file.)
pub fn prepopulate_user_data_dir() {
    if !matches!(std::env::consts::OS, "android" | "macos") {
        return;
    }
    let base_path = user_data_base_path();
    if base_path == Path::new(".") {
        return;
    }

    let apps_dir = base_path.join(APPS_DIR);
    if !apps_dir.is_dir() {
        match std::fs::create_dir(&apps_dir) {
            Ok(()) => {
                log!("Created: {}", apps_dir.display());
            }
            Err(e) => {
                log!("Warning: Couldn't create {}: {}", apps_dir.display(), e);
            }
        }
    }

    fn create_file(path: &Path, content: &str) {
        match std::fs::write(path, content) {
            Ok(()) => {
                log!("Created: {}", path.display());
            }
            Err(e) => {
                log!("Warning: Couldn't create {}: {}", path.display(), e);
            }
        }
    }

    let apps_dir_readme = apps_dir.join("README.txt");
    if !apps_dir_readme.is_file() {
        let content = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tapHLE_apps/README.txt"
        ));
        create_file(&apps_dir_readme, content);
    }

    let user_options = base_path.join(USER_OPTIONS_FILE);
    if !user_options.is_file() {
        let content = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tapHLE_options.txt"));
        create_file(&user_options, content);
    }

    let options_help = base_path.join("OPTIONS_HELP.txt");
    if !options_help.is_file() {
        create_file(&options_help, crate::options::OPTIONS_HELP);
    }
}
