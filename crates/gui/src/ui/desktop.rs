/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The window, composed for a pointer and a large screen.
//!
//! Everything here assumes things a desktop has and a phone does not: a
//! pointer that can hover, enough width for a details panel beside the
//! library, a menu bar, resizable panels, and a keyboard that is always
//! present. Its counterpart is [crate::ui::mobile].

pub mod chrome;
pub mod control_editor;
pub mod details;
pub mod dialogs;
pub mod library_view;
pub mod logpanel;
pub mod settings_dialog;
