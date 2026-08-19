/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The fonts installed on the computer tapHLE is running on.
//!
//! A substitute is second best by definition. Someone who already has the font
//! an app is asking for — because they own a Mac, or bought the family, or
//! extracted it from a device they own — should get *that*, and get it without
//! having to say so font by font.
//!
//! So before falling back to what tapHLE ships, it looks for the real thing
//! here. Only an exact family match counts: "Helvetica" matches an installed
//! Helvetica and nothing else. Anything looser would quietly draw an app in a
//! font neither tapHLE nor the person chose, which is worse than a substitute
//! that is at least written down.
//!
//! The index is built once, on the first question, and only from the
//! directories the platform actually keeps fonts in.

use super::catalogue::{normalise, Style};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// One face found on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostFace {
    pub family: String,
    pub style: Style,
    pub path: PathBuf,
    /// Which face inside the file, for a collection (`.ttc`). Zero otherwise.
    pub index: u32,
}

/// Every face found, keyed by the normalised family name.
pub struct HostFonts {
    by_family: HashMap<String, Vec<HostFace>>,
}

impl HostFonts {
    /// The face for a family and style, if this computer has that family.
    ///
    /// The style falls back within the family the same way a bundled one does:
    /// an installed family with no italic answers with its upright face rather
    /// than sending the caller away.
    pub fn face(&self, family: &str, style: Style) -> Option<&HostFace> {
        let faces = self.by_family.get(&normalise(family))?;
        let exact = faces.iter().find(|face| face.style == style);
        if exact.is_some() {
            return exact;
        }
        // Same order of preference as a bundled family: keep the weight, drop
        // the slant, then give up on both.
        let without_slant = Style::new(style.is_bold(), false);
        faces
            .iter()
            .find(|face| face.style == without_slant)
            .or_else(|| faces.iter().find(|face| face.style == Style::Regular))
            .or(faces.first())
    }

    /// Every family found, by their real names, sorted. For the frontend's
    /// list of fonts a person can choose from.
    pub fn families(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .by_family
            .values()
            .filter_map(|faces| faces.first().map(|face| face.family.clone()))
            .collect();
        names.sort_by_key(|name| name.to_lowercase());
        names.dedup();
        names
    }

    pub fn is_empty(&self) -> bool {
        self.by_family.is_empty()
    }
}

static INDEX: OnceLock<HostFonts> = OnceLock::new();

/// The installed fonts, indexed on first use.
pub fn installed() -> &'static HostFonts {
    INDEX.get_or_init(|| {
        let mut by_family: HashMap<String, Vec<HostFace>> = HashMap::new();
        let mut files = 0usize;
        for directory in font_directories() {
            collect(&directory, 0, &mut by_family, &mut files);
        }
        log!(
            "Indexed {} font file(s) installed on this computer, {} distinct families.",
            files,
            by_family.len()
        );
        HostFonts { by_family }
    })
}

/// Where this platform keeps fonts.
fn font_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    let home = std::env::var_os("HOME").map(PathBuf::from);

    #[cfg(target_os = "windows")]
    {
        if let Some(windir) = std::env::var_os("SystemRoot") {
            directories.push(PathBuf::from(windir).join("Fonts"));
        }
        // Fonts installed for one user rather than for the machine, which is
        // where a font installed without administrator rights lands.
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            directories.push(
                PathBuf::from(local)
                    .join("Microsoft")
                    .join("Windows")
                    .join("Fonts"),
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        directories.push(PathBuf::from("/System/Library/Fonts"));
        directories.push(PathBuf::from("/Library/Fonts"));
        if let Some(home) = &home {
            directories.push(home.join("Library").join("Fonts"));
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        directories.push(PathBuf::from("/usr/share/fonts"));
        directories.push(PathBuf::from("/usr/local/share/fonts"));
        if let Some(home) = &home {
            directories.push(home.join(".fonts"));
            directories.push(home.join(".local").join("share").join("fonts"));
        }
    }

    let _ = &home;
    directories
}

/// How far down a font directory to look. Linux nests by foundry and style,
/// rarely deeper than three; a limit stops a symlink loop turning the first
/// font question into a hang.
const MAX_DEPTH: u32 = 4;

fn collect(
    directory: &Path,
    depth: u32,
    by_family: &mut HashMap<String, Vec<HostFace>>,
    files: &mut usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, depth + 1, by_family, files);
            continue;
        }
        let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !matches!(
            extension.to_ascii_lowercase().as_str(),
            "ttf" | "otf" | "ttc" | "otc"
        ) {
            continue;
        }
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        *files += 1;
        for face in faces_in(&data, &path) {
            by_family
                .entry(normalise(&face.family))
                .or_default()
                .push(face);
        }
    }
}

/// The faces one font file holds. A `.ttc` holds several.
fn faces_in(data: &[u8], path: &Path) -> Vec<HostFace> {
    let count = owned_ttf_parser::fonts_in_collection(data).unwrap_or(1);
    let mut found = Vec::new();
    for index in 0..count {
        let Ok(face) = owned_ttf_parser::Face::from_slice(data, index) else {
            continue;
        };
        let mut family = None;
        let mut subfamily = None;
        let mut typographic_family = None;
        let mut typographic_subfamily = None;
        for name in face.names() {
            let Some(text) = name.to_string() else {
                continue;
            };
            match name.name_id {
                owned_ttf_parser::name_id::FAMILY => family.get_or_insert(text),
                owned_ttf_parser::name_id::SUBFAMILY => subfamily.get_or_insert(text),
                owned_ttf_parser::name_id::TYPOGRAPHIC_FAMILY => {
                    typographic_family.get_or_insert(text)
                }
                owned_ttf_parser::name_id::TYPOGRAPHIC_SUBFAMILY => {
                    typographic_subfamily.get_or_insert(text)
                }
                _ => continue,
            };
        }
        // The typographic names are the ones a family with more than four
        // faces uses, and they are what an app's font name corresponds to.
        let Some(family) = typographic_family.or(family) else {
            continue;
        };
        let subfamily = typographic_subfamily.or(subfamily).unwrap_or_default();
        let style = Style::new(
            face.is_bold() || subfamily.to_ascii_lowercase().contains("bold"),
            face.is_italic() || subfamily.to_ascii_lowercase().contains("italic"),
        );
        found.push(HostFace {
            family: family.trim().to_string(),
            style,
            path: path.to_path_buf(),
            index,
        });
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The index is built from whatever this machine has, so what can be
    /// asserted is its shape rather than its contents: no family may be listed
    /// under a key that does not normalise to itself, or a lookup would never
    /// find it.
    #[test]
    fn every_family_is_filed_under_its_own_normalised_name() {
        let fonts = installed();
        for (key, faces) in &fonts.by_family {
            for face in faces {
                assert_eq!(
                    &normalise(&face.family),
                    key,
                    "{:?} is filed under {:?}",
                    face.family,
                    key
                );
            }
        }
    }

    /// Whatever is installed, asking for something that cannot exist must miss
    /// rather than match something else.
    #[test]
    fn a_family_that_is_not_installed_is_not_found() {
        let fonts = installed();
        assert!(fonts
            .face("No Such Font Exists Anywhere 12345", Style::Regular)
            .is_none());
    }

    /// Windows has Arial, macOS has Helvetica, and a Linux box may have
    /// neither, so this asserts the mechanism rather than a particular font:
    /// anything the index did find must be findable by its own name.
    #[test]
    fn anything_indexed_can_be_looked_up_by_name() {
        let fonts = installed();
        for family in fonts.families().iter().take(20) {
            assert!(
                fonts.face(family, Style::Regular).is_some(),
                "{family} was indexed but cannot be looked up"
            );
        }
    }
}
