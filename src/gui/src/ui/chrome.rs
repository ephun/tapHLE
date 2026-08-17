/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The window's furniture: menu bar, toolbar and status bar.
//!
//! These are the parts that make the program legible to somebody who has used
//! a desktop emulator before. The menu bar carries everything, including the
//! things the toolbar also offers, because a menu is where a person looks for
//! a command they cannot see. The toolbar carries the handful of actions used
//! constantly, with both an icon and a word.

use egui::Ui;

use crate::settings::{IconSize, SortOrder, ViewMode};
use crate::theme;
use crate::ui::{self, Action, Icon};

/// What the chrome needs to know to draw itself correctly.
pub struct ChromeContext<'a> {
    pub selected: Option<&'a str>,
    pub selected_title: Option<&'a str>,
    pub selection_missing: bool,
    pub running_count: usize,
    pub selected_running: bool,
    pub library_count: usize,
    pub shown_count: usize,
    pub view_mode: ViewMode,
    pub icon_size: IconSize,
    pub sort_order: SortOrder,
    pub sort_descending: bool,
    pub favorites_only: bool,
    pub log_visible: bool,
    pub log_errors: u64,
    pub log_warnings: u64,
    pub update_summary: String,
    pub version: &'a str,
    pub developer_mode: bool,
}

pub fn menu_bar(ui: &mut Ui, context: &ChromeContext<'_>, actions: &mut Vec<Action>) {
    egui::MenuBar::new().ui(ui, |ui| {
        let can_play = context.selected.is_some() && !context.selection_missing;
        let selected = context.selected.map(str::to_string);

        ui.menu_button("File", |ui| {
            ui.set_min_width(200.0);
            if ui.button("Add App…").clicked() {
                actions.push(Action::AddApps);
                ui.close();
            }
            if ui.button("Add Folder…").clicked() {
                actions.push(Action::AddFolder);
                ui.close();
            }
            if ui.button("Rescan Library").clicked() {
                actions.push(Action::RefreshLibrary);
                ui.close();
            }
            ui.separator();
            if ui.button("Open tapHLE Folder").clicked() {
                actions.push(Action::OpenUserDataFolder);
                ui.close();
            }
            if ui.button("Open Apps Folder").clicked() {
                actions.push(Action::OpenAppsFolder);
                ui.close();
            }
            ui.separator();
            if ui.button("Exit").clicked() {
                actions.push(Action::Quit);
                ui.close();
            }
        });

        ui.menu_button("Emulation", |ui| {
            ui.set_min_width(220.0);
            if ui
                .add_enabled(
                    can_play && !context.selected_running,
                    egui::Button::new("Play"),
                )
                .clicked()
            {
                if let Some(id) = &selected {
                    actions.push(Action::Play(id.clone()));
                }
                ui.close();
            }
            if ui
                .add_enabled(context.running_count > 0, egui::Button::new("Stop"))
                .clicked()
            {
                actions.push(Action::StopAll);
                ui.close();
            }
            ui.separator();
            if ui
                .add_enabled(
                    context.selected.is_some(),
                    egui::Button::new("Settings for This App…"),
                )
                .clicked()
            {
                if let Some(id) = &selected {
                    actions.push(Action::OpenAppSettings(id.clone()));
                }
                ui.close();
            }
            if ui
                .add_enabled(
                    context.selected.is_some(),
                    egui::Button::new("Compatibility Report…"),
                )
                .clicked()
            {
                if let Some(id) = &selected {
                    actions.push(Action::OpenCompatibilityReport(id.clone()));
                }
                ui.close();
            }
        });

        ui.menu_button("View", |ui| {
            ui.set_min_width(210.0);
            if ui
                .radio(context.view_mode == ViewMode::Grid, "Grid")
                .clicked()
            {
                actions.push(Action::SetViewMode(ViewMode::Grid));
                ui.close();
            }
            if ui
                .radio(context.view_mode == ViewMode::List, "List")
                .clicked()
            {
                actions.push(Action::SetViewMode(ViewMode::List));
                ui.close();
            }
            ui.separator();
            ui.menu_button("Icon Size", |ui| {
                for size in IconSize::ALL {
                    if ui.radio(context.icon_size == *size, size.label()).clicked() {
                        actions.push(Action::SetIconSize(*size));
                        ui.close();
                    }
                }
            });
            ui.menu_button("Sort By", |ui| {
                for order in SortOrder::ALL {
                    if ui
                        .radio(context.sort_order == *order, order.label())
                        .clicked()
                    {
                        actions.push(Action::SetSortOrder(*order));
                        ui.close();
                    }
                }
                ui.separator();
                let mut descending = context.sort_descending;
                if ui.checkbox(&mut descending, "Reverse Order").clicked() {
                    actions.push(Action::ToggleSortDirection);
                    ui.close();
                }
            });
            let mut favourites = context.favorites_only;
            if ui.checkbox(&mut favourites, "Favourites Only").clicked() {
                actions.push(Action::ToggleFavoritesOnly);
                ui.close();
            }
            ui.separator();
            let mut log = context.log_visible;
            if ui.checkbox(&mut log, "Log / Output").clicked() {
                actions.push(Action::ToggleLogPanel);
                ui.close();
            }
        });

        ui.menu_button("Tools", |ui| {
            ui.set_min_width(230.0);
            if ui.button("Settings…").clicked() {
                actions.push(Action::OpenGlobalSettings);
                ui.close();
            }
            ui.separator();
            if ui.button("Refresh Compatibility Ratings").clicked() {
                actions.push(Action::RefreshCompatibility);
                ui.close();
            }
            if ui.button("Check for Updates").clicked() {
                actions.push(Action::CheckForUpdates);
                ui.close();
            }
            ui.separator();
            if ui
                .button("Copy Diagnostics")
                .on_hover_text("Copy build, platform and recent log information")
                .clicked()
            {
                actions.push(Action::CopyDiagnostics(
                    selected.clone().unwrap_or_default(),
                ));
                ui.close();
            }
            if context.developer_mode && ui.button("Clear Log").clicked() {
                actions.push(Action::ClearLog);
                ui.close();
            }
        });

        ui.menu_button("Help", |ui| {
            ui.set_min_width(220.0);
            if ui.button("About tapHLE").clicked() {
                actions.push(Action::ShowAbout);
                ui.close();
            }
            ui.separator();
            if ui.button("Project on GitHub").clicked() {
                actions.push(Action::OpenUrl(
                    "https://github.com/ephun/tapHLE".to_string(),
                ));
                ui.close();
            }
            if ui.button("Compatibility Database").clicked() {
                actions.push(Action::OpenUrl(crate::compat::DATABASE_WEB_URL.to_string()));
                ui.close();
            }
        });
    });
}

pub fn toolbar(
    ui: &mut Ui,
    context: &ChromeContext<'_>,
    search: &mut String,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.add_space(2.0);

        if ui::toolbar_button(
            ui,
            Icon::Add,
            Some("Add App"),
            "Add an .ipa file or .app bundle",
            true,
        )
        .clicked()
        {
            actions.push(Action::AddApps);
        }
        separator(ui);

        let can_play =
            context.selected.is_some() && !context.selection_missing && !context.selected_running;
        let play_hint = match (context.selected, context.selection_missing) {
            (None, _) => "Select an app first",
            (Some(_), true) => "This app's file is missing",
            _ if context.selected_running => "This app is already running",
            _ => "Start the selected app in its own window",
        };
        if ui::toolbar_button(ui, Icon::Play, Some("Play"), play_hint, can_play).clicked() {
            if let Some(id) = context.selected {
                actions.push(Action::Play(id.to_string()));
            }
        }
        let can_stop = context.running_count > 0;
        if ui::toolbar_button(
            ui,
            Icon::Stop,
            Some("Stop"),
            if can_stop {
                "Stop the running app"
            } else {
                "Nothing is running"
            },
            can_stop,
        )
        .clicked()
        {
            actions.push(Action::StopAll);
        }
        separator(ui);

        if ui::toolbar_button(ui, Icon::Refresh, None, "Rescan the library folders", true).clicked()
        {
            actions.push(Action::RefreshLibrary);
        }
        if ui::toolbar_button(
            ui,
            Icon::Settings,
            Some("Settings"),
            "Global settings",
            true,
        )
        .clicked()
        {
            actions.push(Action::OpenGlobalSettings);
        }
        if ui::toolbar_button(
            ui,
            Icon::Log,
            None,
            if context.log_visible {
                "Hide the log panel"
            } else {
                "Show the log panel"
            },
            true,
        )
        .clicked()
        {
            actions.push(Action::ToggleLogPanel);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            let (grid, list) = (
                context.view_mode == ViewMode::Grid,
                context.view_mode == ViewMode::List,
            );
            if ui::toolbar_button(ui, Icon::List, None, "Show as a list", !list).clicked() {
                actions.push(Action::SetViewMode(ViewMode::List));
            }
            if ui::toolbar_button(ui, Icon::Grid, None, "Show as a grid", !grid).clicked() {
                actions.push(Action::SetViewMode(ViewMode::Grid));
            }
            separator(ui);
            ui.add(
                egui::TextEdit::singleline(search)
                    .desired_width(170.0)
                    .hint_text("Search library"),
            );
            let (icon_rect, _) =
                ui.allocate_exact_size(egui::Vec2::splat(14.0), egui::Sense::hover());
            ui::draw_icon(ui.painter(), icon_rect, Icon::Search, theme::LIGHT.text_dim);
        });
    });
}

fn separator(ui: &mut Ui) {
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 18.0), egui::Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        egui::Stroke::new(1.0_f32, theme::LIGHT.border),
    );
    ui.add_space(3.0);
}

pub fn status_bar(ui: &mut Ui, context: &ChromeContext<'_>, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        let palette = &theme::LIGHT;
        let small = |text: String| egui::RichText::new(text).small().color(palette.text_dim);

        let count = if context.shown_count == context.library_count {
            format!(
                "{} app{}",
                context.library_count,
                if context.library_count == 1 { "" } else { "s" }
            )
        } else {
            format!("{} of {} apps", context.shown_count, context.library_count)
        };
        ui.label(small(count));

        if let Some(title) = context.selected_title {
            status_divider(ui);
            ui.label(small(title.to_string()));
        }
        if context.running_count > 0 {
            status_divider(ui);
            ui.label(
                egui::RichText::new(if context.running_count == 1 {
                    "Running".to_string()
                } else {
                    format!("{} running", context.running_count)
                })
                .small()
                .color(palette.accent),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            ui.label(small(context.version.to_string()));
            status_divider(ui);
            ui.label(small(context.update_summary.clone()));
            status_divider(ui);

            // The log indicator is the collapsed panel's affordance: it says
            // whether anything went wrong and opens the panel when clicked.
            let (text, colour) = match (context.log_errors, context.log_warnings) {
                (0, 0) => ("Log".to_string(), palette.text_dim),
                (0, warnings) => (format!("{warnings} warnings"), palette.warning),
                (errors, 0) => (format!("{errors} errors"), palette.error),
                (errors, warnings) => (
                    format!("{errors} errors, {warnings} warnings"),
                    palette.error,
                ),
            };
            let response = ui.add(
                egui::Label::new(egui::RichText::new(text).small().color(colour))
                    .sense(egui::Sense::click()),
            );
            if response.on_hover_text("Show the log panel").clicked() {
                actions.push(Action::ShowLogPanel(true));
            }
        });
    });
}

fn status_divider(ui: &mut Ui) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new("│").small().color(theme::LIGHT.border));
    ui.add_space(2.0);
}
