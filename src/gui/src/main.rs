/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! tapHLE's desktop frontend.
//!
//! This is a separate program from the emulator, not a mode of it. The
//! emulator owns its process — it maps guest memory, drives an SDL event loop
//! and ends a run by calling `exit` — so the library window cannot live in
//! the same one. The frontend launches `tapHLE` as a child process with the
//! same arguments a person would type, and reads its output back. See
//! `dev-docs/gui-architecture.md` for the whole reasoning.
//!
//! Both programs are built from the same workspace and share the emulator
//! library, so app bundles, launch options and tapHLE's file locations have
//! exactly one implementation between them.

// The crate is named after the project, which is not snake case.
#![allow(non_snake_case)]
// A frontend must not open a console window. This is unconditional rather
// than release-only so a debug build behaves the way the shipped one does;
// the frontend's own diagnostics go to its log panel and to
// tapHLE_frontend/frontend_log.txt rather than to a terminal.
#![windows_subsystem = "windows"]

mod app;
mod compat;
mod http;
mod launcher;
mod library;
mod logstore;
mod metadata;
mod process;
mod settings;
mod storage;
mod theme;
mod timefmt;
mod ui;
mod updates;

use std::io::Write;

/// The window's size on a first run.
///
/// Wide enough for five columns of icons beside the details panel, and short
/// enough to fit on a 768-pixel-tall display with room for a taskbar.
const DEFAULT_SIZE: [f32; 2] = [1120.0, 720.0];
/// Below this the details panel and the library cannot both be useful.
const MINIMUM_SIZE: [f32; 2] = [720.0, 440.0];

fn main() -> eframe::Result<()> {
    let (data_dir, notes) = storage::locate_data_dir();
    install_panic_hook();

    let state: settings::UiState = storage::load(storage::STATE_FILE).unwrap_or_default();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("tapHLE")
        .with_app_id("net.ephun.tapHLE")
        .with_inner_size(state.window_size.unwrap_or(DEFAULT_SIZE))
        .with_min_inner_size(MINIMUM_SIZE);
    if let Some(position) = state.window_position {
        // Only restored when it lands somewhere plausible: a saved position
        // from a monitor that is no longer attached would put the window
        // where it cannot be reached.
        if position[0] > -20_000.0 && position[1] > -20_000.0 {
            viewport = viewport.with_position(position);
        }
    }
    if state.maximized {
        viewport = viewport.with_maximized(true);
    }
    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    eframe::run_native(
        "tapHLE",
        eframe::NativeOptions {
            viewport,
            vsync: true,
            ..Default::default()
        },
        Box::new(move |cc| Ok(Box::new(app::Frontend::new(cc, data_dir, notes)))),
    )
}

/// The project's own icon, used for the window and the taskbar.
///
/// It is read from the `res` folder beside the program when there is one, and
/// otherwise from the repository, so a build tree and an installed copy both
/// find it. A missing icon is not worth failing over.
fn load_window_icon() -> Option<egui::IconData> {
    let candidates = [
        storage::data_dir().join("res/icon.png"),
        std::path::PathBuf::from("res/icon.png"),
    ];
    let bytes = candidates
        .iter()
        .find_map(|path| std::fs::read(path).ok())?;
    // tapHLE's own image decoder, which is already linked, rather than a
    // second one: it also reads the CgBI variant of PNG that Apple's tools
    // produce, which is what app icons are.
    let bitmap = tapHLE::app_bundle::decode_image(&bytes).ok()?;
    Some(egui::IconData {
        rgba: bitmap.rgba,
        width: bitmap.width,
        height: bitmap.height,
    })
}

/// Record a panic where it can be read afterwards.
///
/// A windowed program has nowhere to print to, so without this a crash in the
/// frontend would leave nothing at all. The file sits beside the emulator's
/// own log, and the message also reaches the native error box so the window
/// disappearing is at least explained.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = format!(
            "tapHLE frontend panicked: {info}\n{}\n",
            std::backtrace::Backtrace::force_capture()
        );
        if let Ok(dir) = storage::ensure_frontend_dir() {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join(storage::LOG_FILE))
            {
                let _ = writeln!(
                    file,
                    "{} {message}",
                    timefmt::format_datetime(timefmt::now_seconds())
                );
            }
        }
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("tapHLE")
            .set_description(format!(
                "The tapHLE frontend has stopped.\n\n{info}\n\nDetails were written \
                 to {}/{}.",
                storage::DIR,
                storage::LOG_FILE
            ))
            .show();
        previous(info);
    }));
}
