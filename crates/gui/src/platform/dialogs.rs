/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Operating-system document and folder pickers.

use std::path::PathBuf;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn pick_apps() -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new()
        .add_filter("iPhone apps", &["ipa"])
        .set_title("Add apps to the tapHLE library")
        .pick_files()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn pick_apps() -> Option<Vec<PathBuf>> {
    None
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn pick_folder(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(title).pick_folder()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn pick_folder(_title: &str) -> Option<PathBuf> {
    None
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn pick_file(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(title).pick_file()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn pick_file(_title: &str) -> Option<PathBuf> {
    None
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn save_log(suggested: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Save the log")
        .set_file_name(suggested)
        .add_filter("Text", &["txt", "log"])
        .save_file()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn save_log(_suggested: &str) -> Option<PathBuf> {
    None
}
