/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! A read-only view of an app bundle, for tapHLE's own tools.
//!
//! The desktop frontend has to read what an app says about itself, and it
//! must read it the same way the emulator does — an app whose identity the
//! library shows differently from the one a compatibility report cites would
//! be worse than useless. So it reads it through here rather than parsing
//! `Info.plist` a second time.
//!
//! This is a facade on purpose. [crate::bundle] and [crate::fs] stay private:
//! they are the emulator's guest filesystem and are shaped for the emulator's
//! needs, full of guest paths and unit errors, and exposing them would make
//! the whole of that surface part of what tapHLE promises to other programs.
//! What a frontend actually needs is a handful of strings and a bitmap.
//!
//! Nothing here modifies an app. Reading an `.ipa` does not unpack it, and
//! reading an `.app` directory does not write to it.

use crate::bundle::Bundle;
use crate::fs::BundleData;
use std::path::Path;

/// What an app records about itself in its `Info.plist`.
///
/// Every field is copied from the bundle. A key an app does not set is
/// absent rather than guessed: an app with no `CFBundleDisplayName` has no
/// display name, and it is for the caller to decide what to show instead.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppInfo {
    /// `CFBundleDisplayName`, empty when the app does not set one.
    pub display_name: String,
    /// `CFBundleName`, or the bundle directory's name when it has none.
    pub bundle_name: String,
    /// `CFBundleIdentifier`. Required; reading fails without it.
    pub bundle_identifier: String,
    /// `CFBundleVersion`, a build number. Required.
    pub bundle_version: String,
    /// `CFBundleShortVersionString`, the marketing version.
    pub short_version: Option<String>,
    /// `MinimumOSVersion`.
    pub minimum_os_version: Option<String>,
    /// `UIDeviceFamily`, as "iPhone" and "iPad".
    pub device_families: Vec<String>,
    /// `UIRequiredDeviceCapabilities`.
    pub required_capabilities: Vec<String>,
    /// `UISupportedInterfaceOrientations`, or the older singular key.
    pub supported_orientations: Vec<String>,
}

/// A decoded image, in unmultiplied RGBA.
pub struct IconBitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Everything read from one app bundle.
pub struct AppBundleContents {
    pub info: AppInfo,
    /// The app's icon, with the rounded corners and sheen iPhone OS applies.
    pub icon: Option<IconBitmap>,
    /// The raw `iTunesMetadata.plist` beside `Payload/` in an `.ipa`, when
    /// there is one. It is the only record of who published an app, and it is
    /// returned unparsed because it is the caller that cares what is in it.
    pub store_metadata: Option<Vec<u8>>,
    /// Problems that did not prevent the app being read, such as a missing
    /// icon.
    pub warnings: Vec<String>,
}

/// Keys without which the rest of tapHLE cannot work with an app at all.
const REQUIRED_KEYS: &[&str] = &["CFBundleIdentifier", "CFBundleVersion"];

/// Read an `.ipa` file or an `.app` directory.
///
/// The emulator's own readers assume a well-formed `Info.plist` and assert
/// their way through a malformed one, which is reasonable when a run is about
/// to fail anyway but not when a library of a thousand files is being
/// scanned. Reading is therefore caught, and a bundle that cannot be read
/// becomes an error the caller can show rather than the end of the process.
pub fn read(path: &Path) -> Result<AppBundleContents, String> {
    let path = path.to_path_buf();
    match std::panic::catch_unwind(move || read_inner(&path)) {
        Ok(result) => result,
        Err(payload) => Err(format!(
            "The app bundle could not be read: {}",
            panic_message(&payload)
        )),
    }
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "it is not a readable app bundle".to_string()
    }
}

fn read_inner(path: &Path) -> Result<AppBundleContents, String> {
    let mut warnings = Vec::new();
    let mut bundle_data = BundleData::open_any(path)?;

    // Checked before the bundle is built, so that a plist missing a required
    // key produces a sentence about the app rather than a caught panic from
    // somewhere inside the reader.
    let plist_bytes = bundle_data.read_plist()?;
    let plist = plist::Value::from_reader(std::io::Cursor::new(plist_bytes))
        .map_err(|e| format!("Its Info.plist could not be read: {e}"))?
        .into_dictionary()
        .ok_or_else(|| "Its Info.plist is not a property list dictionary.".to_string())?;
    for key in REQUIRED_KEYS {
        if !plist.contains_key(key) {
            return Err(format!(
                "Its Info.plist has no {key}, so tapHLE cannot identify it."
            ));
        }
    }
    if !plist.contains_key("CFBundleExecutable") {
        warnings.push("Its Info.plist names no executable, so it will not run.".to_string());
    }

    // The App Store wrapper has to be read before the bundle takes ownership.
    let store_metadata = bundle_data.read_ipa_root_file("iTunesMetadata.plist");

    let (bundle, fs) = Bundle::new_bundle_and_fs_from_host_path(bundle_data, true)?;

    let icon = match bundle.load_icon(&fs) {
        Ok(image) => {
            let (width, height) = image.dimensions();
            Some(IconBitmap {
                width,
                height,
                rgba: image.pixels().to_vec(),
            })
        }
        Err(e) => {
            warnings.push(format!("Its icon could not be read: {e}"));
            None
        }
    };

    let info = AppInfo {
        display_name: bundle.display_name().to_string(),
        bundle_name: bundle
            .canonical_bundle_name()
            .unwrap_or_else(|| bundle.bundle_name())
            .to_string(),
        bundle_identifier: bundle.bundle_identifier().to_string(),
        bundle_version: bundle.bundle_version().to_string(),
        short_version: bundle.short_version().map(str::to_string),
        minimum_os_version: bundle.minimum_os_version().map(str::to_string),
        device_families: bundle
            .device_family_array()
            .iter()
            .map(|family| family.to_string())
            .collect(),
        required_capabilities: bundle
            .required_device_capabilities()
            .iter()
            .map(|capability| capability.to_string())
            .collect(),
        supported_orientations: bundle
            .supported_interface_orientations()
            .iter()
            .map(|orientation| orientation.to_string())
            .collect(),
    };

    Ok(AppBundleContents {
        info,
        icon,
        store_metadata,
        warnings,
    })
}

/// Decode a PNG, including the CgBI variant Apple's tools produce.
///
/// Exposed because an ordinary PNG decoder cannot read an iPhone app's
/// images, and tapHLE already carries one that can.
pub fn decode_image(bytes: &[u8]) -> Result<IconBitmap, String> {
    let image = crate::image::Image::from_bytes(bytes)?;
    let (width, height) = image.dimensions();
    Ok(IconBitmap {
        width,
        height,
        rgba: image.pixels().to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path that is not a bundle has to come back as an error rather than
    /// as a panic; a library scan walks over whatever is in a folder.
    #[test]
    fn something_that_is_not_a_bundle_is_an_error() {
        let missing = std::env::temp_dir().join("tapHLE-not-an-app-bundle");
        assert!(read(&missing).is_err());
    }

    /// The required keys are what the rest of tapHLE indexes an app by, so
    /// their absence has to be reported as such.
    #[test]
    fn the_required_keys_are_the_identity_ones() {
        assert!(REQUIRED_KEYS.contains(&"CFBundleIdentifier"));
        assert!(REQUIRED_KEYS.contains(&"CFBundleVersion"));
    }

    #[test]
    fn a_non_image_is_not_decoded() {
        assert!(decode_image(b"not an image at all").is_err());
    }
}
