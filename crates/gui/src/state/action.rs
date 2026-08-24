/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! What the person asked for, as data.
//!
//! Every part of the interface reports intent as an [Action] rather than
//! acting on it. The window is built first and the actions are applied
//! afterwards, in [crate::app]. That is not ceremony: an immediate-mode
//! interface is drawn while the state it describes is borrowed, so a menu
//! item that removed a library entry mid-draw would be reaching into the list
//! it is iterating. Collecting intent and applying it once keeps every one of
//! those cases out of the interface code.
//!
//! It lives beside the state rather than in [crate::ui] because it is the
//! vocabulary the two form factors share. A desktop toolbar button and a
//! mobile sheet raise the same [Action], and [crate::app] cannot tell which
//! of them sent it.

use crate::state::settings::{IconSize, SortOrder, ViewMode};

/// Something the person asked for.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    AddApps,
    AddFolder,
    RefreshLibrary,
    Play(String),
    StopAll,
    Select(String),
    OpenGlobalSettings,
    OpenAppSettings(String),
    RemoveFromLibrary(String),
    /// Remove without asking again, which is what the confirmation sends.
    ConfirmedRemove(String),
    ToggleFavorite(String),
    /// Show a different version of an app: its bundle identifier, and the
    /// entry to show for it.
    ChooseVersion {
        bundle_identifier: String,
        entry_id: String,
    },
    CopyText(String),
    OpenPath(std::path::PathBuf),
    OpenUrl(String),
    OpenCompatibilityEntry(String),
    OpenCompatibilityReport(String),
    SetLocalRating(String, Option<u8>),
    ShowAbout,
    ShowLogPanel(bool),
    ToggleLogPanel,
    SetViewMode(ViewMode),
    SetIconSize(IconSize),
    SetSortOrder(SortOrder),
    ToggleSortDirection,
    ToggleFavoritesOnly,
    ClearLog,
    SaveLog,
    CopyLogSelection,
    CopyDiagnostics(String),
    CheckForUpdates,
    RefreshCompatibility,
    OpenUserDataFolder,
    OpenAppsFolder,
    Quit,
    /// Close even though apps are running, which is what the confirmation
    /// shown on close sends.
    ConfirmedQuit,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Actions are compared when deciding whether the same request arrived
    /// twice in a frame, so they have to compare by value.
    #[test]
    fn actions_compare_by_value() {
        assert_eq!(
            Action::Play("com.x@1".to_string()),
            Action::Play("com.x@1".to_string())
        );
        assert_ne!(
            Action::Play("com.x@1".to_string()),
            Action::Play("com.y@1".to_string())
        );
    }
}
