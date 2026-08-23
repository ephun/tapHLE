/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The interface.
//!
//! Split by **form factor**, not by operating system. Windows, Linux and
//! macOS draw the same window — egui paints every pixel itself, so there is
//! nothing for a desktop OS to differ about — and iOS and Android share the
//! mobile composition for the same reason. What does differ per OS is
//! shell-level: where files live, how a folder is opened, how a picker is
//! raised. That lives in [crate::platform], as `#[cfg]` inside one file per
//! concern rather than as a folder per OS.
//!
//! So there are two leaves here and there will only ever be a handful:
//!
//! - [theme] is the visual identity — palette, type scale, spacing. Shared,
//!   and the reason a phone and a desktop look like the same product.
//! - [widgets] are the pieces both are built from.
//! - [desktop] composes them for a pointer and a large screen.
//! - [mobile] composes them for a touchscreen and a small one.
//!
//! Nothing under [theme] or [widgets] may know which leaf is drawing it, and
//! neither leaf holds state: both read [crate::state] and both report a
//! [crate::state::Action]. That is what keeps a mobile screen a different
//! arrangement of the same product rather than a second implementation of it.

pub mod desktop;
pub mod mobile;
pub mod theme;
pub mod widgets;
