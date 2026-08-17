/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! What the frontend looks like.
//!
//! The target is a conventional light desktop emulator: white content on
//! light-grey chrome, hairline dividers, square-ish corners, and the pale
//! blue selection Windows has used for twenty years. Everything is defined
//! once, here, so that a later dark mode is a second palette rather than a
//! search through the interface code.
//!
//! The one deliberate borrowing from the software tapHLE emulates is in the
//! library grid, where the icon size and the spacing between icons follow the
//! iPad's home screen. That belongs in [crate::ui::library_view]; the rest
//! of the window is a desktop program and looks like one.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Margin, Stroke};

/// Every colour the frontend uses.
///
/// Grouped rather than scattered so the whole scheme can be read at once, and
/// so a dark variant would be a second constructor.
pub struct Palette {
    /// Menu bar, toolbar, status bar: the window's chrome.
    pub chrome: Color32,
    /// The library area behind the app grid.
    pub content: Color32,
    /// The details panel and the log panel, a shade off the content.
    pub panel: Color32,
    /// Hairline dividers between regions.
    pub border: Color32,
    /// A heavier border, for controls that need an edge.
    pub border_strong: Color32,
    pub text: Color32,
    /// Secondary text: metadata labels, hints, the status bar.
    pub text_dim: Color32,
    pub selection_fill: Color32,
    pub selection_stroke: Color32,
    pub hover_fill: Color32,
    pub accent: Color32,
    pub error: Color32,
    pub warning: Color32,
    /// Behind the log panel's text, slightly cooler than white so a wall of
    /// monospace does not glare.
    pub log_background: Color32,
    /// A rating star that has been earned.
    pub star: Color32,
    /// A rating star that has not.
    pub star_empty: Color32,
}

pub const LIGHT: Palette = Palette {
    chrome: Color32::from_rgb(0xF0, 0xF0, 0xF0),
    content: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    panel: Color32::from_rgb(0xFA, 0xFA, 0xFA),
    border: Color32::from_rgb(0xD4, 0xD4, 0xD4),
    border_strong: Color32::from_rgb(0xAD, 0xAD, 0xAD),
    text: Color32::from_rgb(0x1E, 0x1E, 0x1E),
    text_dim: Color32::from_rgb(0x63, 0x63, 0x63),
    selection_fill: Color32::from_rgb(0xCC, 0xE4, 0xF7),
    selection_stroke: Color32::from_rgb(0x7F, 0xB2, 0xE5),
    hover_fill: Color32::from_rgb(0xE8, 0xF1, 0xFA),
    accent: Color32::from_rgb(0x16, 0x67, 0xB8),
    error: Color32::from_rgb(0xB4, 0x23, 0x18),
    warning: Color32::from_rgb(0x8A, 0x5A, 0x00),
    log_background: Color32::from_rgb(0xFC, 0xFC, 0xFC),
    star: Color32::from_rgb(0xE0, 0x9B, 0x13),
    star_empty: Color32::from_rgb(0xC8, 0xC8, 0xC8),
};

/// Corner radius used throughout. Two pixels reads as a desktop control;
/// anything more starts to look like a web card.
pub const RADIUS: u8 = 2;

pub fn apply(ctx: &egui::Context, zoom: f32) {
    install_fonts(ctx);
    ctx.set_zoom_factor(zoom.clamp(0.5, 3.0));

    let palette = &LIGHT;
    let mut style = (*ctx.style()).clone();
    let visuals = &mut style.visuals;
    *visuals = egui::Visuals::light();

    visuals.panel_fill = palette.chrome;
    visuals.window_fill = palette.chrome;
    visuals.extreme_bg_color = palette.content;
    visuals.faint_bg_color = Color32::from_rgb(0xF6, 0xF6, 0xF6);
    visuals.code_bg_color = palette.log_background;
    visuals.window_stroke = Stroke::new(1.0_f32, palette.border_strong);
    visuals.window_corner_radius = CornerRadius::same(RADIUS);
    visuals.menu_corner_radius = CornerRadius::same(RADIUS);
    visuals.error_fg_color = palette.error;
    visuals.warn_fg_color = palette.warning;
    visuals.hyperlink_color = palette.accent;
    visuals.override_text_color = Some(palette.text);
    // A window shadow is the one soft edge worth keeping: it is what tells a
    // dialog apart from the window behind it. Everything else is flat.
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 2],
        blur: 10,
        spread: 0,
        color: Color32::from_black_alpha(40),
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 2],
        blur: 6,
        spread: 0,
        color: Color32::from_black_alpha(30),
    };

    visuals.selection.bg_fill = palette.selection_fill;
    visuals.selection.stroke = Stroke::new(1.0_f32, palette.accent);

    let widgets = &mut visuals.widgets;
    // Non-interactive: labels, separators, panel frames.
    widgets.noninteractive.bg_fill = palette.chrome;
    widgets.noninteractive.weak_bg_fill = palette.chrome;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, palette.border);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, palette.text);
    widgets.noninteractive.corner_radius = CornerRadius::same(RADIUS);

    // Idle controls: a flat face with a visible edge, as a desktop button
    // has. egui's default gives buttons no outline at all, which reads as a
    // web interface.
    widgets.inactive.bg_fill = Color32::from_rgb(0xFA, 0xFA, 0xFA);
    widgets.inactive.weak_bg_fill = Color32::from_rgb(0xFA, 0xFA, 0xFA);
    widgets.inactive.bg_stroke = Stroke::new(1.0_f32, palette.border_strong);
    widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.text);
    widgets.inactive.corner_radius = CornerRadius::same(RADIUS);
    widgets.inactive.expansion = 0.0;

    widgets.hovered.bg_fill = palette.hover_fill;
    widgets.hovered.weak_bg_fill = palette.hover_fill;
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.selection_stroke);
    widgets.hovered.fg_stroke = Stroke::new(1.0_f32, palette.text);
    widgets.hovered.corner_radius = CornerRadius::same(RADIUS);
    widgets.hovered.expansion = 0.0;

    widgets.active.bg_fill = palette.selection_fill;
    widgets.active.weak_bg_fill = palette.selection_fill;
    widgets.active.bg_stroke = Stroke::new(1.0_f32, palette.accent);
    widgets.active.fg_stroke = Stroke::new(1.0_f32, palette.text);
    widgets.active.corner_radius = CornerRadius::same(RADIUS);
    widgets.active.expansion = 0.0;

    widgets.open.bg_fill = palette.selection_fill;
    widgets.open.weak_bg_fill = palette.selection_fill;
    widgets.open.bg_stroke = Stroke::new(1.0_f32, palette.selection_stroke);
    widgets.open.fg_stroke = Stroke::new(1.0_f32, palette.text);
    widgets.open.corner_radius = CornerRadius::same(RADIUS);

    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(6.0, 5.0);
    spacing.button_padding = egui::vec2(8.0, 4.0);
    spacing.interact_size.y = 22.0;
    spacing.menu_margin = Margin::symmetric(2, 4);
    spacing.window_margin = Margin::same(10);
    spacing.indent = 18.0;
    spacing.scroll.bar_width = 11.0;
    spacing.scroll.floating = false;

    ctx.set_style(style);
}

/// Text sizes, in points before display scaling.
///
/// Windows draws its interface in 9pt Segoe UI, which is twelve pixels at the
/// standard density. These are a shade larger because egui's rasteriser is
/// lighter than the system's, and a shade larger again for the app names
/// under the icons, which are the one thing read at a glance.
fn text_styles() -> Vec<(egui::TextStyle, egui::FontId)> {
    use egui::{FontId, TextStyle};
    vec![
        (
            TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(16.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        ),
    ]
}

/// Fonts a desktop of each kind is likely to have, most preferred first.
///
/// A system font is loaded rather than bundled: it is what makes the frontend
/// look like it belongs on the machine, and shipping Segoe UI would not be
/// allowed anyway. If none is found, egui's own font is used and everything
/// still works — it just looks less native.
fn system_font_candidates() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &[
            "C:/Windows/Fonts/segoeui.ttf",
            "C:/Windows/Fonts/tahoma.ttf",
            "C:/Windows/Fonts/arial.ttf",
        ]
    }
    #[cfg(target_os = "macos")]
    {
        &[
            "/System/Library/Fonts/SFNS.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
        ]
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
        ]
    }
}

fn system_monospace_candidates() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &["C:/Windows/Fonts/consola.ttf", "C:/Windows/Fonts/cour.ttf"]
    }
    #[cfg(target_os = "macos")]
    {
        &[
            "/System/Library/Fonts/SFNSMono.ttf",
            "/System/Library/Fonts/Menlo.ttc",
        ]
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
        ]
    }
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut load = |candidates: &[&str], name: &str, family: FontFamily| {
        for path in candidates {
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            fonts.font_data.insert(
                name.to_string(),
                std::sync::Arc::new(FontData::from_owned(bytes)),
            );
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.to_string());
            return;
        }
    };
    load(
        system_font_candidates(),
        "system-proportional",
        FontFamily::Proportional,
    );
    load(
        system_monospace_candidates(),
        "system-monospace",
        FontFamily::Monospace,
    );
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.text_styles = text_styles().into_iter().collect();
    ctx.set_style(style);
}

/// A one-pixel horizontal rule in the divider colour.
///
/// egui's own separator reserves space above and below and is drawn in the
/// widget stroke; a panel divider wants neither.
pub fn hairline(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0_f32, LIGHT.border),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every text style has to be defined, or egui falls back to its own
    /// sizes for the missing one and the interface has two type scales.
    #[test]
    fn every_text_style_is_defined() {
        let styles = text_styles();
        for wanted in [
            egui::TextStyle::Small,
            egui::TextStyle::Body,
            egui::TextStyle::Button,
            egui::TextStyle::Heading,
            egui::TextStyle::Monospace,
        ] {
            assert!(
                styles.iter().any(|(style, _)| *style == wanted),
                "{wanted:?} is not defined"
            );
        }
    }

    /// The scheme is meant to be restrained. A palette entry that strayed
    /// into saturated colour would show up here.
    #[test]
    fn the_surfaces_are_greys() {
        for colour in [LIGHT.chrome, LIGHT.content, LIGHT.panel, LIGHT.border] {
            let [r, g, b, _] = colour.to_array();
            let spread = r.max(g).max(b) - r.min(g).min(b);
            assert!(spread <= 4, "{colour:?} is not a neutral surface colour");
        }
    }
}
