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
//! `docs/architecture.md` for the whole reasoning.
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
mod platform;
mod run;
mod shell;
mod state;
mod ui;

use std::io::Write;

/// The window's size on a first run.
///
/// Wide enough for five columns of icons beside the details panel, and short
/// enough to fit on a 768-pixel-tall display with room for a taskbar.
const DEFAULT_SIZE: [f32; 2] = [1120.0, 720.0];
/// Below this the details panel and the library cannot both be useful.
const MINIMUM_SIZE: [f32; 2] = [720.0, 440.0];

fn main() -> Result<(), String> {
    // Before anything prints: the frontend is a windowed program, so it has no
    // console unless it borrows the one it was started from.
    let on_a_terminal = platform::console::attach_to_parent();

    let (data_dir, notes) = platform::storage::locate_data_dir();
    install_panic_hook();

    let state: state::settings::UiState =
        platform::storage::load(platform::storage::STATE_FILE).unwrap_or_default();
    let settings = shell::WindowSettings {
        title: "tapHLE".to_string(),
        size: state.window_size.unwrap_or(DEFAULT_SIZE),
        minimum_size: MINIMUM_SIZE,
        position: state.window_position,
        maximized: state.maximized,
        icon: load_window_icon(),
    };

    shell::run(settings, move |ctx| {
        let frontend = app::Frontend::new(ctx, data_dir, notes);
        // Somebody who started this from a prompt is asking to watch the log
        // go by, rather than to read it in the panel afterwards.
        frontend.mirror_log_to_stderr(on_a_terminal);
        frontend
    })
}

/// The project's own icon, used for the window and the taskbar.
///
/// It is read from the `res` folder beside the program when there is one, and
/// otherwise from the repository, so a build tree and an installed copy both
/// find it. A missing icon is not worth failing over.
fn load_window_icon() -> Option<(Vec<u8>, u32, u32)> {
    let candidates = [
        platform::storage::data_dir().join("res/icon.png"),
        std::path::PathBuf::from("res/icon.png"),
    ];
    let bytes = candidates
        .iter()
        .find_map(|path| std::fs::read(path).ok())?;
    // tapHLE's own image decoder, which is already linked, rather than a
    // second one: it also reads the CgBI variant of PNG that Apple's tools
    // produce, which is what app icons are.
    let bitmap = tapHLE::app_bundle::decode_image(&bytes).ok()?;
    Some((bitmap.rgba, bitmap.width, bitmap.height))
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
        if let Ok(dir) = platform::storage::ensure_frontend_dir() {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join(platform::storage::LOG_FILE))
            {
                let _ = writeln!(
                    file,
                    "{} {message}",
                    state::timefmt::format_datetime(state::timefmt::now_seconds())
                );
            }
        }
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("tapHLE")
            .set_description(format!(
                "The tapHLE frontend has stopped.\n\n{info}\n\nDetails were written \
                 to {}/{}.",
                platform::storage::DIR,
                platform::storage::LOG_FILE
            ))
            .show();
        previous(info);
    }));
}
