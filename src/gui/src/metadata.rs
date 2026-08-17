/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Reading what an app says about itself, and caching its icon.
//!
//! The reading is the emulator's, through `tapHLE::app_bundle`, so the
//! identity the library shows is the identity a run and a compatibility
//! report use. What this module adds is the App Store wrapper — the publisher
//! and genre, which an app's own `Info.plist` never records — and the shape
//! the library stores.
//!
//! Nothing is guessed. An app that does not record its publisher simply has
//! no publisher, and the details panel leaves the row out rather than
//! inventing one.

use std::io::{Read, Write};
use std::path::Path;

use tapHLE::app_bundle;

/// What an app says about itself.
#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppMetadata {
    /// `CFBundleDisplayName`, or the bundle's own name when it has none.
    pub display_name: String,
    /// `CFBundleName`, the name the bundle directory carries.
    pub bundle_name: String,
    pub bundle_identifier: String,
    /// `CFBundleVersion`, which is a build number.
    pub bundle_version: String,
    /// `CFBundleShortVersionString`, the marketing version, when present.
    pub short_version: Option<String>,
    /// `MinimumOSVersion`.
    pub minimum_os_version: Option<String>,
    /// `UIDeviceFamily`, as "iPhone" and "iPad".
    pub device_families: Vec<String>,
    /// `UIRequiredDeviceCapabilities`.
    pub required_capabilities: Vec<String>,
    /// `UISupportedInterfaceOrientations`, or the older singular key.
    pub supported_orientations: Vec<String>,
    /// `artistName` from the App Store wrapper. Nothing in an app's own
    /// `Info.plist` records who published it.
    pub publisher: Option<String>,
    /// `genre` from the App Store wrapper.
    pub genre: Option<String>,
    /// `releaseDate` from the App Store wrapper.
    pub release_date: Option<String>,
    /// Size of the `.ipa` in bytes, when it could be read.
    pub size_bytes: Option<u64>,
}

impl AppMetadata {
    /// The name to show. An app with no display name falls back to its bundle
    /// name, and one with neither to its identifier, so a row is never blank.
    pub fn title(&self) -> &str {
        for candidate in [&self.display_name, &self.bundle_name] {
            if !candidate.trim().is_empty() {
                return candidate;
            }
        }
        &self.bundle_identifier
    }

    /// The version a person would recognise: the marketing version if the app
    /// has one, otherwise the build number.
    pub fn version_for_display(&self) -> &str {
        match self.short_version.as_deref() {
            Some(version) if !version.trim().is_empty() => version,
            _ => &self.bundle_version,
        }
    }

    /// "iPhone", "iPad" or "Universal", as the device family is usually
    /// described.
    pub fn device_family_summary(&self) -> String {
        let has_phone = self.device_families.iter().any(|f| f == "iPhone");
        let has_pad = self.device_families.iter().any(|f| f == "iPad");
        match (has_phone, has_pad) {
            (true, true) => "Universal (iPhone and iPad)".to_string(),
            (true, false) => "iPhone / iPod touch".to_string(),
            (false, true) => "iPad".to_string(),
            (false, false) => self.device_families.join(", "),
        }
    }

    /// The identifier the library, the per-app settings and a compatibility
    /// record all key on.
    ///
    /// A bundle identifier alone is not enough: two versions of one app are
    /// separate rows in the compatibility database and can need different
    /// settings, so the version is part of the key. A file path is not part
    /// of it, so moving an app does not lose its history.
    pub fn stable_id(&self) -> String {
        format!("{}@{}", self.bundle_identifier, self.bundle_version)
    }
}

/// A decoded icon, ready to be handed to the renderer.
pub struct AppIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl From<app_bundle::IconBitmap> for AppIcon {
    fn from(bitmap: app_bundle::IconBitmap) -> Self {
        AppIcon {
            width: bitmap.width,
            height: bitmap.height,
            rgba: bitmap.rgba,
        }
    }
}

/// What one app contributed to the library.
pub struct ReadApp {
    pub metadata: AppMetadata,
    pub icon: Option<AppIcon>,
    /// Things that were wrong but not fatal, such as a missing icon.
    pub warnings: Vec<String>,
}

/// Read an app bundle or `.ipa` through the emulator's own reader.
pub fn read(path: &Path) -> Result<ReadApp, String> {
    let contents = app_bundle::read(path)?;
    let store = contents
        .store_metadata
        .and_then(|bytes| plist::Value::from_reader(std::io::Cursor::new(bytes)).ok())
        .and_then(|value| value.into_dictionary());
    let from_store = |key: &str| -> Option<String> {
        store
            .as_ref()
            .and_then(|dictionary| dictionary.get(key))
            .and_then(|value| value.as_string())
            .map(str::to_string)
            .filter(|text| !text.trim().is_empty())
    };

    let info = contents.info;
    Ok(ReadApp {
        metadata: AppMetadata {
            display_name: info.display_name,
            bundle_name: info.bundle_name,
            bundle_identifier: info.bundle_identifier,
            bundle_version: info.bundle_version,
            short_version: info.short_version,
            minimum_os_version: info.minimum_os_version,
            device_families: info.device_families,
            required_capabilities: info.required_capabilities,
            supported_orientations: info.supported_orientations,
            publisher: from_store("artistName"),
            genre: from_store("genre"),
            release_date: from_store("releaseDate"),
            size_bytes: path
                .is_file()
                .then(|| std::fs::metadata(path).ok().map(|m| m.len()))
                .flatten(),
        },
        icon: contents.icon.map(AppIcon::from),
        warnings: contents.warnings,
    })
}

/// A file name for an app's cached icon.
///
/// The identifier is not usable as a file name — bundle identifiers contain
/// dots and occasionally worse — so it is reduced to safe characters with a
/// hash appended, which keeps two identifiers that reduce to the same text
/// apart.
pub fn icon_cache_name(stable_id: &str) -> String {
    let safe: String = stable_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(60)
        .collect();
    format!("{safe}-{:016x}.icon", fnv1a(stable_id))
}

/// FNV-1a, used only to keep cache file names apart. Nothing depends on it
/// being cryptographic, so a dependency would not earn its place.
fn fnv1a(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The cached-icon file format: a tiny header and the raw pixels, deflated.
///
/// Raw pixels rather than a PNG because tapHLE has a PNG decoder and no
/// encoder, and adding one to write a cache would be a poor trade. Deflate is
/// already in the dependency tree, and an icon compresses to a few kilobytes.
const ICON_CACHE_MAGIC: &[u8; 4] = b"THIC";

pub fn write_icon_cache(dir: &Path, name: &str, icon: &AppIcon) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(ICON_CACHE_MAGIC)
        .and_then(|()| encoder.write_all(&icon.width.to_le_bytes()))
        .and_then(|()| encoder.write_all(&icon.height.to_le_bytes()))
        .and_then(|()| encoder.write_all(&icon.rgba))
        .map_err(|e| format!("Could not compress the icon: {e}"))?;
    let compressed = encoder
        .finish()
        .map_err(|e| format!("Could not compress the icon: {e}"))?;
    let path = dir.join(name);
    std::fs::write(&path, compressed)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))
}

pub fn read_icon_cache(dir: &Path, name: &str) -> Option<AppIcon> {
    let compressed = std::fs::read(dir.join(name)).ok()?;
    let mut decoder = flate2::read::ZlibDecoder::new(compressed.as_slice());
    let mut plain = Vec::new();
    decoder.read_to_end(&mut plain).ok()?;
    if plain.len() < 12 || &plain[0..4] != ICON_CACHE_MAGIC {
        return None;
    }
    let width = u32::from_le_bytes(plain[4..8].try_into().ok()?);
    let height = u32::from_le_bytes(plain[8..12].try_into().ok()?);
    let expected = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if plain.len() - 12 != expected {
        return None;
    }
    Some(AppIcon {
        width,
        height,
        rgba: plain[12..].to_vec(),
    })
}

/// An app's icon, from the cache when it is there and from the app itself
/// when it is not.
///
/// A library outlives its icon cache more easily than it looks: copied to
/// another machine without the `icons` directory beside it, restored from a
/// backup that skipped it, or tidied up by hand. Reading the cache and giving
/// up on a miss is not enough, because an app already in the library is a
/// duplicate on the next scan and its icon is never read again — so the
/// placeholder letter would be permanent. Rebuilding costs one read of an
/// archive that is already on disk, and only for the entries that need it.
pub fn icon_or_rebuild(dir: &Path, name: &str, path: &Path) -> Option<AppIcon> {
    if let Some(icon) = read_icon_cache(dir, name) {
        return Some(icon);
    }
    let icon = read(path).ok()?.icon?;
    // A cache that cannot be written is not worth failing over: the icon is
    // in hand, and the next launch will simply rebuild it again.
    let _ = write_icon_cache(dir, name, &icon);
    Some(icon)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata_with(display: &str, bundle: &str, id: &str) -> AppMetadata {
        AppMetadata {
            display_name: display.to_string(),
            bundle_name: bundle.to_string(),
            bundle_identifier: id.to_string(),
            bundle_version: "1.0".to_string(),
            ..Default::default()
        }
    }

    /// Plenty of apps of this era leave `CFBundleDisplayName` empty, and a
    /// library row with no name in it is useless.
    #[test]
    fn a_missing_display_name_falls_back() {
        assert_eq!(
            metadata_with("Ricky", "RickyApp", "com.x.r").title(),
            "Ricky"
        );
        assert_eq!(metadata_with("", "RickyApp", "com.x.r").title(), "RickyApp");
        assert_eq!(metadata_with("", "", "com.x.r").title(), "com.x.r");
    }

    /// The identifier has to include the version: two versions of one app are
    /// separate compatibility records and may need different settings.
    #[test]
    fn the_stable_identifier_names_the_version() {
        let mut metadata = metadata_with("A", "A", "com.x.a");
        metadata.bundle_version = "2.1".to_string();
        assert_eq!(metadata.stable_id(), "com.x.a@2.1");
    }

    #[test]
    fn device_families_are_summarised() {
        let mut metadata = metadata_with("A", "A", "com.x.a");
        metadata.device_families = vec!["iPhone".to_string()];
        assert_eq!(metadata.device_family_summary(), "iPhone / iPod touch");
        metadata.device_families.push("iPad".to_string());
        assert_eq!(
            metadata.device_family_summary(),
            "Universal (iPhone and iPad)"
        );
    }

    /// A bundle identifier is not a file name. This one is a real shape:
    /// identifiers with a trailing dot and no final segment do occur.
    #[test]
    fn cache_names_are_safe_and_distinct() {
        let a = icon_cache_name("com.eeenmachine.@1.0");
        let b = icon_cache_name("com-eeenmachine--@1.0");
        assert!(!a.contains('.') || a.ends_with(".icon"));
        assert_ne!(a, b, "different identifiers must not share a cache file");
    }

    /// A round trip through the cache has to return the same pixels, or
    /// every library icon would be subtly wrong after the first restart.
    #[test]
    fn the_icon_cache_round_trips() {
        let dir = std::env::temp_dir().join("tapHLE-gui-icon-cache-test");
        let _ = std::fs::remove_dir_all(&dir);
        let icon = AppIcon {
            width: 2,
            height: 2,
            rgba: (0..16).collect(),
        };
        write_icon_cache(&dir, "test.icon", &icon).unwrap();
        let read_back = read_icon_cache(&dir, "test.icon").expect("icon should be readable");
        assert_eq!((read_back.width, read_back.height), (2, 2));
        assert_eq!(read_back.rgba, icon.rgba);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A truncated or foreign file must be rejected rather than producing a
    /// wrongly sized image.
    #[test]
    fn a_damaged_cache_file_is_ignored() {
        let dir = std::env::temp_dir().join("tapHLE-gui-icon-cache-bad");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("bad.icon"), b"not an icon").unwrap();
        assert!(read_icon_cache(&dir, "bad.icon").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
