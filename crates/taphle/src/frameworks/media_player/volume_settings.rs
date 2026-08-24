/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPVolumeSettingsAlert`.
//!
//! The system volume HUD: the panel iPhone OS puts on screen when an app asks
//! for it, showing the ringer or media volume with the hardware buttons wired
//! up to it. Apps that own the whole screen show it so the user can find the
//! volume control without leaving the game.
//!
//! tapHLE has no such panel, and no hardware volume buttons to attach to one.
//! So these do nothing and report that nothing is visible, which is the honest
//! answer rather than a stub: an app that asks whether the alert is showing —
//! typically to pause, or to avoid drawing under it — gets a correct "no" and
//! carries on. What the user loses is the volume control itself, which tapHLE
//! never had here.
//!
//! Resources:
//! - These are undocumented. They are declared in `MPVolumeSettingsAlert.h` in
//!   the MediaPlayer framework headers of the iPhone OS SDK.

use crate::dyld::{export_c_func, FunctionExports};
use crate::Environment;

fn MPVolumeSettingsAlertShow(_env: &mut Environment) {
    log_dbg!("MPVolumeSettingsAlertShow(): tapHLE has no volume HUD, ignoring");
}

fn MPVolumeSettingsAlertHide(_env: &mut Environment) {
    log_dbg!("MPVolumeSettingsAlertHide(): tapHLE has no volume HUD, ignoring");
}

fn MPVolumeSettingsAlertIsVisible(_env: &mut Environment) -> bool {
    false
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(MPVolumeSettingsAlertShow()),
    export_c_func!(MPVolumeSettingsAlertHide()),
    export_c_func!(MPVolumeSettingsAlertIsVisible()),
];
