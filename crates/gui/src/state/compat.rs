/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Compatibility results from tapHLEdb, and the human web-report workflow.
//!
//! tapHLE reads published cumulative states. It does not create a rating or
//! submit a report: the person who tested an app chooses the result in the
//! GitHub-authenticated tapHLEdb form. Its versioned GET contract accepts the
//! app, version and build facts tapHLE already has, while deliberately leaving
//! the compatibility judgement and contributor identity to the signed-in user.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(not(target_os = "android"))]
use sha2::{Digest, Sha256};
#[cfg(not(target_os = "android"))]
use std::io::Read;

use crate::platform::http::Transport;
use crate::state::metadata::AppMetadata;

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

    /// The same five-position notation the tapHLEdb web interface shows.
    pub const fn emoji(self) -> &'static str {
        match self {
            Self::Untested => "❓❓❓❓❓",
            Self::RanFailedHigher => "⭐❌❌❌❌",
            Self::RanHigherUnknown => "⭐❓❓❓❓",
            Self::InteractiveFailedHigher => "⭐⭐❌❌❌",
            Self::InteractiveHigherUnknown => "⭐⭐❓❓❓",
            Self::CoreUseFailedHigher => "⭐⭐⭐❌❌",
            Self::CoreUseHigherUnknown => "⭐⭐⭐❓❓",
            Self::EndToEndFailedFull => "⭐⭐⭐⭐❌",
            Self::EndToEndFullUnknown => "⭐⭐⭐⭐❓",
            Self::FullyWorking => "⭐⭐⭐⭐⭐",
        }
    }

    /// A screen-reader label which preserves unknown versus tested-and-failed.
    pub const fn accessible_label(self) -> &'static str {
        match self {
            Self::Untested => "Compatibility unknown at all five positions",
            Self::RanFailedHigher => {
                "Launch established; all four higher positions tested and failed"
            }
            Self::RanHigherUnknown => "Launch established; all four higher positions unknown",
            Self::InteractiveFailedHigher => {
                "Launch and interaction established; all three higher positions tested and failed"
            }
            Self::InteractiveHigherUnknown => {
                "Launch and interaction established; all three higher positions unknown"
            }
            Self::CoreUseFailedHigher => {
                "Core use established; both higher positions tested and failed"
            }
            Self::CoreUseHigherUnknown => "Core use established; both higher positions unknown",
            Self::EndToEndFailedFull => {
                "End-to-end use established; fully working tested and failed"
            }
            Self::EndToEndFullUnknown => "End-to-end use established; fully working is unknown",
            Self::FullyWorking => "All five compatibility positions established",
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

/// Build facts that can safely become editable draft fields in a browser URL.
///
/// There is intentionally nowhere here to put a rating, source identity or
/// moderation state. Those are user/server decisions, not build metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReportPrefill {
    pub platform: Option<String>,
    pub os_version: Option<String>,
    pub architecture: Option<String>,
    pub taphle_commit: Option<String>,
    pub taphle_release: Option<String>,
    pub artifact_sha256: Option<String>,
    pub build_provenance: Option<String>,
    pub build_profile: Option<String>,
    pub verification_type: Option<String>,
}

/// Facts about this running tapHLE product. Potentially slow product hashing
/// belongs on a worker thread; callers must not run this while painting a UI.
pub fn running_report_prefill() -> ReportPrefill {
    #[cfg(not(target_os = "android"))]
    let artifact_sha256 = std::env::current_exe()
        .ok()
        .and_then(|path| sha256_file(&path).ok());
    // Android's current_exe is the system app_process binary, not tapHLE's
    // APK or native library, so hashing it would assert false provenance.
    #[cfg(target_os = "android")]
    let artifact_sha256 = None;
    let commit = tapHLE_version::GIT_COMMIT.trim();
    let commit = (commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| commit.to_ascii_lowercase());
    let version = tapHLE_version::VERSION.trim();
    let build_provenance = match (
        tapHLE_version::GITHUB_REPOSITORY,
        tapHLE_version::GITHUB_RUN_ID,
    ) {
        (Some(repository), Some(run)) => Some(format!(
            "GitHub Actions build https://github.com/{repository}/actions/runs/{run}"
        )),
        _ => Some(format!("local build of tapHLE {version}")),
    };
    ReportPrefill {
        platform: Some(host_platform().to_string()),
        os_version: host_os_version(),
        architecture: Some(std::env::consts::ARCH.to_string()),
        taphle_commit: commit,
        taphle_release: (!version.is_empty()).then(|| version.to_string()),
        artifact_sha256,
        build_provenance,
        build_profile: Some(if cfg!(debug_assertions) {
            "debug".to_string()
        } else {
            "release".to_string()
        }),
        verification_type: Some("compatibility".to_string()),
    }
}

/// Construct the documented `prefill[v]=1` browser handoff.
///
/// A malformed library entry falls back to the generic form rather than
/// opening a URL the server must reject. Empty optional values are omitted.
pub fn report_form_url(metadata: &AppMetadata, report: &ReportPrefill) -> String {
    if metadata.bundle_identifier.trim().is_empty() || metadata.bundle_version.trim().is_empty() {
        return DATABASE_REPORT_FORM_URL.to_string();
    }

    let mut fields: Vec<(&str, &str)> = vec![
        ("prefill[v]", "1"),
        (
            "prefill[app][bundle_identifier]",
            metadata.bundle_identifier.as_str(),
        ),
        ("prefill[app][display_name]", metadata.title()),
        (
            "prefill[version][bundle_version]",
            metadata.bundle_version.as_str(),
        ),
    ];
    push_optional(
        &mut fields,
        "prefill[version][short_version]",
        metadata.short_version.as_deref(),
    );
    push_optional(
        &mut fields,
        "prefill[version][minimum_os_version]",
        metadata.minimum_os_version.as_deref(),
    );
    push_optional(
        &mut fields,
        "prefill[version][app_artifact_sha256]",
        metadata.app_artifact_sha256.as_deref(),
    );
    for (name, value) in [
        ("prefill[report][platform]", report.platform.as_deref()),
        ("prefill[report][os_version]", report.os_version.as_deref()),
        (
            "prefill[report][architecture]",
            report.architecture.as_deref(),
        ),
        (
            "prefill[report][taphle_commit]",
            report.taphle_commit.as_deref(),
        ),
        (
            "prefill[report][taphle_release]",
            report.taphle_release.as_deref(),
        ),
        (
            "prefill[report][artifact_sha256]",
            report.artifact_sha256.as_deref(),
        ),
        (
            "prefill[report][build_provenance]",
            report.build_provenance.as_deref(),
        ),
        (
            "prefill[report][build_profile]",
            report.build_profile.as_deref(),
        ),
        (
            "prefill[report][verification_type]",
            report.verification_type.as_deref(),
        ),
    ] {
        push_optional(&mut fields, name, value);
    }

    let query = fields
        .into_iter()
        .map(|(name, value)| {
            format!(
                "{}={}",
                percent_encode(name.as_bytes()),
                percent_encode(value.as_bytes())
            )
        })
        .collect::<Vec<_>>()
        .join("&");
    format!("{DATABASE_REPORT_FORM_URL}?{query}")
}

fn push_optional<'a>(
    fields: &mut Vec<(&'static str, &'a str)>,
    name: &'static str,
    value: Option<&'a str>,
) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        fields.push((name, value));
    }
}

#[cfg(not(target_os = "android"))]
fn sha256_file(path: &std::path::Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn host_platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "macOS",
        "android" => "Android",
        "ios" => "iOS",
        "linux" => "Linux",
        other => other,
    }
}

fn host_os_version() -> Option<String> {
    #[cfg(target_os = "windows")]
    let output = std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .output()
        .ok();
    #[cfg(target_os = "macos")]
    let output = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok();
    #[cfg(target_os = "android")]
    let output = std::process::Command::new("getprop")
        .arg("ro.build.version.release")
        .output()
        .ok();
    #[cfg(target_os = "linux")]
    let output = std::process::Command::new("uname").arg("-r").output().ok();
    #[cfg(target_os = "ios")]
    let output: Option<std::process::Output> = None;

    output
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|version| version.trim().to_string())
        .filter(|version| !version.is_empty())
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
    fn all_states_have_the_exact_five_position_emoji_mapping() {
        for (state, emoji) in [
            (CompatibilityState::Untested, "❓❓❓❓❓"),
            (CompatibilityState::RanFailedHigher, "⭐❌❌❌❌"),
            (CompatibilityState::RanHigherUnknown, "⭐❓❓❓❓"),
            (CompatibilityState::InteractiveFailedHigher, "⭐⭐❌❌❌"),
            (CompatibilityState::InteractiveHigherUnknown, "⭐⭐❓❓❓"),
            (CompatibilityState::CoreUseFailedHigher, "⭐⭐⭐❌❌"),
            (CompatibilityState::CoreUseHigherUnknown, "⭐⭐⭐❓❓"),
            (CompatibilityState::EndToEndFailedFull, "⭐⭐⭐⭐❌"),
            (CompatibilityState::EndToEndFullUnknown, "⭐⭐⭐⭐❓"),
            (CompatibilityState::FullyWorking, "⭐⭐⭐⭐⭐"),
        ] {
            assert_eq!(state.emoji(), emoji);
            assert!(!state.accessible_label().trim().is_empty());
            assert!(!state.accessible_label().contains("/5"));
        }
    }

    fn complete_metadata() -> AppMetadata {
        AppMetadata {
            display_name: "Game & Friends".to_string(),
            bundle_identifier: "com.example/game".to_string(),
            bundle_version: "42+beta".to_string(),
            short_version: Some("1.2 β".to_string()),
            minimum_os_version: Some("3.2".to_string()),
            app_artifact_sha256: Some("b".repeat(64)),
            ..Default::default()
        }
    }

    fn complete_report() -> ReportPrefill {
        ReportPrefill {
            platform: Some("Windows".to_string()),
            os_version: Some("11 24H2".to_string()),
            architecture: Some("x86_64".to_string()),
            taphle_commit: Some("a".repeat(40)),
            taphle_release: Some("0.2.4-dev.1".to_string()),
            artifact_sha256: Some("c".repeat(64)),
            build_provenance: Some("workflow 123 & release".to_string()),
            build_profile: Some("release".to_string()),
            verification_type: Some("compatibility".to_string()),
        }
    }

    #[test]
    fn report_url_uses_nested_percent_encoded_prefill_fields() {
        let url = report_form_url(&complete_metadata(), &complete_report());
        for component in [
            "prefill%5Bv%5D=1",
            "prefill%5Bapp%5D%5Bbundle_identifier%5D=com.example%2Fgame",
            "prefill%5Bapp%5D%5Bdisplay_name%5D=Game%20%26%20Friends",
            "prefill%5Bversion%5D%5Bbundle_version%5D=42%2Bbeta",
            "prefill%5Bversion%5D%5Bshort_version%5D=1.2%20%CE%B2",
            "prefill%5Bversion%5D%5Bminimum_os_version%5D=3.2",
            "prefill%5Breport%5D%5Bplatform%5D=Windows",
            "prefill%5Breport%5D%5Bos_version%5D=11%2024H2",
            "prefill%5Breport%5D%5Barchitecture%5D=x86_64",
            "prefill%5Breport%5D%5Btaphle_commit%5D=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "prefill%5Breport%5D%5Btaphle_release%5D=0.2.4-dev.1",
            "prefill%5Breport%5D%5Bbuild_provenance%5D=workflow%20123%20%26%20release",
            "prefill%5Breport%5D%5Bbuild_profile%5D=release",
            "prefill%5Breport%5D%5Bverification_type%5D=compatibility",
        ] {
            assert!(url.contains(component), "missing {component} in {url}");
        }
        assert!(url.contains(&format!(
            "prefill%5Bversion%5D%5Bapp_artifact_sha256%5D={}",
            "b".repeat(64)
        )));
        assert!(url.contains(&format!(
            "prefill%5Breport%5D%5Bartifact_sha256%5D={}",
            "c".repeat(64)
        )));
        assert!(!url.contains("prefill%5Breport%5D%5Bapp_artifact_sha256%5D"));
        assert!(!url.contains("?app="));
    }

    #[test]
    fn unavailable_and_forbidden_prefill_fields_are_omitted() {
        let metadata = AppMetadata {
            display_name: "Game".to_string(),
            bundle_identifier: "com.example.game".to_string(),
            bundle_version: "1".to_string(),
            ..Default::default()
        };
        let url = report_form_url(&metadata, &ReportPrefill::default());
        assert_eq!(
            url,
            concat!(
                "https://taphle.ephun.net/compatibility/reports/new?",
                "prefill%5Bv%5D=1&",
                "prefill%5Bapp%5D%5Bbundle_identifier%5D=com.example.game&",
                "prefill%5Bapp%5D%5Bdisplay_name%5D=Game&",
                "prefill%5Bversion%5D%5Bbundle_version%5D=1"
            )
        );
        for forbidden in [
            "rating",
            "compatibility_state",
            "source_",
            "moderation",
            "trust",
            "credential",
            "token",
            "app%5D=",
        ] {
            assert!(!url.contains(forbidden), "forbidden field in {url}");
        }
        for unavailable in [
            "short_version",
            "minimum_os_version",
            "app_artifact_sha256",
            "tested_at",
            "test_run_id",
            "evidence",
            "logs",
        ] {
            assert!(!url.contains(unavailable), "unavailable field in {url}");
        }
    }

    #[test]
    fn incomplete_identity_keeps_the_generic_form_fallback() {
        let mut metadata = complete_metadata();
        metadata.bundle_identifier.clear();
        assert_eq!(
            report_form_url(&metadata, &complete_report()),
            DATABASE_REPORT_FORM_URL
        );
    }

    #[test]
    fn query_components_are_percent_encoded() {
        assert_eq!(
            percent_encode("app id=a&b/ç".as_bytes()),
            "app%20id%3Da%26b%2F%C3%A7"
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
