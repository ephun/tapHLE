/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use std::path::{Path, PathBuf};
use std::process::Command;

mod version_logic;

fn rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
}

fn git_path(workspace_root: &Path, name: &str) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args(["rev-parse", "--git-path", name])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = PathBuf::from(std::str::from_utf8(&output.stdout).ok()?.trim_end());
    Some(if path.is_absolute() {
        path
    } else {
        workspace_root.join(path)
    })
}

fn rerun_if_git_metadata_changes(workspace_root: &Path) {
    for name in ["HEAD", "index", "refs", "packed-refs", "shallow"] {
        if let Some(path) = git_path(workspace_root, name) {
            rerun_if_changed(&path);
        }
    }
}

fn rerun_if_tracked_file_changes(workspace_root: &Path) {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args(["ls-files", "-z"])
        .output();
    let Ok(output) = output else {
        return;
    };
    if !output.status.success() {
        return;
    }
    for relative_path in output.stdout.split(|byte| *byte == 0) {
        if relative_path.is_empty() {
            continue;
        }
        let Ok(relative_path) = std::str::from_utf8(relative_path) else {
            continue;
        };
        rerun_if_changed(&workspace_root.join(relative_path));
    }
}

pub fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // Read CARGO_MANIFEST_DIR at run time, not via env!(): env!() bakes this
    // worktree's absolute path into the build-script binary, which cargo may
    // reuse from a since-deleted worktree. Cargo always sets it when running a
    // build script.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let package_root = Path::new(&manifest_dir);
    let workspace_root = package_root.join("../..");

    // Try to get the version using tapHLE's own tag namespace, otherwise fall
    // back to the Cargo.toml version. This is used in main.rs.

    let toml_version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let version = Command::new("git")
        .args([
            "describe",
            "--tags",
            "--match",
            "taphle-v*",
            "--always",
            "--dirty",
        ])
        .output();
    let version = match version.as_ref() {
        Ok(version) if version.status.success() => {
            rerun_if_git_metadata_changes(&workspace_root);
            // `--dirty` is useful only if Cargo reruns this build script after
            // an unstaged edit. Track Git's known files explicitly; untracked
            // files do not affect `git describe --dirty` either.
            rerun_if_tracked_file_changes(&workspace_root);
            let git_version = std::str::from_utf8(&version.stdout)
                .unwrap()
                .trim_end()
                .to_string();
            if git_version.starts_with(version_logic::RELEASE_TAG_PREFIX)
                && !version_logic::description_matches_cargo_version(&git_version, &toml_version)
            {
                println!(
                    "cargo:warning=Cargo.toml version ({toml_version}) does not match `git describe` version ({git_version})!"
                );
            }
            version_logic::display_git_description(&git_version).to_string()
        }
        _ => {
            rerun_if_changed(&workspace_root.join("Cargo.toml"));
            format!(
                "{} (git rev. unknown)",
                version_logic::display_release_version(&toml_version)
            )
        }
    };
    std::fs::write(out_dir.join("version.txt"), version).unwrap();
}
