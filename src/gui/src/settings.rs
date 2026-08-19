/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! How the frontend itself is set up, and a door onto the emulator settings
//! it edits.
//!
//! The emulator settings are not defined here. They live in
//! [tapHLE::settings] because the emulator has to read the same settings the
//! frontend writes, and two definitions of one type is how the two drift
//! apart. This module keeps only what is genuinely the frontend's own: how
//! the library is laid out, which columns it sorts by, and where its windows
//! were.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The emulator settings, re-exported so the rest of the frontend can keep
/// saying `crate::settings::EmulatorSettings`.
pub use tapHLE::settings::{
    DeviceFamilyPref, EmulatorSettings, FrameRateLimit, Gles1Pref, OrientationPref,
};

/// How the library is laid out. A compact list is the alternative to the
/// grid, so the library code is written against the entry order rather than
/// against a grid.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum IconSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl IconSize {
    pub const ALL: &'static [IconSize] = &[IconSize::Small, IconSize::Medium, IconSize::Large];

    pub fn label(self) -> &'static str {
        match self {
            IconSize::Small => "Small",
            IconSize::Medium => "Medium",
            IconSize::Large => "Large",
        }
    }

    /// Points, before display scaling. 57 is the size of an iPhone OS icon,
    /// and 72 the iPad's, so the three sizes bracket the originals.
    pub fn points(self) -> f32 {
        match self {
            IconSize::Small => 48.0,
            IconSize::Medium => 64.0,
            IconSize::Large => 88.0,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SortOrder {
    #[default]
    Title,
    Publisher,
    RecentlyPlayed,
    Compatibility,
    DateAdded,
}

impl SortOrder {
    pub const ALL: &'static [SortOrder] = &[
        SortOrder::Title,
        SortOrder::Publisher,
        SortOrder::RecentlyPlayed,
        SortOrder::Compatibility,
        SortOrder::DateAdded,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SortOrder::Title => "Title",
            SortOrder::Publisher => "Publisher",
            SortOrder::RecentlyPlayed => "Recently played",
            SortOrder::Compatibility => "Compatibility",
            SortOrder::DateAdded => "Date added",
        }
    }
}

/// Everything in `settings.json`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FrontendSettings {
    /// Emulator defaults for every app in the library.
    pub emulator: EmulatorSettings,
    /// Folders scanned for apps. Empty means the standard `tapHLE_apps`.
    pub library_folders: Vec<PathBuf>,
    /// An explicitly chosen emulator executable, for an unusual layout.
    pub emulator_path: Option<PathBuf>,
    /// Open the log panel by itself when a run ends badly.
    pub reveal_log_on_crash: bool,
    /// Keep the log panel open and show the developer-facing extras.
    pub developer_mode: bool,
    /// Ask GitHub for a newer release at startup.
    pub check_for_updates: bool,
    /// Ask before taking an app out of the library.
    pub confirm_remove: bool,
    pub log_capacity: usize,
    pub log_show_timestamps: bool,
    /// Interface zoom on top of the display's own scaling.
    pub ui_zoom: f32,
}

impl Default for FrontendSettings {
    fn default() -> Self {
        FrontendSettings {
            emulator: EmulatorSettings::default(),
            library_folders: Vec::new(),
            emulator_path: None,
            reveal_log_on_crash: true,
            developer_mode: false,
            check_for_updates: true,
            confirm_remove: true,
            log_capacity: crate::logstore::DEFAULT_CAPACITY,
            log_show_timestamps: true,
            ui_zoom: 1.0,
        }
    }
}

/// Everything in `state.json`: where the window was and what was on screen,
/// kept apart from settings because it changes constantly and means nothing
/// on another machine.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct UiState {
    pub window_size: Option<[f32; 2]>,
    pub window_position: Option<[f32; 2]>,
    pub maximized: bool,
    pub log_panel_visible: bool,
    pub log_panel_height: f32,
    pub details_panel_width: f32,
    pub selected_app: Option<String>,
    pub view_mode: ViewMode,
    pub icon_size: IconSize,
    pub sort_order: SortOrder,
    pub sort_descending: bool,
    pub favorites_only: bool,
    /// Which version of an app the library shows, keyed by bundle identifier.
    /// Absent means the newest one, which is what a fresh library shows.
    pub chosen_versions: std::collections::HashMap<String, String>,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            window_size: None,
            window_position: None,
            maximized: false,
            log_panel_visible: false,
            log_panel_height: 220.0,
            details_panel_width: 320.0,
            selected_app: None,
            view_mode: ViewMode::default(),
            icon_size: IconSize::default(),
            sort_order: SortOrder::default(),
            sort_descending: false,
            favorites_only: false,
            chosen_versions: std::collections::HashMap::new(),
        }
    }
}
