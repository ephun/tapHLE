/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The interface: shared pieces, and the modules that make up the window.
//!
//! Every part of the interface reports what the person asked for as an
//! [Action] rather than acting on it. The window is built first and the
//! actions are applied afterwards, in [crate::app]. That is not ceremony: an
//! immediate-mode interface is drawn while the state it describes is
//! borrowed, so a menu item that removed a library entry mid-draw would be
//! reaching into the list it is iterating. Collecting intent and applying it
//! once keeps every one of those cases out of the interface code.

pub mod chrome;
pub mod control_editor;
pub mod details;
pub mod dialogs;
pub mod library_view;
pub mod logpanel;
pub mod settings_dialog;

use egui::{Color32, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::settings::{IconSize, SortOrder, ViewMode};
use crate::theme;

/// Something the person asked for.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    AddApps,
    AddFolder,
    RefreshLibrary,
    Play(String),
    StopAll,
    Select(String),
    OpenGlobalSettings,
    OpenAppSettings(String),
    RemoveFromLibrary(String),
    /// Remove without asking again, which is what the confirmation sends.
    ConfirmedRemove(String),
    ToggleFavorite(String),
    /// Show a different version of an app: its bundle identifier, and the
    /// entry to show for it.
    ChooseVersion {
        bundle_identifier: String,
        entry_id: String,
    },
    CopyText(String),
    OpenPath(std::path::PathBuf),
    OpenUrl(String),
    OpenCompatibilityEntry(String),
    OpenCompatibilityReport(String),
    SetLocalRating(String, Option<u8>),
    ShowAbout,
    ShowLogPanel(bool),
    ToggleLogPanel,
    SetViewMode(ViewMode),
    SetIconSize(IconSize),
    SetSortOrder(SortOrder),
    ToggleSortDirection,
    ToggleFavoritesOnly,
    ClearLog,
    SaveLog,
    CopyLogSelection,
    CopyDiagnostics(String),
    CheckForUpdates,
    RefreshCompatibility,
    OpenUserDataFolder,
    OpenAppsFolder,
    Quit,
    /// Close even though apps are running, which is what the confirmation
    /// shown on close sends.
    ConfirmedQuit,
}

/// The icons drawn on toolbar buttons and in menus.
///
/// They are drawn rather than loaded, because a handful of flat shapes needs
/// neither an asset pipeline nor an icon font, and drawing them keeps them
/// crisp at every display scaling instead of blurring at 125%.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Icon {
    Add,
    Play,
    Stop,
    Settings,
    Refresh,
    Log,
    Grid,
    List,
    Star,
    Search,
}

pub fn draw_icon(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let stroke = Stroke::new(1.4_f32, color);
    let centre = rect.center();
    let size = rect.width().min(rect.height());
    let half = size * 0.5;
    match icon {
        Icon::Add => {
            painter.line_segment(
                [
                    egui::pos2(centre.x - half * 0.7, centre.y),
                    egui::pos2(centre.x + half * 0.7, centre.y),
                ],
                Stroke::new(1.8_f32, color),
            );
            painter.line_segment(
                [
                    egui::pos2(centre.x, centre.y - half * 0.7),
                    egui::pos2(centre.x, centre.y + half * 0.7),
                ],
                Stroke::new(1.8_f32, color),
            );
        }
        Icon::Play => {
            let points = vec![
                egui::pos2(centre.x - half * 0.45, centre.y - half * 0.72),
                egui::pos2(centre.x - half * 0.45, centre.y + half * 0.72),
                egui::pos2(centre.x + half * 0.75, centre.y),
            ];
            painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
        }
        Icon::Stop => {
            painter.rect_filled(
                Rect::from_center_size(centre, Vec2::splat(size * 0.62)),
                0.0,
                color,
            );
        }
        Icon::Settings => {
            // A gear reads as settings at any size; six teeth is enough to
            // suggest one without turning into a smudge at 16 pixels.
            let outer = half * 0.85;
            let inner = half * 0.42;
            for step in 0..6 {
                let angle = std::f32::consts::TAU * step as f32 / 6.0;
                let (sin, cos) = angle.sin_cos();
                painter.line_segment(
                    [
                        egui::pos2(centre.x + cos * inner, centre.y + sin * inner),
                        egui::pos2(centre.x + cos * outer, centre.y + sin * outer),
                    ],
                    Stroke::new(2.0_f32, color),
                );
            }
            painter.circle_stroke(centre, inner, stroke);
        }
        Icon::Refresh => {
            // Three quarters of a circle with an arrow head, the usual
            // rescan glyph.
            let radius = half * 0.68;
            let mut points = Vec::new();
            for step in 0..=24 {
                let angle = std::f32::consts::TAU * (0.12 + 0.76 * step as f32 / 24.0);
                let (sin, cos) = angle.sin_cos();
                points.push(egui::pos2(centre.x + cos * radius, centre.y + sin * radius));
            }
            painter.add(egui::Shape::line(points, stroke));
            let tip = egui::pos2(centre.x + radius, centre.y - half * 0.1);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(tip.x - half * 0.28, tip.y - half * 0.1),
                    egui::pos2(tip.x + half * 0.22, tip.y - half * 0.1),
                    egui::pos2(tip.x - half * 0.03, tip.y + half * 0.38),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Log => {
            // A window with a title bar and two lines of text in it.
            let frame = Rect::from_center_size(centre, Vec2::new(size * 0.78, size * 0.66));
            painter.rect_stroke(frame, 1.0, stroke, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    egui::pos2(frame.left(), frame.top() + frame.height() * 0.3),
                    egui::pos2(frame.right(), frame.top() + frame.height() * 0.3),
                ],
                stroke,
            );
            for row in 0..2 {
                let y = frame.top() + frame.height() * (0.52 + 0.24 * row as f32);
                painter.line_segment(
                    [
                        egui::pos2(frame.left() + 2.5, y),
                        egui::pos2(frame.right() - 2.5 - row as f32 * 3.0, y),
                    ],
                    Stroke::new(1.2_f32, color),
                );
            }
        }
        Icon::Grid => {
            for row in 0..2 {
                for column in 0..2 {
                    let cell = Rect::from_min_size(
                        egui::pos2(
                            centre.x - half * 0.72 + column as f32 * half * 0.78,
                            centre.y - half * 0.72 + row as f32 * half * 0.78,
                        ),
                        Vec2::splat(half * 0.58),
                    );
                    painter.rect_stroke(cell, 0.0, stroke, egui::StrokeKind::Inside);
                }
            }
        }
        Icon::List => {
            for row in 0..3 {
                let y = centre.y + (row as f32 - 1.0) * half * 0.6;
                painter.circle_filled(egui::pos2(centre.x - half * 0.7, y), 1.3, color);
                painter.line_segment(
                    [
                        egui::pos2(centre.x - half * 0.35, y),
                        egui::pos2(centre.x + half * 0.75, y),
                    ],
                    Stroke::new(1.3_f32, color),
                );
            }
        }
        Icon::Star => draw_star(painter, centre, half * 0.9, color, true),
        Icon::Search => {
            painter.circle_stroke(
                egui::pos2(centre.x - half * 0.15, centre.y - half * 0.15),
                half * 0.5,
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(centre.x + half * 0.2, centre.y + half * 0.2),
                    egui::pos2(centre.x + half * 0.7, centre.y + half * 0.7),
                ],
                Stroke::new(1.6_f32, color),
            );
        }
    }
}

fn draw_star(
    painter: &egui::Painter,
    centre: egui::Pos2,
    radius: f32,
    color: Color32,
    filled: bool,
) {
    let mut points = Vec::with_capacity(10);
    for step in 0..10 {
        let angle = std::f32::consts::TAU * step as f32 / 10.0 - std::f32::consts::FRAC_PI_2;
        let distance = if step % 2 == 0 { radius } else { radius * 0.45 };
        let (sin, cos) = angle.sin_cos();
        points.push(egui::pos2(
            centre.x + cos * distance,
            centre.y + sin * distance,
        ));
    }
    if filled {
        painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
    } else {
        points.push(points[0]);
        painter.add(egui::Shape::line(points, Stroke::new(1.2_f32, color)));
    }
}

/// A flat toolbar button: icon, optional label, hover highlight.
pub fn toolbar_button(
    ui: &mut Ui,
    icon: Icon,
    label: Option<&str>,
    tooltip: &str,
    enabled: bool,
) -> Response {
    let icon_size = 16.0;
    let padding = 6.0;
    let text_width = label.map_or(0.0, |label| {
        ui.painter()
            .layout_no_wrap(
                label.to_string(),
                egui::TextStyle::Button.resolve(ui.style()),
                Color32::BLACK,
            )
            .size()
            .x
            + 5.0
    });
    let size = Vec2::new(icon_size + padding * 2.0 + text_width, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let response = response.on_hover_text(tooltip);

    let palette = &theme::LIGHT;
    if enabled && (response.hovered() || response.is_pointer_button_down_on()) {
        let fill = if response.is_pointer_button_down_on() {
            palette.selection_fill
        } else {
            palette.hover_fill
        };
        ui.painter().rect(
            rect,
            theme::RADIUS,
            fill,
            Stroke::new(1.0_f32, palette.selection_stroke),
            egui::StrokeKind::Inside,
        );
    }
    let color = if enabled {
        palette.text
    } else {
        palette.star_empty
    };
    let icon_rect = Rect::from_min_size(
        egui::pos2(rect.left() + padding, rect.center().y - icon_size / 2.0),
        Vec2::splat(icon_size),
    );
    draw_icon(ui.painter(), icon_rect, icon, color);
    if let Some(label) = label {
        ui.painter().text(
            egui::pos2(icon_rect.right() + 5.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::TextStyle::Button.resolve(ui.style()),
            color,
        );
    }
    if enabled {
        response
    } else {
        // A disabled button must not report clicks, but still shows its
        // tooltip so the reason can be explained there.
        response.on_disabled_hover_text(tooltip);
        ui.allocate_response(Vec2::ZERO, Sense::hover())
    }
}

/// Five stars, filled to `rating`. Read-only.
pub fn stars(ui: &mut Ui, rating: Option<u8>, size: f32) -> Response {
    let palette = &theme::LIGHT;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(size * 5.6, size), Sense::hover());
    let filled = rating.unwrap_or(0);
    for index in 0..5u8 {
        let centre = egui::pos2(
            rect.left() + size * 0.5 + index as f32 * size * 1.15,
            rect.center().y,
        );
        let earned = index < filled;
        draw_star(
            ui.painter(),
            centre,
            size * 0.5,
            if earned {
                palette.star
            } else {
                palette.star_empty
            },
            earned,
        );
    }
    response
}

/// Five stars the person can click, plus a way back to "not rated".
///
/// Returns the new rating when it changed.
pub fn star_picker(ui: &mut Ui, rating: Option<u8>) -> Option<Option<u8>> {
    let palette = &theme::LIGHT;
    let size = 16.0;
    let mut changed = None;
    ui.horizontal(|ui| {
        for index in 0..5u8 {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(size + 2.0), Sense::click());
            let response = response.on_hover_text(format!("Rate {} of 5", index + 1));
            let earned = index < rating.unwrap_or(0);
            let color = if response.hovered() {
                palette.accent
            } else if earned {
                palette.star
            } else {
                palette.star_empty
            };
            draw_star(ui.painter(), rect.center(), size * 0.5, color, earned);
            if response.clicked() {
                changed = Some(Some(index + 1));
            }
        }
        if rating.is_some()
            && ui
                .small_button("Clear")
                .on_hover_text("Remove this machine's own rating")
                .clicked()
        {
            changed = Some(None);
        }
    });
    changed
}

/// A metadata row: a dim label and a value that wraps.
pub fn field(ui: &mut Ui, label: &str, value: &str) {
    if value.trim().is_empty() {
        return;
    }
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let label_width = 96.0;
        ui.allocate_ui_with_layout(
            Vec2::new(label_width, 0.0),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(label).color(theme::LIGHT.text_dim))
                        .wrap(),
                );
            },
        );
        ui.add(egui::Label::new(value).wrap());
    });
}

/// A field whose value can be clicked to copy.
///
/// It reads in the same face as every other row on purpose. An identifier and
/// a file path were set in monospace at first, on the reasoning that a
/// character-exact string deserves a character-exact font; beside a panel of
/// proportional text it simply looked like a mistake, and a long path in a
/// narrow column looked worse.
pub fn selectable_field(ui: &mut Ui, label: &str, value: &str) -> Response {
    let mut response = None;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.allocate_ui_with_layout(
            Vec2::new(96.0, 0.0),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(label).color(theme::LIGHT.text_dim))
                        .wrap(),
                );
            },
        );
        response = Some(ui.add(egui::Label::new(value).wrap().sense(Sense::click())));
    });
    response.expect("the row always allocates a response")
}

/// A heading inside a panel: small, dim, with a rule under it.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(title.to_uppercase())
            .small()
            .color(theme::LIGHT.text_dim),
    );
    theme::hairline(ui);
    ui.add_space(4.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Actions are compared when deciding whether the same request arrived
    /// twice in a frame, so they have to compare by value.
    #[test]
    fn actions_compare_by_value() {
        assert_eq!(
            Action::Play("com.x@1".to_string()),
            Action::Play("com.x@1".to_string())
        );
        assert_ne!(
            Action::Play("com.x@1".to_string()),
            Action::Play("com.y@1".to_string())
        );
    }
}
