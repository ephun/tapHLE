/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The dialogs: About, the import report, the crash notice, and the
//! compatibility report.
//!
//! The crash notice is the one that matters most. A game that stops is the
//! normal outcome of compatibility work, and letting its window simply
//! vanish tells nobody anything. The notice says what happened, in what
//! terms can be known, and puts the log and a report one click away — while
//! the output that explains it is still sitting in the store.

use egui::{Id, Ui};

use crate::compat::ReportDraft;
use crate::library::ImportOutcome;
use crate::theme;
use crate::ui::{self, Action};
use crate::updates::UpdateStatus;

/// What the About window shows, gathered where it is known rather than here.
pub struct AboutInfo {
    pub version: String,
    pub branding: String,
    pub cargo_version: String,
    pub platform: String,
    pub build_profile: &'static str,
    pub build_details: Vec<(String, String)>,
    pub update: UpdateStatus,
    pub transport: String,
    pub compatibility_source: String,
    pub update_source: String,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum AboutTab {
    About,
    Build,
    Credits,
    Licenses,
}

pub struct AboutDialog {
    pub open: bool,
    pub tab: AboutTab,
    /// The licence text is large, so it is read once and kept.
    pub licenses: Option<String>,
}

impl Default for AboutDialog {
    fn default() -> Self {
        AboutDialog {
            open: true,
            tab: AboutTab::About,
            licenses: None,
        }
    }
}

pub fn show_about(
    ctx: &egui::Context,
    dialog: &mut AboutDialog,
    info: &AboutInfo,
    actions: &mut Vec<Action>,
) {
    let response = egui::Modal::new(Id::new("taphle-about")).show(ctx, |ui| {
        ui.set_width(560.0);
        ui.horizontal(|ui| {
            ui.heading("tapHLE");
            ui.label(egui::RichText::new(&info.version).color(theme::LIGHT.text_dim));
            if !info.branding.is_empty() {
                ui.label(
                    egui::RichText::new(&info.branding)
                        .small()
                        .color(theme::LIGHT.warning),
                );
            }
        });
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            for (tab, label) in [
                (AboutTab::About, "About"),
                (AboutTab::Build, "Build"),
                (AboutTab::Credits, "Credits"),
                (AboutTab::Licenses, "Licences"),
            ] {
                if ui.selectable_label(dialog.tab == tab, label).clicked() {
                    dialog.tab = tab;
                }
            }
        });
        theme::hairline(ui);
        ui.add_space(6.0);

        crate::ui::settings_dialog::page(ui, 540.0, |ui| match dialog.tab {
            AboutTab::About => about_tab(ui, actions),
            AboutTab::Build => build_tab(ui, info, actions),
            AboutTab::Credits => credits_tab(ui, actions),
            AboutTab::Licenses => licenses_tab(ui, dialog),
        });

        ui.add_space(8.0);
        theme::hairline(ui);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    dialog.open = false;
                }
            });
        });
    });
    if response.should_close() {
        dialog.open = false;
    }
}

/// tapHLE's own repository, linked from several places here.
const PROJECT_URL: &str = "https://github.com/ephun/tapHLE";

/// A hyperlink that opens in the desktop's browser.
///
/// egui's own `Hyperlink` opens the link itself; going through an [Action]
/// keeps every outward-facing thing the frontend does in one place, and means
/// a link and a menu item reach the browser the same way.
fn link(ui: &mut Ui, text: &str, url: &str, actions: &mut Vec<Action>) {
    if ui.link(text).on_hover_text(url).clicked() {
        actions.push(Action::OpenUrl(url.to_string()));
    }
}

/// How wide the project column of the credits is, so the names beside them
/// line up instead of starting wherever the previous word ended.
const CREDIT_COLUMN: f32 = 120.0;

/// One credit: a project on the left, the people behind it on the right.
fn credit(
    ui: &mut Ui,
    project: (&str, &str),
    people: &[(&str, &str)],
    trailing: &str,
    with: Option<(&str, &str)>,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        // Spacing is applied by hand here: the default word gap would put a
        // space before each comma.
        ui.spacing_mut().item_spacing.x = 0.0;
        let start = ui.cursor().min.x;
        link(ui, project.0, project.1, actions);
        let used = ui.cursor().min.x - start;
        ui.add_space((CREDIT_COLUMN - used).max(8.0));

        for (index, (name, url)) in people.iter().enumerate() {
            if index > 0 {
                ui.label(",");
                ui.add_space(5.0);
            }
            link(ui, name, url, actions);
        }
        if let Some((text, url)) = with {
            ui.add_space(5.0);
            ui.label(egui::RichText::new("with").color(theme::LIGHT.text_dim));
            ui.add_space(5.0);
            link(ui, text, url, actions);
        }
        if !trailing.is_empty() {
            ui.add_space(5.0);
            ui.label(egui::RichText::new(trailing).color(theme::LIGHT.text_dim));
        }
    });
}

fn about_tab(ui: &mut Ui, actions: &mut Vec<Action>) {
    ui.label(
        egui::RichText::new("tapHLE: A high level emulator for early iOS applications").strong(),
    );
    theme::hairline(ui);
    ui.add_space(8.0);

    ui.label(
        "tapHLE works by running an app's 32-bit code and then runs its own \
         implementations of the frameworks it calls — Foundation, UIKit, \
         OpenGL ES, OpenAL and the rest. More information can be found on the \
         project's GitHub, which is linked below.",
    );
    ui.add_space(8.0);
    ui.label("tapHLE is coded with AI assistance.");
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(
            "Not affiliated with or endorsed by Apple Inc. iPhone, iOS, iPod, \
             iPod touch and iPad are Apple trademarks.",
        )
        .small()
        .color(theme::LIGHT.text_dim),
    );

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        link(ui, "tapHLE on GitHub", PROJECT_URL, actions);
        ui.add_space(12.0);
        link(
            ui,
            "Compatibility database",
            crate::compat::DATABASE_WEB_URL,
            actions,
        );
    });
}

fn build_tab(ui: &mut Ui, info: &AboutInfo, actions: &mut Vec<Action>) {
    ui::field(ui, "Version", &info.version);
    ui::field(ui, "Package version", &info.cargo_version);
    ui::field(ui, "Build", info.build_profile);
    ui::field(ui, "Platform", &info.platform);
    for (label, value) in &info.build_details {
        ui::field(ui, label, value);
    }
    ui.add_space(8.0);
    ui::section(ui, "Services");
    ui::field(ui, "Network", &info.transport);
    ui::field(ui, "Ratings", &info.compatibility_source);
    ui::field(ui, "Update source", &info.update_source);
    ui::field(ui, "Updates", &info.update.summary());
    ui.label(
        egui::RichText::new(info.update.detail())
            .small()
            .color(theme::LIGHT.text_dim),
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Check now").clicked() {
            actions.push(Action::CheckForUpdates);
        }
        if ui.button("Copy build information").clicked() {
            let mut text = format!(
                "tapHLE {}\nPackage version: {}\nBuild: {}\nPlatform: {}\n",
                info.version, info.cargo_version, info.build_profile, info.platform
            );
            for (label, value) in &info.build_details {
                text.push_str(&format!("{label}: {value}\n"));
            }
            actions.push(Action::CopyText(text));
        }
    });
}

fn credits_tab(ui: &mut Ui, actions: &mut Vec<Action>) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label("tapHLE is an unaffiliated fork of the amazing");
        link(ui, "touchHLE project", "https://touchhle.org/", actions);
        ui.label("where it bases much of its architecture.");
    });
    ui.add_space(10.0);

    credit(
        ui,
        ("touchHLE", "https://github.com/touchHLE/touchHLE"),
        &[
            ("hikari_no_yume", "https://github.com/hikari-no-yume"),
            ("ciciplusplus", "https://github.com/ciciplusplus"),
        ],
        "and contributors",
        None,
        actions,
    );
    credit(
        ui,
        ("tapHLE", PROJECT_URL),
        &[("ephun", "https://github.com/ephun")],
        "",
        Some((
            "AI assistance",
            "https://github.com/ephun/tapHLE#a-note-on-ai-from-the-maintainer",
        )),
        actions,
    );

    ui.add_space(12.0);
    ui.label(
        "Special thanks to the following projects for inspiration / tweaks for \
         implementation in tapHLE:",
    );
    ui.add_space(6.0);
    credit(
        ui,
        (
            "Applesauce",
            "https://github.com/johnny901901901/Applesauce",
        ),
        &[("johnny901901901", "https://github.com/johnny901901901")],
        "and contributors",
        None,
        actions,
    );
    credit(
        ui,
        ("HyperHLE", "https://github.com/HyperHLE/HyperHLE"),
        &[("TimofeyLednev", "https://github.com/TimofeyLednev")],
        "and contributors",
        None,
        actions,
    );
    credit(
        ui,
        ("LiveExec32", "https://github.com/LiveContainer/LiveExec32"),
        &[("khanhduytran0", "https://github.com/khanhduytran0")],
        "and contributors",
        None,
        actions,
    );

    ui.add_space(12.0);
    ui.label(
        egui::RichText::new(
            "Bundled dynamic libraries and fonts carry their own notices in \
             the tapHLE_dylibs and tapHLE_fonts folders.",
        )
        .small()
        .color(theme::LIGHT.text_dim),
    );
}

fn licenses_tab(ui: &mut Ui, dialog: &mut AboutDialog) {
    let text = dialog
        .licenses
        .get_or_insert_with(tapHLE::licenses::get_text);
    ui.label(
        egui::RichText::new(
            "The emulator's source is under the Mozilla Public License 2.0. \
             Because of its dependencies, distributed binaries are under the \
             GNU General Public License, version 3 or later.",
        )
        .small()
        .color(theme::LIGHT.text_dim),
    );
    ui.add_space(6.0);
    ui.add(
        egui::TextEdit::multiline(&mut text.as_str())
            .font(egui::TextStyle::Monospace)
            .desired_width(f32::INFINITY)
            .desired_rows(16),
    );
}

/// The report shown after files are dropped or chosen.
pub struct ImportReport {
    pub open: bool,
    pub outcomes: Vec<String>,
    pub added: usize,
    pub duplicates: usize,
    pub failures: usize,
}

impl ImportReport {
    pub fn from_outcomes(outcomes: &[ImportOutcome], library: &crate::library::Library) -> Self {
        let mut report = ImportReport {
            open: true,
            outcomes: Vec::new(),
            added: 0,
            duplicates: 0,
            failures: 0,
        };
        for outcome in outcomes {
            match outcome {
                ImportOutcome::Added { .. } => report.added += 1,
                ImportOutcome::Duplicate { .. } => report.duplicates += 1,
                _ => report.failures += 1,
            }
            report.outcomes.push(outcome.describe(library));
        }
        report
    }

    /// Whether the report is worth showing at all.
    ///
    /// Adding apps that all worked needs no dialog; the library filling up is
    /// the feedback. Anything else does.
    pub fn worth_showing(&self) -> bool {
        self.duplicates > 0 || self.failures > 0
    }
}

pub fn show_import_report(ctx: &egui::Context, report: &mut ImportReport) {
    let response = egui::Modal::new(Id::new("taphle-import")).show(ctx, |ui| {
        ui.set_width(480.0);
        ui.heading("Adding apps");
        ui.add_space(4.0);
        let summary = format!(
            "{} added, {} already in the library, {} could not be added.",
            report.added, report.duplicates, report.failures
        );
        ui.label(summary);
        ui.add_space(6.0);
        theme::hairline(ui);
        crate::ui::settings_dialog::page(ui, 460.0, |ui| {
            for line in &report.outcomes {
                ui.label(line);
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    report.open = false;
                }
            });
        });
    });
    if response.should_close() {
        report.open = false;
    }
}

/// Shown when a run ends badly.
pub struct CrashNotice {
    pub open: bool,
    pub entry_id: String,
    pub app_title: String,
    pub explanation: String,
    /// The last lines of the run's output, kept so they survive the log being
    /// cleared or filtered afterwards.
    pub excerpt: String,
}

pub fn show_crash(ctx: &egui::Context, notice: &mut CrashNotice, actions: &mut Vec<Action>) {
    let response = egui::Modal::new(Id::new("taphle-crash")).show(ctx, |ui| {
        ui.set_width(560.0);
        ui.heading(format!("{} stopped", notice.app_title));
        ui.add_space(4.0);
        ui.label(&notice.explanation);
        if !notice.excerpt.trim().is_empty() {
            ui.add_space(8.0);
            ui::section(ui, "Last output");
            ui.add(
                egui::TextEdit::multiline(&mut notice.excerpt.as_str())
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(9),
            );
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Show Log").clicked() {
                actions.push(Action::ShowLogPanel(true));
                notice.open = false;
            }
            if ui.button("Copy Diagnostics").clicked() {
                actions.push(Action::CopyDiagnostics(notice.entry_id.clone()));
            }
            if ui.button("Save Log…").clicked() {
                actions.push(Action::SaveLog);
            }
            if ui.button("Compatibility Report…").clicked() {
                actions.push(Action::OpenCompatibilityReport(notice.entry_id.clone()));
                notice.open = false;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    notice.open = false;
                }
            });
        });
    });
    if response.should_close() {
        notice.open = false;
    }
}

/// The compatibility report window.
pub struct ReportDialog {
    pub open: bool,
    pub entry_id: String,
    pub stars: Option<u8>,
    pub notes: String,
    pub include_log: bool,
    pub draft: ReportDraft,
}

pub fn show_report(
    ctx: &egui::Context,
    dialog: &mut ReportDialog,
    limitation: &str,
    actions: &mut Vec<Action>,
) {
    let response = egui::Modal::new(Id::new("taphle-report")).show(ctx, |ui| {
        ui.set_width(600.0);
        ui.heading("Compatibility report");
        ui.label(egui::RichText::new(&dialog.draft.display_name).color(theme::LIGHT.text_dim));
        ui.add_space(6.0);
        theme::hairline(ui);
        ui.add_space(6.0);

        crate::ui::settings_dialog::page(ui, 580.0, |ui| {
            match (
                &dialog.draft.existing_entry,
                dialog.draft.database_consulted,
            ) {
                (Some(entry), _) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("The database already has a record for this app:");
                        ui.label(egui::RichText::new(&entry.name).strong());
                    });
                    ui.label(
                        egui::RichText::new(
                            "Report against that record rather than creating a \
                                 second one for the same app.",
                        )
                        .small()
                        .color(theme::LIGHT.text_dim),
                    );
                    if ui.button("Open the existing record").clicked() {
                        actions.push(Action::OpenUrl(entry.url.clone()));
                    }
                }
                (None, true) => {
                    ui.label(
                        "The database has no record for this bundle identifier \
                             yet, so this would be a new one.",
                    );
                }
                (None, false) => {
                    ui.label(
                        egui::RichText::new(
                            "The database could not be reached, so whether a \
                                 record already exists is unknown. Check before \
                                 submitting, or a duplicate may be created.",
                        )
                        .color(theme::LIGHT.warning),
                    );
                }
            }

            ui::section(ui, "Rating");
            if let Some(new_rating) = ui::star_picker(ui, dialog.stars) {
                dialog.stars = new_rating;
                dialog.draft.stars = new_rating;
            }

            ui::section(ui, "Notes");
            if ui
                .add(
                    egui::TextEdit::multiline(&mut dialog.notes)
                        .desired_width(f32::INFINITY)
                        .desired_rows(4)
                        .hint_text("What worked, what did not, and where it stopped"),
                )
                .changed()
            {
                dialog.draft.notes = dialog.notes.clone();
            }

            ui::section(ui, "Report contents");
            ui.checkbox(&mut dialog.include_log, "Include the recent log output");
            let text = report_text(dialog);
            ui.add(
                egui::TextEdit::multiline(&mut text.as_str())
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(10),
            );
        });

        ui.add_space(8.0);
        theme::hairline(ui);
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(limitation)
                .small()
                .color(theme::LIGHT.text_dim),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Copy Report").clicked() {
                actions.push(Action::CopyText(report_text(dialog)));
            }
            if ui.button("Open the Database").clicked() {
                actions.push(Action::OpenUrl(crate::compat::DATABASE_WEB_URL.to_string()));
            }
            if ui
                .button("Save my rating")
                .on_hover_text("Keep this rating on this computer")
                .clicked()
            {
                actions.push(Action::SetLocalRating(
                    dialog.entry_id.clone(),
                    dialog.stars,
                ));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    dialog.open = false;
                }
                ui.add_enabled(false, egui::Button::new("Submit"))
                    .on_disabled_hover_text(limitation);
            });
        });
    });
    if response.should_close() {
        dialog.open = false;
    }
}

fn report_text(dialog: &ReportDialog) -> String {
    let mut draft = ReportDraft {
        display_name: dialog.draft.display_name.clone(),
        bundle_identifier: dialog.draft.bundle_identifier.clone(),
        bundle_version: dialog.draft.bundle_version.clone(),
        short_version: dialog.draft.short_version.clone(),
        taphle_version: dialog.draft.taphle_version.clone(),
        taphle_build: dialog.draft.taphle_build.clone(),
        platform: dialog.draft.platform.clone(),
        stars: dialog.stars,
        notes: dialog.notes.clone(),
        launch_options: dialog.draft.launch_options.clone(),
        log_excerpt: String::new(),
        existing_entry: dialog.draft.existing_entry.clone(),
        database_consulted: dialog.draft.database_consulted,
    };
    if dialog.include_log {
        draft.log_excerpt = dialog.draft.log_excerpt.clone();
    }
    draft.to_text()
}

/// A plain question with two answers, used before anything irreversible.
pub struct Confirmation {
    pub open: bool,
    pub title: String,
    pub message: String,
    pub confirm_label: String,
    pub action: Action,
}

pub fn show_confirmation(
    ctx: &egui::Context,
    confirmation: &mut Confirmation,
    actions: &mut Vec<Action>,
) {
    let response = egui::Modal::new(Id::new("taphle-confirm")).show(ctx, |ui| {
        ui.set_width(400.0);
        ui.heading(&confirmation.title);
        ui.add_space(6.0);
        ui.label(&confirmation.message);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(&confirmation.confirm_label).clicked() {
                    actions.push(confirmation.action.clone());
                    confirmation.open = false;
                }
                if ui.button("Cancel").clicked() {
                    confirmation.open = false;
                }
            });
        });
    });
    if response.should_close() {
        confirmation.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;

    /// A clean import needs no dialog; the apps appearing is the feedback.
    /// Anything unexpected does need one.
    #[test]
    fn a_clean_import_does_not_interrupt() {
        let library = Library::default();
        let clean = ImportReport::from_outcomes(
            &[ImportOutcome::Added {
                id: "com.x@1".to_string(),
                warnings: Vec::new(),
            }],
            &library,
        );
        assert!(!clean.worth_showing());

        let messy = ImportReport::from_outcomes(
            &[
                ImportOutcome::Added {
                    id: "com.x@1".to_string(),
                    warnings: Vec::new(),
                },
                ImportOutcome::Unsupported {
                    path: "a.txt".into(),
                    reason: "not an app".to_string(),
                },
            ],
            &library,
        );
        assert!(messy.worth_showing());
        assert_eq!(messy.added, 1);
        assert_eq!(messy.failures, 1);
    }

    /// Leaving the log out of a report has to actually leave it out.
    #[test]
    fn the_log_is_only_included_when_asked_for() {
        let mut dialog = ReportDialog {
            open: true,
            entry_id: "com.x@1".to_string(),
            stars: Some(3),
            notes: String::new(),
            include_log: false,
            draft: ReportDraft {
                display_name: "X".to_string(),
                bundle_identifier: "com.x".to_string(),
                bundle_version: "1".to_string(),
                short_version: None,
                taphle_version: "test".to_string(),
                taphle_build: "test".to_string(),
                platform: "windows x86_64".to_string(),
                stars: Some(3),
                notes: String::new(),
                launch_options: Vec::new(),
                log_excerpt: "SECRET LOG LINE".to_string(),
                existing_entry: None,
                database_consulted: true,
            },
        };
        assert!(!report_text(&dialog).contains("SECRET LOG LINE"));
        dialog.include_log = true;
        assert!(report_text(&dialog).contains("SECRET LOG LINE"));
    }
}
