/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The program: state, the frame, and what each action does.
//!
//! The window is assembled from the outside in — menu bar, toolbar, status
//! bar, log panel, details panel, and the library in what is left. That order
//! is what puts the status bar below the log panel and the log panel across
//! the full width, which is where a person expects to find them.
//!
//! Slow work never happens on this thread. Reading an app's archive, asking
//! GitHub about releases and fetching compatibility ratings all run on worker
//! threads and arrive as [Background] messages.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::platform::http::{CurlTransport, Transport};
use crate::platform::storage;
use crate::run::launcher::{self, Launcher};
use crate::state::compat::{
    self, CompatibilityProvider, DatabaseSnapshot, ReportDraft, TapHledbProvider,
};
use crate::state::library::{self, ImportOutcome, Library, ScanResult, VersionGroup, ViewFilter};
use crate::state::logstore::{self, LogLevel, SharedLog};
use crate::state::metadata::AppIcon;
use crate::state::settings::{EmulatorSettings, FrontendSettings, UiState};
use crate::state::timefmt;
use crate::state::updates::{self, GitHubReleaseProvider, ReleaseProvider, UpdateStatus};
use crate::state::Action;
use crate::ui::desktop::chrome::ChromeContext;
use crate::ui::desktop::details::DetailsContext;
use crate::ui::desktop::dialogs::{
    AboutDialog, AboutInfo, Confirmation, CrashNotice, ImportReport, ReportDialog,
};
use crate::ui::desktop::library_view::LibraryContext;
use crate::ui::desktop::logpanel::LogView;
use crate::ui::desktop::settings_dialog::{AppDialog, Category, GlobalDialog, Outcome};
use crate::ui::theme;

/// A result from a worker thread.
enum Background {
    /// A cached icon, ready to be uploaded.
    Icon {
        id: String,
        icon: Box<AppIcon>,
    },
    /// Apps read from disk during a scan or an import.
    Scanned {
        results: Vec<ScanResult>,
        /// Whether to report the outcome. A startup scan should not open a
        /// dialog about apps that were already there.
        report: bool,
    },
    Compatibility(Result<DatabaseSnapshot, String>),
    Update(UpdateStatus),
    Note(LogLevel, String),
}

/// A channel to the interface that also wakes it.
struct BackgroundSender {
    sender: Sender<Background>,
    repaint: egui::Context,
}

impl BackgroundSender {
    fn send(&self, message: Background) {
        // A closed channel means the window has gone; there is nothing to
        // report it to, and the thread is about to end anyway.
        if self.sender.send(message).is_ok() {
            self.repaint.request_repaint();
        }
    }
}

/// How often state is written back to disk while the program runs.
const SAVE_INTERVAL: Duration = Duration::from_secs(3);
/// How many recent lines a crash notice and a diagnostics copy carry.
const EXCERPT_LINES: usize = 40;

pub struct Frontend {
    settings: FrontendSettings,
    state: UiState,
    library: Library,
    log: SharedLog,
    launcher: Launcher,
    icons: HashMap<String, egui::TextureHandle>,
    database: DatabaseSnapshot,
    database_available: bool,
    update: UpdateStatus,
    search: String,
    data_dir: PathBuf,
    transport: Arc<dyn Transport>,
    log_view: LogView,

    global_dialog: Option<GlobalDialog>,
    app_dialog: Option<AppDialog>,
    control_editor: Option<crate::ui::desktop::control_editor::ControlEditor>,
    about: Option<AboutDialog>,
    import_report: Option<ImportReport>,
    crash: Option<CrashNotice>,
    report: Option<ReportDialog>,
    confirmation: Option<Confirmation>,

    background: (Sender<Background>, Receiver<Background>),
    /// A handle used only to wake the interface when a worker thread has
    /// something. Without it the frame loop sleeps until the next mouse
    /// movement, and a library scan or a rating fetch would sit unread in
    /// the channel until somebody happened to touch the window.
    repaint: egui::Context,
    /// Bumped whenever the library changes, so the sorted order is rebuilt
    /// exactly when it needs to be and not every frame.
    library_revision: u64,
    order_cache: Vec<usize>,
    /// The same order with each app's versions collapsed into one place.
    group_cache: Vec<VersionGroup>,
    order_key: String,
    dirty: Dirty,
    last_save: Instant,
    /// Set once the person has agreed to close with apps still running.
    closing_confirmed: bool,
    applied_zoom: f32,
}

#[derive(Default)]
struct Dirty {
    library: bool,
    settings: bool,
    state: bool,
}

impl Frontend {
    pub fn new(egui_ctx: &egui::Context, data_dir: PathBuf, notes: Vec<String>) -> Self {
        let log = logstore::new_shared();
        let settings: FrontendSettings = report_load(&log, storage::SETTINGS_FILE);
        let state: UiState = report_load(&log, storage::STATE_FILE);
        let mut library: Library = report_load(&log, storage::LIBRARY_FILE);
        library.mark_missing();

        // The shared settings file is the store; the fields still read out of
        // settings.json and library.json are only there to carry an existing
        // install into it. Where the shared file has an answer it wins, and
        // where it does not the older value stays and gets written out on the
        // first save.
        let mut settings = settings;
        match storage::load_shared_settings() {
            Ok(shared) => {
                if shared != Default::default() {
                    settings.emulator = shared.global.clone();
                    for entry in library.entries.iter_mut() {
                        entry.overrides = shared.apps.get(&entry.id).cloned().unwrap_or_default();
                    }
                }
            }
            Err(e) => logstore::note(&log, LogLevel::Warning, e),
        }

        for note in notes {
            logstore::note(&log, LogLevel::Warning, note);
        }
        logstore::note(
            &log,
            LogLevel::Info,
            format!(
                "tapHLE {} frontend started in {}",
                tapHLE_version::VERSION.trim(),
                storage::display_path(&data_dir)
            ),
        );

        if let Ok(mut store) = log.lock() {
            store.set_capacity(settings.log_capacity);
        }
        theme::apply(egui_ctx, settings.ui_zoom);

        let transport: Arc<dyn Transport> = Arc::new(CurlTransport);
        let mut frontend = Frontend {
            log_view: {
                let mut view = LogView::default();
                view.show_timestamps = settings.log_show_timestamps;
                view
            },
            launcher: Launcher::new(log.clone()),
            applied_zoom: settings.ui_zoom,
            settings,
            state,
            library,
            log,
            icons: HashMap::new(),
            database: report_load_or_default(storage::COMPAT_CACHE_FILE),
            database_available: false,
            update: UpdateStatus::NotChecked,
            search: String::new(),
            data_dir,
            transport,
            global_dialog: None,
            app_dialog: None,
            control_editor: None,
            about: None,
            import_report: None,
            crash: None,
            report: None,
            confirmation: None,
            background: channel(),
            repaint: egui_ctx.clone(),
            library_revision: 0,
            order_cache: Vec::new(),
            group_cache: Vec::new(),
            order_key: String::new(),
            dirty: Dirty::default(),
            last_save: Instant::now(),
            closing_confirmed: false,
        };
        frontend.database_available = !frontend.database.is_empty();
        if frontend.settings.developer_mode {
            frontend.state.log_panel_visible = true;
        }
        frontend.load_cached_icons();
        frontend.rescan_library(false);
        frontend.refresh_compatibility();
        if frontend.settings.check_for_updates {
            frontend.check_for_updates();
        } else {
            frontend.update = UpdateStatus::Disabled;
        }
        frontend
    }

    /// A sender that wakes the interface after each message.
    fn background_sender(&self) -> BackgroundSender {
        BackgroundSender {
            sender: self.background.0.clone(),
            repaint: self.repaint.clone(),
        }
    }

    /// The in-memory settings, in the shape they are stored in.
    ///
    /// Built rather than kept, so there is no second copy to fall out of step
    /// with what the interface is editing.
    fn shared_settings(&self) -> tapHLE::settings::SettingsFile {
        let mut file = tapHLE::settings::SettingsFile {
            global: self.settings.emulator.clone(),
            ..Default::default()
        };
        for entry in &self.library.entries {
            if !entry.overrides.is_empty() {
                file.apps.insert(entry.id.clone(), entry.overrides.clone());
            }
        }
        file
    }

    /// Open the control editor for whichever app's settings are showing.
    ///
    /// The canvas has to be the shape of the app's own screen, so the device
    /// family and orientation are resolved here the same way the emulator
    /// resolves them. An app that says nothing about either gets the iPhone's
    /// portrait screen, which is what the emulator would give it too.
    fn open_control_editor(&mut self) {
        let Some(dialog) = &mut self.app_dialog else {
            return;
        };
        dialog.open_control_editor = false;
        let entry_id = dialog.entry_id.clone();
        let title = dialog.title.clone();
        let draft = dialog.draft.controls.clone().unwrap_or_default();
        let inherited = dialog.inherited.controls.clone().unwrap_or_default();

        // What the app itself says it runs on, so the canvas is the screen
        // the emulator will actually use rather than the one preferred.
        let supported: Vec<tapHLE::controls::GuestDevice> = self
            .library
            .find(&entry_id)
            .map(|entry| {
                entry
                    .metadata
                    .device_families
                    .iter()
                    .filter_map(|family| match family.as_str() {
                        "iPhone" => Some(tapHLE::controls::GuestDevice::iPhone),
                        "iPad" => Some(tapHLE::controls::GuestDevice::iPad),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let screen = self.effective_settings(&entry_id).guest_screen(&supported);

        self.control_editor = Some(crate::ui::desktop::control_editor::ControlEditor::new(
            entry_id, title, draft, inherited, screen,
        ));
    }

    /// Run the app so the editor can take a picture of it to draw on.
    ///
    /// The app stays up until somebody asks for the picture, because only
    /// they can tell a title card from the screen they want to map.
    ///
    /// Its own short-lived process rather than the one Play starts: this is
    /// not a play session and must not count as one, and the capture has to
    /// be armed by environment variables set before launch, so an app that is
    /// already running could not be asked anyway.
    fn start_control_capture(&mut self) {
        let Some(editor) = &mut self.control_editor else {
            return;
        };
        editor.wants_capture = false;
        let entry_id = editor.entry_id.clone();

        let Some(entry) = self.library.find(&entry_id) else {
            return;
        };
        let app_path = entry.path.clone();
        let missing = entry.missing;
        if missing {
            self.note(
                LogLevel::Error,
                "That app's file is not where the library expects it, so its                  screen cannot be captured."
                    .to_string(),
            );
            return;
        }
        let Some(emulator) = self.emulator_path() else {
            self.note(
                LogLevel::Error,
                "The tapHLE emulator program could not be found. Set its                  location in Settings ▸ Paths."
                    .to_string(),
            );
            return;
        };

        // The same options a real run would get, so the picture is the screen
        // somebody will actually see rather than a differently configured one.
        let arguments = self.effective_settings(&entry_id).to_args();
        match crate::run::capture::Capture::start(
            emulator,
            app_path,
            self.data_dir.clone(),
            arguments,
        ) {
            Ok(capture) => {
                if let Some(editor) = &mut self.control_editor {
                    editor.capturing(capture);
                }
            }
            Err(e) => self.note(LogLevel::Error, e),
        }
    }

    /// The settings that apply to an app: its own over the global defaults.
    fn effective_settings(&self, entry_id: &str) -> EmulatorSettings {
        let overrides = self
            .library
            .find(entry_id)
            .map(|entry| entry.overrides.clone())
            .unwrap_or_default();
        EmulatorSettings::inherit(&self.settings.emulator, &overrides)
    }

    fn emulator_path(&self) -> Option<PathBuf> {
        launcher::find_emulator(self.settings.emulator_path.as_deref(), &self.data_dir)
    }

    fn note(&self, level: LogLevel, text: impl Into<String>) {
        logstore::note(&self.log, level, text);
    }

    fn library_folders(&self) -> Vec<PathBuf> {
        let mut folders = library::default_folders();
        for folder in &self.settings.library_folders {
            if !folders.contains(folder) {
                folders.push(folder.clone());
            }
        }
        folders
    }

    /// Read every cached icon on a worker thread, rebuilding the ones whose
    /// cache file has gone.
    fn load_cached_icons(&self) {
        let sender = self.background_sender();
        let wanted: Vec<(String, String, PathBuf)> = self
            .library
            .entries
            .iter()
            .filter_map(|entry| {
                Some((
                    entry.id.clone(),
                    entry.icon_cache.clone()?,
                    entry.path.clone(),
                ))
            })
            .collect();
        if wanted.is_empty() {
            return;
        }
        let dir = storage::icon_cache_dir();
        std::thread::spawn(move || {
            let mut rebuilt = 0_usize;
            for (id, name, path) in wanted {
                let cached = crate::state::metadata::read_icon_cache(&dir, &name).is_some();
                let Some(icon) = crate::state::metadata::icon_or_rebuild(&dir, &name, &path) else {
                    continue;
                };
                if !cached {
                    rebuilt += 1;
                }
                sender.send(Background::Icon {
                    id,
                    icon: Box::new(icon),
                });
            }
            // Rereading archives is slow enough to be worth admitting to,
            // and a cache that keeps vanishing is worth noticing.
            if rebuilt > 0 {
                sender.send(Background::Note(
                    LogLevel::Info,
                    format!(
                        "Rebuilt {rebuilt} icon{} whose cached copy was missing",
                        if rebuilt == 1 { "" } else { "s" }
                    ),
                ));
            }
        });
    }

    /// Look through the library folders for apps, on a worker thread.
    fn rescan_library(&mut self, report: bool) {
        let folders = self.library_folders();
        let sender = self.background_sender();
        std::thread::spawn(move || {
            let mut paths = Vec::new();
            for folder in folders {
                if !folder.is_dir() {
                    continue;
                }
                match library::scan_folder(&folder) {
                    Ok(found) => paths.extend(found),
                    Err(e) => {
                        sender.send(Background::Note(LogLevel::Warning, e));
                    }
                }
            }
            let results = paths.iter().map(|p| library::read_for_import(p)).collect();
            sender.send(Background::Scanned { results, report });
        });
    }

    /// Read the given files and add them, on a worker thread.
    fn import_paths(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let sender = self.background_sender();
        std::thread::spawn(move || {
            let results = paths.iter().map(|p| library::read_for_import(p)).collect();
            sender.send(Background::Scanned {
                results,
                report: true,
            });
        });
    }

    fn refresh_compatibility(&self) {
        let sender = self.background_sender();
        let provider = TapHledbProvider::new(self.transport.clone());
        std::thread::spawn(move || {
            sender.send(Background::Compatibility(provider.fetch()));
        });
    }

    fn check_for_updates(&mut self) {
        self.update = UpdateStatus::Checking;
        let sender = self.background_sender();
        let provider = GitHubReleaseProvider::new(self.transport.clone());
        std::thread::spawn(move || {
            let status = match provider.releases() {
                Ok(releases) => updates::evaluate(&releases, &updates::running_version()),
                Err(e) => UpdateStatus::Failed(e),
            };
            sender.send(Background::Update(status));
        });
    }

    fn drain_background(&mut self, ctx: &egui::Context) {
        let messages: Vec<Background> = self.background.1.try_iter().collect();
        for message in messages {
            match message {
                Background::Icon { id, icon } => self.upload_icon(ctx, &id, &icon),
                Background::Scanned { results, report } => self.absorb_scan(ctx, results, report),
                Background::Compatibility(Ok(snapshot)) => {
                    self.note(
                        LogLevel::Info,
                        format!(
                            "Read {} compatibility ratings from the database.",
                            snapshot.entries.len()
                        ),
                    );
                    self.database = snapshot;
                    self.database_available = true;
                    let _ = storage::save(storage::COMPAT_CACHE_FILE, &self.database);
                    self.invalidate_order();
                }
                Background::Compatibility(Err(e)) => {
                    self.note(
                        LogLevel::Warning,
                        format!("Could not read the compatibility database: {e}"),
                    );
                }
                Background::Update(status) => {
                    if let UpdateStatus::Failed(reason) = &status {
                        self.note(LogLevel::Warning, format!("Update check failed: {reason}"));
                    }
                    self.update = status;
                }
                Background::Note(level, text) => self.note(level, text),
            }
        }
    }

    fn upload_icon(&mut self, ctx: &egui::Context, id: &str, icon: &AppIcon) {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [icon.width as usize, icon.height as usize],
            &icon.rgba,
        );
        let texture = ctx.load_texture(format!("icon-{id}"), image, egui::TextureOptions::LINEAR);
        self.icons.insert(id.to_string(), texture);
    }

    fn absorb_scan(&mut self, ctx: &egui::Context, results: Vec<ScanResult>, report: bool) {
        let icon_dir = storage::icon_cache_dir();
        let mut outcomes = Vec::new();
        for result in results {
            // Keep the icon before the library takes ownership of the rest,
            // so a newly added app shows its icon without a restart.
            let pending = match &result {
                ScanResult::Read { read, .. } => read.icon.as_ref().map(|icon| AppIcon {
                    width: icon.width,
                    height: icon.height,
                    rgba: icon.rgba.clone(),
                }),
                _ => None,
            };
            let outcome = self.library.absorb(result, &icon_dir);
            if let (ImportOutcome::Added { id, .. }, Some(icon)) = (&outcome, pending) {
                let id = id.clone();
                self.upload_icon(ctx, &id, &icon);
            }
            outcomes.push(outcome);
        }
        if let Some(id) = outcomes
            .iter()
            .filter(|outcome| outcome.is_success())
            .find_map(|outcome| outcome.entry_id())
        {
            self.state.selected_app = Some(id.to_string());
            self.dirty.state = true;
        }
        let added = outcomes.iter().filter(|o| o.is_success()).count();
        if added > 0 {
            self.dirty.library = true;
            self.invalidate_order();
            self.note(
                LogLevel::Info,
                format!("Added {added} app{} to the library.", plural(added)),
            );
        }
        for outcome in &outcomes {
            if let ImportOutcome::Failed { path, reason } = outcome {
                self.note(
                    LogLevel::Warning,
                    format!("Could not read {}: {reason}", path.display()),
                );
            }
        }
        if report {
            let candidate = ImportReport::from_outcomes(&outcomes, &self.library);
            if candidate.worth_showing() || candidate.added == 0 {
                self.import_report = Some(candidate);
            }
        }
    }

    fn invalidate_order(&mut self) {
        self.library_revision += 1;
    }

    /// The apps to show, rebuilt only when something it depends on moved.
    ///
    /// Each is one app with its visible versions, so a collection holding
    /// four builds of one game takes one place in the library rather than
    /// four.
    fn visible_groups(&mut self) -> &[VersionGroup] {
        let key = format!(
            "{}|{}|{:?}|{}|{}|{}",
            self.library_revision,
            self.search,
            self.state.sort_order,
            self.state.sort_descending,
            self.state.favorites_only,
            self.library.entries.len()
        );
        if key != self.order_key {
            self.order_key = key;
            let filter = ViewFilter {
                search: &self.search,
                favorites_only: self.state.favorites_only,
                sort: self.state.sort_order,
                descending: self.state.sort_descending,
            };
            self.order_cache = library::visible_entries(&self.library, &filter, &self.database);
            self.group_cache = library::group_versions(
                &self.library,
                &self.order_cache,
                &self.state.chosen_versions,
            );
        }
        &self.group_cache
    }

    /// Every version of the app the selection is in, newest first.
    fn versions_of_selection(&self) -> Vec<&crate::state::library::LibraryEntry> {
        let Some(entry) = self.selected_entry() else {
            return Vec::new();
        };
        self.group_cache
            .iter()
            .find(|group| {
                group
                    .versions
                    .iter()
                    .any(|&index| self.library.entries[index].id == entry.id)
            })
            .map(|group| {
                group
                    .versions
                    .iter()
                    .map(|&index| &self.library.entries[index])
                    .collect()
            })
            .unwrap_or_else(|| vec![entry])
    }

    fn selected_entry(&self) -> Option<&crate::state::library::LibraryEntry> {
        let id = self.state.selected_app.as_deref()?;
        self.library.find(id)
    }

    /// The last lines of output, for a crash notice or a report.
    fn recent_output(&self, run_ids: &[u64]) -> String {
        let Ok(store) = self.log.lock() else {
            return String::new();
        };
        let mut lines: Vec<String> = store
            .iter()
            .filter(|line| {
                run_ids.is_empty() || line.origin.run_id().is_some_and(|id| run_ids.contains(&id))
            })
            .map(|line| line.full_text())
            .collect();
        if lines.len() > EXCERPT_LINES {
            lines.drain(..lines.len() - EXCERPT_LINES);
        }
        lines.join("\n")
    }

    fn run_ids_for(&self, entry_id: &str) -> Vec<u64> {
        self.launcher
            .running()
            .iter()
            .filter(|run| run.entry_id == entry_id)
            .map(|run| run.id)
            .collect()
    }

    fn diagnostics(&self, entry_id: &str) -> String {
        let mut text = format!(
            "tapHLE {}\nPlatform: {}\nInstallation: {}\n",
            tapHLE_version::VERSION.trim(),
            compat::platform_description(),
            self.data_dir.display()
        );
        if let Some(entry) = self.library.find(entry_id) {
            text.push_str(&format!(
                "App: {}\nBundle identifier: {}\nBundle version: {}\nOptions: {}\n",
                entry.title(),
                entry.metadata.bundle_identifier,
                entry.metadata.bundle_version,
                self.effective_settings(entry_id).to_args().join(" ")
            ));
        }
        text.push_str("\nRecent output:\n");
        text.push_str(&self.recent_output(&[]));
        text.push('\n');
        text
    }

    fn save_if_due(&mut self, force: bool) {
        if !force && self.last_save.elapsed() < SAVE_INTERVAL {
            return;
        }
        self.last_save = Instant::now();
        let shared_settings_dirty = self.dirty.library || self.dirty.settings;
        if self.dirty.library {
            if let Err(e) = storage::save(storage::LIBRARY_FILE, &self.library) {
                self.note(LogLevel::Warning, e);
            }
            self.dirty.library = false;
        }
        if self.dirty.settings {
            if let Err(e) = storage::save(storage::SETTINGS_FILE, &self.settings) {
                self.note(LogLevel::Warning, e);
            }
            self.dirty.settings = false;
        }
        // Written whenever either scope changed, because both scopes live in
        // the one file: a per-app override is a library edit and a default is
        // a settings edit, and either one makes the file out of date.
        if shared_settings_dirty {
            if let Err(e) = storage::save_shared_settings(&self.shared_settings()) {
                self.note(LogLevel::Warning, e);
            }
        }
        if self.dirty.state {
            if let Err(e) = storage::save(storage::STATE_FILE, &self.state) {
                self.note(LogLevel::Warning, e);
            }
            self.dirty.state = false;
        }
    }

    fn remember_geometry(&mut self, ctx: &egui::Context) {
        let (size, position, maximized) = ctx.input(|input| {
            let viewport = input.viewport();
            (
                viewport
                    .inner_rect
                    .map(|rect| [rect.width(), rect.height()]),
                viewport.outer_rect.map(|rect| [rect.min.x, rect.min.y]),
                viewport.maximized.unwrap_or(false),
            )
        });
        // A maximised window's size is the screen's, which is not what should
        // be restored when it is un-maximised later.
        if !maximized {
            if size.is_some() && size != self.state.window_size {
                self.state.window_size = size;
                self.dirty.state = true;
            }
            if position.is_some() && position != self.state.window_position {
                self.state.window_position = position;
                self.dirty.state = true;
            }
        }
        if maximized != self.state.maximized {
            self.state.maximized = maximized;
            self.dirty.state = true;
        }
    }

    fn collect_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.note(
                LogLevel::Info,
                format!(
                    "Adding {} dropped file{}…",
                    dropped.len(),
                    plural(dropped.len())
                ),
            );
            self.import_paths(dropped);
        }
    }

    fn collect_finished_runs(&mut self) {
        for (run, outcome) in self.launcher.poll() {
            let seconds = run.elapsed_seconds();
            self.library.record_play(&run.entry_id, seconds);
            self.dirty.library = true;
            self.invalidate_order();

            let explanation = launcher::explain_outcome(&outcome, &run.app_name);
            if outcome.is_failure() && !run.arguments.is_empty() {
                self.note(
                    LogLevel::Info,
                    format!("It was launched with: {}", run.arguments.join(" ")),
                );
            }
            let level = if outcome.is_failure() {
                LogLevel::Error
            } else {
                LogLevel::Info
            };
            self.note(
                level,
                format!(
                    "{explanation} It ran for {}.",
                    timefmt::format_duration(seconds)
                ),
            );
            if outcome.is_failure() {
                let excerpt = self.recent_output(&[run.id]);
                if self.settings.reveal_log_on_crash {
                    self.state.log_panel_visible = true;
                    self.dirty.state = true;
                }
                self.crash = Some(CrashNotice {
                    open: true,
                    entry_id: run.entry_id.clone(),
                    app_title: run.app_name.to_string(),
                    explanation,
                    excerpt,
                });
            }
            let _ = outcome;
        }
    }

    fn play(&mut self, entry_id: &str) {
        let Some(entry) = self.library.find(entry_id) else {
            return;
        };
        if entry.missing {
            self.note(
                LogLevel::Error,
                format!(
                    "{} cannot be started: {} is not there.",
                    entry.title(),
                    storage::display_path(&entry.path)
                ),
            );
            return;
        }
        let title = entry.title().to_string();
        let path = entry.path.clone();
        let settings = self.effective_settings(entry_id);
        let problems = settings.validate();
        if !problems.is_empty() {
            self.note(
                LogLevel::Error,
                format!("{title} was not started: {}", problems.join("; ")),
            );
            return;
        }
        let Some(emulator) = self.emulator_path() else {
            self.note(
                LogLevel::Error,
                "The tapHLE emulator program could not be found. Set its \
                 location in Settings ▸ Paths."
                    .to_string(),
            );
            return;
        };
        // The emulator reads the shared settings file itself, so a run gets
        // these settings whether it was started here or from a terminal. It
        // has to be on disk before the child starts, and saving is otherwise
        // throttled, so a setting changed a moment ago would not have landed.
        if let Err(e) = storage::save_shared_settings(&self.shared_settings()) {
            self.note(LogLevel::Warning, e);
        }

        let data_dir = self.data_dir.clone();
        let result = self.launcher.launch(launcher::LaunchRequest {
            emulator: &emulator,
            working_directory: &data_dir,
            entry_id,
            app_name: &title,
            app_path: &path,
            // Deliberately no settings here. Passing them as arguments is
            // what used to make the frontend's settings invisible to a
            // command-line run, and made an unset one fall through to a file
            // the frontend never showed.
            arguments: &[],
            environment: &settings.to_env(),
        });
        match result {
            Ok(_) => {
                if let Some(entry) = self.library.find_mut(entry_id) {
                    entry.last_played = Some(timefmt::now_seconds());
                }
                self.dirty.library = true;
            }
            Err(e) => self.note(LogLevel::Error, e),
        }
    }

    fn open_report(&mut self, entry_id: &str) {
        let Some(entry) = self.library.find(entry_id) else {
            return;
        };
        let draft = ReportDraft {
            display_name: entry.title().to_string(),
            bundle_identifier: entry.metadata.bundle_identifier.clone(),
            bundle_version: entry.metadata.bundle_version.clone(),
            short_version: entry.metadata.short_version.clone(),
            taphle_version: tapHLE_version::VERSION.trim().to_string(),
            taphle_build: build_description(),
            platform: compat::platform_description(),
            stars: entry.local_rating.stars,
            notes: entry.local_rating.notes.clone(),
            launch_options: self.effective_settings(entry_id).to_args(),
            log_excerpt: self.recent_output(&self.run_ids_for(entry_id)),
            existing_entry: self
                .database
                .find(&entry.metadata.bundle_identifier)
                .cloned(),
            database_consulted: self.database_available,
        };
        self.report = Some(ReportDialog {
            open: true,
            entry_id: entry_id.to_string(),
            stars: draft.stars,
            notes: draft.notes.clone(),
            include_log: true,
            draft,
        });
    }

    fn apply(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::AddApps => {
                if let Some(files) = rfd::FileDialog::new()
                    .add_filter("iPhone apps", &["ipa"])
                    .set_title("Add apps to the tapHLE library")
                    .pick_files()
                {
                    self.import_paths(files);
                }
            }
            Action::AddFolder => {
                if let Some(folder) = rfd::FileDialog::new()
                    .set_title("Add every app in a folder")
                    .pick_folder()
                {
                    match library::scan_folder(&folder) {
                        Ok(paths) if paths.is_empty() => {
                            self.note(
                                LogLevel::Warning,
                                format!("No apps were found in {}.", folder.display()),
                            );
                        }
                        Ok(paths) => self.import_paths(paths),
                        Err(e) => self.note(LogLevel::Warning, e),
                    }
                }
            }
            Action::RefreshLibrary => {
                self.library.mark_missing();
                self.rescan_library(true);
                self.invalidate_order();
            }
            Action::Play(id) => self.play(&id),
            Action::StopAll => self.launcher.stop_all(),
            Action::Select(id) => {
                if self.state.selected_app.as_deref() != Some(id.as_str()) {
                    self.state.selected_app = Some(id);
                    self.dirty.state = true;
                }
            }
            Action::OpenGlobalSettings => {
                self.global_dialog = Some(GlobalDialog::new(&self.settings));
            }
            Action::OpenAppSettings(id) => {
                if let Some(entry) = self.library.find(&id) {
                    self.app_dialog = Some(AppDialog {
                        entry_id: id.clone(),
                        title: entry.title().to_string(),
                        category: Category::Display,
                        draft: entry.overrides.clone(),
                        inherited: self.settings.emulator.clone(),
                        open_control_editor: false,
                    });
                }
            }
            Action::RemoveFromLibrary(id) => {
                if !self.settings.confirm_remove {
                    self.apply(ctx, Action::ConfirmedRemove(id));
                } else if self.confirmation.is_none() {
                    let title = self
                        .library
                        .find(&id)
                        .map(|entry| entry.title().to_string())
                        .unwrap_or_else(|| id.clone());
                    self.confirmation = Some(Confirmation {
                        open: true,
                        title: "Remove from library".to_string(),
                        message: format!(
                            "Take {title} out of the library? Its file is not deleted, \
                             but its settings, its rating and how long it has been \
                             played are forgotten."
                        ),
                        confirm_label: "Remove".to_string(),
                        action: Action::ConfirmedRemove(id),
                    });
                }
            }
            Action::ConfirmedRemove(id) => {
                if let Some(entry) = self.library.remove(&id) {
                    self.icons.remove(&id);
                    self.note(
                        LogLevel::Info,
                        format!("{} was removed from the library.", entry.title()),
                    );
                }
                if self.state.selected_app.as_deref() == Some(id.as_str()) {
                    self.state.selected_app = None;
                }
                self.dirty.library = true;
                self.dirty.state = true;
                self.invalidate_order();
            }
            Action::ToggleFavorite(id) => {
                if let Some(entry) = self.library.find_mut(&id) {
                    entry.favorite = !entry.favorite;
                    self.dirty.library = true;
                    self.invalidate_order();
                }
            }
            Action::ChooseVersion {
                bundle_identifier,
                entry_id,
            } => {
                // Selecting the version too is the point of picking one:
                // the rating, the per-app settings and the Play button all
                // act on the selection, and leaving it on the version that
                // was just replaced would make every one of them lie.
                self.state
                    .chosen_versions
                    .insert(bundle_identifier, entry_id.clone());
                self.state.selected_app = Some(entry_id);
                self.dirty.state = true;
                self.invalidate_order();
            }
            Action::CopyText(text) => {
                if !text.is_empty() {
                    ctx.copy_text(text);
                }
            }
            Action::OpenPath(path) => {
                if !path.exists() {
                    self.note(
                        LogLevel::Warning,
                        format!("{} does not exist yet.", path.display()),
                    );
                } else if let Err(e) =
                    crate::platform::process::open_in_desktop(&path.display().to_string())
                {
                    self.note(LogLevel::Warning, e);
                }
            }
            Action::OpenUrl(url) => {
                if let Err(e) = crate::platform::process::open_in_desktop(&url) {
                    self.note(LogLevel::Warning, e);
                }
            }
            Action::OpenCompatibilityEntry(id) => {
                let url = self
                    .library
                    .find(&id)
                    .and_then(|entry| self.database.find(&entry.metadata.bundle_identifier))
                    .map(|record| record.url.clone());
                match url {
                    Some(url) => self.apply(ctx, Action::OpenUrl(url)),
                    None => self.note(
                        LogLevel::Warning,
                        "The compatibility database has no record for this app.".to_string(),
                    ),
                }
            }
            Action::OpenCompatibilityReport(id) => self.open_report(&id),
            Action::SetLocalRating(id, stars) => {
                if let Some(entry) = self.library.find_mut(&id) {
                    entry.local_rating.set_stars(stars);
                    self.dirty.library = true;
                    self.invalidate_order();
                }
            }
            Action::ShowAbout => self.about = Some(AboutDialog::default()),
            Action::ShowLogPanel(visible) => {
                self.state.log_panel_visible = visible;
                self.dirty.state = true;
            }
            Action::ToggleLogPanel => {
                self.state.log_panel_visible = !self.state.log_panel_visible;
                self.dirty.state = true;
            }
            Action::SetViewMode(mode) => {
                self.state.view_mode = mode;
                self.dirty.state = true;
            }
            Action::SetIconSize(size) => {
                self.state.icon_size = size;
                self.dirty.state = true;
            }
            Action::SetSortOrder(order) => {
                self.state.sort_order = order;
                self.dirty.state = true;
            }
            Action::ToggleSortDirection => {
                self.state.sort_descending = !self.state.sort_descending;
                self.dirty.state = true;
            }
            Action::ToggleFavoritesOnly => {
                self.state.favorites_only = !self.state.favorites_only;
                self.dirty.state = true;
            }
            Action::ClearLog => {
                if let Ok(mut store) = self.log.lock() {
                    store.clear();
                }
                self.log_view.clear_selection();
            }
            Action::SaveLog => self.save_log(),
            Action::CopyLogSelection => {
                let text = self
                    .log
                    .lock()
                    .map(|store| self.log_view.text(&store, true))
                    .unwrap_or_default();
                if !text.is_empty() {
                    ctx.copy_text(text);
                }
            }
            Action::CopyDiagnostics(id) => {
                let text = self.diagnostics(&id);
                ctx.copy_text(text);
                self.note(LogLevel::Info, "Diagnostics copied to the clipboard.");
            }
            Action::CheckForUpdates => self.check_for_updates(),
            Action::RefreshCompatibility => self.refresh_compatibility(),
            Action::OpenUserDataFolder => self.apply(ctx, Action::OpenPath(self.data_dir.clone())),
            Action::OpenAppsFolder => {
                let folder = self.data_dir.join(tapHLE::paths::APPS_DIR);
                self.apply(ctx, Action::OpenPath(folder))
            }
            Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::ConfirmedQuit => {
                self.closing_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn save_log(&mut self) {
        let suggested = format!(
            "tapHLE-log-{}.txt",
            timefmt::format_file_stamp(timefmt::now_seconds())
        );
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save the log")
            .set_file_name(suggested)
            .add_filter("Text", &["txt", "log"])
            .save_file()
        else {
            return;
        };
        let text = match self.log.lock() {
            Ok(store) => self.log_view.text(&store, false),
            Err(_) => String::new(),
        };
        let header = format!(
            "tapHLE {} — {}\n{}\n\n",
            tapHLE_version::VERSION.trim(),
            compat::platform_description(),
            timefmt::format_datetime(timefmt::now_seconds())
        );
        match std::fs::write(&path, header + &text) {
            Ok(()) => self.note(
                LogLevel::Info,
                format!("Log written to {}.", path.display()),
            ),
            Err(e) => self.note(
                LogLevel::Error,
                format!("Could not write {}: {e}", path.display()),
            ),
        }
    }

    fn about_info(&self) -> AboutInfo {
        let mut build_details = Vec::new();
        for (label, value) in [
            ("Repository", tapHLE_version::GITHUB_REPOSITORY),
            ("Branch or tag", tapHLE_version::GITHUB_REF_NAME),
            ("Workflow run", tapHLE_version::GITHUB_RUN_ID),
        ] {
            if let Some(value) = value {
                build_details.push((label.to_string(), value.to_string()));
            }
        }
        if build_details.is_empty() {
            build_details.push((
                "Built".to_string(),
                "locally, from this working tree".to_string(),
            ));
        }
        AboutInfo {
            version: tapHLE_version::VERSION.trim().to_string(),
            branding: tapHLE_version::branding().to_string(),
            cargo_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: compat::platform_description(),
            build_profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            build_details,
            update: self.update.clone(),
            transport: if self.transport.is_available() {
                self.transport.describe()
            } else {
                format!("{} (not available)", self.transport.describe())
            },
            compatibility_source: {
                let provider = TapHledbProvider::new(self.transport.clone());
                if self.database_available {
                    format!(
                        "{} ratings from {}",
                        self.database.entries.len(),
                        provider.describe()
                    )
                } else {
                    format!("{} — not read yet", provider.describe())
                }
            },
            update_source: GitHubReleaseProvider::new(self.transport.clone()).describe(),
        }
    }
}

/// A description of how this build was produced, for a report.
fn build_description() -> String {
    match (
        tapHLE_version::GITHUB_REPOSITORY,
        tapHLE_version::GITHUB_RUN_ID,
    ) {
        (Some(repository), Some(run)) => format!("{repository} run {run}"),
        _ => format!(
            "local {} build",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        ),
    }
}

/// How many apps the library holds, counting every version of one app once.
fn distinct_apps(library: &Library) -> usize {
    let mut identifiers: Vec<&str> = library
        .entries
        .iter()
        .map(|entry| entry.metadata.bundle_identifier.as_str())
        .collect();
    identifiers.sort_unstable();
    identifiers.dedup();
    identifiers.len()
}

fn plural(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

fn report_load<T: serde::de::DeserializeOwned + Default>(log: &SharedLog, file: &str) -> T {
    match storage::load(file) {
        Ok(value) => value,
        Err(e) => {
            logstore::note(
                log,
                LogLevel::Error,
                format!("{e}. Starting from defaults for this file."),
            );
            T::default()
        }
    }
}

fn report_load_or_default<T: serde::de::DeserializeOwned + Default>(file: &str) -> T {
    storage::load(file).unwrap_or_default()
}

impl Frontend {
    /// Send every log line to stderr as well as to the panel.
    ///
    /// Only when the frontend was started from a terminal — see
    /// [crate::platform::console].
    pub fn mirror_log_to_stderr(&self, mirror: bool) {
        if let Ok(mut store) = self.log.lock() {
            store.mirror_to_stderr(mirror);
        }
    }
}

impl crate::shell::Application for Frontend {
    fn update(&mut self, ctx: &egui::Context) {
        self.drain_background(ctx);
        self.collect_dropped_files(ctx);
        self.collect_finished_runs();
        self.remember_geometry(ctx);

        if (self.settings.ui_zoom - self.applied_zoom).abs() > f32::EPSILON {
            ctx.set_zoom_factor(self.settings.ui_zoom.clamp(0.5, 3.0));
            self.applied_zoom = self.settings.ui_zoom;
        }

        let mut actions: Vec<Action> = Vec::new();
        // The order is settled before the chrome is built so the status bar
        // can say how many apps are shown this frame rather than last.
        let groups = self.visible_groups().to_vec();
        // The toolbar owns the search box while it is drawn, and the chrome
        // context borrows the rest, so the string is lifted out and put back.
        let mut search = std::mem::take(&mut self.search);
        let context = self.chrome_context();

        egui::TopBottomPanel::top("menu-bar")
            .frame(chrome_frame(2))
            .show(ctx, |ui| {
                crate::ui::desktop::chrome::menu_bar(ui, &context, &mut actions);
            });
        egui::TopBottomPanel::top("toolbar")
            .frame(chrome_frame(3))
            .show(ctx, |ui| {
                crate::ui::desktop::chrome::toolbar(ui, &context, &mut search, &mut actions);
            });
        egui::TopBottomPanel::bottom("status-bar")
            .frame(chrome_frame(2))
            .show(ctx, |ui| {
                crate::ui::desktop::chrome::status_bar(ui, &context, &mut actions);
            });

        if self.state.log_panel_visible {
            let run_ids = self
                .state
                .selected_app
                .as_deref()
                .map(|id| self.run_ids_for(id))
                .unwrap_or_default();
            let response = egui::TopBottomPanel::bottom("log-panel")
                .resizable(true)
                .default_height(self.state.log_panel_height)
                .min_height(80.0)
                .frame(
                    egui::Frame::new()
                        .fill(theme::LIGHT.panel)
                        .stroke(egui::Stroke::new(1.0_f32, theme::LIGHT.border)),
                )
                .show(ctx, |ui| {
                    if let Ok(store) = self.log.lock() {
                        crate::ui::desktop::logpanel::show(
                            ui,
                            &mut self.log_view,
                            &store,
                            &run_ids,
                            &mut actions,
                        );
                    }
                });
            let height = response.response.rect.height();
            if (height - self.state.log_panel_height).abs() > 1.0 {
                self.state.log_panel_height = height;
                self.dirty.state = true;
            }
        }

        let details_response = egui::SidePanel::right("details")
            .resizable(true)
            .default_width(self.state.details_panel_width)
            .width_range(240.0..=520.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::LIGHT.panel)
                    .stroke(egui::Stroke::new(1.0_f32, theme::LIGHT.border))
                    .inner_margin(egui::Margin::symmetric(10, 6)),
            )
            .show(ctx, |ui| {
                let entry = self.selected_entry();
                let details = DetailsContext {
                    icon: entry.and_then(|entry| self.icons.get(&entry.id)),
                    running: entry.is_some_and(|entry| self.launcher.is_running(&entry.id)),
                    entry,
                    database: &self.database,
                    database_available: self.database_available,
                    versions: self.versions_of_selection(),
                };
                crate::ui::desktop::details::show(ui, &details, &mut actions);
            });
        let details_width = details_response.response.rect.width();
        if (details_width - self.state.details_panel_width).abs() > 1.0 {
            self.state.details_panel_width = details_width;
            self.dirty.state = true;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::LIGHT.content))
            .show(ctx, |ui| {
                let running: Vec<String> = self
                    .launcher
                    .running()
                    .iter()
                    .map(|run| run.entry_id.clone())
                    .collect();
                let icon_points = self.state.icon_size.points();
                let view = self.state.view_mode;
                let selected = self.state.selected_app.clone();
                let library_is_empty = self.library.entries.is_empty();
                let context = LibraryContext {
                    library: &self.library,
                    groups: &groups,
                    icons: &self.icons,
                    database: &self.database,
                    selected: selected.as_deref(),
                    running: &running,
                    icon_points,
                    view,
                    library_is_empty,
                };
                crate::ui::desktop::library_view::show(ui, &context, &mut actions);
            });

        self.search = search;
        self.show_dialogs(ctx, &mut actions);
        self.handle_close_request(ctx);

        for action in actions {
            self.apply(ctx, action);
        }
        self.save_if_due(false);

        // The interface is otherwise only redrawn on input, which would leave
        // a running app's output frozen on screen.
        if self.launcher.any_running() {
            ctx.request_repaint_after(Duration::from_millis(120));
        } else if self.state.log_panel_visible {
            ctx.request_repaint_after(Duration::from_millis(400));
        }
    }

    fn on_exit(&mut self) {
        self.launcher.stop_all();
        self.save_if_due(true);
    }
}

fn chrome_frame(vertical: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(theme::LIGHT.chrome)
        .inner_margin(egui::Margin::symmetric(2, vertical))
        .stroke(egui::Stroke::new(1.0_f32, theme::LIGHT.border))
}

impl Frontend {
    fn chrome_context(&self) -> ChromeContext<'_> {
        let entry = self.selected_entry();
        let (errors, warnings) = self
            .log
            .lock()
            .map(|store| (store.errors(), store.warnings()))
            .unwrap_or((0, 0));
        ChromeContext {
            selected: entry.map(|entry| entry.id.as_str()),
            selected_title: entry.map(|entry| entry.title()),
            selection_missing: entry.is_some_and(|entry| entry.missing),
            running_count: self.launcher.running().len(),
            selected_running: entry.is_some_and(|entry| self.launcher.is_running(&entry.id)),
            // Counted in apps rather than files, to agree with what the
            // library shows now that versions of one app share a place.
            library_count: distinct_apps(&self.library),
            shown_count: self.group_cache.len(),
            view_mode: self.state.view_mode,
            icon_size: self.state.icon_size,
            sort_order: self.state.sort_order,
            sort_descending: self.state.sort_descending,
            favorites_only: self.state.favorites_only,
            log_visible: self.state.log_panel_visible,
            log_errors: errors,
            log_warnings: warnings,
            update_summary: self.update.summary(),
            version: tapHLE_version::VERSION.trim(),
            developer_mode: self.settings.developer_mode,
        }
    }

    fn show_dialogs(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if let Some(dialog) = &mut self.global_dialog {
            match crate::ui::desktop::settings_dialog::show_global(ctx, dialog) {
                Outcome::Continue => (),
                Outcome::Cancel => self.global_dialog = None,
                outcome => {
                    let draft = dialog.draft.clone();
                    let close = outcome == Outcome::Accept;
                    self.adopt_settings(ctx, draft);
                    if close {
                        self.global_dialog = None;
                    }
                }
            }
        }
        // Opened from the per-app Controls page. Handled here rather than
        // inside the dialog because placing a control needs the app's device
        // family and orientation, which the dialog does not have.
        if self
            .app_dialog
            .as_ref()
            .is_some_and(|d| d.open_control_editor)
        {
            self.open_control_editor();
        }
        if self
            .control_editor
            .as_ref()
            .is_some_and(|e| e.wants_capture)
        {
            self.start_control_capture();
        }
        if let Some(editor) = &mut self.control_editor {
            match crate::ui::desktop::control_editor::show(ctx, editor) {
                crate::ui::desktop::control_editor::Outcome::Continue => (),
                crate::ui::desktop::control_editor::Outcome::Cancel => self.control_editor = None,
                crate::ui::desktop::control_editor::Outcome::Save => {
                    let layout = editor.draft.clone();
                    let for_app = editor.entry_id.clone();
                    if let Some(dialog) = &mut self.app_dialog {
                        // The editor names the app it was opened for, and the
                        // save only lands if that is still the app being
                        // edited. Writing one app's controls into another's
                        // would look like the editor had silently lost them.
                        if dialog.entry_id == for_app {
                            dialog.draft.controls = (!layout.is_empty()).then_some(layout);
                        }
                    }
                    self.control_editor = None;
                }
            }
        }
        if let Some(dialog) = &mut self.app_dialog {
            match crate::ui::desktop::settings_dialog::show_app(ctx, dialog) {
                Outcome::Continue => (),
                Outcome::Cancel => self.app_dialog = None,
                outcome => {
                    let (id, draft) = (dialog.entry_id.clone(), dialog.draft.clone());
                    let close = outcome == Outcome::Accept;
                    if let Some(entry) = self.library.find_mut(&id) {
                        entry.overrides = draft;
                        self.dirty.library = true;
                    }
                    if close {
                        self.app_dialog = None;
                    }
                }
            }
        }
        if self.about.is_some() {
            let info = self.about_info();
            if let Some(dialog) = &mut self.about {
                crate::ui::desktop::dialogs::show_about(ctx, dialog, &info, actions);
                if !dialog.open {
                    self.about = None;
                }
            }
        }
        if let Some(report) = &mut self.import_report {
            crate::ui::desktop::dialogs::show_import_report(ctx, report);
            if !report.open {
                self.import_report = None;
            }
        }
        if let Some(notice) = &mut self.crash {
            crate::ui::desktop::dialogs::show_crash(ctx, notice, actions);
            if !notice.open {
                self.crash = None;
            }
        }
        if let Some(dialog) = &mut self.report {
            let limitation = TapHledbProvider::new(self.transport.clone()).submission_limitation();
            crate::ui::desktop::dialogs::show_report(ctx, dialog, limitation, actions);
            if !dialog.open {
                self.report = None;
            }
        }
        if let Some(confirmation) = &mut self.confirmation {
            crate::ui::desktop::dialogs::show_confirmation(ctx, confirmation, actions);
            if !confirmation.open {
                self.confirmation = None;
            }
        }
    }

    fn adopt_settings(&mut self, ctx: &egui::Context, draft: FrontendSettings) {
        let capacity_changed = draft.log_capacity != self.settings.log_capacity;
        let updates_changed = draft.check_for_updates != self.settings.check_for_updates;
        self.log_view.show_timestamps = draft.log_show_timestamps;
        if draft.developer_mode && !self.settings.developer_mode {
            self.state.log_panel_visible = true;
            self.dirty.state = true;
        }
        self.settings = draft;
        self.dirty.settings = true;
        if capacity_changed {
            if let Ok(mut store) = self.log.lock() {
                store.set_capacity(self.settings.log_capacity);
            }
        }
        if updates_changed {
            if self.settings.check_for_updates {
                self.check_for_updates();
            } else {
                self.update = UpdateStatus::Disabled;
            }
        }
        ctx.set_zoom_factor(self.settings.ui_zoom.clamp(0.5, 3.0));
        self.applied_zoom = self.settings.ui_zoom;
        self.save_if_due(true);
    }

    /// Closing with a game still running would break the pipe its output is
    /// written to, so the runs are ended deliberately and the person is told
    /// that is what will happen.
    fn handle_close_request(&mut self, ctx: &egui::Context) {
        let requested = ctx.input(|input| input.viewport().close_requested());
        if !requested {
            return;
        }
        if self.launcher.any_running() && !self.closing_confirmed {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.confirmation.is_none() {
                let count = self.launcher.running().len();
                self.confirmation = Some(Confirmation {
                    open: true,
                    title: "Close tapHLE".to_string(),
                    message: format!(
                        "{count} app{} still running. Closing tapHLE will stop {}.",
                        if count == 1 { " is" } else { "s are" },
                        if count == 1 { "it" } else { "them" }
                    ),
                    confirm_label: "Close anyway".to_string(),
                    action: Action::ConfirmedQuit,
                });
            }
            return;
        }
        self.save_if_due(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_outside_ci_says_so() {
        let description = build_description();
        assert!(description.contains("local") || description.contains("run"));
    }

    #[test]
    fn counts_are_pluralised() {
        assert_eq!(plural(1), "");
        assert_eq!(plural(0), "s");
    }
}
