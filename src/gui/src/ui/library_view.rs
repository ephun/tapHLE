/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The library itself: a grid of app icons, or a compact list.
//!
//! The grid takes its proportions from an iPad home screen in landscape —
//! icons on a regular lattice, the name centred beneath, generous but
//! unfussy spacing — because that is what a shelf of iPhone and iPad apps
//! looks like, and it says what the library holds before a word is read. It
//! is not a reproduction: there are no pages, the columns follow the window
//! rather than the device, and it scrolls the way a desktop list scrolls.
//!
//! Both views are drawn a row at a time through [egui::ScrollArea::show_rows],
//! which only builds the rows on screen. A library of five apps and a library
//! of five thousand cost the same to draw.

use std::collections::HashMap;

use egui::{Color32, Rect, Sense, Stroke, Ui, Vec2};

use crate::compat::DatabaseSnapshot;
use crate::library::{Library, LibraryEntry, VersionGroup};
use crate::settings::ViewMode;
use crate::theme;
use crate::timefmt;
use crate::ui::{self, Action};

/// Everything the library view needs to draw itself.
pub struct LibraryContext<'a> {
    pub library: &'a Library,
    /// One place per app, already filtered and sorted, each holding the
    /// versions of that app that survived the filter.
    pub groups: &'a [VersionGroup],
    pub icons: &'a HashMap<String, egui::TextureHandle>,
    pub database: &'a DatabaseSnapshot,
    pub selected: Option<&'a str>,
    /// Entry identifiers with a run in progress.
    pub running: &'a [String],
    pub icon_points: f32,
    pub view: ViewMode,
    /// Whether anything is in the library at all, as opposed to filtered out.
    pub library_is_empty: bool,
}

pub fn show(ui: &mut Ui, context: &LibraryContext<'_>, actions: &mut Vec<Action>) {
    ui.painter()
        .rect_filled(ui.max_rect(), 0.0, theme::LIGHT.content);

    if context.groups.is_empty() {
        show_empty(ui, context, actions);
        return;
    }

    match context.view {
        ViewMode::Grid => show_grid(ui, context, actions),
        ViewMode::List => show_list(ui, context, actions),
    }
}

fn show_empty(ui: &mut Ui, context: &LibraryContext<'_>, actions: &mut Vec<Action>) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.28);
        if context.library_is_empty {
            ui.label(
                egui::RichText::new("No apps in the library")
                    .heading()
                    .color(theme::LIGHT.text_dim),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(
                    "Add an .ipa file or an .app bundle, or drop one onto this window.",
                )
                .color(theme::LIGHT.text_dim),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                // Centre the pair of buttons by padding to the left of them.
                let width = 250.0;
                ui.add_space((ui.available_width() - width).max(0.0) / 2.0);
                if ui.button("Add App…").clicked() {
                    actions.push(Action::AddApps);
                }
                if ui.button("Add Folder…").clicked() {
                    actions.push(Action::AddFolder);
                }
                if ui.button("Rescan tapHLE_apps").clicked() {
                    actions.push(Action::RefreshLibrary);
                }
            });
        } else {
            ui.label(
                egui::RichText::new("Nothing matches the current filter")
                    .color(theme::LIGHT.text_dim),
            );
        }
    });
}

/// The size of one cell in the grid.
///
/// The horizontal and vertical multipliers are where the home-screen rhythm
/// lives: an iPad leaves roughly nine tenths of an icon's width between
/// columns and rather less between rows, because the name below the icon
/// already separates them.
fn cell_size(icon_points: f32) -> Vec2 {
    let width = (icon_points * 1.85).max(84.0);
    let label_height = 30.0;
    Vec2::new(width, icon_points + label_height + 16.0)
}

fn show_grid(ui: &mut Ui, context: &LibraryContext<'_>, actions: &mut Vec<Action>) {
    let cell = cell_size(context.icon_points);
    let available = ui.available_width() - ui.spacing().scroll.bar_width - 8.0;
    let columns = ((available / cell.x).floor() as usize).max(1);
    let rows = context.groups.len().div_ceil(columns);
    // Spread the leftover width so the lattice sits centred rather than
    // hard against the left edge with a ragged gap on the right.
    let margin = ((available - columns as f32 * cell.x) / 2.0).max(0.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, cell.y, rows, |ui, row_range| {
            for row in row_range {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.add_space(margin + 4.0);
                    for column in 0..columns {
                        let Some(group) = context.groups.get(row * columns + column) else {
                            break;
                        };
                        grid_cell(ui, context, group, cell, actions);
                    }
                });
            }
        });
}

fn grid_cell(
    ui: &mut Ui,
    context: &LibraryContext<'_>,
    group: &VersionGroup,
    cell: Vec2,
    actions: &mut Vec<Action>,
) {
    let entry = &context.library.entries[group.shown];
    let (rect, response) = ui.allocate_exact_size(cell, Sense::click());
    let selected = is_selected(context, group);
    let running = context.running.contains(&entry.id);
    let palette = &theme::LIGHT;

    if selected {
        ui.painter().rect(
            rect.shrink(3.0),
            4.0,
            palette.selection_fill,
            Stroke::new(1.0_f32, palette.selection_stroke),
            egui::StrokeKind::Inside,
        );
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect.shrink(3.0), 4.0, palette.hover_fill);
    }

    let icon_rect = Rect::from_center_size(
        egui::pos2(
            rect.center().x,
            rect.top() + 10.0 + context.icon_points / 2.0,
        ),
        Vec2::splat(context.icon_points),
    );
    draw_entry_icon(ui, context, entry, icon_rect);

    if running {
        // A small filled triangle in the corner: the app is running now.
        let badge = Rect::from_center_size(
            egui::pos2(icon_rect.right() - 3.0, icon_rect.bottom() - 3.0),
            Vec2::splat(15.0),
        );
        ui.painter()
            .circle_filled(badge.center(), 7.5, Color32::WHITE);
        ui.painter()
            .circle_stroke(badge.center(), 7.5, Stroke::new(1.0_f32, palette.accent));
        ui::draw_icon(
            ui.painter(),
            badge.shrink(4.0),
            ui::Icon::Play,
            palette.accent,
        );
    }
    if entry.favorite {
        let badge = Rect::from_center_size(
            egui::pos2(icon_rect.left() + 3.0, icon_rect.top() + 3.0),
            Vec2::splat(13.0),
        );
        ui::draw_icon(ui.painter(), badge, ui::Icon::Star, palette.star);
    }

    let label_rect = Rect::from_min_max(
        egui::pos2(rect.left() + 4.0, icon_rect.bottom() + 5.0),
        egui::pos2(rect.right() - 4.0, rect.bottom() - 2.0),
    );
    let colour = if entry.missing {
        palette.text_dim
    } else {
        palette.text
    };
    // Two lines at most, with an ellipsis: long app names are the norm here
    // and must not push the lattice out of alignment.
    let galley = {
        let mut job = egui::text::LayoutJob::simple(
            entry.title().to_string(),
            egui::TextStyle::Small.resolve(ui.style()),
            colour,
            label_rect.width(),
        );
        job.halign = egui::Align::Center;
        job.wrap = egui::text::TextWrapping {
            max_width: label_rect.width(),
            max_rows: 2,
            break_anywhere: false,
            overflow_character: Some('…'),
        };
        ui.painter().layout_job(job)
    };
    ui.painter().galley(
        egui::pos2(label_rect.center().x, label_rect.top()),
        galley,
        colour,
    );

    // Which version is on show is decided in the details sidebar, and only
    // there. A dropdown in every cell put a control in the middle of what is
    // meant to read as a home screen, and repeated it once per app; the
    // sidebar already names the selected version next to the rating, the
    // settings and the Play button that all belong to it.
    handle_entry_interaction(ui, context, group, &response, actions);
}

/// Whether the selection is anywhere in this group.
///
/// The selection follows a version, not an app, because settings, ratings and
/// play time all belong to the exact build. A group is highlighted when any
/// of its versions is selected, so picking a version out of the dropdown does
/// not look like it deselected the app.
fn is_selected(context: &LibraryContext<'_>, group: &VersionGroup) -> bool {
    let Some(selected) = context.selected else {
        return false;
    };
    group
        .versions
        .iter()
        .any(|&index| context.library.entries[index].id == selected)
}

/// One version's name, as distinct from the others in its group.
fn group_label(context: &LibraryContext<'_>, group: &VersionGroup, entry: &LibraryEntry) -> String {
    crate::library::version_label(
        entry,
        group
            .versions
            .iter()
            .map(|&index| &context.library.entries[index]),
    )
}

/// The list of versions, wherever one can be picked.
fn version_menu(
    ui: &mut Ui,
    context: &LibraryContext<'_>,
    group: &VersionGroup,
    actions: &mut Vec<Action>,
) {
    ui.set_min_width(180.0);
    let shown = context.library.entries[group.shown].id.as_str();
    for &index in &group.versions {
        let entry = &context.library.entries[index];
        let mut label = format!("Version {}", group_label(context, group, entry));
        if entry.missing {
            label.push_str(" — file missing");
        }
        if ui
            .selectable_label(entry.id == shown, label)
            .on_hover_text(&entry.id)
            .clicked()
        {
            actions.push(Action::ChooseVersion {
                bundle_identifier: entry.metadata.bundle_identifier.clone(),
                entry_id: entry.id.clone(),
            });
            ui.close();
        }
    }
}

fn draw_entry_icon(ui: &mut Ui, context: &LibraryContext<'_>, entry: &LibraryEntry, rect: Rect) {
    let palette = &theme::LIGHT;
    match context.icons.get(&entry.id) {
        Some(texture) => {
            let tint = if entry.missing {
                Color32::from_white_alpha(90)
            } else {
                Color32::WHITE
            };
            egui::Image::new((texture.id(), rect.size()))
                .tint(tint)
                .paint_at(ui, rect);
        }
        None => {
            // A placeholder shaped like an icon, so the lattice stays even
            // for an app whose icon could not be read.
            ui.painter().rect(
                rect,
                rect.width() * 0.175,
                Color32::from_gray(0xE8),
                Stroke::new(1.0_f32, palette.border),
                egui::StrokeKind::Inside,
            );
            let initial = entry
                .title()
                .chars()
                .find(|c| c.is_alphanumeric())
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                initial,
                egui::FontId::proportional(rect.width() * 0.45),
                palette.text_dim,
            );
        }
    }
    if entry.missing {
        ui.painter().text(
            rect.center_bottom(),
            egui::Align2::CENTER_CENTER,
            "file missing",
            egui::FontId::proportional(9.0),
            palette.error,
        );
    }
}

fn show_list(ui: &mut Ui, context: &LibraryContext<'_>, actions: &mut Vec<Action>) {
    let row_height = 26.0;
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show_rows(ui, row_height, context.groups.len(), |ui, row_range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for row in row_range {
                list_row(ui, context, &context.groups[row], row_height, row, actions);
            }
        });
}

fn list_row(
    ui: &mut Ui,
    context: &LibraryContext<'_>,
    group: &VersionGroup,
    height: f32,
    row: usize,
    actions: &mut Vec<Action>,
) {
    let entry = &context.library.entries[group.shown];
    let width = ui.available_width().max(560.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let selected = is_selected(context, group);
    let palette = &theme::LIGHT;

    if selected {
        ui.painter().rect_filled(rect, 0.0, palette.selection_fill);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, palette.hover_fill);
    } else if row % 2 == 1 {
        // Alternating rows, the way a desktop list view has always done it.
        ui.painter()
            .rect_filled(rect, 0.0, Color32::from_gray(0xFA));
    }

    let icon_rect = Rect::from_center_size(
        egui::pos2(rect.left() + 14.0, rect.center().y),
        Vec2::splat(18.0),
    );
    draw_entry_icon(ui, context, entry, icon_rect);

    let font = egui::TextStyle::Body.resolve(ui.style());
    let colour = if entry.missing {
        palette.text_dim
    } else {
        palette.text
    };
    let mut x = rect.left() + 28.0;
    let columns: [(f32, String); 4] = [
        (250.0, entry.title().to_string()),
        (160.0, entry.metadata.publisher.clone().unwrap_or_default()),
        (90.0, entry.metadata.version_for_display().to_string()),
        (
            110.0,
            entry
                .last_played
                .map(|when| timefmt::format_relative(when, timefmt::now_seconds()))
                .unwrap_or_default(),
        ),
    ];
    for (column_width, text) in columns {
        let galley = {
            let mut job =
                egui::text::LayoutJob::simple(text, font.clone(), colour, column_width - 8.0);
            job.wrap = egui::text::TextWrapping {
                max_width: column_width - 8.0,
                max_rows: 1,
                break_anywhere: false,
                overflow_character: Some('…'),
            };
            ui.painter().layout_job(job)
        };
        ui.painter().galley(
            egui::pos2(x, rect.center().y - galley.size().y / 2.0),
            galley,
            colour,
        );
        x += column_width;
    }

    let rating = entry.local_rating.stars.or_else(|| {
        context
            .database
            .find(&entry.metadata.bundle_identifier)?
            .rating
    });
    if let Some(rating) = rating {
        for star in 0..rating.min(5) {
            ui.painter().circle_filled(
                egui::pos2(x + 6.0 + star as f32 * 9.0, rect.center().y),
                3.0,
                palette.star,
            );
        }
    }

    handle_entry_interaction(ui, context, group, &response, actions);
}

/// Clicking, double-clicking and the context menu, shared by both views.
fn handle_entry_interaction(
    _ui: &mut Ui,
    context: &LibraryContext<'_>,
    group: &VersionGroup,
    response: &egui::Response,
    actions: &mut Vec<Action>,
) {
    let entry = &context.library.entries[group.shown];
    if response.clicked() || response.secondary_clicked() {
        actions.push(Action::Select(entry.id.clone()));
    }
    if response.double_clicked() {
        actions.push(Action::Play(entry.id.clone()));
    }
    response.clone().context_menu(|ui| {
        ui.set_min_width(190.0);
        let running = context.running.contains(&entry.id);
        if ui
            .add_enabled(!entry.missing, egui::Button::new("Play"))
            .clicked()
        {
            actions.push(Action::Play(entry.id.clone()));
            ui.close();
        }
        if running && ui.button("Stop").clicked() {
            actions.push(Action::StopAll);
            ui.close();
        }
        if group.has_choice() {
            ui.menu_button(
                format!("Version ({})", group_label(context, group, entry)),
                |ui| version_menu(ui, context, group, actions),
            );
        }
        ui.separator();
        if ui.button("Settings for this App…").clicked() {
            actions.push(Action::OpenAppSettings(entry.id.clone()));
            ui.close();
        }
        if ui
            .button(if entry.favorite {
                "Remove from Favourites"
            } else {
                "Add to Favourites"
            })
            .clicked()
        {
            actions.push(Action::ToggleFavorite(entry.id.clone()));
            ui.close();
        }
        ui.separator();
        if ui.button("Copy Bundle ID").clicked() {
            actions.push(Action::CopyText(entry.metadata.bundle_identifier.clone()));
            ui.close();
        }
        if ui.button("Copy App Details").clicked() {
            actions.push(Action::CopyText(describe_entry(entry)));
            ui.close();
        }
        if ui.button("Show App File").clicked() {
            let folder = entry
                .path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| entry.path.clone());
            actions.push(Action::OpenPath(folder));
            ui.close();
        }
        if ui.button("Open Saved Data Folder").clicked() {
            actions.push(Action::OpenPath(sandbox_folder(entry)));
            ui.close();
        }
        ui.separator();
        let has_entry = context
            .database
            .find(&entry.metadata.bundle_identifier)
            .is_some();
        if ui
            .add_enabled(has_entry, egui::Button::new("Open Compatibility Entry"))
            .on_disabled_hover_text("The compatibility database has no record for this app yet")
            .clicked()
        {
            actions.push(Action::OpenCompatibilityEntry(entry.id.clone()));
            ui.close();
        }
        if ui.button("Compatibility Report…").clicked() {
            actions.push(Action::OpenCompatibilityReport(entry.id.clone()));
            ui.close();
        }
        ui.separator();
        if ui.button("Remove from Library").clicked() {
            actions.push(Action::RemoveFromLibrary(entry.id.clone()));
            ui.close();
        }
    });
}

/// Where the emulator keeps an app's saved data.
///
/// This mirrors the emulator's own layout rather than asking it, because the
/// folder only comes into existence once the app has run.
pub fn sandbox_folder(entry: &LibraryEntry) -> std::path::PathBuf {
    crate::storage::data_dir()
        .join(tapHLE::paths::SANDBOX_DIR)
        .join(&entry.metadata.bundle_identifier)
}

/// The app's metadata as text, for the clipboard.
pub fn describe_entry(entry: &LibraryEntry) -> String {
    let metadata = &entry.metadata;
    let mut text = format!(
        "{}\nBundle identifier: {}\nBundle version: {}\n",
        metadata.title(),
        metadata.bundle_identifier,
        metadata.bundle_version
    );
    if let Some(short) = &metadata.short_version {
        text.push_str(&format!("Short version: {short}\n"));
    }
    if let Some(publisher) = &metadata.publisher {
        text.push_str(&format!("Publisher: {publisher}\n"));
    }
    if let Some(minimum) = &metadata.minimum_os_version {
        text.push_str(&format!("Minimum OS version: {minimum}\n"));
    }
    text.push_str(&format!(
        "Device family: {}\nPath: {}\n",
        metadata.device_family_summary(),
        crate::storage::display_path(&entry.path)
    ));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lattice has to breathe: an iPad's icons are nowhere near
    /// touching. A cell narrower than the icon would overlap them.
    #[test]
    fn a_cell_is_wider_than_its_icon() {
        for icon in [48.0, 64.0, 88.0] {
            let cell = cell_size(icon);
            assert!(cell.x > icon * 1.5, "cells are too tight at {icon}pt");
            assert!(cell.y > icon + 30.0, "no room for the name at {icon}pt");
        }
    }

    /// A very small icon size must still give a cell wide enough for a name
    /// to be readable, which is why there is a floor.
    #[test]
    fn small_icons_still_get_a_usable_cell() {
        assert!(cell_size(24.0).x >= 84.0);
    }
}
