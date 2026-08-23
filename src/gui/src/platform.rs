/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! What the operating system does differently.
//!
//! Split by OS rather than by form factor, which is the opposite of
//! [crate::ui] and the reason the two are not nested inside one another.
//! Windows, Linux and macOS share every pixel of the interface and disagree
//! about where a user's files live; iOS and Android share the mobile
//! interface and disagree about the same thing.
//!
//! The differences are small enough to be `#[cfg]` inside one file per
//! concern. A folder per operating system would mean five copies of a
//! three-line function.

pub mod http;
pub mod process;
pub mod storage;
