/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The set of apps this installation knows about.
//!
//! An entry is keyed by the app's own identity — bundle identifier and
//! version — rather than by where its file happens to sit, so moving a file
//! keeps its settings, its rating and how long it has been played. The path
//! is recorded too, because something has to be launched, but it is data
//! about the entry rather than the entry's name.
//!
//! Nothing here modifies an app. Importing reads the bundle and records what
//! it said; it never copies, unpacks or rewrites the file.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::compat::{DatabaseSnapshot, LocalRating};
use crate::metadata::{self, AppMetadata, ReadApp};
use crate::settings::{EmulatorSettings, SortOrder};

/// One app in the library.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryEntry {
    /// [AppMetadata::stable_id]: bundle identifier and version.
    pub id: String,
    pub path: PathBuf,
    pub metadata: AppMetadata,
    /// File name of the cached icon, if one was written.
    pub icon_cache: Option<String>,
    /// Unix seconds when the app entered the library.
    pub added: u64,
    pub last_played: Option<u64>,
    /// Total time the emulator has been running this app, in seconds.
    pub play_seconds: u64,
    pub play_count: u32,
    pub favorite: bool,
    /// Settings that apply to this app only. Anything unset is inherited.
    ///
    /// Held here while the frontend is running, but stored in the settings
    /// file both programs share rather than in this one, so that a run
    /// started from a terminal gets the same settings as one started from the
    /// library. Still *read* from an older library file, which is what
    /// carries an existing library's overrides into the shared file the first
    /// time it is saved.
    #[serde(default, skip_serializing)]
    pub overrides: EmulatorSettings,
    /// This machine's own rating. Never sent anywhere.
    pub local_rating: LocalRating,
    /// Whether the file was there the last time the library was checked.
    /// Not stored: it is about this machine right now.
    #[serde(skip)]
    pub missing: bool,
}

impl LibraryEntry {
    pub fn title(&self) -> &str {
        self.metadata.title()
    }
}

/// Why an import did not add an app, in the terms the person needs.
#[derive(Debug, PartialEq)]
pub enum ImportOutcome {
    /// Added, along with anything that was wrong but not fatal — a missing
    /// icon, most often. Worth saying, not worth refusing the app over.
    Added { id: String, warnings: Vec<String> },
    /// The app is already in the library. `path_updated` says whether the
    /// entry was repointed at the newly given file, which is what happens
    /// when the old one has gone missing or the file has moved.
    Duplicate { id: String, path_updated: bool },
    /// The file is not something tapHLE opens.
    Unsupported { path: PathBuf, reason: String },
    /// It is the right kind of file but could not be read.
    Failed { path: PathBuf, reason: String },
}

impl ImportOutcome {
    pub fn is_success(&self) -> bool {
        matches!(self, ImportOutcome::Added { .. })
    }

    /// The library entry this outcome refers to, when there is one.
    pub fn entry_id(&self) -> Option<&str> {
        match self {
            ImportOutcome::Added { id, .. } | ImportOutcome::Duplicate { id, .. } => Some(id),
            _ => None,
        }
    }

    /// A sentence for the import report.
    pub fn describe(&self, library: &Library) -> String {
        match self {
            ImportOutcome::Added { id, warnings } => {
                let name = library.find(id).map_or(id.as_str(), |e| e.title());
                if warnings.is_empty() {
                    format!("Added {name}.")
                } else {
                    format!("Added {name}, but: {}", warnings.join(" "))
                }
            }
            ImportOutcome::Duplicate { id, path_updated } => {
                let name = library.find(id).map_or(id.as_str(), |e| e.title());
                if *path_updated {
                    format!("{name} was already in the library; its location was updated.")
                } else {
                    format!("{name} is already in the library.")
                }
            }
            ImportOutcome::Unsupported { path, reason } => {
                format!("{}: {reason}", file_label(path))
            }
            ImportOutcome::Failed { path, reason } => {
                format!("{}: {reason}", file_label(path))
            }
        }
    }
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Whether a path is the kind of thing tapHLE opens, and why not if it isn't.
///
/// This mirrors what the emulator's own bundle reader accepts, on purpose:
/// rather than letting a stray file produce the emulator's terse message,
/// the frontend says what it accepts.
pub fn check_supported(path: &Path) -> Result<(), String> {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    match extension.as_deref() {
        Some("ipa") if path.is_file() => Ok(()),
        Some("ipa") => Err("this .ipa file could not be found".to_string()),
        Some("app") if path.is_dir() => Ok(()),
        Some("app") => Err("an .app bundle has to be a folder".to_string()),
        _ if path.is_dir() => Err(
            "this folder is not an .app bundle. Use File ▸ Add Folder to scan it for apps"
                .to_string(),
        ),
        _ if !path.exists() => Err("this file could not be found".to_string()),
        _ => Err("tapHLE opens .ipa files and .app bundles".to_string()),
    }
}

/// Everything in `library.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub entries: Vec<LibraryEntry>,
}

impl Library {
    pub fn find(&self, id: &str) -> Option<&LibraryEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut LibraryEntry> {
        self.entries.iter_mut().find(|entry| entry.id == id)
    }

    pub fn remove(&mut self, id: &str) -> Option<LibraryEntry> {
        let index = self.entries.iter().position(|entry| entry.id == id)?;
        Some(self.entries.remove(index))
    }

    /// Note which entries no longer have a file behind them.
    ///
    /// A missing app is kept rather than deleted: its settings, rating and
    /// play time are worth more than the tidiness, and a removable drive
    /// comes back.
    pub fn mark_missing(&mut self) {
        for entry in &mut self.entries {
            entry.missing = !entry.path.exists();
        }
    }

    /// Add an app that has already been read.
    ///
    /// `icon_dir` is where the icon is cached; a failure to write the cache
    /// is not a failure to import, since the icon can be read again later.
    ///
    /// Reading is the slow part — an `.ipa` is an archive, and its icon has
    /// to be decompressed and decoded — so [read_for_import] does it on a
    /// worker thread and the result is handed here.
    pub fn absorb(&mut self, result: ScanResult, icon_dir: &Path) -> ImportOutcome {
        let (canonical, read) = match result {
            ScanResult::Unsupported { path, reason } => {
                return ImportOutcome::Unsupported { path, reason }
            }
            ScanResult::Failed { path, reason } => return ImportOutcome::Failed { path, reason },
            ScanResult::Read { path, read } => (path, *read),
        };
        let warnings = read.warnings.clone();

        let id = read.metadata.stable_id();
        if let Some(existing) = self.find_mut(&id) {
            // The same app given again from a different place is not a
            // second copy. Repoint the entry when the old file has gone, so
            // moving a collection does not orphan its history.
            let path_updated = existing.path != canonical && !existing.path.exists();
            if path_updated {
                existing.path = canonical;
                existing.missing = false;
            }
            return ImportOutcome::Duplicate { id, path_updated };
        }

        let icon_cache = read.icon.as_ref().and_then(|icon| {
            let name = metadata::icon_cache_name(&id);
            metadata::write_icon_cache(icon_dir, &name, icon)
                .ok()
                .map(|()| name)
        });

        self.entries.push(LibraryEntry {
            id: id.clone(),
            path: canonical,
            metadata: read.metadata,
            icon_cache,
            added: crate::timefmt::now_seconds(),
            ..Default::default()
        });
        ImportOutcome::Added { id, warnings }
    }

    /// Record that a run finished.
    pub fn record_play(&mut self, id: &str, seconds: u64) {
        if let Some(entry) = self.find_mut(id) {
            entry.play_seconds += seconds;
            entry.play_count += 1;
            entry.last_played = Some(crate::timefmt::now_seconds());
        }
    }
}

/// An app read from disk, or the reason it could not be.
pub enum ScanResult {
    Unsupported { path: PathBuf, reason: String },
    Failed { path: PathBuf, reason: String },
    Read { path: PathBuf, read: Box<ReadApp> },
}

/// Read one app in preparation for adding it to a library.
///
/// This is the expensive half of an import and is meant to be run off the
/// interface thread.
pub fn read_for_import(path: &Path) -> ScanResult {
    if let Err(reason) = check_supported(path) {
        return ScanResult::Unsupported {
            path: path.to_path_buf(),
            reason,
        };
    }
    // A canonical path is what makes the same file given twice by different
    // routes — a shortcut, a relative path, a drag from a search result —
    // recognisable as the same file.
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    match metadata::read(&canonical) {
        // Read, and then turned away. A 64-bit app is not a damaged file and
        // must not be reported as one: it is a whole class of app tapHLE does
        // not emulate, so it is refused by name and kept out of the library
        // rather than sitting in the grid as something that will never start.
        Ok(read) if read.architecture == tapHLE::app_bundle::Architecture::Arm64Only => {
            ScanResult::Unsupported {
                path: canonical,
                reason: tapHLE::app_bundle::SIXTY_FOUR_BIT_MESSAGE.to_string(),
            }
        }
        Ok(read) => ScanResult::Read {
            path: canonical,
            read: Box::new(read),
        },
        Err(reason) => ScanResult::Failed {
            path: path.to_path_buf(),
            reason,
        },
    }
}

/// The app files directly inside a folder.
///
/// One level only, the same as the emulator's own app picker: a collection is
/// a folder of apps, and descending further would sweep up the contents of
/// every `.app` bundle it found.
pub fn scan_folder(folder: &Path) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    let entries = std::fs::read_dir(folder)
        .map_err(|e| format!("Could not read {}: {e}", folder.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if check_supported(&path).is_ok() {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

/// The folders scanned when the library is refreshed.
pub fn default_folders() -> Vec<PathBuf> {
    vec![crate::storage::data_dir().join(tapHLE::paths::APPS_DIR)]
}

/// What the library view is showing, in order.
///
/// Kept as a function over entries rather than as a property of the grid, so
/// that a list view, a search box or another filter is a change here and not
/// a rewrite of the view.
pub struct ViewFilter<'a> {
    pub search: &'a str,
    pub favorites_only: bool,
    pub sort: SortOrder,
    pub descending: bool,
}

pub fn visible_entries(
    library: &Library,
    filter: &ViewFilter<'_>,
    database: &DatabaseSnapshot,
) -> Vec<usize> {
    let needle = filter.search.trim().to_lowercase();
    let mut indices: Vec<usize> = library
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| !filter.favorites_only || entry.favorite)
        .filter(|(_, entry)| needle.is_empty() || matches_search(entry, &needle))
        .map(|(index, _)| index)
        .collect();

    indices.sort_by(|&a, &b| {
        let (a, b) = (&library.entries[a], &library.entries[b]);
        let ordering = match filter.sort {
            SortOrder::Title => title_key(a).cmp(&title_key(b)),
            SortOrder::Publisher => publisher_key(a).cmp(&publisher_key(b)),
            // Never played sorts last whichever way the order runs, because
            // "recently played" is a question about the ones that have been.
            SortOrder::RecentlyPlayed => b.last_played.cmp(&a.last_played),
            SortOrder::Compatibility => {
                let rating = |entry: &LibraryEntry| {
                    entry
                        .local_rating
                        .stars
                        .or_else(|| database.find(&entry.metadata.bundle_identifier)?.rating)
                        .unwrap_or(0)
                };
                rating(b).cmp(&rating(a))
            }
            SortOrder::DateAdded => b.added.cmp(&a.added),
        };
        ordering.then_with(|| title_key(a).cmp(&title_key(b)))
    });
    if filter.descending {
        indices.reverse();
    }
    indices
}

/// The versions of one app that are currently visible, and which of them the
/// library is showing.
///
/// A collection accumulates versions — four BabyMonkeys, five Labyrinths —
/// and shown as separate icons they crowd out every app that has only one.
/// They are the same app, so they get one place in the library and a way to
/// say which version that place stands for.
#[derive(Clone, Debug)]
pub struct VersionGroup {
    /// Every visible version, newest first.
    pub versions: Vec<usize>,
    /// The one on display. Always a member of `versions`.
    pub shown: usize,
}

impl VersionGroup {
    pub fn has_choice(&self) -> bool {
        self.versions.len() > 1
    }
}

/// Collapse an ordered list of entries so each app appears once.
///
/// Grouping is by bundle identifier, which is what makes two files the same
/// app: it is the field the compatibility database matches on, and it is the
/// only one that survives a publisher renaming their game between releases.
/// Titles are not used — "Labyrinth 2" and "Labyrinth 2 HD" read as versions
/// of each other and are separate apps with separate identifiers.
///
/// A group takes the position of its best-placed version, so the sort the
/// person chose still decides where it sits.
pub fn group_versions(
    library: &Library,
    order: &[usize],
    chosen: &HashMap<String, String>,
) -> Vec<VersionGroup> {
    let mut groups: Vec<VersionGroup> = Vec::new();
    let mut position: HashMap<&str, usize> = HashMap::new();
    for &index in order {
        let identifier = library.entries[index].metadata.bundle_identifier.as_str();
        match position.get(identifier) {
            Some(&at) => groups[at].versions.push(index),
            None => {
                position.insert(identifier, groups.len());
                groups.push(VersionGroup {
                    versions: vec![index],
                    shown: index,
                });
            }
        }
    }

    for group in &mut groups {
        group.versions.sort_by(|&a, &b| {
            let (a, b) = (&library.entries[a], &library.entries[b]);
            version_key(&b.metadata.bundle_version)
                .cmp(&version_key(&a.metadata.bundle_version))
                .then_with(|| b.metadata.bundle_version.cmp(&a.metadata.bundle_version))
        });
        // The newest version is the default, and an explicit choice wins —
        // but only while that version is still visible, or a search would
        // show an app under a version it had filtered out.
        let identifier = &library.entries[group.versions[0]]
            .metadata
            .bundle_identifier;
        group.shown = chosen
            .get(identifier)
            .and_then(|id| {
                group
                    .versions
                    .iter()
                    .copied()
                    .find(|&index| library.entries[index].id == *id)
            })
            .unwrap_or(group.versions[0]);
    }
    groups
}

/// How one version is named where it sits beside the others.
///
/// Normally the marketing version, which is what the app calls itself and
/// what a person recognises. Two builds can carry the same marketing version
/// and differ in their build number, though, and a dropdown offering the same
/// label twice is a dropdown that cannot be used — so where that happens, the
/// build number is added to the ones that collide.
pub fn version_label<'a>(
    entry: &LibraryEntry,
    siblings: impl IntoIterator<Item = &'a LibraryEntry>,
) -> String {
    let display = entry.metadata.version_for_display();
    let shared = siblings
        .into_iter()
        .any(|other| other.id != entry.id && other.metadata.version_for_display() == display);
    if shared {
        format!("{display} (build {})", entry.metadata.bundle_version)
    } else {
        display.to_string()
    }
}

/// Sort key for a version string: its numbers, in order.
///
/// Compared as text, 1.10 sorts before 1.9 and 3.086 before 3.1, which is the
/// wrong way round for both. Only the numbers are taken, so a build suffix
/// falls back to comparing the strings themselves.
fn version_key(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

fn matches_search(entry: &LibraryEntry, needle: &str) -> bool {
    let metadata = &entry.metadata;
    [
        metadata.title(),
        &metadata.bundle_identifier,
        metadata.publisher.as_deref().unwrap_or(""),
        metadata.genre.as_deref().unwrap_or(""),
    ]
    .iter()
    .any(|field| field.to_lowercase().contains(needle))
}

/// Titles sort as a person reads them, ignoring case.
fn title_key(entry: &LibraryEntry) -> String {
    entry.title().to_lowercase()
}

/// An app with no recorded publisher sorts after those that have one, rather
/// than under an empty heading at the top.
fn publisher_key(entry: &LibraryEntry) -> (bool, String) {
    match entry.metadata.publisher.as_deref() {
        Some(publisher) if !publisher.trim().is_empty() => (false, publisher.to_lowercase()),
        _ => (true, String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, id: &str) -> LibraryEntry {
        LibraryEntry {
            id: format!("{id}@1.0"),
            metadata: AppMetadata {
                display_name: title.to_string(),
                bundle_identifier: id.to_string(),
                bundle_version: "1.0".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn library_of(entries: Vec<LibraryEntry>) -> Library {
        Library { entries }
    }

    #[test]
    fn only_ipa_files_and_app_folders_are_accepted() {
        assert!(
            check_supported(Path::new("game.ipa")).is_err(),
            "an .ipa that is not there is still not usable"
        );

        // A file that exists but is the wrong kind is the case where the
        // message has to say what tapHLE does accept; a path that is simply
        // absent gets the other message, tested below.
        let wrong_kind = std::env::temp_dir().join("tapHLE-gui-not-an-app.zip");
        std::fs::write(&wrong_kind, b"not an app").unwrap();
        let error = check_supported(&wrong_kind).unwrap_err();
        let _ = std::fs::remove_file(&wrong_kind);
        assert!(
            error.contains(".ipa") && error.contains(".app"),
            "the message should say what is accepted, got {error:?}"
        );
    }

    /// A missing file and an unsupported file are different problems and
    /// need different messages, or the person cannot tell a typo from an
    /// unsupported format.
    #[test]
    fn a_missing_file_says_so() {
        let error = check_supported(Path::new("definitely-not-here.bin")).unwrap_err();
        assert!(error.contains("could not be found"));
    }

    #[test]
    fn search_covers_the_fields_a_person_would_type() {
        let mut game = entry("Baby Monkey", "com.kihon.babymonkey");
        game.metadata.publisher = Some("Kihon".to_string());
        let library = library_of(vec![game]);
        let database = DatabaseSnapshot::default();
        for needle in ["monkey", "KIHON", "com.kihon"] {
            let filter = ViewFilter {
                search: needle,
                favorites_only: false,
                sort: SortOrder::Title,
                descending: false,
            };
            assert_eq!(
                visible_entries(&library, &filter, &database).len(),
                1,
                "searching for {needle:?} should find the app"
            );
        }
    }

    #[test]
    fn titles_sort_without_regard_to_case() {
        let library = library_of(vec![
            entry("zebra", "com.a"),
            entry("Apple", "com.b"),
            entry("banana", "com.c"),
        ]);
        let filter = ViewFilter {
            search: "",
            favorites_only: false,
            sort: SortOrder::Title,
            descending: false,
        };
        let order = visible_entries(&library, &filter, &DatabaseSnapshot::default());
        let titles: Vec<&str> = order.iter().map(|&i| library.entries[i].title()).collect();
        assert_eq!(titles, ["Apple", "banana", "zebra"]);
    }

    /// The database rating stands in when this machine has no opinion, so
    /// sorting by compatibility works before anyone has rated anything.
    #[test]
    fn compatibility_sorting_falls_back_to_the_database() {
        let library = library_of(vec![entry("Low", "com.low"), entry("High", "com.high")]);
        let database = crate::compat::parse_snapshot(
            r#"{"apps":[
                {"app_id":1,"name":"Low","rating":1,
                 "extra":{"bundle_identifier":"com.low"},"url":"/a/1"},
                {"app_id":2,"name":"High","rating":5,
                 "extra":{"bundle_identifier":"com.high"},"url":"/a/2"}]}"#,
            0,
        )
        .unwrap();
        let filter = ViewFilter {
            search: "",
            favorites_only: false,
            sort: SortOrder::Compatibility,
            descending: false,
        };
        let order = visible_entries(&library, &filter, &database);
        assert_eq!(library.entries[order[0]].title(), "High");
    }

    /// A local rating is this machine's own answer and outranks the shared
    /// one for the purposes of this user's own view.
    #[test]
    fn a_local_rating_outranks_the_database_for_sorting() {
        let mut low = entry("Low", "com.low");
        low.local_rating.stars = Some(5);
        let library = library_of(vec![low, entry("High", "com.high")]);
        let database = crate::compat::parse_snapshot(
            r#"{"apps":[{"app_id":2,"name":"High","rating":4,
                 "extra":{"bundle_identifier":"com.high"},"url":"/a/2"}]}"#,
            0,
        )
        .unwrap();
        let filter = ViewFilter {
            search: "",
            favorites_only: false,
            sort: SortOrder::Compatibility,
            descending: false,
        };
        let order = visible_entries(&library, &filter, &database);
        assert_eq!(library.entries[order[0]].title(), "Low");
    }

    fn versioned(title: &str, id: &str, version: &str) -> LibraryEntry {
        LibraryEntry {
            id: format!("{id}@{version}"),
            metadata: AppMetadata {
                display_name: title.to_string(),
                bundle_identifier: id.to_string(),
                bundle_version: version.to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn grouped(library: &Library, chosen: &HashMap<String, String>) -> Vec<VersionGroup> {
        let filter = ViewFilter {
            search: "",
            favorites_only: false,
            sort: SortOrder::Title,
            descending: false,
        };
        let order = visible_entries(library, &filter, &DatabaseSnapshot::default());
        group_versions(library, &order, chosen)
    }

    /// Four copies of one game are one app in the library, not four icons.
    #[test]
    fn versions_of_one_app_collapse_into_one_place() {
        let library = library_of(vec![
            versioned("Baby Monkey", "com.kihon.babymonkey", "1.01"),
            versioned("Baby Monkey", "com.kihon.babymonkey", "1.3.5"),
            versioned("Baby Monkey", "com.kihon.babymonkey", "1.2.3"),
            versioned("Bookworm", "com.popcap.bookworm", "1.0"),
        ]);
        let groups = grouped(&library, &HashMap::new());
        assert_eq!(groups.len(), 2, "two apps, not four entries");
        let monkey = &groups[0];
        assert_eq!(monkey.versions.len(), 3);
        assert!(monkey.has_choice());
        assert!(!groups[1].has_choice());
    }

    /// Versions are offered newest first, and "newest" is by number: 1.10 is
    /// newer than 1.9 even though it sorts before it as text.
    #[test]
    fn versions_are_ordered_by_number_not_by_text() {
        let library = library_of(vec![
            versioned("Game", "com.a", "1.9"),
            versioned("Game", "com.a", "1.10"),
            versioned("Game", "com.a", "1.2"),
        ]);
        let groups = grouped(&library, &HashMap::new());
        let versions: Vec<&str> = groups[0]
            .versions
            .iter()
            .map(|&i| library.entries[i].metadata.bundle_version.as_str())
            .collect();
        assert_eq!(versions, ["1.10", "1.9", "1.2"]);
        assert_eq!(
            library.entries[groups[0].shown].metadata.bundle_version, "1.10",
            "the newest version is what the library shows by default"
        );
    }

    #[test]
    fn a_chosen_version_is_the_one_shown() {
        let library = library_of(vec![
            versioned("Game", "com.a", "1.0"),
            versioned("Game", "com.a", "2.0"),
        ]);
        let chosen = HashMap::from([("com.a".to_string(), "com.a@1.0".to_string())]);
        let groups = grouped(&library, &chosen);
        assert_eq!(library.entries[groups[0].shown].id, "com.a@1.0");
    }

    /// A choice that is filtered out cannot be what the group displays, or an
    /// app would appear under a version the filter had just excluded.
    #[test]
    fn a_chosen_version_that_is_not_visible_is_ignored() {
        let mut old = versioned("Game", "com.a", "1.0");
        old.favorite = false;
        let mut new = versioned("Game", "com.a", "2.0");
        new.favorite = true;
        let library = library_of(vec![old, new]);
        let chosen = HashMap::from([("com.a".to_string(), "com.a@1.0".to_string())]);
        let filter = ViewFilter {
            search: "",
            favorites_only: true,
            sort: SortOrder::Title,
            descending: false,
        };
        let order = visible_entries(&library, &filter, &DatabaseSnapshot::default());
        let groups = group_versions(&library, &order, &chosen);
        assert_eq!(groups.len(), 1);
        assert_eq!(library.entries[groups[0].shown].id, "com.a@2.0");
    }

    /// Grouping must not disturb the order the person asked for: the group
    /// sits where its best-placed version sat.
    #[test]
    fn grouping_keeps_the_chosen_sort_order() {
        let library = library_of(vec![
            versioned("Zebra", "com.z", "1.0"),
            versioned("Apple", "com.a", "1.0"),
            versioned("Apple", "com.a", "2.0"),
        ]);
        let groups = grouped(&library, &HashMap::new());
        let titles: Vec<&str> = groups
            .iter()
            .map(|group| library.entries[group.shown].title())
            .collect();
        assert_eq!(titles, ["Apple", "Zebra"]);
    }

    /// Two builds that call themselves the same thing have to be told apart,
    /// or the dropdown offers the same label twice and neither can be picked
    /// deliberately.
    #[test]
    fn versions_sharing_a_marketing_version_are_told_apart() {
        let mut old = versioned("Game", "com.a", "1.01");
        old.metadata.short_version = Some("1.0".to_string());
        let mut new = versioned("Game", "com.a", "1.02");
        new.metadata.short_version = Some("1.0".to_string());
        let mut only = versioned("Game", "com.a", "2.0");
        only.metadata.short_version = Some("2.0".to_string());
        let siblings = [&old, &new, &only];

        assert_eq!(version_label(&old, siblings), "1.0 (build 1.01)");
        assert_eq!(version_label(&new, siblings), "1.0 (build 1.02)");
        assert_eq!(
            version_label(&only, siblings),
            "2.0",
            "a version nothing collides with keeps its plain label"
        );
    }

    #[test]
    fn play_time_accumulates() {
        let mut library = library_of(vec![entry("A", "com.a")]);
        library.record_play("com.a@1.0", 60);
        library.record_play("com.a@1.0", 30);
        let entry = library.find("com.a@1.0").unwrap();
        assert_eq!(entry.play_seconds, 90);
        assert_eq!(entry.play_count, 2);
        assert!(entry.last_played.is_some());
    }

    /// A real two-pixel PNG, so a test bundle can have an icon that
    /// actually decodes without carrying an image file around.
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x06, 0x00, 0x00, 0x00, 0x72,
        0xB6, 0x0D, 0x24, 0x00, 0x00, 0x00, 0x15, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x3C,
        0xA1, 0xA1, 0xF1, 0x9F, 0x81, 0x81, 0x81, 0x81, 0x09, 0x44, 0x80, 0x30, 0x00, 0x20, 0x48,
        0x02, 0x1B, 0xA1, 0x71, 0x64, 0x29, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
        0x42, 0x60, 0x82,
    ];

    /// Build a minimal but genuine `.app` bundle in a temporary directory.
    ///
    /// A real bundle rather than a stubbed reader: this is the one test that
    /// proves the whole import path — the emulator's plist reading, the icon
    /// decode, the identity, the duplicate check — actually fits together.
    /// It carries no proprietary content, so it runs anywhere.
    fn write_test_bundle(name: &str, identifier: &str, version: &str) -> PathBuf {
        let bundle = std::env::temp_dir()
            .join(format!("tapHLE-gui-test-{name}"))
            .join(format!("{name}.app"));
        let _ = std::fs::remove_dir_all(bundle.parent().unwrap());
        std::fs::create_dir_all(&bundle).unwrap();
        std::fs::write(bundle.join("Icon.png"), TINY_PNG).unwrap();
        std::fs::write(bundle.join(name), b"not really an executable").unwrap();

        let mut plist = plist::dictionary::Dictionary::new();
        for (key, value) in [
            ("CFBundleIdentifier", identifier),
            ("CFBundleVersion", version),
            ("CFBundleShortVersionString", "1.2"),
            ("CFBundleDisplayName", "Test Game"),
            ("CFBundleName", name),
            ("CFBundleExecutable", name),
            ("CFBundleIconFile", "Icon.png"),
            ("MinimumOSVersion", "3.0"),
        ] {
            plist.insert(key.to_string(), plist::Value::String(value.to_string()));
        }
        plist.insert(
            "UIDeviceFamily".to_string(),
            plist::Value::Array(vec![plist::Value::Integer(1.into())]),
        );
        plist::Value::Dictionary(plist)
            .to_file_xml(bundle.join("Info.plist"))
            .unwrap();
        bundle
    }

    /// The whole import path over a real bundle: read it, add it, and refuse
    /// to add it twice.
    /// An app already in the library is a duplicate on every later scan, so
    /// nothing reads its icon again. If the cached copy disappears the icon
    /// has to be rebuilt from the app itself, or the placeholder letter is
    /// permanent — which is what a library copied without its `icons`
    /// directory looks like.
    #[test]
    fn a_missing_icon_cache_is_rebuilt_from_the_app() {
        let bundle = write_test_bundle("Rebuildable", "com.example.rebuildable", "1.0");
        let icons = std::env::temp_dir().join("tapHLE-gui-test-rebuild-icons");
        let _ = std::fs::remove_dir_all(&icons);
        let mut library = Library::default();
        library.absorb(read_for_import(&bundle), &icons);

        let entry = &library.entries[0];
        let name = entry.icon_cache.clone().expect("the import should cache");
        let path = entry.path.clone();
        let cached = metadata::read_icon_cache(&icons, &name).expect("cache should be readable");

        std::fs::remove_file(icons.join(&name)).unwrap();
        assert!(
            metadata::read_icon_cache(&icons, &name).is_none(),
            "the cache file should be gone"
        );

        let rebuilt = metadata::icon_or_rebuild(&icons, &name, &path)
            .expect("the icon should come back from the app itself");
        assert_eq!(
            (rebuilt.width, rebuilt.height),
            (cached.width, cached.height)
        );
        assert_eq!(rebuilt.rgba, cached.rgba);
        assert!(
            metadata::read_icon_cache(&icons, &name).is_some(),
            "rebuilding should put the cache back, so the next launch is cheap"
        );

        let _ = std::fs::remove_dir_all(bundle.parent().unwrap());
        let _ = std::fs::remove_dir_all(&icons);
    }

    /// An app whose file has gone cannot have its icon rebuilt, and asking
    /// must fail quietly rather than panicking on the missing path.
    #[test]
    fn an_icon_cannot_be_rebuilt_from_an_app_that_is_gone() {
        let icons = std::env::temp_dir().join("tapHLE-gui-test-noapp-icons");
        let _ = std::fs::remove_dir_all(&icons);
        std::fs::create_dir_all(&icons).unwrap();
        let gone = std::env::temp_dir().join("tapHLE-gui-test-no-such-app.ipa");
        let _ = std::fs::remove_file(&gone);
        assert!(metadata::icon_or_rebuild(&icons, "whatever.icon", &gone).is_none());
        let _ = std::fs::remove_dir_all(&icons);
    }

    #[test]
    fn a_real_bundle_imports_once_and_is_then_a_duplicate() {
        let bundle = write_test_bundle("Importable", "com.example.importable", "1.0");
        let icons = std::env::temp_dir().join("tapHLE-gui-test-icons");
        let _ = std::fs::remove_dir_all(&icons);
        let mut library = Library::default();

        let outcome = library.absorb(read_for_import(&bundle), &icons);
        assert!(
            outcome.is_success(),
            "the bundle should import, got {outcome:?}"
        );
        assert_eq!(library.entries.len(), 1);

        let entry = &library.entries[0];
        assert_eq!(entry.id, "com.example.importable@1.0");
        assert_eq!(entry.title(), "Test Game");
        assert_eq!(entry.metadata.version_for_display(), "1.2");
        assert_eq!(entry.metadata.minimum_os_version.as_deref(), Some("3.0"));
        assert_eq!(
            entry.metadata.device_family_summary(),
            "iPhone / iPod touch"
        );
        assert!(
            entry.icon_cache.is_some(),
            "the icon should have been read and cached"
        );

        // The same file again is the same app, not a second copy.
        let again = library.absorb(read_for_import(&bundle), &icons);
        assert!(matches!(again, ImportOutcome::Duplicate { .. }));
        assert_eq!(library.entries.len(), 1);

        let _ = std::fs::remove_dir_all(bundle.parent().unwrap());
        let _ = std::fs::remove_dir_all(&icons);
    }

    /// A bundle without the keys tapHLE identifies an app by has to be
    /// refused with a sentence about the app, not a caught panic from inside
    /// the reader.
    #[test]
    fn a_bundle_missing_its_identity_is_refused_clearly() {
        let bundle = std::env::temp_dir()
            .join("tapHLE-gui-test-nameless")
            .join("Nameless.app");
        let _ = std::fs::remove_dir_all(bundle.parent().unwrap());
        std::fs::create_dir_all(&bundle).unwrap();
        let mut plist = plist::dictionary::Dictionary::new();
        plist.insert(
            "CFBundleName".to_string(),
            plist::Value::String("Nameless".to_string()),
        );
        plist::Value::Dictionary(plist)
            .to_file_xml(bundle.join("Info.plist"))
            .unwrap();

        let mut library = Library::default();
        let outcome = library.absorb(read_for_import(&bundle), &std::env::temp_dir());
        match &outcome {
            ImportOutcome::Failed { reason, .. } => {
                assert!(
                    reason.contains("CFBundleIdentifier"),
                    "the message should name the missing key, got {reason:?}"
                );
            }
            other => panic!("expected a clear failure, got {other:?}"),
        }
        assert!(library.entries.is_empty());
        let _ = std::fs::remove_dir_all(bundle.parent().unwrap());
    }

    /// Importing the same app twice must not create a second row; the
    /// library keys on the app's identity, not on where the file is.
    #[test]
    fn a_duplicate_is_recognized_by_identity() {
        let library = library_of(vec![entry("A", "com.a")]);
        assert!(library.find("com.a@1.0").is_some());
        let outcome = ImportOutcome::Duplicate {
            id: "com.a@1.0".to_string(),
            path_updated: false,
        };
        assert_eq!(outcome.entry_id(), Some("com.a@1.0"));
        assert!(!outcome.is_success());
        assert!(outcome
            .describe(&library)
            .contains("already in the library"));
    }
}
