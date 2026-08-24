/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use std::path::Path;

fn rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
}

fn main() {
    // Read CARGO_MANIFEST_DIR at run time. env!() bakes this worktree's
    // absolute path into the compiled build-script binary, which cargo can
    // reuse from a since-deleted sibling worktree, leaving a dead path passed
    // to the C/C++ compiler. Cargo always sets this variable when running a
    // build script, so a runtime read is correct for whichever tree is
    // building.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let package_root = Path::new(&manifest_dir);
    let workspace_root = workspace_root(package_root);

    cc::Build::new()
        .include(&workspace_root)
        .file(package_root.join("lib.cpp"))
        .cpp(true)
        .compile("pvrt_decompress_wrapper");
    rerun_if_changed(&package_root.join("lib.cpp"));
    rerun_if_changed(&workspace_root.join("vendor/PVRTDecompress/PVRTDecompress.cpp"));
    rerun_if_changed(&workspace_root.join("vendor/PVRTDecompress/PVRTDecompress.h"));
}

/// The workspace root, found rather than counted.
///
/// This used to be `package_root.join("../../..")`, which was correct until
/// the crate moved and then silently pointed somewhere with no `vendor` in
/// it. Walking up until the vendored sources appear cannot drift.
fn workspace_root(package_root: &std::path::Path) -> std::path::PathBuf {
    package_root
        .ancestors()
        .find(|dir| dir.join("vendor").is_dir())
        .unwrap_or_else(|| panic!("no vendor directory above {}", package_root.display()))
        .to_path_buf()
}
