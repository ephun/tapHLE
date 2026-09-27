/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Compatibility results from tapHLEdb, and the human web-report workflow.
//!
//! tapHLE reads published cumulative states. It does not create a rating or
//! submit a report: the person who tested an app chooses the result in the
//! GitHub-authenticated tapHLEdb form. The form's documented GET contract can
//! preselect an existing app with `app=<id>` or an existing version with
//! `version=<id>`. The public API exposes app IDs but not version IDs, so the
//! frontend uses only the former and does not invent unsupported prefill keys.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::platform::http::Transport;

/// The site the compatibility database is served from.
pub const DATABASE_SITE: &str = "https://taphle.ephun.net";
/// The public, credential-free list of apps and their ratings.
pub const DATABASE_APPS_URL: &str = "https://taphle.ephun.net/compatibility/api/apps";
/// Where a person goes to read records.
pub const DATABASE_WEB_URL: &str = "https://taphle.ephun.net/compatibility";
/// The GitHub-authenticated human report form.
pub const DATABASE_REPORT_FORM_URL: &str = "https://taphle.ephun.net/compatibility/reports/new";

/// The ten cumulative states accepted by compatibility-model-v2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompatibilityState {
    #[serde(rename = "?????")]
    Untested,
    #[serde(rename = "*XXXX")]
    RanFailedHigher,
    #[serde(rename = "*????")]
    RanHigherUnknown,
    #[serde(rename = "**XXX")]
    InteractiveFailedHigher,
    #[serde(rename = "**???")]
    InteractiveHigherUnknown,
    #[serde(rename = "***XX")]
    CoreUseFailedHigher,
    #[serde(rename = "***??")]
    CoreUseHigherUnknown,
    #[serde(rename = "****X")]
    EndToEndFailedFull,
    #[serde(rename = "****?")]
    EndToEndFullUnknown,
    #[serde(rename = "*****")]
    FullyWorking,
}

impl CompatibilityState {
    pub const fn stars(self) -> u8 {
        match self {
            Self::Untested => 0,
            Self::RanFailedHigher | Self::RanHigherUnknown => 1,
            Self::InteractiveFailedHigher | Self::InteractiveHigherUnknown => 2,
            Self::CoreUseFailedHigher | Self::CoreUseHigherUnknown => 3,
            Self::EndToEndFailedFull | Self::EndToEndFullUnknown => 4,
            Self::FullyWorking => 5,
        }
    }
}

/// One app as the database describes it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseEntry {
    pub app_id: u64,
    pub name: String,
    pub compatibility_state: Option<CompatibilityState>,
    /// Derived stars for the existing read-only interface.
    pub rating: Option<u8>,
    pub states_by_platform: BTreeMap<String, CompatibilityState>,
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
    compatibility_state: Option<CompatibilityState>,
    #[serde(default)]
    states_by_platform: BTreeMap<String, CompatibilityState>,
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
        .map(|app| {
            let rating = app
                .compatibility_state
                .map(CompatibilityState::stars)
                .or_else(|| app.rating.filter(|stars| (1..=5).contains(stars)))
                .filter(|stars| *stars > 0);
            DatabaseEntry {
                app_id: app.app_id,
                name: app.name,
                compatibility_state: app.compatibility_state,
                rating,
                states_by_platform: app.states_by_platform,
                bundle_identifier: app
                    .extra
                    .bundle_identifier
                    .filter(|id| !id.trim().is_empty()),
                developer_publisher: app
                    .extra
                    .developer_publisher
                    .filter(|name| !name.trim().is_empty()),
                url: absolute_url(&app.url),
            }
        })
        .collect();
    Ok(DatabaseSnapshot { fetched, entries })
}

/// Build the only prefilled web-form URL supported by the deployed revision.
pub fn report_form_url(bundle_identifier: &str, database: Option<&DatabaseSnapshot>) -> String {
    match database.and_then(|snapshot| snapshot.find(bundle_identifier)) {
        Some(entry) if entry.app_id > 0 => {
            append_query_parameter(DATABASE_REPORT_FORM_URL, "app", &entry.app_id.to_string())
        }
        _ => DATABASE_REPORT_FORM_URL.to_string(),
    }
}

fn append_query_parameter(url: &str, name: &str, value: &str) -> String {
    format!(
        "{url}?{}={}",
        percent_encode(name.as_bytes()),
        percent_encode(value.as_bytes())
    )
}

fn percent_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(bytes.len());
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    encoded
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

/// The host platform shown in diagnostics and About.
pub fn platform_description() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Where the shared ratings come from. A trait so the interface never talks
/// to the network directly, and so a different source could be substituted.
pub trait CompatibilityProvider: Send + Sync {
    fn describe(&self) -> String;
    fn fetch(&self) -> Result<DatabaseSnapshot, String>;
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
        parse_snapshot(&response.body, crate::state::timefmt::now_seconds())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of a real response from the live database, trimmed to two
    /// entries. Pinning it here means a change to the API is a test failure
    /// rather than a blank rating column.
    const SAMPLE: &str = r#"{"apps":[
        {"app_id":4,"name":"Baby Monkey","rating":3,
         "compatibility_state":"***??",
         "states_by_platform":{"Windows":"***??"},
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
        let baby = snapshot.find("com.kihon.babymonkey").unwrap();
        assert_eq!(
            baby.compatibility_state,
            Some(CompatibilityState::CoreUseHigherUnknown)
        );
        assert_eq!(
            baby.states_by_platform.get("Windows"),
            Some(&CompatibilityState::CoreUseHigherUnknown)
        );
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

    #[test]
    fn structured_state_outranks_the_legacy_numeric_rating() {
        let snapshot = parse_snapshot(
            r#"{"apps":[{"app_id":1,"name":"X","rating":5,
                "compatibility_state":"*????","extra":{"bundle_identifier":"com.x"},
                "url":"/a/1"}]}"#,
            0,
        )
        .unwrap();
        assert_eq!(snapshot.entries[0].rating, Some(1));
    }

    #[test]
    fn impossible_and_invented_states_are_rejected() {
        assert!(serde_json::from_str::<CompatibilityState>(r#""***X?""#).is_err());
        assert!(serde_json::from_str::<CompatibilityState>(r#""⭐⭐⭐❓❓""#).is_err());
    }

    #[test]
    fn report_url_prefills_only_the_canonical_existing_app() {
        let snapshot = parse_snapshot(SAMPLE, 0).unwrap();
        assert_eq!(
            report_form_url("COM.KIHON.BABYMONKEY", Some(&snapshot)),
            "https://taphle.ephun.net/compatibility/reports/new?app=4"
        );
        let url = report_form_url("com.unknown", Some(&snapshot));
        assert_eq!(url, DATABASE_REPORT_FORM_URL);
        assert!(!url.contains("rating"));
        assert!(!url.contains("compatibility_state"));
        assert_eq!(
            report_form_url("com.kihon.babymonkey", None),
            DATABASE_REPORT_FORM_URL
        );
    }

    #[test]
    fn query_components_are_percent_encoded() {
        assert_eq!(
            append_query_parameter("https://example.invalid/form", "app id", "a&b/ç"),
            "https://example.invalid/form?app%20id=a%26b%2F%C3%A7"
        );
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
        let provider = TapHledbProvider::new(Arc::new(crate::platform::http::CurlTransport));
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
}
