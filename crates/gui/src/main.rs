/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The thin desktop entry point for tapHLE’s shared egui frontend.

#![windows_subsystem = "windows"]

fn main() -> Result<(), String> {
    tapHLE_gui::run()
}
