/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! What the interface is about, with no opinion on how it is drawn.
//!
//! The library and its metadata, the settings, the log, the compatibility
//! database, the update check — and [Action], the vocabulary every part of
//! the interface reports intent in. Nothing here may mention egui or a form
//! factor: a phone and a desktop show the same things differently, so the
//! things themselves are shared and only the arrangement is not.

pub mod action;
pub mod compat;
pub mod library;
pub mod logstore;
pub mod metadata;
pub mod settings;
pub mod timefmt;
pub mod updates;

pub use action::Action;
