/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use std::env;
use std::path::Path;

/*fn rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
}*/
fn link_search(path: &Path) {
    println!("cargo:rustc-link-search=native={}", path.to_str().unwrap());
}
fn link_lib(lib: &str) {
    if cfg!(feature = "static") {
        println!("cargo:rustc-link-lib=static={lib}");
    } else {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
}

fn link_framework(framework: &str) {
    println!("cargo:rustc-link-lib=framework={framework}");
}

fn main() {
    let os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS was not set");
    if cfg!(feature = "static") {
        // Read CARGO_MANIFEST_DIR at run time, not via env!(): env!() bakes
        // this worktree's absolute path into the build-script binary, which
        // cargo may reuse from a since-deleted worktree. Cargo always sets it
        // when running a build script.
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let package_root = Path::new(&manifest_dir);
        let workspace_root = workspace_root(package_root);

        let mut build = cmake::Config::new(workspace_root.join("vendor/openal-soft"));

        // Prevent any out dir re-use for Android. For some reason, this causes
        // weird linker errors when cross-compiling on macOS.
        let out_dir = Path::new(&env::var("OUT_DIR").unwrap()).join(&os);
        if os.eq_ignore_ascii_case("android") {
            let _ = std::fs::remove_dir_all(&out_dir);
        }
        build.out_dir(out_dir);

        // The vendored openal-soft declares an old `cmake_minimum_required`.
        // CMake 4.x removed compatibility with policies older than 3.5, so tell
        // it to configure anyway. This mirrors the dynarmic wrapper.
        build.define("CMAKE_POLICY_VERSION_MINIMUM", "3.5");

        build.define("LIBTYPE", "STATIC");

        // Don't build extras, we don't need them and they can encounter issues
        // when cross-compiling.
        build.define("ALSOFT_UTILS", "OFF");
        build.define("ALSOFT_NO_CONFIG_UTIL", "ON");
        build.define("ALSOFT_EXAMPLES", "OFF");

        let openal_soft_out = build.build();

        link_search(&openal_soft_out.join("lib"));
        // some Linux systems
        link_search(&openal_soft_out.join("lib64"));

        // Some dependencies of OpenAL Soft.
        if os.eq_ignore_ascii_case("linux") {
            // OpenAL on Linux depends on sndio, needs to be dynamically linked
            println!("cargo:rustc-link-lib=dylib=sndio");
        }
        if os.eq_ignore_ascii_case("android") {
            println!("cargo:rustc-link-lib=dylib=OpenSLES");
        }
        if os.eq_ignore_ascii_case("macos") {
            link_framework("AudioToolbox");
            link_framework("CoreAudio");
            link_framework("CoreFoundation");
        }
    }

    // see also src/audio/openal.rs
    link_lib(if os.eq_ignore_ascii_case("windows") {
        "OpenAL32"
    } else {
        "openal"
    });
    // rerun-if-changed seems to not work if pointed to a directory :(
    //rerun_if_changed(&workspace_root.join("vendor/openal-soft"));
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
