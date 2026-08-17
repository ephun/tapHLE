/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Compatibility ratings: the shared ones, this machine's own, and reports.
//!
//! Two ratings exist for an app and they are deliberately different things.
//! The **database rating** is tapHLEdb's published record, which the frontend
//! only ever reads. The **local rating** is what this user found on this
//! machine; it is stored with the library entry and never sent anywhere. The
//! interface shows them apart, and choosing a local rating never touches the
//! database value — an unmoderated opinion must not be able to overwrite a
//! published one.
//!
//! ## What is implemented
//!
//! Reading is complete. `GET /compatibility/api/apps` is a real, public,
//! credential-free endpoint that returns every app the database knows with
//! its current star rating, so the frontend can show the shared rating,
//! link to the entry, and tell whether an app already has a record before
//! anybody drafts a new report.
//!
//! ## What is not
//!
//! Submitting is not implemented, and the interface says so rather than
//! offering a button that does nothing. The database accepts reports from an
//! agent token that belongs to the maintainer, not from arbitrary users; a
//! user-facing submission needs the GitHub sign-in that the project has not
//! built yet. Until it exists, [ReportDraft] assembles the exact contents of
//! a report so it can be copied into the web form, which is a real workflow
//! rather than a placeholder.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::http::Transport;

/// The site the compatibility database is served from.
pub const DATABASE_SITE: &str = "https://taphle.ephun.net";
/// The public, credential-free list of apps and their ratings.
pub const DATABASE_APPS_URL: &str = "https://taphle.ephun.net/compatibility/api/apps";
/// Where a person goes to read or submit records.
pub const DATABASE_WEB_URL: &str = "https://taphle.ephun.net/compatibility";

/// One app as the database describes it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseEntry {
    pub app_id: u64,
    pub name: String,
    /// Stars, 1 to 5. An app with a record but no rating yet has none.
    pub rating: Option<u8>,
    pub bundle_identifier: Option<String>,
    pub developer_publisher: Option<String>,
    /// Absolute address of the entry's page.
    pub url: String,
}

/// Everything the database said, and when it said it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseSnapshot {
    /// Unix seconds. Zero means nothing has been fetched.
    pub fetched: u64,
    pub entries: Vec<DatabaseEntry>,
}

impl DatabaseSnapshot {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The record for a bundle identifier, if the database has one.
    ///
    /// Matching is case-insensitive because a reverse-DNS identifier is not
    /// case-sensitive in practice and records have been entered both ways.
    pub fn find(&self, bundle_identifier: &str) -> Option<&DatabaseEntry> {
        self.entries.iter().find(|entry| {
            entry
                .bundle_identifier
                .as_deref()
                .is_some_and(|id| id.eq_ignore_ascii_case(bundle_identifier))
        })
    }
}

/// The response shape of `GET /compatibility/api/apps`.
#[derive(Deserialize)]
struct ApiResponse {
    #[serde(default)]
    apps: Vec<ApiApp>,
}

#[derive(Deserialize)]
struct ApiApp {
    #[serde(default)]
    app_id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    rating: Option<u8>,
    #[serde(default)]
    extra: ApiExtra,
    #[serde(default)]
    url: String,
}

#[derive(Default, Deserialize)]
struct ApiExtra {
    #[serde(default)]
    bundle_identifier: Option<String>,
    #[serde(default)]
    developer_publisher: Option<String>,
}

/// Turn the API's JSON into a snapshot.
pub fn parse_snapshot(body: &str, fetched: u64) -> Result<DatabaseSnapshot, String> {
    let response: ApiResponse = serde_json::from_str(body)
        .map_err(|e| format!("The compatibility database sent something unreadable: {e}"))?;
    let entries = response
        .apps
        .into_iter()
        .map(|app| DatabaseEntry {
            app_id: app.app_id,
            name: app.name,
            rating: app.rating.filter(|stars| (1..=5).contains(stars)),
            bundle_identifier: app
                .extra
                .bundle_identifier
                .filter(|id| !id.trim().is_empty()),
            developer_publisher: app
                .extra
                .developer_publisher
                .filter(|name| !name.trim().is_empty()),
            url: absolute_url(&app.url),
        })
        .collect();
    Ok(DatabaseSnapshot { fetched, entries })
}

/// The API returns site-relative addresses; a browser needs the whole thing.
fn absolute_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else if url.is_empty() {
        DATABASE_WEB_URL.to_string()
    } else if let Some(path) = url.strip_prefix('/') {
        format!("{DATABASE_SITE}/{path}")
    } else {
        format!("{DATABASE_SITE}/{url}")
    }
}

/// Where the shared ratings come from. A trait so the interface never talks
/// to the network directly, and so a different source could be substituted.
pub trait CompatibilityProvider: Send + Sync {
    fn describe(&self) -> String;
    fn fetch(&self) -> Result<DatabaseSnapshot, String>;
    /// Why the frontend cannot submit a report yet. Shown in the report
    /// window, so the limitation is visible rather than implied by a button
    /// that does nothing.
    fn submission_limitation(&self) -> &'static str;
}

pub struct TapHledbProvider {
    transport: Arc<dyn Transport>,
}

impl TapHledbProvider {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        TapHledbProvider { transport }
    }
}

impl CompatibilityProvider for TapHledbProvider {
    fn describe(&self) -> String {
        format!(
            "tapHLEdb at {DATABASE_SITE} (via {})",
            self.transport.describe()
        )
    }

    fn fetch(&self) -> Result<DatabaseSnapshot, String> {
        let response = self.transport.get(DATABASE_APPS_URL, 15)?;
        if response.status != 200 {
            return Err(format!(
                "The compatibility database answered with status {}",
                response.status
            ));
        }
        parse_snapshot(&response.body, crate::timefmt::now_seconds())
    }

    fn submission_limitation(&self) -> &'static str {
        "Submitting a report from tapHLE is not implemented. The database \
         accepts reports from the maintainer's own credentials, and a \
         sign-in for other people has not been built yet. Copy this report \
         and paste it into the database's web form."
    }
}

/// This machine's own rating for an app, kept with its library entry.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalRating {
    /// Stars, 1 to 5. None means this user has not rated it.
    pub stars: Option<u8>,
    pub notes: String,
    /// The tapHLE version the rating was formed on, so an old opinion is
    /// recognisable as old.
    pub taphle_version: Option<String>,
    /// Unix seconds.
    pub updated: Option<u64>,
}

impl LocalRating {
    pub fn set_stars(&mut self, stars: Option<u8>) {
        self.stars = stars.filter(|s| (1..=5).contains(s));
        self.touch();
    }

    pub fn touch(&mut self) {
        self.updated = Some(crate::timefmt::now_seconds());
        self.taphle_version = Some(tapHLE_version::VERSION.trim().to_string());
    }
}

/// Everything a compatibility report should say, assembled from what the
/// frontend actually knows.
///
/// Nothing here is invented: the identity comes from the app's own bundle,
/// the build from the version crate, the options from the settings that were
/// used, and the log excerpt from the run's own output.
pub struct ReportDraft {
    pub display_name: String,
    pub bundle_identifier: String,
    pub bundle_version: String,
    pub short_version: Option<String>,
    pub taphle_version: String,
    pub taphle_build: String,
    pub platform: String,
    pub stars: Option<u8>,
    pub notes: String,
    pub launch_options: Vec<String>,
    pub log_excerpt: String,
    /// A record already in the database for this bundle identifier, so a
    /// second entry is not created for an app that already has one.
    pub existing_entry: Option<DatabaseEntry>,
    /// Whether the database was reachable when this draft was made. Without
    /// it, "no existing entry" only means "not known".
    pub database_consulted: bool,
}

impl ReportDraft {
    /// The report as text, ready for the clipboard and the web form.
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        let mut line = |label: &str, value: &str| {
            text.push_str(label);
            text.push_str(": ");
            text.push_str(value);
            text.push('\n');
        };
        line("App", &self.display_name);
        line("Bundle identifier", &self.bundle_identifier);
        line("Bundle version", &self.bundle_version);
        if let Some(short) = &self.short_version {
            line("Short version", short);
        }
        line("tapHLE version", &self.taphle_version);
        line("tapHLE build", &self.taphle_build);
        line("Platform", &self.platform);
        line(
            "Rating",
            &match self.stars {
                Some(stars) => format!("{stars} of 5 stars"),
                None => "not rated".to_string(),
            },
        );
        line(
            "Launch options",
            &if self.launch_options.is_empty() {
                "(defaults)".to_string()
            } else {
                self.launch_options.join(" ")
            },
        );
        line(
            "Existing database entry",
            &match (&self.existing_entry, self.database_consulted) {
                (Some(entry), _) => format!("{} — {}", entry.name, entry.url),
                (None, true) => "none found for this bundle identifier".to_string(),
                (None, false) => "not checked (the database was not reachable)".to_string(),
            },
        );
        if !self.notes.trim().is_empty() {
            text.push_str("\nNotes:\n");
            text.push_str(self.notes.trim());
            text.push('\n');
        }
        if !self.log_excerpt.trim().is_empty() {
            text.push_str("\nLog excerpt:\n");
            text.push_str(self.log_excerpt.trim_end());
            text.push('\n');
        }
        text
    }
}

/// The host platform, as a report should record it.
pub fn platform_description() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of a real response from the live database, trimmed to two
    /// entries. Pinning it here means a change to the API is a test failure
    /// rather than a blank rating column.
    const SAMPLE: &str = r#"{"apps":[
        {"app_id":4,"name":"Baby Monkey","rating":3,
         "extra":{"bundle_identifier":"com.kihon.babymonkey"},
         "url":"/compatibility/apps/4"},
        {"app_id":5,"name":"Cops & Robbers","rating":2,
         "extra":{"bundle_identifier":"com.glu.thief3d",
                  "developer_publisher":"Glu Mobile","release_year":"2009"},
         "url":"/compatibility/apps/5"}]}"#;

    #[test]
    fn the_public_app_list_is_understood() {
        let snapshot = parse_snapshot(SAMPLE, 1000).unwrap();
        assert_eq!(snapshot.entries.len(), 2);
        let entry = snapshot
            .find("com.glu.thief3d")
            .expect("entry should be found");
        assert_eq!(entry.name, "Cops & Robbers");
        assert_eq!(entry.rating, Some(2));
        assert_eq!(entry.developer_publisher.as_deref(), Some("Glu Mobile"));
        assert_eq!(entry.url, "https://taphle.ephun.net/compatibility/apps/5");
    }

    /// Records have been entered with different capitalisation, and a missed
    /// match is what causes a duplicate entry to be created.
    #[test]
    fn identifiers_match_regardless_of_case() {
        let snapshot = parse_snapshot(SAMPLE, 1000).unwrap();
        assert!(snapshot.find("COM.KIHON.BabyMonkey").is_some());
        assert!(snapshot.find("com.kihon.nothing").is_none());
    }

    /// An unreadable or truncated response must be an error, not an empty
    /// database that would make every app look unrecorded.
    #[test]
    fn a_broken_response_is_an_error() {
        assert!(parse_snapshot("{\"apps\":", 0).is_err());
        assert!(parse_snapshot("<html>404</html>", 0).is_err());
    }

    /// A rating outside one to five stars is not a rating tapHLE uses.
    #[test]
    fn out_of_range_ratings_are_dropped() {
        let snapshot = parse_snapshot(
            r#"{"apps":[{"app_id":1,"name":"X","rating":0,
                "extra":{"bundle_identifier":"com.x"},"url":"/a/1"}]}"#,
            0,
        )
        .unwrap();
        assert_eq!(snapshot.entries[0].rating, None);
    }

    /// A draft has to name the existing entry when there is one; that is the
    /// whole point of consulting the database before reporting.
    #[test]
    fn a_draft_reports_whether_an_entry_already_exists() {
        let snapshot = parse_snapshot(SAMPLE, 0).unwrap();
        let draft = ReportDraft {
            display_name: "Baby Monkey".to_string(),
            bundle_identifier: "com.kihon.babymonkey".to_string(),
            bundle_version: "1.3.5".to_string(),
            short_version: None,
            taphle_version: "test".to_string(),
            taphle_build: "test".to_string(),
            platform: "windows x86_64".to_string(),
            stars: Some(3),
            notes: String::new(),
            launch_options: vec!["--landscape-native".to_string()],
            log_excerpt: String::new(),
            existing_entry: snapshot.find("com.kihon.babymonkey").cloned(),
            database_consulted: true,
        };
        let text = draft.to_text();
        assert!(text.contains("com.kihon.babymonkey"));
        assert!(text.contains("compatibility/apps/4"));
        assert!(text.contains("--landscape-native"));
    }

    /// Fetch the real database, through the real transport, and check that
    /// every record it sends is understood.
    ///
    /// Ignored by default: it needs the network, and a test suite that fails
    /// when a server is down is a test suite people stop believing. Run it
    /// deliberately with `cargo test -p tapHLE_gui -- --ignored` after
    /// changing anything about the API or this parser. SAMPLE above pins the
    /// shape for the offline suite; this checks the shape is still real.
    #[test]
    #[ignore = "requires the network"]
    fn the_live_database_is_understood() {
        let provider = TapHledbProvider::new(Arc::new(crate::http::CurlTransport));
        let snapshot = provider.fetch().expect("the database should answer");
        assert!(
            !snapshot.is_empty(),
            "the database answered with no entries at all"
        );
        for entry in &snapshot.entries {
            assert!(!entry.name.trim().is_empty(), "an entry has no name");
            assert!(
                entry.url.starts_with("https://"),
                "{} kept a relative address: {}",
                entry.name,
                entry.url
            );
            if let Some(stars) = entry.rating {
                assert!((1..=5).contains(&stars), "{} rated {stars}", entry.name);
            }
        }
        let rated = snapshot.entries.iter().filter(|e| e.rating.is_some());
        assert!(
            rated.count() > 0,
            "no entry carried a rating, so the rating column would be blank"
        );
        let identified = snapshot
            .entries
            .iter()
            .filter(|e| e.bundle_identifier.is_some());
        assert!(
            identified.count() > 0,
            "no entry carried a bundle identifier, so nothing could ever match a library app"
        );
    }

    /// When the database could not be reached, the draft must not claim that
    /// no entry exists — that is how duplicates get created.
    #[test]
    fn an_unreachable_database_is_not_reported_as_no_entry() {
        let draft = ReportDraft {
            display_name: "X".to_string(),
            bundle_identifier: "com.x".to_string(),
            bundle_version: "1".to_string(),
            short_version: None,
            taphle_version: "test".to_string(),
            taphle_build: "test".to_string(),
            platform: "windows x86_64".to_string(),
            stars: None,
            notes: String::new(),
            launch_options: Vec::new(),
            log_excerpt: String::new(),
            existing_entry: None,
            database_consulted: false,
        };
        assert!(draft.to_text().contains("not checked"));
    }
}
