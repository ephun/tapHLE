/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Where the frontend keeps its own files, and how they are read and written.
//!
//! Everything here is machine-local: which apps this computer knows about,
//! how long they have been played, what this user set. None of it is a
//! compatibility claim, and none of it is meant to be shared — that
//! distinction is why the local star rating in [crate::compat] never
//! overwrites a database rating.
//!
//! The files live in a `tapHLE_frontend` directory beside `tapHLE_sandbox`
//! and `tapHLE_apps`, so a portable install stays portable: copying the
//! tapHLE directory to another machine carries the library with it. They are
//! ordinary indented JSON, meant to be readable and hand-editable.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Directory holding every file the frontend owns.
pub const DIR: &str = "tapHLE_frontend";
/// Global settings, both emulator defaults and frontend preferences.
pub const SETTINGS_FILE: &str = "settings.json";
/// The app library: entries, per-app overrides, play statistics, ratings.
pub const LIBRARY_FILE: &str = "library.json";
/// Volatile interface state: window geometry, panel sizes, last selection.
pub const STATE_FILE: &str = "state.json";
/// Cached icon bitmaps, so a rescan does not reopen every archive.
pub const ICON_CACHE_DIR: &str = "icons";
/// The last compatibility ratings read from the database, so they are shown
/// straight away at startup and while offline.
pub const COMPAT_CACHE_FILE: &str = "compatibility.json";
/// The frontend's own log, kept separately from the emulator's
/// `tapHLE_log.txt` because both may be written at the same time.
pub const LOG_FILE: &str = "frontend_log.txt";

/// The directory tapHLE reads its own resources from, and the working
/// directory an emulator process is launched in.
///
/// This is [tapHLE::paths::user_data_base_path], which on desktop platforms is
/// the current directory. [locate_data_dir] is what makes that dependable.
pub fn data_dir() -> PathBuf {
    tapHLE::paths::user_data_base_path().to_path_buf()
}

pub fn frontend_dir() -> PathBuf {
    data_dir().join(DIR)
}

pub fn icon_cache_dir() -> PathBuf {
    frontend_dir().join(ICON_CACHE_DIR)
}

/// A directory that looks like a tapHLE installation.
///
/// `tapHLE_dylibs` is the marker: the emulator cannot run without it, so its
/// presence is what distinguishes an installation directory from whatever
/// directory a shortcut happened to start in.
fn looks_like_data_dir(path: &Path) -> bool {
    path.join(tapHLE::paths::DYLIBS_DIR).is_dir()
}

/// Move to the tapHLE installation directory, and report where that ended up.
///
/// The emulator resolves its resources relative to the current directory, and
/// a windowed program does not get a useful one: Explorer starts it in the
/// executable's directory, a Start menu shortcut in whatever the shortcut
/// says, and a debugger in the workspace root. Rather than teach every later
/// path lookup about that, the frontend picks the directory once at startup
/// and moves there, so it and the emulator processes it spawns agree.
///
/// The candidates, in order, are the current directory, the directory holding
/// this executable, and that directory's grandparent — which is where a
/// `target/debug` or `target/release` build sits relative to the checkout.
pub fn locate_data_dir() -> (PathBuf, Vec<String>) {
    let mut notes = Vec::new();
    let mut candidates: Vec<PathBuf> = Vec::new();

    match std::env::current_dir() {
        Ok(dir) => candidates.push(dir),
        Err(e) => notes.push(format!("Could not read the current directory: {e}")),
    }
    match std::env::current_exe() {
        Ok(exe) => {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.to_path_buf());
                if let Some(grandparent) = dir.parent().and_then(|p| p.parent()) {
                    candidates.push(grandparent.to_path_buf());
                }
            }
        }
        Err(e) => notes.push(format!("Could not locate this program: {e}")),
    }

    for candidate in &candidates {
        if !looks_like_data_dir(candidate) {
            continue;
        }
        if let Err(e) = std::env::set_current_dir(candidate) {
            notes.push(format!("Could not switch to {}: {e}", candidate.display()));
            continue;
        }
        return (candidate.clone(), notes);
    }

    let fallback = candidates
        .first()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."));
    notes.push(format!(
        "No tapHLE installation directory was found (looked for a {} directory). \
         Using {}. Launching apps will not work until tapHLE's files are found.",
        tapHLE::paths::DYLIBS_DIR,
        fallback.display()
    ));
    (fallback, notes)
}

/// A path as a person should see it.
///
/// Canonicalising a path on Windows produces the extended-length form,
/// `\\?\C:\...`. It is the right thing to store — it is what makes a long
/// path work — and the wrong thing to show, because nobody types it and
/// nothing else displays it.
pub fn display_path(path: &Path) -> String {
    let text = path.display().to_string();
    match text.strip_prefix(r"\\?\") {
        // The UNC form keeps its double backslash, which is how a network
        // path is written.
        Some(rest) => rest
            .strip_prefix("UNC\\")
            .map_or_else(|| rest.to_string(), |share| format!(r"\\{share}")),
        None => text,
    }
}

/// Create the frontend's directory if it is not there yet.
pub fn ensure_frontend_dir() -> Result<PathBuf, String> {
    let dir = frontend_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Read one of the frontend's JSON files.
///
/// A missing file is not an error: it is what a first run looks like, and the
/// caller gets the default value. A file that exists but cannot be parsed *is*
/// reported, because silently starting over would throw away a library.
pub fn load<T: DeserializeOwned + Default>(file: &str) -> Result<T, String> {
    let path = frontend_dir().join(file);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(e) => return Err(format!("Could not read {}: {e}", path.display())),
    };
    serde_json::from_str(&text).map_err(|e| format!("Could not parse {}: {e}", path.display()))
}

/// Write one of the frontend's JSON files.
///
/// The write goes to a temporary file that is then renamed over the target, so
/// an interrupted write cannot leave a half-written library behind.
pub fn save<T: Serialize>(file: &str, value: &T) -> Result<(), String> {
    ensure_frontend_dir()?;
    let path = frontend_dir().join(file);
    let temporary = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Could not serialize {file}: {e}"))?;
    std::fs::write(&temporary, text)
        .map_err(|e| format!("Could not write {}: {e}", temporary.display()))?;
    std::fs::rename(&temporary, &path)
        .map_err(|e| format!("Could not replace {}: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::looks_like_data_dir;

    /// The marker has to be the emulator's own resource directory. A
    /// directory that merely exists is not an installation, and treating one
    /// as such would leave the frontend launching apps that cannot start.
    #[test]
    fn a_directory_without_the_dylibs_is_not_an_installation() {
        let dir = std::env::temp_dir();
        assert!(!looks_like_data_dir(&dir.join("tapHLE-no-such-directory")));
    }

    /// Every library entry's path has been canonicalised, so without this
    /// the details panel would show `\\?\C:\...` for every app.
    #[test]
    fn the_extended_length_prefix_is_not_shown() {
        assert_eq!(
            super::display_path(std::path::Path::new(r"\\?\C:\apps\Game.ipa")),
            r"C:\apps\Game.ipa"
        );
        assert_eq!(
            super::display_path(std::path::Path::new(r"\\?\UNC\server\share\Game.ipa")),
            r"\\server\share\Game.ipa"
        );
        assert_eq!(
            super::display_path(std::path::Path::new("/home/user/Game.ipa")),
            "/home/user/Game.ipa"
        );
    }
}
