/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The configuration dialogs: one for everything, one for a single app.
//!
//! Both are the same shape, and it is an old shape on purpose: a list of
//! categories down the left, the chosen category's settings on the right, and
//! OK, Cancel and Apply along the bottom. Editing happens on a copy, so
//! Cancel really does cancel.
//!
//! Every emulator setting is a tri-state. The checkbox on the left of a row
//! says whether this level decides the setting at all; with it clear, the row
//! shows what would apply instead and the control is disabled. That is how
//! "per-app setting unset means the global default applies" is made visible
//! rather than merely documented — and it is why an unset setting produces no
//! command-line argument at all, so the options files keep their say.

use egui::{Id, Ui};

use crate::settings::{
    DeviceFamilyPref, EmulatorSettings, FrameRateLimit, FrontendSettings, Gles1Pref,
    OrientationPref,
};
use crate::theme;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Category {
    General,
    Display,
    Graphics,
    Controls,
    Fonts,
    System,
    Logging,
    Paths,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::General => "General",
            Category::Display => "Display",
            Category::Graphics => "Graphics",
            Category::Controls => "Controls",
            Category::Fonts => "Fonts",
            Category::System => "System",
            Category::Logging => "Logging",
            Category::Paths => "Paths",
        }
    }

    /// Categories of the global dialog, in the order they are listed.
    pub const GLOBAL: &'static [Category] = &[
        Category::General,
        Category::Display,
        Category::Graphics,
        Category::Controls,
        Category::Fonts,
        Category::System,
        Category::Logging,
        Category::Paths,
    ];

    /// Categories of the per-app dialog. General and Paths are absent: they
    /// are about the frontend and its folders, which cannot vary per app.
    pub const PER_APP: &'static [Category] = &[
        Category::Display,
        Category::Graphics,
        Category::Controls,
        Category::Fonts,
        Category::System,
        Category::Logging,
    ];
}

/// What the person did with the dialog.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// Still open, nothing decided.
    Continue,
    /// Keep the changes and close.
    Accept,
    /// Keep the changes and stay open.
    Apply,
    /// Discard the changes and close.
    Cancel,
}

/// The global settings dialog's own state.
pub struct GlobalDialog {
    pub category: Category,
    pub draft: FrontendSettings,
}

impl GlobalDialog {
    pub fn new(settings: &FrontendSettings) -> Self {
        GlobalDialog {
            category: Category::General,
            draft: settings.clone(),
        }
    }
}

/// The per-app settings dialog's own state.
pub struct AppDialog {
    pub entry_id: String,
    pub title: String,
    pub category: Category,
    pub draft: EmulatorSettings,
    /// The settings that apply when this app overrides nothing, shown as the
    /// inherited value beside each row.
    pub inherited: EmulatorSettings,
    /// Set when somebody asks to place this app's controls on its screen. The
    /// dialog cannot open the editor itself — it does not own the library —
    /// so it raises the request and the frontend acts on it.
    pub open_control_editor: bool,
}

pub fn show_global(ctx: &egui::Context, dialog: &mut GlobalDialog) -> Outcome {
    let mut outcome = Outcome::Continue;
    let response = egui::Modal::new(Id::new("taphle-settings")).show(ctx, |ui| {
        ui.set_width(600.0);
        ui.heading("Settings");
        ui.add_space(4.0);
        theme::hairline(ui);
        ui.add_space(6.0);

        ui.horizontal_top(|ui| {
            ui.set_height(PAGE_HEIGHT);
            category_list(ui, Category::GLOBAL, &mut dialog.category);
            ui.separator();
            page(ui, 430.0, |ui| match dialog.category {
                Category::General => general_page(ui, &mut dialog.draft),
                Category::Paths => paths_page(ui, &mut dialog.draft),
                Category::Logging => {
                    logging_page(ui, &mut dialog.draft.emulator, None);
                    ui.add_space(8.0);
                    frontend_logging_page(ui, &mut dialog.draft);
                }
                other => emulator_page(ui, other, &mut dialog.draft.emulator, None),
            });
        });

        ui.add_space(8.0);
        theme::hairline(ui);
        ui.add_space(6.0);
        outcome = buttons(ui, &dialog.draft.emulator);
    });
    if response.should_close() && outcome == Outcome::Continue {
        outcome = Outcome::Cancel;
    }
    outcome
}

pub fn show_app(ctx: &egui::Context, dialog: &mut AppDialog) -> Outcome {
    let mut outcome = Outcome::Continue;
    let response = egui::Modal::new(Id::new("taphle-app-settings")).show(ctx, |ui| {
        ui.set_width(600.0);
        ui.heading(format!("Settings for {}", dialog.title));
        ui.label(
            egui::RichText::new(
                "Anything not set here follows the global settings, then the \
                 options files.",
            )
            .small()
            .color(theme::LIGHT.text_dim),
        );
        ui.add_space(4.0);
        theme::hairline(ui);
        ui.add_space(6.0);

        ui.horizontal_top(|ui| {
            ui.set_height(PAGE_HEIGHT);
            category_list(ui, Category::PER_APP, &mut dialog.category);
            ui.separator();
            page(ui, 430.0, |ui| {
                let inherited = Some(&dialog.inherited);
                match dialog.category {
                    Category::Logging => logging_page(ui, &mut dialog.draft, inherited),
                    Category::Controls => {
                        // This app's own mapping comes first. It is the thing
                        // that is per-app; the settings under it describe the
                        // controller and are the same everywhere, so burying
                        // the mapping below six tilt sliders would put the
                        // reason somebody opened this page last.
                        crate::ui::section(ui, "This app's controls");
                        // Cloned rather than borrowed: the section reads the
                        // inherited layout while writing the draft, and the
                        // rows below need the inherited settings again.
                        let inherited_layout =
                            dialog.inherited.controls.clone().unwrap_or_default();
                        let mut open_editor = dialog.open_control_editor;
                        app_control_layout(
                            ui,
                            &mut dialog.draft,
                            &inherited_layout,
                            &mut open_editor,
                        );
                        dialog.open_control_editor = open_editor;
                        ui.add_space(12.0);
                        controls_page(ui, &mut dialog.draft, Some(&dialog.inherited));
                    }
                    other => emulator_page(ui, other, &mut dialog.draft, inherited),
                }
            });
        });

        ui.add_space(8.0);
        theme::hairline(ui);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .button("Reset to Global")
                .on_hover_text("Remove every setting specific to this app")
                .clicked()
            {
                dialog.draft = EmulatorSettings::default();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                outcome = button_row(ui, &dialog.draft);
            });
        });
    });
    if response.should_close() && outcome == Outcome::Continue {
        outcome = Outcome::Cancel;
    }
    outcome
}

/// How tall a dialog page is.
///
/// Fixed, and the same for every category. Two other arrangements were tried
/// and both were worse. Letting the scroll area take the height available
/// grew the dialog until it covered the window, because a modal lays its
/// contents out in unbounded space and `max_height` does not stop a scroll
/// area that has been told not to shrink. Letting it fit its contents
/// instead left the dialog empty: with no bound anywhere in the chain, egui
/// had nothing to lay the page out against.
///
/// So the page is bounded here, once, and every category gets the same
/// height. A short category has some room below it; a tall one scrolls.
const PAGE_HEIGHT: f32 = 320.0;

/// A dialog's scrolling page.
pub fn page(ui: &mut egui::Ui, width: f32, contents: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.set_height(PAGE_HEIGHT);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, contents);
    });
}

fn category_list(ui: &mut Ui, categories: &[Category], current: &mut Category) {
    ui.vertical(|ui| {
        ui.set_min_width(120.0);
        for category in categories {
            let selected = *current == *category;
            if ui.selectable_label(selected, category.label()).clicked() {
                *current = *category;
            }
        }
    });
}

fn buttons(ui: &mut Ui, draft: &EmulatorSettings) -> Outcome {
    let mut outcome = Outcome::Continue;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            outcome = button_row(ui, draft);
        });
    });
    outcome
}

fn button_row(ui: &mut Ui, draft: &EmulatorSettings) -> Outcome {
    let problems = draft.validate();
    let mut outcome = Outcome::Continue;
    // Laid out right to left, so Apply is added first and ends up rightmost.
    if ui
        .add_enabled(problems.is_empty(), egui::Button::new("Apply"))
        .clicked()
    {
        outcome = Outcome::Apply;
    }
    if ui.button("Cancel").clicked() {
        outcome = Outcome::Cancel;
    }
    if ui
        .add_enabled(problems.is_empty(), egui::Button::new("OK"))
        .clicked()
    {
        outcome = Outcome::Accept;
    }
    if !problems.is_empty() {
        ui.label(
            egui::RichText::new(problems.join("; "))
                .small()
                .color(theme::LIGHT.error),
        );
    }
    outcome
}

/// One tri-state row: the "set" checkbox, the label, then the control.
///
/// `inherited` is what would apply if this row were left unset, shown after
/// the control so the consequence of clearing the checkbox is visible.
fn optional_row<T: Clone + PartialEq>(
    ui: &mut Ui,
    label: &str,
    value: &mut Option<T>,
    default_value: T,
    inherited: Option<String>,
    control: impl FnOnce(&mut Ui, &mut T),
) {
    ui.horizontal(|ui| {
        let mut set = value.is_some();
        if ui
            .checkbox(&mut set, "")
            .on_hover_text(if inherited.is_some() {
                "Set this for this app"
            } else {
                "Set this. Leave it clear to use tapHLE's own default and the \
                 options files."
            })
            .changed()
        {
            *value = set.then(|| value.clone().unwrap_or_else(|| default_value.clone()));
        }
        ui.add_sized(
            [150.0, 18.0],
            egui::Label::new(label).halign(egui::Align::LEFT),
        );
        match value {
            Some(inner) => {
                control(ui, inner);
            }
            None => {
                ui.add_enabled_ui(false, |ui| {
                    let mut ghost = default_value.clone();
                    control(ui, &mut ghost);
                });
                if let Some(inherited) = inherited {
                    ui.label(
                        egui::RichText::new(inherited)
                            .small()
                            .color(theme::LIGHT.text_dim),
                    );
                }
            }
        }
    });
}

fn describe<T: std::fmt::Debug>(value: &Option<T>, describe: impl Fn(&T) -> String) -> String {
    match value {
        Some(value) => format!("inherits {}", describe(value)),
        None => "inherits tapHLE's default".to_string(),
    }
}

fn on_off(value: bool) -> String {
    if value {
        "on".to_string()
    } else {
        "off".to_string()
    }
}

fn emulator_page(
    ui: &mut Ui,
    category: Category,
    draft: &mut EmulatorSettings,
    inherited: Option<&EmulatorSettings>,
) {
    match category {
        Category::Display => display_page(ui, draft, inherited),
        Category::Graphics => graphics_page(ui, draft, inherited),
        Category::Controls => controls_page(ui, draft, inherited),
        Category::Fonts => fonts_page(ui, draft, inherited),
        Category::System => system_page(ui, draft, inherited),
        _ => (),
    }
}

fn display_page(ui: &mut Ui, draft: &mut EmulatorSettings, inherited: Option<&EmulatorSettings>) {
    crate::ui::section(ui, "Window");
    optional_row(
        ui,
        "Start in full screen",
        &mut draft.fullscreen,
        false,
        inherited.map(|i| describe(&i.fullscreen, |v| on_off(*v))),
        |ui, value| {
            ui.checkbox(value, if *value { "Full screen" } else { "Windowed" });
        },
    );
    optional_row(
        ui,
        "Internal resolution",
        &mut draft.scale_hack,
        1,
        inherited.map(|i| describe(&i.scale_hack, |v| format!("{v}×"))),
        |ui, value| {
            egui::ComboBox::from_id_salt("scale-hack")
                .selected_text(format!("{value}×"))
                .width(120.0)
                .show_ui(ui, |ui| {
                    for scale in 1..=8u32 {
                        ui.selectable_value(value, scale, format!("{scale}×"));
                    }
                });
        },
    );
    ui.label(
        egui::RichText::new(
            "Renders the app at a multiple of its native resolution. It is a \
             hack, and not every app copes with it.",
        )
        .small()
        .color(theme::LIGHT.text_dim),
    );

    crate::ui::section(ui, "Device");
    optional_row(
        ui,
        "Emulated device",
        &mut draft.device_family,
        DeviceFamilyPref::IPhone,
        inherited.map(|i| describe(&i.device_family, |v| v.label().to_string())),
        |ui, value| {
            egui::ComboBox::from_id_salt("device-family")
                .selected_text(value.label())
                .width(190.0)
                .show_ui(ui, |ui| {
                    for family in DeviceFamilyPref::ALL {
                        ui.selectable_value(value, *family, family.label());
                    }
                });
        },
    );
    optional_row(
        ui,
        "Starting orientation",
        &mut draft.orientation,
        OrientationPref::Portrait,
        inherited.map(|i| describe(&i.orientation, |v| v.label().to_string())),
        |ui, value| {
            egui::ComboBox::from_id_salt("orientation")
                .selected_text(value.label())
                .width(190.0)
                .show_ui(ui, |ui| {
                    for orientation in OrientationPref::ALL {
                        ui.selectable_value(value, *orientation, orientation.label());
                    }
                });
        },
    );
    ui.label(
        egui::RichText::new(
            "Most apps tell tapHLE which way up they want to be. Landscape \
             (native) is for the ones that draw landscape directly and come \
             out sideways otherwise.",
        )
        .small()
        .color(theme::LIGHT.text_dim),
    );
}

fn graphics_page(ui: &mut Ui, draft: &mut EmulatorSettings, inherited: Option<&EmulatorSettings>) {
    crate::ui::section(ui, "Renderer");
    optional_row(
        ui,
        "OpenGL ES 1.1 backend",
        &mut draft.gles1,
        Gles1Pref::Native,
        inherited.map(|i| describe(&i.gles1, |v| v.label().to_string())),
        |ui, value| {
            egui::ComboBox::from_id_salt("gles1")
                .selected_text(value.label())
                .width(190.0)
                .show_ui(ui, |ui| {
                    for option in Gles1Pref::ALL {
                        ui.selectable_value(value, *option, option.label());
                    }
                });
        },
    );
    optional_row(
        ui,
        "Force composition",
        &mut draft.force_composition,
        false,
        inherited.map(|i| describe(&i.force_composition, |v| on_off(*v))),
        |ui, value| {
            ui.checkbox(value, "Present through the compositor");
        },
    );
    optional_row(
        ui,
        "Ignore OpenGL errors",
        &mut draft.ignore_gl_errors,
        false,
        inherited.map(|i| describe(&i.ignore_gl_errors, |v| on_off(*v))),
        |ui, value| {
            ui.checkbox(value, "Hide host errors from the app");
        },
    );

    crate::ui::section(ui, "Frame rate");
    optional_row(
        ui,
        "Frame rate limit",
        &mut draft.frame_rate_limit,
        FrameRateLimit::Fps(60.0),
        inherited.map(|i| {
            describe(&i.frame_rate_limit, |v| match v {
                FrameRateLimit::Off => "no limit".to_string(),
                FrameRateLimit::Fps(fps) => format!("{fps} fps"),
            })
        }),
        |ui, value| {
            let text = match value {
                FrameRateLimit::Off => "No limit".to_string(),
                FrameRateLimit::Fps(fps) => format!("{fps} fps"),
            };
            egui::ComboBox::from_id_salt("fps-limit")
                .selected_text(text)
                .width(120.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(value, FrameRateLimit::Fps(60.0), "60 fps");
                    ui.selectable_value(value, FrameRateLimit::Fps(30.0), "30 fps");
                    ui.selectable_value(value, FrameRateLimit::Fps(120.0), "120 fps");
                    ui.selectable_value(value, FrameRateLimit::Off, "No limit");
                });
        },
    );
    ui.label(
        egui::RichText::new(
            "The original hardware ran at 60 fps and many apps assume it. \
             Raising the limit rarely makes an app faster.",
        )
        .small()
        .color(theme::LIGHT.text_dim),
    );
    optional_row(
        ui,
        "Log the frame rate",
        &mut draft.print_fps,
        false,
        inherited.map(|i| describe(&i.print_fps, |v| on_off(*v))),
        |ui, value| {
            ui.checkbox(value, "Once per second, to the log");
        },
    );
}

/// How a person drives the emulated device.
///
/// Grouped by the physical thing somebody is holding rather than by which
/// emulator option each setting happens to be, because "my stick is too
/// twitchy" is a thought about a controller, not about a deadzone.
///
/// What each control does *on the app's screen* is not here. That is per-app —
/// a touch target only means something against one app's layout — and lives in
/// that app's own settings.
fn controls_page(ui: &mut Ui, draft: &mut EmulatorSettings, inherited: Option<&EmulatorSettings>) {
    caption(ui, "Where each button touches the screen is set per app.");
    ui.add_space(6.0);

    crate::ui::section(ui, "Analog sticks");
    optional_row(
        ui,
        "Dead zone",
        &mut draft.deadzone,
        0.1,
        inherited.map(|i| describe(&i.deadzone, |v| format!("{v}"))),
        |ui, value| {
            ui.add(egui::Slider::new(value, 0.0..=1.0).fixed_decimals(2))
                .on_hover_text("How far a stick moves before tapHLE notices it.");
        },
    );

    crate::ui::section(ui, "Virtual cursor");
    caption(ui, "The right stick moves a pointer. Press it to tap.");
    optional_row(
        ui,
        "Steady it",
        &mut draft.virtual_cursor_stabilization,
        (0.1, 10.0),
        inherited.map(|i| {
            describe(&i.virtual_cursor_stabilization, |(s, r)| {
                format!("{s}s, {r}px")
            })
        }),
        |ui, value| {
            ui.vertical(|ui| {
                slider_row(ui, "Smoothing", |ui| {
                    ui.add(
                        egui::Slider::new(&mut value.0, 0.0..=0.5)
                            .fixed_decimals(2)
                            .suffix(" s"),
                    )
                    .on_hover_text("Softens sharp movement. Costs response.");
                });
                slider_row(ui, "Ignore movement under", |ui| {
                    ui.add(
                        egui::Slider::new(&mut value.1, 0.0..=40.0)
                            .fixed_decimals(0)
                            .suffix(" px"),
                    )
                    .on_hover_text("Keeps a tap from being read as a drag.");
                });
            });
        },
    );

    crate::ui::section(ui, "Tilting the device");
    caption(
        ui,
        "A desktop has no accelerometer, so a stick stands in for it.",
    );
    caption(ui, "You can also tilt by holding the right mouse button.");
    optional_row(
        ui,
        "Tilt with the left stick",
        &mut draft.analog_stick_tilt,
        true,
        inherited.map(|i| describe(&i.analog_stick_tilt, |v| on_off(*v))),
        |ui, value| {
            ui.checkbox(value, "The left stick tilts the device")
                .on_hover_text("Turn off to leave the stick free for the app.");
        },
    );
    for (label, field, default, hover) in [
        (
            "Sideways range",
            &mut draft.x_tilt_range,
            60.0f32,
            "How far it tilts left and right at full stick.",
        ),
        (
            "Forward range",
            &mut draft.y_tilt_range,
            60.0,
            "How far it tilts towards and away from you.",
        ),
    ] {
        optional_row(ui, label, field, default, None, |ui, value| {
            ui.add(egui::Slider::new(value, 0.0..=180.0).suffix("°"))
                .on_hover_text(hover);
        });
    }
    for (label, field, hover) in [
        (
            "Sideways resting angle",
            &mut draft.x_tilt_offset,
            "Where level is. Usually zero.",
        ),
        (
            "Forward resting angle",
            &mut draft.y_tilt_offset,
            "Racing games often expect the device tipped towards you.",
        ),
    ] {
        optional_row(ui, label, field, 0.0f32, None, |ui, value| {
            ui.add(egui::Slider::new(value, -90.0..=90.0).suffix("°"))
                .on_hover_text(hover);
        });
    }
}

/// What this app's controller does on its own screen, and the way in.
///
/// A summary rather than the editor: where a control sits only means anything
/// against the app's screen, so placing one belongs on a canvas the shape of
/// that screen rather than in a list of numbers in a settings panel.
fn app_control_layout(
    ui: &mut Ui,
    draft: &mut EmulatorSettings,
    inherited: &tapHLE::controls::ControlLayout,
    open_editor: &mut bool,
) {
    let layout = draft.controls.clone().unwrap_or_default();

    if layout.is_empty() {
        if inherited.is_empty() {
            caption(
                ui,
                "No controller mapping. The app is played with the mouse.",
            );
        } else {
            caption(
                ui,
                &format!(
                    "{} inherited mapping(s). Editing makes a copy for this app.",
                    inherited.bindings.len()
                ),
            );
        }
    } else {
        caption(
            ui,
            &format!("{} control(s) set for this app.", layout.bindings.len()),
        );
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.button("Place controls on the screen…").clicked() {
            *open_editor = true;
        }
        if !layout.is_empty() && ui.button("Remove them").clicked() {
            draft.controls = None;
        }
    });
}

/// A quiet line of explanation under a heading.
fn caption(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .small()
            .color(theme::LIGHT.text_dim),
    );
}

/// A slider with its name in front of it, for when several sit inside one row
/// and the row's own label cannot say which is which.
fn slider_row(ui: &mut Ui, label: &str, slider: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [150.0, ui.spacing().interact_size.y],
            egui::Label::new(
                egui::RichText::new(label)
                    .small()
                    .color(theme::LIGHT.text_dim),
            ),
        );
        slider(ui);
    });
}

/// What tapHLE draws when an app asks for one of the iPhone's fonts.
///
/// One row per iPhone font, because that is the unit somebody thinks in: they
/// have opinions about Helvetica, not about "the sans-serif slot". The row says
/// what will be drawn now and why, and the choice next to it can be a family
/// tapHLE ships or any font installed on this computer.
fn fonts_page(ui: &mut Ui, draft: &mut EmulatorSettings, inherited: Option<&EmulatorSettings>) {
    use tapHLE::font::catalogue;

    optional_row(
        ui,
        "Installed fonts",
        &mut draft.use_host_fonts,
        true,
        inherited.map(|i| describe(&i.use_host_fonts, |v| on_off(*v))),
        |ui, value| {
            ui.checkbox(value, "Use mine when the name matches");
        },
    );

    ui.add_space(6.0);

    let installed = tapHLE::font::host::installed().families();

    egui::ScrollArea::vertical()
        .max_height(340.0)
        .show(ui, |ui| {
            egui::Grid::new("font-substitutes")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .striped(true)
                .show(ui, |ui| {
                    for substitution in catalogue::SUBSTITUTIONS {
                        font_row(ui, draft, substitution, &installed);
                        ui.end_row();
                    }
                });
        });
}

/// One iPhone font and the font drawn for it.
///
/// Deliberately just the two: the catalogue knows how close each substitute is
/// and why it was chosen, and that belongs in the catalogue. A settings row
/// that explains itself is a settings row nobody reads.
fn font_row(
    ui: &mut Ui,
    draft: &mut EmulatorSettings,
    substitution: &tapHLE::font::catalogue::Substitution,
    installed: &[String],
) {
    use tapHLE::font::catalogue;

    let family = substitution.ios_family;
    let default_name = catalogue::bundled(substitution.bundled)
        .map_or(substitution.bundled, |bundled| bundled.name);
    // The font is what the row is about, so it leads; "default" is a note
    // about where the choice came from and belongs after it.
    let default_label = format!("{default_name} (default)");

    ui.label(family);

    let current = draft.font_choices.get(family).cloned();
    let shown = match &current {
        None => default_label.clone(),
        Some(choice) => match catalogue::bundled(choice) {
            Some(bundled) => bundled.name.to_string(),
            None => choice.clone(),
        },
    };

    egui::ComboBox::from_id_salt(("font", family))
        .selected_text(shown)
        .width(240.0)
        .show_ui(ui, |ui| {
            // The list is the bundled families plus every font on the
            // computer, which is over a hundred here; without a height it
            // grows past the bottom of the screen and the last entries cannot
            // be reached at all.
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    let mut chosen: Option<Option<String>> = None;
                    if ui
                        .selectable_label(current.is_none(), &default_label)
                        .clicked()
                    {
                        chosen = Some(None);
                    }
                    ui.separator();
                    // The two group headings stay. With a hundred and forty
                    // entries, knowing which half you are in is navigation
                    // rather than commentary.
                    ui.label(
                        egui::RichText::new("Bundled")
                            .small()
                            .color(theme::LIGHT.text_dim),
                    );
                    for bundled in catalogue::BUNDLED {
                        let selected = current.as_deref() == Some(bundled.id);
                        if ui.selectable_label(selected, bundled.name).clicked() {
                            chosen = Some(Some(bundled.id.to_string()));
                        }
                    }
                    if !installed.is_empty() {
                        ui.separator();
                        ui.label(
                            egui::RichText::new("Installed")
                                .small()
                                .color(theme::LIGHT.text_dim),
                        );
                        for name in installed {
                            let selected = current.as_deref() == Some(name.as_str());
                            if ui.selectable_label(selected, name).clicked() {
                                chosen = Some(Some(name.clone()));
                            }
                        }
                    }

                    match chosen {
                        Some(None) => {
                            draft.font_choices.remove(family);
                        }
                        Some(Some(choice)) => {
                            draft.font_choices.insert(family.to_string(), choice);
                        }
                        None => (),
                    }
                });
        });
}

fn system_page(ui: &mut Ui, draft: &mut EmulatorSettings, inherited: Option<&EmulatorSettings>) {
    crate::ui::section(ui, "Behaviour");
    optional_row(
        ui,
        "Network access",
        &mut draft.network_access,
        false,
        inherited.map(|i| describe(&i.network_access, |v| on_off(*v))),
        |ui, value| {
            ui.checkbox(value, "Let apps reach the network");
        },
    );
    optional_row(
        ui,
        "Error message box",
        &mut draft.error_popup,
        true,
        inherited.map(|i| describe(&i.error_popup, |v| on_off(*v))),
        |ui, value| {
            ui.checkbox(value, "Show a box when a run fails");
        },
    );
    optional_row(
        ui,
        "Preferred languages",
        &mut draft.preferred_languages,
        "en".to_string(),
        inherited.map(|i| describe(&i.preferred_languages, |v| v.clone())),
        |ui, value| {
            ui.add(
                egui::TextEdit::singleline(value)
                    .desired_width(180.0)
                    .hint_text("en,fr,de"),
            );
        },
    );

    crate::ui::section(ui, "Advanced");
    optional_row(
        ui,
        "Direct memory access",
        &mut draft.direct_memory_access,
        true,
        inherited.map(|i| describe(&i.direct_memory_access, |v| on_off(*v))),
        |ui, value| {
            ui.checkbox(value, "Fast path for guest memory");
        },
    );
    optional_row(
        ui,
        "Extra arguments",
        &mut draft.extra_arguments,
        String::new(),
        inherited.map(|i| describe(&i.extra_arguments, |v| v.clone())),
        |ui, value| {
            ui.add(
                egui::TextEdit::singleline(value)
                    .desired_width(230.0)
                    .hint_text("--button-to-touch=A,0.5,0.9"),
            );
        },
    );
    ui.label(
        egui::RichText::new(
            "Anything tapHLE's command line accepts. It is checked before a \
             run starts, and OPTIONS_HELP.txt lists every option.",
        )
        .small()
        .color(theme::LIGHT.text_dim),
    );
}

fn logging_page(ui: &mut Ui, draft: &mut EmulatorSettings, inherited: Option<&EmulatorSettings>) {
    crate::ui::section(ui, "Emulator tracing");
    optional_row(
        ui,
        "Verbose modules",
        &mut draft.log_modules,
        String::new(),
        inherited.map(|i| describe(&i.log_modules, |v| v.clone())),
        |ui, value| {
            ui.add(
                egui::TextEdit::singleline(value)
                    .desired_width(230.0)
                    .hint_text("tapHLE::frameworks::uikit"),
            );
        },
    );
    ui.label(
        egui::RichText::new(
            "A comma-separated list of tapHLE modules to trace in detail, or \
             `all`. This is the TAPHLE_LOG_MODULES environment variable, and \
             it makes runs considerably slower.",
        )
        .small()
        .color(theme::LIGHT.text_dim),
    );
}

fn frontend_logging_page(ui: &mut Ui, draft: &mut FrontendSettings) {
    crate::ui::section(ui, "Log panel");
    ui.checkbox(
        &mut draft.reveal_log_on_crash,
        "Open the log panel when a run ends badly",
    );
    ui.checkbox(&mut draft.log_show_timestamps, "Show timestamps");
    ui.horizontal(|ui| {
        ui.add_sized(
            [150.0, 18.0],
            egui::Label::new("Lines kept").halign(egui::Align::LEFT),
        );
        ui.add(
            egui::DragValue::new(&mut draft.log_capacity)
                .range(1000..=2_000_000)
                .speed(1000.0),
        );
    });
    ui.label(
        egui::RichText::new(
            "Older lines are discarded once this many are held. The panel says \
             how many have been dropped.",
        )
        .small()
        .color(theme::LIGHT.text_dim),
    );
}

fn general_page(ui: &mut Ui, draft: &mut FrontendSettings) {
    crate::ui::section(ui, "Interface");
    ui.horizontal(|ui| {
        ui.add_sized(
            [150.0, 18.0],
            egui::Label::new("Interface scale").halign(egui::Align::LEFT),
        );
        ui.add(
            egui::Slider::new(&mut draft.ui_zoom, 0.75..=2.0)
                .fixed_decimals(2)
                .suffix("×"),
        );
    });
    ui.label(
        egui::RichText::new("On top of the display's own scaling, which tapHLE already follows.")
            .small()
            .color(theme::LIGHT.text_dim),
    );
    ui.add_space(4.0);
    ui.checkbox(
        &mut draft.confirm_remove,
        "Ask before removing an app from the library",
    );
    ui.checkbox(
        &mut draft.developer_mode,
        "Developer mode: keep the log panel open and show extra tools",
    );

    crate::ui::section(ui, "Updates");
    ui.checkbox(
        &mut draft.check_for_updates,
        "Check GitHub for a newer tapHLE at startup",
    );
    ui.label(
        egui::RichText::new(
            "tapHLE has not published a release yet, so this currently reports \
             that there is nothing to compare against.",
        )
        .small()
        .color(theme::LIGHT.text_dim),
    );
}

fn paths_page(ui: &mut Ui, draft: &mut FrontendSettings) {
    crate::ui::section(ui, "tapHLE");
    let data_dir = crate::storage::data_dir();
    crate::ui::field(ui, "Installation", &crate::storage::display_path(&data_dir));
    crate::ui::field(
        ui,
        "Frontend files",
        &crate::storage::display_path(&crate::storage::frontend_dir()),
    );
    crate::ui::field(
        ui,
        "Saved app data",
        &crate::storage::display_path(&data_dir.join(tapHLE::paths::SANDBOX_DIR)),
    );

    crate::ui::section(ui, "Emulator");
    ui.horizontal(|ui| {
        let text = draft
            .emulator_path
            .as_ref()
            .map(|p| crate::storage::display_path(p))
            .unwrap_or_else(|| "found automatically".to_string());
        ui.add_sized(
            [150.0, 18.0],
            egui::Label::new("tapHLE program").halign(egui::Align::LEFT),
        );
        ui.add(egui::Label::new(egui::RichText::new(text).small()).truncate());
    });
    ui.horizontal(|ui| {
        ui.add_space(156.0);
        if ui.button("Choose…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Choose the tapHLE emulator program")
                .pick_file()
            {
                draft.emulator_path = Some(path);
            }
        }
        if draft.emulator_path.is_some() && ui.button("Reset").clicked() {
            draft.emulator_path = None;
        }
    });

    crate::ui::section(ui, "Library folders");
    ui.label(
        egui::RichText::new("Rescanning looks in these folders. tapHLE_apps is always included.")
            .small()
            .color(theme::LIGHT.text_dim),
    );
    let mut remove = None;
    for (index, folder) in draft.library_folders.iter().enumerate() {
        ui.horizontal(|ui| {
            if ui.small_button("Remove").clicked() {
                remove = Some(index);
            }
            ui.add(
                egui::Label::new(egui::RichText::new(crate::storage::display_path(folder)).small())
                    .truncate(),
            );
        });
    }
    if let Some(index) = remove {
        draft.library_folders.remove(index);
    }
    if ui.button("Add Folder…").clicked() {
        if let Some(folder) = rfd::FileDialog::new()
            .set_title("Choose a folder to scan for apps")
            .pick_folder()
        {
            if !draft.library_folders.contains(&folder) {
                draft.library_folders.push(folder);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The per-app dialog must not offer the categories that only make sense
    /// once, or a person could set an interface scale "for this app".
    #[test]
    fn per_app_settings_exclude_the_frontend_only_categories() {
        assert!(!Category::PER_APP.contains(&Category::General));
        assert!(!Category::PER_APP.contains(&Category::Paths));
        for category in Category::PER_APP {
            assert!(Category::GLOBAL.contains(category));
        }
    }

    #[test]
    fn inherited_values_are_described_in_words() {
        assert_eq!(describe(&Some(true), |v| on_off(*v)), "inherits on");
        assert_eq!(
            describe(&None::<bool>, |v| on_off(*v)),
            "inherits tapHLE's default"
        );
    }
}
