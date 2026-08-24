/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

pub const RELEASE_TAG_PREFIX: &str = "taphle-v";

pub fn release_tag(cargo_version: &str) -> String {
    format!("{RELEASE_TAG_PREFIX}{cargo_version}")
}

pub fn display_release_version(cargo_version: &str) -> String {
    format!("v{cargo_version}")
}

#[allow(dead_code)]
pub fn display_development_version(
    cargo_version: &str,
    git_revision: &str,
    dirty: bool,
) -> Option<String> {
    let (base, sequence) = cargo_version.split_once("-dev.")?;
    let git_revision = git_revision
        .rsplit_once("-g")
        .map_or(git_revision, |(_, revision)| revision);
    let mut numbers = base.split('.');
    if numbers.clone().count() != 3
        || numbers.any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        || sequence.is_empty()
        || !sequence.bytes().all(|byte| byte.is_ascii_digit())
        || sequence.parse::<u64>().ok()? == 0
        || !(7..=40).contains(&git_revision.len())
        || !git_revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    let dirty = if dirty { ".dirty" } else { "" };
    Some(format!("{cargo_version}+g{git_revision}{dirty}"))
}

/// Convert the fork-specific Git tag namespace to the shorter user-facing
/// version produced by `git describe`.
// This module is shared with build.rs; these helpers are build-only outside
// tests when the same file is compiled as part of the library.
#[allow(dead_code)]
pub fn display_git_description(git_description: &str) -> &str {
    git_description
        .strip_prefix("taphle-")
        .unwrap_or(git_description)
}

/// Check whether a `git describe` result belongs to the Cargo version.
///
/// Accepted shapes are the exact release tag, that tag with `-dirty`, and the
/// normal `<tag>-<count>-g<hash>` descendant form with an optional `-dirty`.
#[allow(dead_code)]
pub fn description_matches_cargo_version(git_description: &str, cargo_version: &str) -> bool {
    let tag = release_tag(cargo_version);
    let Some(suffix) = git_description.strip_prefix(&tag) else {
        return false;
    };

    if suffix.is_empty() || suffix == "-dirty" {
        return true;
    }

    let Some(suffix) = suffix.strip_prefix('-') else {
        return false;
    };
    let mut parts = suffix.split('-');
    let Some(commit_count) = parts.next() else {
        return false;
    };
    let Some(commit_hash) = parts.next() else {
        return false;
    };
    if commit_count.is_empty() || !commit_count.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    if !commit_hash.starts_with('g')
        || commit_hash.len() == 1
        || !commit_hash[1..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return false;
    }

    matches!(
        (parts.next(), parts.next()),
        (None, None) | (Some("dirty"), None)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_tag_has_short_user_facing_version() {
        assert_eq!(release_tag("0.2.4"), "taphle-v0.2.4");
        assert_eq!(
            display_git_description("taphle-v0.2.4-4-gabc1234-dirty"),
            "v0.2.4-4-gabc1234-dirty"
        );
        assert_eq!(display_git_description("abc1234-dirty"), "abc1234-dirty");
    }

    #[test]
    fn development_versions_include_commit_provenance() {
        assert_eq!(
            display_development_version("0.2.4-dev.1", "2b5b4089", false),
            Some("0.2.4-dev.1+g2b5b4089".to_string())
        );
        assert_eq!(
            display_development_version("0.2.4-dev.27", "ABCDEF12", true),
            Some("0.2.4-dev.27+gABCDEF12.dirty".to_string())
        );
        assert_eq!(
            display_development_version("0.2.5-dev.1", "taphle-v0.2.4-12-gdeadbeef", false),
            Some("0.2.5-dev.1+gdeadbeef".to_string())
        );
        assert_eq!(
            display_development_version("0.2.4", "2b5b4089", false),
            None
        );
        assert_eq!(
            display_development_version("0.2.4-dev.0", "2b5b4089", false),
            None
        );
        assert_eq!(
            display_development_version("0.2.4-dev.1", "not-a-hash", false),
            None
        );
    }

    #[test]
    fn matching_description_shapes_are_bounded() {
        let version = "0.2.4";
        for description in [
            "taphle-v0.2.4",
            "taphle-v0.2.4-dirty",
            "taphle-v0.2.4-1-gabc1234",
            "taphle-v0.2.4-12-gABCDEF-dirty",
        ] {
            assert!(description_matches_cargo_version(description, version));
        }

        for description in [
            "v0.2.4",
            "taphle-v0.2.5",
            "taphle-v0.2.4-preview",
            "taphle-v0.2.4-1-abc1234",
            "taphle-v0.2.4-1-gxyz",
            "taphle-v0.2.4-1-gabc-extra",
        ] {
            assert!(!description_matches_cargo_version(description, version));
        }
    }
}
