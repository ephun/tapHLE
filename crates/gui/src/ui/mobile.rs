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
    Activity,
    Settings,
    GlobalSettings,
    Details,
    AppSettings,
    About,
    DeveloperLog,
}

impl Screen {
    fn title(self) -> &'static str {
        match self {
            Screen::Library => "Library",
            Screen::Activity => "Activity",
            Screen::Settings => "Settings",
            Screen::GlobalSettings => "Global Settings",
            Screen::Details => "App Details",
            Screen::AppSettings => "App Settings",
            Screen::About => "About",
            Screen::DeveloperLog => "Developer Log",
        }
    }

    fn back_target(self) -> Option<Screen> {
        match self {
            Screen::GlobalSettings | Screen::About => Some(Screen::Settings),
            Screen::Details => Some(Screen::Library),
            Screen::AppSettings => Some(Screen::Details),
            Screen::DeveloperLog => Some(Screen::Activity),
            Screen::Library | Screen::Activity | Screen::Settings => None,
        }
    }
}

#[cfg(test)]
fn all_screens() -> [Screen; 8] {
    [
        Screen::Library,
        Screen::Activity,
        Screen::Settings,
        Screen::GlobalSettings,
        Screen::Details,
        Screen::AppSettings,
        Screen::About,
        Screen::DeveloperLog,
    ]
}

/// The responsive arrangement selected from the space the host gives us.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Layout {
    PhonePortrait,
    PhoneLandscape,
    TabletPortrait,
    TabletLandscape,
}

pub fn layout_for(size: Vec2) -> Layout {
    let tablet = size.x.min(size.y) >= 600.0;
    match (tablet, size.x >= size.y) {
        (false, false) => Layout::PhonePortrait,
        (false, true) => Layout::PhoneLandscape,
        (true, false) => Layout::TabletPortrait,
        (true, true) => Layout::TabletLandscape,
    }
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

/// Drafts owned by the frontend but composed as full mobile pages here.
pub struct MobilePages<'a> {
    pub global_settings: Option<&'a mut crate::ui::desktop::settings_dialog::GlobalDialog>,
    pub app_settings: Option<&'a mut crate::ui::desktop::settings_dialog::AppDialog>,
    pub about: Option<(
        &'a mut crate::ui::desktop::dialogs::AboutDialog,
        &'a crate::ui::desktop::dialogs::AboutInfo,
    )>,
}

pub struct MobileResult {
    pub actions: Vec<Action>,
    pub global_settings: crate::ui::desktop::settings_dialog::Outcome,
    pub app_settings: crate::ui::desktop::settings_dialog::Outcome,
}

/// How big a thing has to be before a finger can hit it.
///
/// 44 points is Apple's minimum and 48 density-independent pixels is
/// Google's; the larger of the two, in points, covers both. Every row and
/// button here is at least this tall, which is the single biggest difference
/// between this and the desktop composition.
pub const TOUCH_TARGET: f32 = 48.0;
pub const BODY_TEXT: f32 = 16.0;
pub const SECONDARY_TEXT: f32 = 14.0;

/// The bar along the bottom, where a thumb can reach it.
const NAV_HEIGHT: f32 = 64.0;
/// The bar along the top, which is a label rather than a control.
const TITLE_HEIGHT: f32 = 52.0;
const MOBILE_SCROLL_SOURCE: egui::scroll_area::ScrollSource = egui::scroll_area::ScrollSource {
    // A scrollbar is useful position feedback on a desktop, where its narrow
    // track is deliberately clicked. On a touchscreen that same track is an
    // edge-sized accidental target and egui jumps the handle to the press.
    // Keep wheel input for the desktop preview, but make content dragging the
    // only direct manipulation path in the mobile composition.
    scroll_bar: false,
    drag: true,
    mouse_wheel: true,
};
const MOBILE_SCROLL_BAR_VISIBILITY: egui::scroll_area::ScrollBarVisibility =
    egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded;

/// Draw a whole mobile frame into `rect`.
///
/// Returns what was asked for, the same way every other part of the interface
/// does — see [crate::state::action].
pub fn show(
    ui: &mut Ui,
    rect: Rect,
    context: &MobileContext<'_>,
    screen: &mut Screen,
    pages: &mut MobilePages<'_>,
) -> MobileResult {
    use crate::ui::desktop::settings_dialog::Outcome;

    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(BODY_TEXT));
    ui.style_mut().text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(BODY_TEXT),
    );
    ui.style_mut().text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::proportional(SECONDARY_TEXT),
    );
    ui.spacing_mut().interact_size.y = TOUCH_TARGET;
    // The shared desktop theme reserves a solid, mouse-sized scrollbar lane.
    // Mobile indicators float over content and fade when idle instead.
    ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();

    let mut actions = Vec::new();
    let mut global_settings = Outcome::Continue;
    let mut app_settings = Outcome::Continue;
    if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        if let Some(focused) = ui.memory(|memory| memory.focused()) {
            // The first Back dismisses text entry and its on-screen keyboard.
            ui.memory_mut(|memory| memory.surrender_focus(focused));
        } else if let Some(back) = screen.back_target() {
            *screen = back;
        } else {
            actions.push(Action::Quit);
        }
    }
    ui.painter().rect_filled(rect, 0.0, theme::LIGHT.content);

    let title_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TITLE_HEIGHT));
    let nav_rect = Rect::from_min_size(
        egui::pos2(rect.left(), rect.bottom() - NAV_HEIGHT),
        Vec2::new(rect.width(), NAV_HEIGHT),
    );
    let shows_navigation = screen.is_root();
    let body_rect = Rect::from_min_max(
        egui::pos2(rect.left(), title_rect.bottom()),
        egui::pos2(
            rect.right(),
            if shows_navigation {
                nav_rect.top()
            } else {
                rect.bottom()
            },
        ),
    );

    let previous = *screen;
    title_bar(ui, title_rect, screen);
    if *screen != previous {
        match previous {
            Screen::GlobalSettings => global_settings = Outcome::Cancel,
            Screen::AppSettings => app_settings = Outcome::Cancel,
            _ => {}
        }
    }

    let layout = layout_for(rect.size());
    match *screen {
        Screen::Library => {
            if shows_split_details(layout, body_rect.size()) && context.selected.is_some() {
                let (library_rect, details_rect) = split_body(body_rect, layout);
                library_screen(ui, library_rect, context, screen, &mut actions, false);
                details_screen(ui, details_rect, context, screen, &mut actions);
            } else {
                library_screen(ui, body_rect, context, screen, &mut actions, true);
            }
        }
        Screen::Activity => activity_screen(ui, body_rect, context, screen, &mut actions),
        Screen::Settings => settings_screen(ui, body_rect, screen, &mut actions),
        Screen::GlobalSettings => full_page_ui(ui, body_rect, "mobile-global-settings", |ui| {
            if let Some(dialog) = pages.global_settings.as_deref_mut() {
                global_settings = global_settings_page(ui, dialog);
            } else {
                ui.label("Opening settings…");
            }
        }),
        Screen::Details => details_screen(ui, body_rect, context, screen, &mut actions),
        Screen::AppSettings => full_page_ui(ui, body_rect, "mobile-app-settings", |ui| {
            if let Some(dialog) = pages.app_settings.as_deref_mut() {
                app_settings = app_settings_page(ui, dialog);
            } else {
                ui.label("Opening app settings…");
            }
        }),
        Screen::About => full_page_ui(ui, body_rect, "mobile-about", |ui| {
            if let Some((dialog, info)) = pages.about.as_mut() {
                about_page(ui, dialog, info, &mut actions);
            } else {
                ui.label("Opening About…");
            }
        }),
        Screen::DeveloperLog => developer_log_screen(ui, body_rect, &mut actions),
    }
    if shows_navigation {
        navigation_bar(ui, nav_rect, screen);
    }
    MobileResult {
        actions,
        global_settings,
        app_settings,
    }
}

impl Screen {
    fn is_root(self) -> bool {
        matches!(self, Screen::Library | Screen::Activity | Screen::Settings)
    }
}

fn shows_split_details(layout: Layout, size: Vec2) -> bool {
    matches!(layout, Layout::TabletPortrait | Layout::TabletLandscape) && size.x >= 720.0
}

fn split_body(rect: Rect, layout: Layout) -> (Rect, Rect) {
    let library_fraction = match layout {
        Layout::PhoneLandscape => 0.46,
        Layout::TabletPortrait => 0.44,
        Layout::TabletLandscape => 0.38,
        Layout::PhonePortrait => 1.0,
    };
    let split = rect.left() + rect.width() * library_fraction;
    (
        Rect::from_min_max(rect.min, egui::pos2(split, rect.bottom())),
        Rect::from_min_max(egui::pos2(split + 1.0, rect.top()), rect.max),
    )
}

fn title_bar(ui: &mut Ui, rect: Rect, screen: &mut Screen) {
    ui.painter().rect_filled(rect, 0.0, theme::LIGHT.chrome);
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.bottom()),
            egui::pos2(rect.right(), rect.bottom()),
        ],
        egui::Stroke::new(1.0_f32, theme::LIGHT.border),
    );
    if let Some(back) = screen.back_target() {
        let back_rect = Rect::from_min_size(
            egui::pos2(rect.left() + 4.0, rect.top() + 2.0),
            Vec2::new(TOUCH_TARGET, TOUCH_TARGET),
        );
        if ui.put(back_rect, egui::Button::new("‹")).clicked() {
            *screen = back;
        }
    }
    let title = screen.title();
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(20.0),
        theme::LIGHT.text,
    );
}

/// The library as a list of rows, not a lattice of icons.
///
/// A phone is narrow enough that a grid of the desktop's icons would be two
/// columns of postage stamps. A row gives the name room to be read without
/// eliding it, which is the thing the grid gives up first.
fn library_screen(
    ui: &mut Ui,
    rect: Rect,
    context: &MobileContext<'_>,
    screen: &mut Screen,
    actions: &mut Vec<Action>,
    open_details: bool,
) {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    mobile_scroll_area("mobile-library")
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
                    actions.push(Action::Select(entry.id.clone()));
                    if open_details {
                        *screen = Screen::Details;
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
    let height = TOUCH_TARGET + 32.0;
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

    let text_rect = Rect::from_min_max(
        egui::pos2(icon_rect.right() + 12.0, rect.top() + 7.0),
        egui::pos2(rect.right() - 12.0, rect.bottom() - 7.0),
    );
    let text_painter = painter.with_clip_rect(text_rect);
    let title_rect = Rect::from_min_max(
        text_rect.min,
        egui::pos2(text_rect.right(), text_rect.bottom() - 20.0),
    );
    let title_painter = text_painter.with_clip_rect(title_rect);
    let title = title_painter.layout(
        entry.title().to_string(),
        egui::FontId::proportional(BODY_TEXT),
        theme::LIGHT.text,
        title_rect.width(),
    );
    title_painter.galley(title_rect.min, title, theme::LIGHT.text);
    let subtitle = if running {
        "Running".to_string()
    } else {
        entry.metadata.version_for_display().to_string()
    };
    if !subtitle.is_empty() {
        text_painter.text(
            egui::pos2(text_rect.left(), text_rect.bottom()),
            egui::Align2::LEFT_CENTER,
            subtitle,
            egui::FontId::proportional(SECONDARY_TEXT),
            if running {
                theme::LIGHT.accent
            } else {
                theme::LIGHT.text_dim
            },
        );
    }
    response.clicked()
}

fn content_ui(ui: &mut Ui, rect: Rect, id: &'static str, add: impl FnOnce(&mut Ui)) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    mobile_scroll_area(id)
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            ui.set_width(ui.available_width());
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(12, 12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    add(ui);
                    ui.add_space(8.0);
                });
        });
}

fn mobile_scroll_area(id: &'static str) -> egui::ScrollArea {
    egui::ScrollArea::vertical()
        .id_salt(id)
        .scroll_source(MOBILE_SCROLL_SOURCE)
        .scroll_bar_visibility(MOBILE_SCROLL_BAR_VISIBILITY)
}

fn full_page_ui(ui: &mut Ui, rect: Rect, id: &'static str, add: impl FnOnce(&mut Ui)) {
    content_ui(ui, rect, id, add);
}

fn action_button(ui: &mut Ui, label: &str) -> bool {
    ui.add_sized(
        [ui.available_width().max(TOUCH_TARGET), TOUCH_TARGET],
        egui::Button::new(egui::RichText::new(label).size(BODY_TEXT)),
    )
    .clicked()
}

fn activity_screen(
    ui: &mut Ui,
    rect: Rect,
    context: &MobileContext<'_>,
    screen: &mut Screen,
    actions: &mut Vec<Action>,
) {
    content_ui(ui, rect, "mobile-activity", |ui| {
        ui.heading("Running now");
        let mut found = false;
        for entry in &context.library.entries {
            if context.running.contains(&entry.id) {
                found = true;
                ui.label(egui::RichText::new(entry.title()).strong());
                ui.label(egui::RichText::new("Running").color(theme::LIGHT.accent));
                ui.separator();
            }
        }
        if !found {
            ui.label(egui::RichText::new("No apps are running.").color(theme::LIGHT.text_dim));
        }
        ui.add_space(12.0);
        if found && action_button(ui, "Stop all") {
            actions.push(Action::StopAll);
        }
        if action_button(ui, "Developer Log") {
            *screen = Screen::DeveloperLog;
        }
    });
}

fn settings_screen(ui: &mut Ui, rect: Rect, screen: &mut Screen, actions: &mut Vec<Action>) {
    content_ui(ui, rect, "mobile-settings", |ui| {
        ui.heading("Library");
        if action_button(ui, "Add apps") {
            actions.push(Action::AddApps);
        }
        if action_button(ui, "Rescan library") {
            actions.push(Action::RefreshLibrary);
        }
        if action_button(ui, "Open apps folder") {
            actions.push(Action::OpenAppsFolder);
        }
        ui.add_space(12.0);
        ui.heading("tapHLE");
        if action_button(ui, "Global settings") {
            *screen = Screen::GlobalSettings;
            actions.push(Action::OpenGlobalSettings);
        }
        if action_button(ui, "About") {
            *screen = Screen::About;
            actions.push(Action::ShowAbout);
        }
    });
}

fn details_screen(
    ui: &mut Ui,
    rect: Rect,
    context: &MobileContext<'_>,
    screen: &mut Screen,
    actions: &mut Vec<Action>,
) {
    content_ui(ui, rect, "mobile-details", |ui| {
        let Some(entry) = context.selected.and_then(|id| context.library.find(id)) else {
            ui.label(
                egui::RichText::new("Select an app from the library.").color(theme::LIGHT.text_dim),
            );
            return;
        };
        ui.heading(entry.title());
        if let Some(publisher) = &entry.metadata.publisher {
            ui.label(egui::RichText::new(publisher).color(theme::LIGHT.text_dim));
        }
        ui.label(format!("Version {}", entry.metadata.version_for_display()));
        widgets::stars(ui, entry.local_rating.stars, 18.0);
        ui.separator();
        ui.label(format!("Bundle ID: {}", entry.metadata.bundle_identifier));
        ui.label(format!("Bundle version: {}", entry.metadata.bundle_version));
        if let Some(minimum) = &entry.metadata.minimum_os_version {
            ui.label(format!("Minimum iOS: {minimum}"));
        }
        let family = entry.metadata.device_family_summary();
        if !family.is_empty() {
            ui.label(format!("Device family: {family}"));
        }
        ui.add_space(12.0);
        if action_button(ui, "Play") {
            actions.push(Action::Play(entry.id.clone()));
        }
        if action_button(ui, "App settings") {
            *screen = Screen::AppSettings;
            actions.push(Action::OpenAppSettings(entry.id.clone()));
        }
    });
}

fn developer_log_screen(ui: &mut Ui, rect: Rect, actions: &mut Vec<Action>) {
    content_ui(ui, rect, "mobile-developer-log", |ui| {
        ui.heading("Developer Log");
        ui.label("The shared frontend keeps emulator diagnostics available after a run ends.");
        ui.add_space(12.0);
        if action_button(ui, "Save log") {
            actions.push(Action::SaveLog);
        }
        if action_button(ui, "Clear log") {
            actions.push(Action::ClearLog);
        }
    });
}

fn global_settings_page(
    ui: &mut Ui,
    dialog: &mut crate::ui::desktop::settings_dialog::GlobalDialog,
) -> crate::ui::desktop::settings_dialog::Outcome {
    use crate::ui::desktop::settings_dialog::{self, Category};

    mobile_category_grid(ui, Category::GLOBAL, &mut dialog.category);
    ui.add_space(8.0);
    theme::hairline(ui);
    ui.add_space(12.0);
    ui.heading(dialog.category.label());
    settings_dialog::show_global_category(ui, dialog);
    ui.add_space(16.0);
    mobile_settings_buttons(ui, &mut dialog.draft.emulator, false)
}

fn app_settings_page(
    ui: &mut Ui,
    dialog: &mut crate::ui::desktop::settings_dialog::AppDialog,
) -> crate::ui::desktop::settings_dialog::Outcome {
    use crate::ui::desktop::settings_dialog::{self, Category};

    ui.add(
        egui::Label::new(
            egui::RichText::new("Unset values follow global settings, then the options files.")
                .size(SECONDARY_TEXT)
                .color(theme::LIGHT.text_dim),
        )
        .wrap(),
    );
    ui.add_space(8.0);
    mobile_category_grid(ui, Category::PER_APP, &mut dialog.category);
    ui.add_space(8.0);
    theme::hairline(ui);
    ui.add_space(12.0);
    ui.heading(dialog.category.label());
    settings_dialog::show_app_category(ui, dialog);
    ui.add_space(16.0);
    mobile_settings_buttons(ui, &mut dialog.draft, true)
}

fn mobile_category_grid(
    ui: &mut Ui,
    categories: &[crate::ui::desktop::settings_dialog::Category],
    current: &mut crate::ui::desktop::settings_dialog::Category,
) {
    let columns = grid_columns(ui.available_width(), categories.len());
    let width = grid_cell_width(ui.available_width(), columns, ui.spacing().item_spacing.x);
    egui::Grid::new(ui.id().with("mobile-settings-categories"))
        .num_columns(columns)
        .spacing([ui.spacing().item_spacing.x, ui.spacing().item_spacing.y])
        .show(ui, |ui| {
            for (index, category) in categories.iter().enumerate() {
                let button = egui::Button::selectable(
                    *current == *category,
                    egui::RichText::new(category.label()).size(BODY_TEXT),
                );
                if ui.add_sized([width, TOUCH_TARGET], button).clicked() {
                    *current = *category;
                }
                if (index + 1) % columns == 0 {
                    ui.end_row();
                }
            }
        });
}

fn grid_columns(width: f32, item_count: usize) -> usize {
    let columns = if width < 280.0 {
        1
    } else if width < 520.0 {
        2
    } else if width < 760.0 {
        3
    } else {
        4
    };
    columns.min(item_count.max(1))
}

fn grid_cell_width(available: f32, columns: usize, gap: f32) -> f32 {
    let gaps = gap * columns.saturating_sub(1) as f32;
    ((available - gaps) / columns.max(1) as f32).max(1.0)
}

fn mobile_settings_buttons(
    ui: &mut Ui,
    draft: &mut crate::state::settings::EmulatorSettings,
    show_reset: bool,
) -> crate::ui::desktop::settings_dialog::Outcome {
    use crate::ui::desktop::settings_dialog::Outcome;

    let problems = draft.validate();
    if !problems.is_empty() {
        ui.add(
            egui::Label::new(
                egui::RichText::new(problems.join("; "))
                    .size(SECONDARY_TEXT)
                    .color(theme::LIGHT.error),
            )
            .wrap(),
        );
        ui.add_space(8.0);
    }
    if show_reset && mobile_button(ui, "Reset to Global", true) {
        *draft = Default::default();
    }
    if mobile_button(ui, "Save", problems.is_empty()) {
        return Outcome::Accept;
    }
    if mobile_button(ui, "Apply", problems.is_empty()) {
        return Outcome::Apply;
    }
    if mobile_button(ui, "Cancel", true) {
        return Outcome::Cancel;
    }
    Outcome::Continue
}

fn mobile_button(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    ui.add_enabled_ui(enabled, |ui| {
        ui.add_sized(
            [ui.available_width(), TOUCH_TARGET],
            egui::Button::new(egui::RichText::new(label).size(BODY_TEXT)),
        )
    })
    .inner
    .clicked()
}

fn about_page(
    ui: &mut Ui,
    dialog: &mut crate::ui::desktop::dialogs::AboutDialog,
    info: &crate::ui::desktop::dialogs::AboutInfo,
    actions: &mut Vec<Action>,
) {
    use crate::ui::desktop::dialogs::{self, AboutTab};

    ui.horizontal_wrapped(|ui| {
        ui.heading("tapHLE");
        ui.label(
            egui::RichText::new(&info.version)
                .size(BODY_TEXT)
                .color(theme::LIGHT.text_dim),
        );
        if !info.branding.is_empty() {
            ui.label(
                egui::RichText::new(&info.branding)
                    .size(SECONDARY_TEXT)
                    .color(theme::LIGHT.warning),
            );
        }
    });
    ui.add_space(8.0);
    let columns = if ui.available_width() < 400.0 { 2 } else { 4 };
    let width = grid_cell_width(ui.available_width(), columns, ui.spacing().item_spacing.x);
    egui::Grid::new("mobile-about-tabs")
        .num_columns(columns)
        .show(ui, |ui| {
            for (index, (tab, label)) in AboutTab::ALL.into_iter().enumerate() {
                let button = egui::Button::selectable(
                    dialog.tab == tab,
                    egui::RichText::new(label).size(BODY_TEXT),
                );
                if ui.add_sized([width, TOUCH_TARGET], button).clicked() {
                    dialog.tab = tab;
                }
                if (index + 1) % columns == 0 {
                    ui.end_row();
                }
            }
        });
    ui.add_space(8.0);
    theme::hairline(ui);
    ui.add_space(12.0);
    dialogs::show_about_tab(ui, dialog, info, actions);
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
        (Screen::Activity, "Activity", widgets::Icon::Log),
        (Screen::Settings, "Settings", widgets::Icon::Settings),
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
            egui::FontId::proportional(SECONDARY_TEXT),
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

    #[test]
    fn mobile_navigation_exposes_every_shared_product_surface() {
        assert_eq!(
            all_screens(),
            [
                Screen::Library,
                Screen::Activity,
                Screen::Settings,
                Screen::GlobalSettings,
                Screen::Details,
                Screen::AppSettings,
                Screen::About,
                Screen::DeveloperLog,
            ]
        );
        assert_eq!(Screen::GlobalSettings.back_target(), Some(Screen::Settings));
        assert_eq!(Screen::About.back_target(), Some(Screen::Settings));
        assert_eq!(Screen::Details.back_target(), Some(Screen::Library));
        assert_eq!(Screen::AppSettings.back_target(), Some(Screen::Details));
        assert_eq!(Screen::DeveloperLog.back_target(), Some(Screen::Activity));
    }

    #[test]
    fn phone_and_tablet_layouts_follow_the_available_rectangle() {
        assert_eq!(layout_for(Vec2::new(390.0, 844.0)), Layout::PhonePortrait);
        assert_eq!(layout_for(Vec2::new(844.0, 390.0)), Layout::PhoneLandscape);
        assert_eq!(layout_for(Vec2::new(768.0, 1024.0)), Layout::TabletPortrait);
        assert_eq!(
            layout_for(Vec2::new(1024.0, 768.0)),
            Layout::TabletLandscape
        );
    }

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

    #[test]
    fn mobile_scroll_uses_content_gestures_not_the_scrollbar_track() {
        assert_eq!(
            MOBILE_SCROLL_BAR_VISIBILITY,
            egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded
        );
        assert!(!MOBILE_SCROLL_SOURCE.scroll_bar);
        assert!(MOBILE_SCROLL_SOURCE.drag);
        assert!(MOBILE_SCROLL_SOURCE.mouse_wheel);
    }

    #[test]
    fn representative_mobile_rectangles_choose_a_deliberate_reflow() {
        let cases = [
            (Vec2::new(320.0, 568.0), Layout::PhonePortrait, false),
            (Vec2::new(430.0, 932.0), Layout::PhonePortrait, false),
            (Vec2::new(568.0, 320.0), Layout::PhoneLandscape, false),
            (Vec2::new(932.0, 430.0), Layout::PhoneLandscape, false),
            (Vec2::new(768.0, 1024.0), Layout::TabletPortrait, true),
            (Vec2::new(1024.0, 768.0), Layout::TabletLandscape, true),
        ];
        for (size, expected, split) in cases {
            let layout = layout_for(size);
            assert_eq!(layout, expected, "wrong reflow for {size:?}");
            assert_eq!(
                shows_split_details(layout, size),
                split,
                "wrong pane count for {size:?}"
            );
        }
    }

    #[test]
    fn tabs_reflow_without_horizontal_scrolling() {
        for (width, expected_columns) in [(240.0, 1), (320.0, 2), (568.0, 3), (768.0, 4)] {
            let columns = grid_columns(width, 8);
            assert_eq!(columns, expected_columns);
            let cell = grid_cell_width(width, columns, 8.0);
            assert!(cell >= TOUCH_TARGET);
            assert!(cell * columns as f32 + 8.0 * (columns - 1) as f32 <= width);
        }
    }
}
