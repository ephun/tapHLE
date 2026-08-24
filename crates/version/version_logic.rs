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
        assert_eq!(release_tag("0.3.0-alpha.1"), "taphle-v0.3.0-alpha.1");
        assert_eq!(
            display_git_description("taphle-v0.3.0-alpha.1-4-gabc1234-dirty"),
            "v0.3.0-alpha.1-4-gabc1234-dirty"
        );
        assert_eq!(display_git_description("abc1234-dirty"), "abc1234-dirty");
    }

    #[test]
    fn matching_description_shapes_are_bounded() {
        let version = "0.3.0-alpha.1";
        for description in [
            "taphle-v0.3.0-alpha.1",
            "taphle-v0.3.0-alpha.1-dirty",
            "taphle-v0.3.0-alpha.1-1-gabc1234",
            "taphle-v0.3.0-alpha.1-12-gABCDEF-dirty",
        ] {
            assert!(description_matches_cargo_version(description, version));
        }

        for description in [
            "v0.3.0-alpha.1",
            "taphle-v0.3.0-alpha.2",
            "taphle-v0.3.0-alpha.1-preview",
            "taphle-v0.3.0-alpha.1-1-abc1234",
            "taphle-v0.3.0-alpha.1-1-gxyz",
            "taphle-v0.3.0-alpha.1-1-gabc-extra",
        ] {
            assert!(!description_matches_cargo_version(description, version));
        }
    }
}
