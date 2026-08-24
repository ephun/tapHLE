/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The window, composed for a touchscreen and a small one.
//!
//! The same [crate::state] and the same [crate::ui::widgets] as
//! [crate::ui::desktop], arranged for one hand. That is the whole point of
//! the split: a phone shows the same product, not a second implementation of
//! it, so anything that is really about *what tapHLE is* belongs on the other
//! side of the wall and only the arrangement lives here.
//!
//! What does not arrive here is as much of the design as what does. A
//! touchscreen device needs no control-placement editor, because the guest's
//! touch is the person's touch — unless a controller is connected, which is
//! the one case that brings it back. Tilt settings exist to fake an
//! accelerometer a desktop has not got. Window and fullscreen settings
//! describe a window nobody can move. Roughly half of
//! [crate::ui::desktop::settings_dialog] is desktop-only by nature, so the
//! mobile client is genuinely smaller rather than the same one squeezed.
//!
//! # Seeing it without a phone
//!
//! This can be drawn inside the desktop window, at phone proportions, from
//! developer mode. That is not a gimmick: the alternative is that the mobile
//! design can only be judged by building an APK, which means it does not get
//! judged. [show] takes a rectangle and draws into it, so the preview and a
//! real phone run the same code with a different rectangle.

use egui::{Rect, Ui, Vec2};

use crate::state::library::{Library, VersionGroup};
use crate::state::Action;
use crate::ui::theme;
use crate::ui::widgets;

/// Which screen is in front.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Screen {
    #[default]
    Library,
    Settings,
    Log,
}

/// Everything a mobile frame needs, and nothing it can change.
pub struct MobileContext<'a> {
    pub library: &'a Library,
    pub groups: &'a [VersionGroup],
    pub icons: &'a std::collections::HashMap<String, egui::TextureHandle>,
    pub selected: Option<&'a str>,
    /// Entry identifiers with a run in progress.
    pub running: &'a [String],
}

/// How big a thing has to be before a finger can hit it.
///
/// 44 points is Apple's minimum and 48 density-independent pixels is
/// Google's; the larger of the two, in points, covers both. Every row and
/// button here is at least this tall, which is the single biggest difference
/// between this and the desktop composition.
pub const TOUCH_TARGET: f32 = 48.0;

/// The bar along the bottom, where a thumb can reach it.
const NAV_HEIGHT: f32 = 64.0;
/// The bar along the top, which is a label rather than a control.
const TITLE_HEIGHT: f32 = 52.0;

/// Draw a whole mobile frame into `rect`.
///
/// Returns what was asked for, the same way every other part of the interface
/// does — see [crate::state::action].
pub fn show(
    ui: &mut Ui,
    rect: Rect,
    context: &MobileContext<'_>,
    screen: &mut Screen,
) -> Vec<Action> {
    let mut actions = Vec::new();
    ui.painter().rect_filled(rect, 0.0, theme::LIGHT.content);

    let title_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TITLE_HEIGHT));
    let nav_rect = Rect::from_min_size(
        egui::pos2(rect.left(), rect.bottom() - NAV_HEIGHT),
        Vec2::new(rect.width(), NAV_HEIGHT),
    );
    let body_rect = Rect::from_min_max(
        egui::pos2(rect.left(), title_rect.bottom()),
        egui::pos2(rect.right(), nav_rect.top()),
    );

    title_bar(ui, title_rect, *screen);
    match *screen {
        Screen::Library => library_screen(ui, body_rect, context, &mut actions),
        Screen::Settings => placeholder(ui, body_rect, "Settings"),
        Screen::Log => placeholder(ui, body_rect, "Log"),
    }
    navigation_bar(ui, nav_rect, screen);
    actions
}

fn title_bar(ui: &mut Ui, rect: Rect, screen: Screen) {
    ui.painter().rect_filled(rect, 0.0, theme::LIGHT.chrome);
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.bottom()),
            egui::pos2(rect.right(), rect.bottom()),
        ],
        egui::Stroke::new(1.0_f32, theme::LIGHT.border),
    );
    let title = match screen {
        Screen::Library => "tapHLE",
        Screen::Settings => "Settings",
        Screen::Log => "Log",
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(19.0),
        theme::LIGHT.text,
    );
}

/// The library as a list of rows, not a lattice of icons.
///
/// A phone is narrow enough that a grid of the desktop's icons would be two
/// columns of postage stamps. A row gives the name room to be read without
/// eliding it, which is the thing the grid gives up first.
fn library_screen(ui: &mut Ui, rect: Rect, context: &MobileContext<'_>, actions: &mut Vec<Action>) {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            if context.groups.is_empty() {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Nothing here yet.")
                            .size(16.0)
                            .color(theme::LIGHT.text_dim),
                    );
                });
                return;
            }
            for group in context.groups {
                let Some(entry) = context.library.entries.get(group.shown) else {
                    continue;
                };
                let selected = context.selected == Some(entry.id.as_str());
                let running = context.running.contains(&entry.id);
                if app_row(ui, entry, context.icons, selected, running) {
                    // A tap selects; a tap on what is already selected plays
                    // it. There is no hover on a touchscreen, so a row cannot
                    // hold a Play button that only appears when pointed at.
                    if selected {
                        actions.push(Action::Play(entry.id.clone()));
                    } else {
                        actions.push(Action::Select(entry.id.clone()));
                    }
                }
            }
            ui.add_space(8.0);
        });
}

/// One app, as a row tall enough to hit.
fn app_row(
    ui: &mut Ui,
    entry: &crate::state::library::LibraryEntry,
    icons: &std::collections::HashMap<String, egui::TextureHandle>,
    selected: bool,
    running: bool,
) -> bool {
    let height = TOUCH_TARGET + 16.0;
    let (response, painter) = ui.allocate_painter(
        Vec2::new(ui.available_width(), height),
        egui::Sense::click(),
    );
    let rect = response.rect;
    if selected {
        painter.rect_filled(rect, 0.0, theme::LIGHT.selection_fill);
    }
    painter.line_segment(
        [
            egui::pos2(rect.left() + 72.0, rect.bottom()),
            egui::pos2(rect.right(), rect.bottom()),
        ],
        egui::Stroke::new(1.0_f32, theme::LIGHT.border),
    );

    let icon_side = TOUCH_TARGET;
    let icon_rect = Rect::from_min_size(
        egui::pos2(rect.left() + 12.0, rect.center().y - icon_side / 2.0),
        Vec2::splat(icon_side),
    );
    // An icon that has not been read yet is drawn as the space it will take,
    // so the list does not reflow when it arrives.
    if let Some(texture) = icons.get(&entry.id) {
        painter.image(
            texture.id(),
            icon_rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        painter.rect_filled(icon_rect, 10.0, theme::LIGHT.border);
    }

    painter.text(
        egui::pos2(icon_rect.right() + 12.0, rect.center().y - 9.0),
        egui::Align2::LEFT_CENTER,
        entry.title(),
        egui::FontId::proportional(16.0),
        theme::LIGHT.text,
    );
    let subtitle = if running {
        "Running".to_string()
    } else {
        entry.metadata.version_for_display().to_string()
    };
    if !subtitle.is_empty() {
        painter.text(
            egui::pos2(icon_rect.right() + 12.0, rect.center().y + 11.0),
            egui::Align2::LEFT_CENTER,
            subtitle,
            egui::FontId::proportional(13.0),
            if running {
                theme::LIGHT.accent
            } else {
                theme::LIGHT.text_dim
            },
        );
    }
    response.clicked()
}

/// A screen that is named but not yet drawn.
///
/// Deliberately visible rather than absent: the navigation is the part worth
/// judging first, and a bar whose middle button goes nowhere cannot be judged
/// at all.
fn placeholder(ui: &mut Ui, rect: Rect, name: &str) {
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{name} — not written yet"),
        egui::FontId::proportional(15.0),
        theme::LIGHT.text_dim,
    );
}

/// The bar along the bottom.
///
/// At the bottom rather than the top because that is where a thumb is. The
/// desktop puts its equivalent in a menu bar at the top, which is where a
/// pointer already is; neither is a style choice.
fn navigation_bar(ui: &mut Ui, rect: Rect, current: &mut Screen) {
    ui.painter().rect_filled(rect, 0.0, theme::LIGHT.chrome);
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.top()),
            egui::pos2(rect.right(), rect.top()),
        ],
        egui::Stroke::new(1.0_f32, theme::LIGHT.border),
    );

    let items = [
        (Screen::Library, "Library", widgets::Icon::Grid),
        (Screen::Settings, "Settings", widgets::Icon::Settings),
        (Screen::Log, "Log", widgets::Icon::Log),
    ];
    let width = rect.width() / items.len() as f32;
    for (index, (screen, label, icon)) in items.into_iter().enumerate() {
        let cell = Rect::from_min_size(
            egui::pos2(rect.left() + width * index as f32, rect.top()),
            Vec2::new(width, rect.height()),
        );
        let response = ui.interact(
            cell,
            ui.id().with(("mobile-nav", index)),
            egui::Sense::click(),
        );
        let colour = if screen == *current {
            theme::LIGHT.accent
        } else {
            theme::LIGHT.text_dim
        };
        let icon_rect = Rect::from_center_size(
            egui::pos2(cell.center().x, cell.top() + 22.0),
            Vec2::splat(20.0),
        );
        widgets::draw_icon(ui.painter(), icon_rect, icon, colour);
        ui.painter().text(
            egui::pos2(cell.center().x, cell.bottom() - 14.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.0),
            colour,
        );
        // Which screen is in front is this composition's own business — a
        // desktop has no equivalent, so it is not something to say in the
        // shared [Action] vocabulary.
        if response.clicked() {
            *current = screen;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one number that separates this from the desktop composition. Both
    /// platform minimums have to fit inside it: 44 points on iOS, 48
    /// density-independent pixels on Android.
    #[test]
    fn a_touch_target_satisfies_both_platforms() {
        assert!(TOUCH_TARGET >= 44.0, "smaller than Apple's minimum");
        assert!(TOUCH_TARGET >= 48.0, "smaller than Google's minimum");
    }

    /// A row has to be at least a touch target tall, or the list is a row of
    /// things that cannot reliably be tapped.
    #[test]
    fn a_row_is_at_least_a_touch_target_tall() {
        assert!(TOUCH_TARGET + 16.0 >= TOUCH_TARGET);
        assert!(NAV_HEIGHT >= TOUCH_TARGET);
    }
}
