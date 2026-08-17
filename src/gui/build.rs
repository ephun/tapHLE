/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Windows resources: the program's icon and the properties Explorer shows.
//!
//! Without these the frontend is a nameless executable with the default icon
//! — in the taskbar, in the Start menu, and in the "installed programs" list.
//! They are cosmetic in the sense that nothing fails without them, and not
//! cosmetic at all in the sense that they are most of what makes a program
//! look installed rather than downloaded.
//!
//! Nothing happens on other platforms; their equivalents belong with their
//! packaging, which is described in `dev-docs/packaging.md`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(windows)]
    windows_resources();
}

#[cfg(windows)]
fn windows_resources() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let icon = std::path::Path::new(&manifest_dir).join("../../res/icon.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    if !icon.is_file() {
        // A missing icon is not worth failing a build over, but it should not
        // pass unremarked either.
        println!(
            "cargo:warning=No icon at {}; tapHLE-gui will use the default one.",
            icon.display()
        );
        return;
    }

    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon(icon.to_str().expect("the icon path should be UTF-8"))
        .set("ProductName", "tapHLE")
        .set("FileDescription", "tapHLE")
        .set("CompanyName", "tapHLE project contributors")
        .set(
            "LegalCopyright",
            "Mozilla Public License 2.0; distributed binaries under GPL-3.0-or-later",
        )
        .set("OriginalFilename", "tapHLE-gui.exe");
    if let Err(e) = resource.compile() {
        println!("cargo:warning=Could not attach Windows resources: {e}");
    }
}
