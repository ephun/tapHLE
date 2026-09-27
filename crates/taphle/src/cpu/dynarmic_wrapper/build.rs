/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use std::env;
use std::path::Path;

fn rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
}
fn link_search(path: &Path) {
    println!("cargo:rustc-link-search=native={}", path.to_str().unwrap());
}
fn link_lib(lib: &str) {
    println!("cargo:rustc-link-lib=static={lib}");
}

fn build_type_windows() -> &'static str {
    let os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS was not set");
    if os.eq_ignore_ascii_case("windows") {
        if cfg!(debug_assertions) {
            "Debug"
        } else {
            "Release"
        }
    } else {
        ""
    }
}

fn main() {
    // Read CARGO_MANIFEST_DIR at run time, not via env!(): env!() bakes this
    // worktree's absolute path into the build-script binary, which cargo may
    // reuse from a since-deleted worktree. Cargo always sets it when running a
    // build script.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let package_root = Path::new(&manifest_dir);
    let workspace_root = workspace_root(package_root);
    let dynarmic_root = workspace_root.join("vendor/dynarmic");

    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let source = if os == "ios" && arch == "aarch64" {
        ios_dynarmic_source(&dynarmic_root, package_root)
    } else {
        dynarmic_root.clone()
    };
    let mut build = cmake::Config::new(&source);
    build.define("DYNARMIC_FRONTENDS", "A32"); // We don't need 64-bit
    build.define("DYNARMIC_WARNINGS_AS_ERRORS", "OFF");
    build.define("DYNARMIC_TESTS", "OFF");
    build.define("DYNARMIC_USE_BUNDLED_EXTERNALS", "ON");
    build.define("CMAKE_POLICY_VERSION_MINIMUM", "3.5");

    // This is Windows- and Android-specific because on macOS or Linux, you can
    // easily get Boost with a package manager.
    let os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS was not set");
    let boost_path = workspace_root.join("vendor/boost");
    if (os.eq_ignore_ascii_case("windows") || os.eq_ignore_ascii_case("android"))
        && !boost_path.is_dir()
    {
        panic!("Could not find Boost. Download it from https://www.boost.org/users/download/ and put it at vendor/boost");
    }
    // Allow providing Boost manually regardless of what platform we're on
    // (or whether the target platform was detected correctly…)
    if boost_path.is_dir() {
        build.define("Boost_INCLUDE_DIR", boost_path);
    }
    // Prevent CMake from using macOS-only linker commands when cross-compiling
    // for Android.
    // https://stackoverflow.com/questions/69697715/cross-compiling-c-program-for-android-on-mac-failed-using-ndks-clang
    if os.eq_ignore_ascii_case("android") {
        build.define("CMAKE_SYSTEM_NAME", "Android");
        build.define("CMAKE_SYSTEM_VERSION", "21");
        build.define("ANDROID", "ON");
    }
    // dynarmic can't be dynamically linked
    let dynarmic_out = build.build();

    if os.eq_ignore_ascii_case("android") {
        // Work around weird issue with the NDK where there are missing
        // references to compiler-rt/libgcc symbols.
        // Translated from: https://github.com/termux/termux-packages/issues/8029#issuecomment-1369150244
        let mut cc_command = cc::Build::new().get_compiler().to_command();
        let libclang_rt_path = cc_command
            .arg("-print-libgcc-file-name")
            .output()
            .unwrap()
            .stdout;
        let libclang_rt_path: &Path = std::str::from_utf8(&libclang_rt_path).unwrap().as_ref();
        link_search(libclang_rt_path.parent().unwrap());
        link_lib(
            libclang_rt_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .trim()
                .strip_prefix("lib")
                .unwrap()
                .strip_suffix(".a")
                .unwrap(),
        );
    }

    link_search(&dynarmic_out.join("lib"));
    link_search(&dynarmic_out.join("lib64")); // some Linux systems
    link_lib("dynarmic");
    link_search(
        &dynarmic_out
            .join("build/externals/fmt")
            .join(build_type_windows()),
    );
    link_lib(if cfg!(debug_assertions) {
        "fmtd"
    } else {
        "fmt"
    });
    link_search(
        &dynarmic_out
            .join("build/externals/mcl/src")
            .join(build_type_windows()),
    );
    link_lib("mcl");
    let arch = env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH was not set");
    if arch.eq_ignore_ascii_case("x86_64") {
        link_search(
            &dynarmic_out
                .join("build/externals/zydis")
                .join(build_type_windows()),
        );
        link_lib("Zydis");
    }

    // Track actual sources: .git is a file in a linked worktree, and a
    // nonexistent .git/modules path makes Cargo rebuild on every invocation.
    rerun_if_changed(&dynarmic_root);

    cc::Build::new()
        .file(package_root.join("lib.cpp"))
        .cpp(true)
        .std("c++17")
        .include(dynarmic_out.join("include"))
        .compile("dynarmic_wrapper");
    rerun_if_changed(&package_root.join("lib.cpp"));
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

// Build an isolated overlay of the pinned dependency. Never edit a submodule
// in place or require an unpublished submodule commit to reproduce this build.
fn ios_dynarmic_source(source: &Path, package: &Path) -> std::path::PathBuf {
    fn copy_tree(source: &Path, dest: &Path) {
        std::fs::create_dir_all(dest).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() == ".git" {
                continue;
            }
            let target = dest.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else if !entry
                .path()
                .ends_with("externals/oaknut/include/oaknut/code_block.hpp")
                && !entry
                    .path()
                    .ends_with("src/dynarmic/backend/arm64/address_space.cpp")
                && !entry
                    .path()
                    .ends_with("src/dynarmic/common/spin_lock_arm64.cpp")
                && std::fs::read(&target).ok() != Some(std::fs::read(entry.path()).unwrap())
            {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    fn write_changed(path: &Path, bytes: &[u8]) {
        if std::fs::read(path).ok().as_deref() != Some(bytes) {
            std::fs::write(path, bytes).unwrap();
        }
    }
    let dest =
        std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("ios-dynarmic-source");
    copy_tree(source, &dest);
    let header = package.join("ios_code_block.hpp");
    rerun_if_changed(&header);
    write_changed(
        &dest.join("externals/oaknut/include/oaknut/code_block.hpp"),
        &std::fs::read(header).unwrap(),
    );
    let relative = "src/dynarmic/backend/arm64/address_space.cpp";
    let path = dest.join(relative);
    let text = std::fs::read_to_string(source.join(relative)).unwrap();
    // Oaknut's constructor takes the writable view first, executable second.
    // This covers the main emitter and both relocation/link-patching emitters.
    let old = "mem.ptr(), mem.ptr()";
    assert_eq!(
        text.matches(old).count(),
        3,
        "review iOS overlay after Dynarmic update"
    );
    write_changed(
        &path,
        text.replace(old, "mem.writable_ptr(), mem.ptr()")
            .as_bytes(),
    );
    // Host locks must work before the host has prepared its sole JIT lease.
    // Keep the emitted helpers and their shared 32-bit 0/1 lock ABI unchanged.
    let relative = "src/dynarmic/common/spin_lock_arm64.cpp";
    let text = std::fs::read_to_string(source.join(relative)).unwrap();
    let (emitters, host) = text
        .split_once("namespace {\n\nstruct SpinLockImpl")
        .expect("review iOS spin lock overlay after Dynarmic update");
    assert!(host.ends_with("}  // namespace Dynarmic\n"));
    let emitters = emitters
        .replace("#include <mutex>\n", "")
        .replace("#include <oaknut/code_block.hpp>\n", "");
    let host = r#"void SpinLock::Lock() {
    while (__atomic_exchange_n(&storage, 1, __ATOMIC_ACQUIRE)) {
        while (__atomic_load_n(&storage, __ATOMIC_RELAXED)) {}
    }
}

void SpinLock::Unlock() {
    __atomic_store_n(&storage, 0, __ATOMIC_RELEASE);
}

}  // namespace Dynarmic
"#;
    write_changed(&dest.join(relative), format!("{emitters}{host}").as_bytes());
    dest
}
