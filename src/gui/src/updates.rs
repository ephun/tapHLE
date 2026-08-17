/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Asking whether a newer tapHLE has been published.
//!
//! ## What exists today
//!
//! tapHLE's release process is documented in `dev-docs/releases.md` and its
//! release tags live in their own namespace, `taphle-v*`, to keep them apart
//! from the `v*` tags inherited from touchHLE. As of writing, **no tapHLE
//! release has been published**: the repository's releases list is empty, and
//! `releases/latest` answers 404. Every `v*` tag in the repository predates
//! the fork.
//!
//! So the honest current answer to "is there an update" is "there is no
//! release channel yet", and that is exactly what this module reports. It is
//! not a stub: the check really runs, really asks GitHub, and really filters
//! for tapHLE's tag namespace. The day a `taphle-v*` release is published it
//! will start finding it, with no further work.
//!
//! ## What remains
//!
//! Nothing is downloaded or installed. The frontend offers to open the
//! releases page; deciding what to do with an update is left to the person,
//! which is the right default for a program that is also a build tree.

use serde::Deserialize;
use std::sync::Arc;

use crate::http::Transport;

pub const REPOSITORY: &str = "ephun/tapHLE";
pub const RELEASES_API_URL: &str = "https://api.github.com/repos/ephun/tapHLE/releases";
pub const RELEASES_WEB_URL: &str = "https://github.com/ephun/tapHLE/releases";
/// tapHLE's own release tags. A `v*` tag is inherited from upstream and is
/// not a tapHLE release, so matching on the prefix is what keeps the check
/// from announcing touchHLE 0.2.3 as an update.
pub const RELEASE_TAG_PREFIX: &str = "taphle-v";

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub tag: String,
    /// The version without the tag namespace, as a person would say it.
    pub version: String,
    pub name: String,
    pub url: String,
    pub published: Option<String>,
    pub prerelease: bool,
}

/// The outcome of a check, as the status bar and the About window show it.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateStatus {
    /// Checking is switched off in settings.
    Disabled,
    /// Nothing has been asked yet.
    NotChecked,
    Checking,
    /// GitHub answered, but tapHLE has published no releases at all.
    NoReleasesPublished,
    UpToDate {
        latest: String,
    },
    Available(Release),
    Failed(String),
}

impl UpdateStatus {
    /// The one-line form for the status bar.
    pub fn summary(&self) -> String {
        match self {
            UpdateStatus::Disabled => "Update checks off".to_string(),
            UpdateStatus::NotChecked => "Updates not checked".to_string(),
            UpdateStatus::Checking => "Checking for updates…".to_string(),
            UpdateStatus::NoReleasesPublished => "No releases published yet".to_string(),
            UpdateStatus::UpToDate { .. } => "Up to date".to_string(),
            UpdateStatus::Available(release) => format!("Update available: {}", release.version),
            UpdateStatus::Failed(_) => "Update check failed".to_string(),
        }
    }

    /// The longer form, for the About window, where the reason matters.
    pub fn detail(&self) -> String {
        match self {
            UpdateStatus::Disabled => {
                "Automatic update checks are switched off in Settings.".to_string()
            }
            UpdateStatus::NotChecked => "No update check has run yet.".to_string(),
            UpdateStatus::Checking => "Asking GitHub for the latest release…".to_string(),
            UpdateStatus::NoReleasesPublished => format!(
                "{REPOSITORY} has not published a tapHLE release yet, so there is nothing \
                 to compare against. This build identifies itself by its commit."
            ),
            UpdateStatus::UpToDate { latest } => {
                format!("This is the latest published release ({latest}).")
            }
            UpdateStatus::Available(release) => format!(
                "{} was published{}. This build is {}.",
                release.version,
                release
                    .published
                    .as_deref()
                    .map(|d| format!(" on {}", &d[..d.len().min(10)]))
                    .unwrap_or_default(),
                running_version()
            ),
            UpdateStatus::Failed(reason) => format!("The update check failed: {reason}"),
        }
    }
}

/// Where releases are looked for. A trait so that the check can be replaced
/// or switched off without the interface knowing how it is done.
pub trait ReleaseProvider: Send + Sync {
    fn describe(&self) -> String;
    /// Every published tapHLE release, newest first. An empty list means the
    /// repository has none, which is different from an error.
    fn releases(&self) -> Result<Vec<Release>, String>;
}

pub struct GitHubReleaseProvider {
    transport: Arc<dyn Transport>,
}

impl GitHubReleaseProvider {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        GitHubReleaseProvider { transport }
    }
}

#[derive(Deserialize)]
struct ApiRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

impl ReleaseProvider for GitHubReleaseProvider {
    fn describe(&self) -> String {
        format!(
            "GitHub releases for {REPOSITORY} (via {})",
            self.transport.describe()
        )
    }

    fn releases(&self) -> Result<Vec<Release>, String> {
        let response = self.transport.get(RELEASES_API_URL, 15)?;
        match response.status {
            200 => parse_releases(&response.body),
            403 | 429 => Err("GitHub is rate-limiting update checks".to_string()),
            404 => Err(format!("{REPOSITORY} was not found on GitHub")),
            status => Err(format!("GitHub answered with status {status}")),
        }
    }
}

pub fn parse_releases(body: &str) -> Result<Vec<Release>, String> {
    let releases: Vec<ApiRelease> =
        serde_json::from_str(body).map_err(|e| format!("GitHub sent something unreadable: {e}"))?;
    Ok(releases
        .into_iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let version = release
                .tag_name
                .strip_prefix(RELEASE_TAG_PREFIX)?
                .to_string();
            Some(Release {
                name: release.name.unwrap_or_else(|| release.tag_name.clone()),
                tag: release.tag_name,
                version,
                url: if release.html_url.is_empty() {
                    RELEASES_WEB_URL.to_string()
                } else {
                    release.html_url
                },
                published: release.published_at,
                prerelease: release.prerelease,
            })
        })
        .collect())
}

/// The version this build claims to be, for comparison against a release.
///
/// The Cargo version is used rather than the `git describe` string because a
/// development build's description is a commit hash, which cannot be
/// compared with a release version at all.
pub fn running_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Decide what to report from a list of releases.
pub fn evaluate(releases: &[Release], current: &str) -> UpdateStatus {
    let Some(latest) = releases
        .iter()
        .max_by(|a, b| compare_versions(&a.version, &b.version))
    else {
        return UpdateStatus::NoReleasesPublished;
    };
    if compare_versions(&latest.version, current) == std::cmp::Ordering::Greater {
        UpdateStatus::Available(latest.clone())
    } else {
        UpdateStatus::UpToDate {
            latest: latest.version.clone(),
        }
    }
}

/// Order two version strings the way semantic versioning does.
///
/// tapHLE's versions are `0.3.0-alpha.1` shaped: dotted numbers, then an
/// optional pre-release tail. The rules that matter are that numbers compare
/// numerically rather than as text — so 0.10 is after 0.9 — and that a
/// pre-release sorts before the release it leads to, so an alpha must never
/// be announced as an update to the final version.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    fn split(version: &str) -> (&str, Option<&str>) {
        match version.split_once('-') {
            Some((core, tail)) => (core, Some(tail)),
            None => (version, None),
        }
    }

    let (a_core, a_pre) = split(a.trim().trim_start_matches('v'));
    let (b_core, b_pre) = split(b.trim().trim_start_matches('v'));

    let mut a_parts = a_core.split('.');
    let mut b_parts = b_core.split('.');
    loop {
        match (a_parts.next(), b_parts.next()) {
            (None, None) => break,
            (left, right) => {
                let left: u64 = left.unwrap_or("0").parse().unwrap_or(0);
                let right: u64 = right.unwrap_or("0").parse().unwrap_or(0);
                match left.cmp(&right) {
                    Ordering::Equal => (),
                    other => return other,
                }
            }
        }
    }

    match (a_pre, b_pre) {
        (None, None) => Ordering::Equal,
        // A release outranks any pre-release of the same version.
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(left), Some(right)) => compare_prerelease(left, right),
    }
}

fn compare_prerelease(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut a_parts = a.split('.');
    let mut b_parts = b.split('.');
    loop {
        match (a_parts.next(), b_parts.next()) {
            (None, None) => return Ordering::Equal,
            // Fewer identifiers sorts first, as in semantic versioning.
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(left), Some(right)) => {
                let ordering = match (left.parse::<u64>(), right.parse::<u64>()) {
                    (Ok(left), Ok(right)) => left.cmp(&right),
                    // A numeric identifier always sorts before an alphabetic
                    // one.
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => left.cmp(right),
                };
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    /// The exact answer GitHub gives for this repository today. The check
    /// must read it as "nothing published", not as a failure.
    #[test]
    fn an_empty_release_list_is_not_a_failure() {
        let releases = parse_releases("[]").unwrap();
        assert!(releases.is_empty());
        assert_eq!(
            evaluate(&releases, "0.3.0-alpha.1"),
            UpdateStatus::NoReleasesPublished
        );
    }

    /// The repository still carries the `v*` tags it inherited from
    /// touchHLE. Announcing one of those as a tapHLE update would be wrong,
    /// so only the fork's own namespace counts.
    #[test]
    fn inherited_upstream_tags_are_not_tapHLE_releases() {
        let body = r#"[
            {"tag_name":"v0.2.3","html_url":"https://x/1","draft":false,
             "prerelease":false},
            {"tag_name":"taphle-v0.3.0","html_url":"https://x/2","draft":false,
             "prerelease":false}
        ]"#;
        let releases = parse_releases(body).unwrap();
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].version, "0.3.0");
    }

    #[test]
    fn drafts_are_not_published_releases() {
        let body = r#"[{"tag_name":"taphle-v9.9.9","html_url":"https://x/1",
                        "draft":true,"prerelease":false}]"#;
        assert!(parse_releases(body).unwrap().is_empty());
    }

    #[test]
    fn a_newer_release_is_offered() {
        let body = r#"[{"tag_name":"taphle-v0.4.0","html_url":"https://x/1",
                        "draft":false,"prerelease":false,
                        "published_at":"2026-09-01T00:00:00Z"}]"#;
        let releases = parse_releases(body).unwrap();
        match evaluate(&releases, "0.3.0-alpha.1") {
            UpdateStatus::Available(release) => assert_eq!(release.version, "0.4.0"),
            other => panic!("expected an update, got {other:?}"),
        }
    }

    #[test]
    fn the_current_release_is_not_an_update() {
        let body = r#"[{"tag_name":"taphle-v0.3.0-alpha.1",
                        "html_url":"https://x/1","draft":false,
                        "prerelease":true}]"#;
        let releases = parse_releases(body).unwrap();
        assert_eq!(
            evaluate(&releases, "0.3.0-alpha.1"),
            UpdateStatus::UpToDate {
                latest: "0.3.0-alpha.1".to_string()
            }
        );
    }

    /// Text comparison would put 0.10.0 before 0.9.0 and never offer the
    /// update.
    #[test]
    fn versions_compare_numerically() {
        assert_eq!(compare_versions("0.10.0", "0.9.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0", "0.99.99"), Ordering::Greater);
        assert_eq!(compare_versions("0.3.0", "0.3"), Ordering::Equal);
    }

    /// An alpha leads to the release, not the other way round; getting this
    /// backwards would nag every user of a final build to install an alpha.
    #[test]
    fn a_prerelease_sorts_before_its_release() {
        assert_eq!(compare_versions("0.3.0-alpha.1", "0.3.0"), Ordering::Less);
        assert_eq!(
            compare_versions("0.3.0-alpha.2", "0.3.0-alpha.1"),
            Ordering::Greater
        );
        assert_eq!(
            compare_versions("0.3.0-alpha.1", "0.3.0-beta.1"),
            Ordering::Less
        );
    }

    /// Whatever GitHub sends, the check must not panic; it runs at startup.
    #[test]
    fn nonsense_is_an_error_not_a_panic() {
        assert!(parse_releases("not json").is_err());
        assert!(parse_releases("{}").is_err());
    }
}
