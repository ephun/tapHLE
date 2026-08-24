/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The panel on the right: everything known about the selected app.
//!
//! Selecting something and reading about it beside the list is an old,
//! comfortable desktop arrangement. The restraint here is deliberate: rows of
//! label and value, a rule between sections, no cards and no colour beyond
//! the compatibility stars. A field with nothing behind it is not shown at
//! all rather than shown empty, because half of these apps record nothing but
//! their identifier and a panel full of "Unknown" tells the reader less than
//! a short one.

use egui::Ui;

use crate::state::compat::DatabaseSnapshot;
use crate::state::library::LibraryEntry;
use crate::state::timefmt;
use crate::state::Action;
use crate::ui::theme;
use crate::ui::widgets;

pub struct DetailsContext<'a> {
    pub entry: Option<&'a LibraryEntry>,
    pub icon: Option<&'a egui::TextureHandle>,
    pub database: &'a DatabaseSnapshot,
    pub running: bool,
    /// Whether the compatibility database has been read successfully.
    pub database_available: bool,
    /// Every version of the selected app, newest first. One element when the
    /// library holds only the one.
    pub versions: Vec<&'a LibraryEntry>,
}

pub fn show(ui: &mut Ui, context: &DetailsContext<'_>, actions: &mut Vec<Action>) {
    let Some(entry) = context.entry else {
        ui.vertical_centered(|ui| {
            ui.add_space(28.0);
            ui.label(
                egui::RichText::new("Select an app to see its details")
                    .color(theme::LIGHT.text_dim),
            );
        });
        return;
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            header(ui, context, entry, actions);
            compatibility(ui, context, entry, actions);
            details(ui, entry, actions);
            activity(ui, entry);
            ui.add_space(10.0);
        });
}

fn header(
    ui: &mut Ui,
    context: &DetailsContext<'_>,
    entry: &LibraryEntry,
    actions: &mut Vec<Action>,
) {
    ui.add_space(4.0);
    ui.horizontal_top(|ui| {
        let size = 56.0;
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::hover());
        match context.icon {
            Some(texture) => {
                egui::Image::new((texture.id(), rect.size())).paint_at(ui, rect);
            }
            None => {
                ui.painter().rect(
                    rect,
                    size * 0.175,
                    egui::Color32::from_gray(0xE8),
                    egui::Stroke::new(1.0_f32, theme::LIGHT.border),
                    egui::StrokeKind::Inside,
                );
            }
        }
        ui.vertical(|ui| {
            ui.add(egui::Label::new(egui::RichText::new(entry.title()).heading()).wrap());
            if let Some(publisher) = &entry.metadata.publisher {
                ui.label(egui::RichText::new(publisher).color(theme::LIGHT.text_dim));
            }
            version_row(ui, context, entry, actions);
        });
    });

    if entry.missing {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("The app's file is not where the library expects it.")
                .color(theme::LIGHT.error),
        );
        ui.label(
            egui::RichText::new(crate::platform::storage::display_path(&entry.path))
                .small()
                .color(theme::LIGHT.text_dim),
        );
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let play = egui::Button::new(if context.running { "Running" } else { "Play" })
            .min_size(egui::vec2(84.0, 26.0));
        if ui
            .add_enabled(!entry.missing && !context.running, play)
            .on_hover_text("Start this app in its own window")
            .clicked()
        {
            actions.push(Action::Play(entry.id.clone()));
        }
        if context.running && ui.button("Stop").clicked() {
            actions.push(Action::StopAll);
        }
        if ui
            .button("Settings…")
            .on_hover_text("Settings that apply to this app only")
            .clicked()
        {
            actions.push(Action::OpenAppSettings(entry.id.clone()));
        }
    });
    if !entry.overrides.is_empty() {
        ui.label(
            egui::RichText::new("This app has its own settings.")
                .small()
                .color(theme::LIGHT.accent),
        );
    }
}

/// The version line under the app's name.
///
/// With one version it is what it always was: a line of dim text. With
/// several it becomes the dropdown that decides which of them everything
/// below — the rating, the settings, the Play button — is about, because
/// those belong to the exact build rather than to the app in general.
fn version_row(
    ui: &mut Ui,
    context: &DetailsContext<'_>,
    entry: &LibraryEntry,
    actions: &mut Vec<Action>,
) {
    if context.versions.len() < 2 {
        ui.label(
            egui::RichText::new(format!("Version {}", entry.metadata.version_for_display()))
                .small()
                .color(theme::LIGHT.text_dim),
        );
        return;
    }

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Version")
                .small()
                .color(theme::LIGHT.text_dim),
        );
        egui::ComboBox::from_id_salt("details-version")
            .selected_text(crate::state::library::version_label(
                entry,
                context.versions.iter().copied(),
            ))
            .width(120.0)
            .show_ui(ui, |ui| {
                for other in &context.versions {
                    let mut label = crate::state::library::version_label(
                        other,
                        context.versions.iter().copied(),
                    );
                    if other.missing {
                        label.push_str(" — missing");
                    }
                    if ui
                        .selectable_label(other.id == entry.id, label)
                        .on_hover_text(&other.id)
                        .clicked()
                    {
                        actions.push(Action::ChooseVersion {
                            bundle_identifier: other.metadata.bundle_identifier.clone(),
                            entry_id: other.id.clone(),
                        });
                    }
                }
            })
            .response
            .on_hover_text(format!(
                "The library holds {} versions of this app",
                context.versions.len()
            ));
    });
}

fn compatibility(
    ui: &mut Ui,
    context: &DetailsContext<'_>,
    entry: &LibraryEntry,
    actions: &mut Vec<Action>,
) {
    widgets::section(ui, "Compatibility");
    let record = context.database.find(&entry.metadata.bundle_identifier);

    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(egui::RichText::new("Database").color(theme::LIGHT.text_dim)).wrap(),
        );
        match record.and_then(|record| record.rating) {
            Some(rating) => {
                widgets::stars(ui, Some(rating), 13.0);
                ui.label(egui::RichText::new(format!("{rating}/5")).small());
            }
            None if !context.database_available => {
                ui.label(
                    egui::RichText::new("not available")
                        .small()
                        .color(theme::LIGHT.text_dim),
                );
            }
            None => {
                ui.label(
                    egui::RichText::new("no record yet")
                        .small()
                        .color(theme::LIGHT.text_dim),
                );
            }
        }
    });

    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(egui::RichText::new("This machine").color(theme::LIGHT.text_dim))
                .wrap(),
        );
        if let Some(new_rating) = widgets::star_picker(ui, entry.local_rating.stars) {
            actions.push(Action::SetLocalRating(entry.id.clone(), new_rating));
        }
    });
    if entry.local_rating.stars.is_some() {
        ui.label(
            egui::RichText::new(
                "Your own rating. It is kept on this computer and is not sent anywhere.",
            )
            .small()
            .color(theme::LIGHT.text_dim),
        );
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(record.is_some(), egui::Button::new("Open entry"))
            .on_disabled_hover_text("The database has no record for this app yet")
            .clicked()
        {
            actions.push(Action::OpenCompatibilityEntry(entry.id.clone()));
        }
        if ui
            .button("Report…")
            .on_hover_text("Assemble a compatibility report for this app")
            .clicked()
        {
            actions.push(Action::OpenCompatibilityReport(entry.id.clone()));
        }
    });
}

fn details(ui: &mut Ui, entry: &LibraryEntry, actions: &mut Vec<Action>) {
    widgets::section(ui, "Details");
    let metadata = &entry.metadata;

    let identifier = widgets::selectable_field(ui, "Bundle ID", &metadata.bundle_identifier);
    if identifier.clicked() {
        actions.push(Action::CopyText(metadata.bundle_identifier.clone()));
    }
    identifier.on_hover_text("Click to copy");

    widgets::field(ui, "Bundle version", &metadata.bundle_version);
    if let Some(short) = &metadata.short_version {
        widgets::field(ui, "Short version", short);
    }
    if let Some(minimum) = &metadata.minimum_os_version {
        widgets::field(ui, "Minimum iOS", minimum);
    }
    widgets::field(ui, "Device family", &metadata.device_family_summary());
    if !metadata.supported_orientations.is_empty() {
        widgets::field(
            ui,
            "Orientations",
            &metadata
                .supported_orientations
                .iter()
                .map(|o| {
                    o.trim_start_matches("UIInterfaceOrientation")
                        .trim_start_matches("UIDeviceOrientation")
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if !metadata.required_capabilities.is_empty() {
        widgets::field(ui, "Requires", &metadata.required_capabilities.join(", "));
    }
    if let Some(genre) = &metadata.genre {
        widgets::field(ui, "Genre", genre);
    }
    if let Some(released) = &metadata.release_date {
        // The store records a full timestamp; the date is the useful part.
        widgets::field(ui, "Released", &released[..released.len().min(10)]);
    }
    if let Some(size) = metadata.size_bytes {
        widgets::field(ui, "Size", &format_size(size));
    }

    let path = crate::platform::storage::display_path(&entry.path);
    let path_row = widgets::selectable_field(ui, "Location", &path);
    if path_row.clicked() {
        actions.push(Action::CopyText(path.clone()));
    }
    path_row.on_hover_text(&path);
}

fn activity(ui: &mut Ui, entry: &LibraryEntry) {
    widgets::section(ui, "Activity");
    let now = timefmt::now_seconds();
    match entry.last_played {
        Some(when) => {
            let row =
                widgets::selectable_field(ui, "Last played", &timefmt::format_relative(when, now));
            row.on_hover_text(timefmt::format_datetime(when));
        }
        None => widgets::field(ui, "Last played", "never"),
    }
    if entry.play_seconds > 0 {
        widgets::field(
            ui,
            "Time played",
            &timefmt::format_duration(entry.play_seconds),
        );
    }
    if entry.play_count > 0 {
        widgets::field(ui, "Times launched", &entry.play_count.to_string());
    }
    if entry.added > 0 {
        widgets::field(ui, "Added", &timefmt::format_date(entry.added));
    }
}

/// A file size in the units a person reads.
pub fn format_size(bytes: u64) -> String {
    const MEGABYTE: f64 = 1024.0 * 1024.0;
    if bytes >= (MEGABYTE as u64) {
        format!("{:.1} MB", bytes as f64 / MEGABYTE)
    } else {
        format!("{} KB", (bytes as f64 / 1024.0).ceil() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::format_size;

    #[test]
    fn sizes_read_the_way_a_file_manager_shows_them() {
        assert_eq!(format_size(1024), "1 KB");
        assert_eq!(format_size(1_500), "2 KB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(format_size(12_582_912), "12.0 MB");
    }
}
