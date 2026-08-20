/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Placing an app's controls by pointing at the screen they act on.
//!
//! The mapping this edits has always existed — `--button-to-touch=A,470,310`
//! — but as a coordinate somebody had to work out by eye and then type. The
//! whole point of this editor is that nobody should have to know the number:
//! drag the marker onto the app's own button, and the number is whatever it
//! needs to be.
//!
//! Editing happens on a **draft**. Nothing here touches the layout that is
//! actually in use until Save, which is what makes Undo, Redo and Cancel
//! honest and what stops a half-placed control from reaching a run.
//!
//! Geometry is a fraction of the guest screen throughout, so the canvas can be
//! any size on screen and the app can be any shape. See [tapHLE::controls].

use crate::theme;
use egui::{Color32, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use tapHLE::controls::{Binding, ControlLayout, Geometry, Source, Target, TargetKind};
use tapHLE::options::Button;

/// Every button a layout can bind, in the order the emulator lists them.
const BUTTONS: &[Button] = &[
    Button::A,
    Button::B,
    Button::X,
    Button::Y,
    Button::Start,
    Button::LeftShoulder,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
];

fn button_name(button: Button) -> &'static str {
    match button {
        Button::A => "A",
        Button::B => "B",
        Button::X => "X",
        Button::Y => "Y",
        Button::Start => "Start",
        Button::LeftShoulder => "Left shoulder",
        Button::DPadUp => "D-pad up",
        Button::DPadDown => "D-pad down",
        Button::DPadLeft => "D-pad left",
        Button::DPadRight => "D-pad right",
    }
}

fn source_name(source: Source) -> String {
    match source {
        Source::Button(button) => button_name(button).to_string(),
        Source::Dpad => "D-pad".to_string(),
        Source::LeftStick => "Left stick".to_string(),
    }
}

#[derive(PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Save,
    Cancel,
}

/// What the editor is doing with the next click on the canvas.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Placing {
    Nothing,
    Touch,
    StickZone,
}

pub struct ControlEditor {
    pub entry_id: String,
    pub title: String,
    /// The layout being edited. Only this is ever changed.
    pub draft: ControlLayout,
    /// What applies when this app overrides nothing — the shipped defaults and
    /// anything set globally. Drawn faintly so somebody can see what they are
    /// replacing before they replace it.
    pub inherited: ControlLayout,
    /// The guest screen, used for the canvas shape and to show real pixels.
    pub screen: (f32, f32),
    selected: Option<String>,
    placing: Placing,
    undo: Vec<ControlLayout>,
    redo: Vec<ControlLayout>,
    /// The app's own screen, drawn under the controls. Without it somebody is
    /// placing a marker on a black rectangle and guessing where the app's
    /// button is, which is the thing this editor exists to stop.
    backdrop: Option<egui::TextureHandle>,
    capture: Option<crate::capture::Capture>,
    capture_error: Option<String>,
    /// Raised when somebody asks for a screen. The editor cannot launch the
    /// emulator itself — it does not know where it is or what arguments this
    /// app needs — so the frontend does it.
    pub wants_capture: bool,
}

impl ControlEditor {
    pub fn new(
        entry_id: String,
        title: String,
        draft: ControlLayout,
        inherited: ControlLayout,
        screen: (f32, f32),
    ) -> ControlEditor {
        ControlEditor {
            entry_id,
            title,
            draft,
            inherited,
            screen,
            selected: None,
            placing: Placing::Nothing,
            undo: Vec::new(),
            redo: Vec::new(),
            backdrop: None,
            capture: None,
            capture_error: None,
            wants_capture: false,
        }
    }

    /// Take a copy of the draft before changing it.
    ///
    /// Called before every edit rather than after, so an Undo returns to the
    /// state somebody could actually see rather than to the one after the
    /// change they wanted to take back.
    fn checkpoint(&mut self) {
        self.undo.push(self.draft.clone());
        // A new edit makes any redone future unreachable, which is what every
        // other editor does and what people expect.
        self.redo.clear();
        // Bounded so a long session cannot grow without limit.
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
    }

    fn unused_id(&self, stem: &str) -> String {
        if !self.draft.targets.iter().any(|t| t.id == stem) {
            return stem.to_string();
        }
        (2..)
            .map(|n| format!("{stem}-{n}"))
            .find(|id| !self.draft.targets.iter().any(|t| &t.id == id))
            .expect("an unused id always exists")
    }

    /// Which sources are bound more than once.
    ///
    /// Not an error. Two controls on one button is a legitimate thing to want
    /// — a button that touches two places at once — so this reports it and
    /// lets somebody decide.
    fn duplicated_sources(&self) -> Vec<Source> {
        let mut seen: Vec<(Source, usize)> = Vec::new();
        for binding in &self.draft.bindings {
            match seen.iter_mut().find(|(s, _)| *s == binding.source) {
                Some((_, count)) => *count += 1,
                None => seen.push((binding.source, 1)),
            }
        }
        seen.into_iter()
            .filter(|(_, count)| *count > 1)
            .map(|(source, _)| source)
            .collect()
    }

    fn binding_for(&self, target_id: &str) -> Option<&Binding> {
        self.draft.bindings.iter().find(|b| b.target == target_id)
    }

    /// Take over a capture the frontend started.
    pub fn capturing(&mut self, capture: crate::capture::Capture) {
        self.capture = Some(capture);
        self.capture_error = None;
    }

    fn poll_capture(&mut self, ctx: &egui::Context) {
        let Some(capture) = &mut self.capture else {
            return;
        };
        match capture.poll() {
            crate::capture::Progress::Working => {
                // The worker is on another thread, so nothing would otherwise
                // wake the interface when the frame lands.
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
            crate::capture::Progress::Ready(frame) => {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [frame.width, frame.height],
                    &frame.rgba,
                );
                self.backdrop =
                    Some(ctx.load_texture("control-editor-backdrop", image, Default::default()));
                self.capture = None;
            }
            crate::capture::Progress::Failed(e) => {
                self.capture_error = Some(e);
                self.capture = None;
            }
        }
    }

    fn remove(&mut self, target_id: &str) {
        self.draft.targets.retain(|t| t.id != target_id);
        self.draft.bindings.retain(|b| b.target != target_id);
    }
}

/// Draw the editor. Returns what the person asked for.
pub fn show(ctx: &egui::Context, editor: &mut ControlEditor) -> Outcome {
    let mut outcome = Outcome::Continue;
    editor.poll_capture(ctx);
    let response = egui::Modal::new(Id::new("taphle-control-editor")).show(ctx, |ui| {
        ui.set_width(820.0);
        ui.heading(format!("Controls for {}", editor.title));
        ui.label(
            egui::RichText::new("Add a control, then drag it onto the app's own button.")
                .small()
                .color(theme::LIGHT.text_dim),
        );
        ui.add_space(4.0);
        theme::hairline(ui);
        ui.add_space(6.0);

        ui.horizontal_top(|ui| {
            canvas(ui, editor);
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.set_width(300.0);
                inspector(ui, editor);
            });
        });

        ui.add_space(8.0);
        theme::hairline(ui);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let can_undo = !editor.undo.is_empty();
            let can_redo = !editor.redo.is_empty();
            if ui
                .add_enabled(can_undo, egui::Button::new("Undo"))
                .clicked()
            {
                if let Some(previous) = editor.undo.pop() {
                    editor.redo.push(editor.draft.clone());
                    editor.draft = previous;
                    editor.selected = None;
                }
            }
            if ui
                .add_enabled(can_redo, egui::Button::new("Redo"))
                .clicked()
            {
                if let Some(next) = editor.redo.pop() {
                    editor.undo.push(editor.draft.clone());
                    editor.draft = next;
                    editor.selected = None;
                }
            }
            if ui
                .button("Clear all")
                .on_hover_text("Remove every control this app sets")
                .clicked()
            {
                editor.checkpoint();
                editor.draft = ControlLayout::default();
                editor.selected = None;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Save").clicked() {
                    outcome = Outcome::Save;
                }
                if ui.button("Cancel").clicked() {
                    outcome = Outcome::Cancel;
                }
            });
        });
    });
    if response.should_close() && outcome == Outcome::Continue {
        outcome = Outcome::Cancel;
    }
    outcome
}

/// The guest screen, and the controls sitting on it.
fn canvas(ui: &mut Ui, editor: &mut ControlEditor) {
    let (guest_w, guest_h) = editor.screen;
    // Fit the guest's own proportions into the space available, so a landscape
    // app is a landscape canvas and a mapping is placed against the shape it
    // will actually meet.
    let available = Vec2::new(460.0, 380.0);
    let scale = (available.x / guest_w).min(available.y / guest_h);
    let size = Vec2::new(guest_w * scale, guest_h * scale);

    let (response, painter) = ui.allocate_painter(size, Sense::click_and_drag());
    let rect = response.rect;

    painter.rect_filled(rect, 2.0, Color32::from_gray(28));
    if let Some(backdrop) = &editor.backdrop {
        // Filled to the canvas, which is already the guest screen's own
        // proportions, so the picture and the coordinates agree even though
        // the capture's pixel size is whatever the scale hack made it.
        painter.image(
            backdrop.id(),
            rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0_f32, theme::LIGHT.border),
        egui::StrokeKind::Inside,
    );

    let to_screen = |g: Geometry| -> Pos2 {
        Pos2::new(
            rect.left() + g.x * rect.width(),
            rect.top() + g.y * rect.height(),
        )
    };

    // What this app inherits, drawn faintly and not editable: somebody should
    // be able to see the mapping they are about to replace.
    for target in &editor.inherited.targets {
        let at = to_screen(target.geometry);
        match target.kind {
            TargetKind::Touch => {
                painter.circle_stroke(at, 13.0_f32, Stroke::new(1.0_f32, Color32::from_gray(90)));
            }
            TargetKind::StickZone => {
                let zone = Rect::from_min_size(
                    at,
                    Vec2::new(
                        target.geometry.width * rect.width(),
                        target.geometry.height * rect.height(),
                    ),
                );
                painter.rect_stroke(
                    zone,
                    4.0,
                    Stroke::new(1.0_f32, Color32::from_gray(90)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    // Placing a new control: the next click on the canvas puts it there.
    if editor.placing != Placing::Nothing {
        if let Some(at) = response.interact_pointer_pos() {
            if response.clicked() {
                let g = Geometry {
                    x: ((at.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
                    y: ((at.y - rect.top()) / rect.height()).clamp(0.0, 1.0),
                    width: if editor.placing == Placing::StickZone {
                        0.25
                    } else {
                        0.0
                    },
                    height: if editor.placing == Placing::StickZone {
                        0.25
                    } else {
                        0.0
                    },
                };
                let (kind, stem, source) = match editor.placing {
                    Placing::StickZone => (TargetKind::StickZone, "stick", Source::LeftStick),
                    _ => (TargetKind::Touch, "touch", Source::Button(Button::A)),
                };
                let id = editor.unused_id(stem);
                editor.checkpoint();
                editor.draft.targets.push(Target {
                    id: id.clone(),
                    kind,
                    geometry: g,
                    label: None,
                });
                editor.draft.bindings.push(Binding {
                    source,
                    target: id.clone(),
                });
                editor.selected = Some(id);
                editor.placing = Placing::Nothing;
            }
        }
    }

    // Existing controls, newest last so a click picks the one on top.
    let mut clicked_target: Option<String> = None;
    let mut dragged: Option<(String, Vec2)> = None;
    for target in &editor.draft.targets {
        let at = to_screen(target.geometry);
        let selected = editor.selected.as_deref() == Some(target.id.as_str());
        let accent = if selected {
            theme::LIGHT.accent
        } else {
            Color32::from_rgb(120, 170, 255)
        };

        let hit = match target.kind {
            TargetKind::Touch => Rect::from_center_size(at, Vec2::splat(30.0)),
            TargetKind::StickZone => Rect::from_min_size(
                at,
                Vec2::new(
                    target.geometry.width * rect.width(),
                    target.geometry.height * rect.height(),
                ),
            ),
        };

        match target.kind {
            TargetKind::Touch => {
                painter.circle_filled(at, 15.0, accent.linear_multiply(0.35));
                painter.circle_stroke(
                    at,
                    15.0,
                    Stroke::new(if selected { 2.0_f32 } else { 1.0_f32 }, accent),
                );
            }
            TargetKind::StickZone => {
                painter.rect_filled(hit, 6.0, accent.linear_multiply(0.22));
                painter.rect_stroke(
                    hit,
                    6.0,
                    Stroke::new(if selected { 2.0_f32 } else { 1.0_f32 }, accent),
                    egui::StrokeKind::Inside,
                );
            }
        }

        let caption = target.label.clone().unwrap_or_else(|| {
            match editor.draft.bindings.iter().find(|b| b.target == target.id) {
                Some(binding) => source_name(binding.source),
                None => target.id.clone(),
            }
        });
        painter.text(
            hit.center(),
            egui::Align2::CENTER_CENTER,
            caption,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );

        if let Some(pointer) = response.interact_pointer_pos() {
            if hit.contains(pointer) {
                if response.clicked() {
                    clicked_target = Some(target.id.clone());
                }
                if response.dragged() {
                    dragged = Some((target.id.clone(), response.drag_delta()));
                }
            }
        }
    }

    if let Some(id) = clicked_target {
        editor.selected = Some(id);
    }
    if let Some((id, delta)) = dragged {
        if delta != Vec2::ZERO {
            // One checkpoint for a whole drag, not one per frame, or Undo
            // would step back a pixel at a time.
            if !editor.undo.last().is_some_and(|l| *l == editor.draft) {
                editor.checkpoint();
            }
            if let Some(target) = editor.draft.targets.iter_mut().find(|t| t.id == id) {
                target.geometry.x = (target.geometry.x + delta.x / rect.width())
                    .clamp(0.0, 1.0 - target.geometry.width);
                target.geometry.y = (target.geometry.y + delta.y / rect.height())
                    .clamp(0.0, 1.0 - target.geometry.height);
            }
            editor.selected = Some(id);
        }
    }
}

/// The properties of whatever is selected, and the way to add something.
fn inspector(ui: &mut Ui, editor: &mut ControlEditor) {
    crate::ui::section(ui, "The app's screen");
    if editor.capture.is_some() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(
                egui::RichText::new("Starting the app and taking a picture…")
                    .small()
                    .color(theme::LIGHT.text_dim),
            );
        });
    } else {
        ui.horizontal(|ui| {
            let label = if editor.backdrop.is_some() {
                "Take another picture"
            } else {
                "Show the app's screen"
            };
            if ui
                .button(label)
                .on_hover_text(
                    "Runs the app briefly, takes one frame, and closes it.                      This does not count as playing it.",
                )
                .clicked()
            {
                editor.wants_capture = true;
            }
        });
        if editor.backdrop.is_none() {
            caption_dim(
                ui,
                "Without it you are placing controls on an empty screen.",
            );
        }
    }
    if let Some(error) = &editor.capture_error {
        ui.label(
            egui::RichText::new(error)
                .small()
                .color(theme::LIGHT.warning),
        );
    }

    ui.add_space(8.0);
    crate::ui::section(ui, "Add");
    ui.horizontal(|ui| {
        let touch = ui.selectable_label(editor.placing == Placing::Touch, "Touch");
        if touch.clicked() {
            editor.placing = if editor.placing == Placing::Touch {
                Placing::Nothing
            } else {
                Placing::Touch
            };
        }
        let zone = ui.selectable_label(editor.placing == Placing::StickZone, "Stick zone");
        if zone.clicked() {
            editor.placing = if editor.placing == Placing::StickZone {
                Placing::Nothing
            } else {
                Placing::StickZone
            };
        }
    });
    if editor.placing != Placing::Nothing {
        ui.label(
            egui::RichText::new("Click the screen to place it.")
                .small()
                .color(theme::LIGHT.accent),
        );
    }

    ui.add_space(8.0);
    crate::ui::section(ui, "Selected");

    let Some(selected) = editor.selected.clone() else {
        ui.label(
            egui::RichText::new("Nothing selected. Click a control to change it.")
                .small()
                .color(theme::LIGHT.text_dim),
        );
        return;
    };

    let Some(index) = editor.draft.targets.iter().position(|t| t.id == selected) else {
        editor.selected = None;
        return;
    };

    // Read what is needed before borrowing mutably, so the inspector can talk
    // about the binding while editing the target.
    let kind = editor.draft.targets[index].kind;
    let current_source = editor.binding_for(&selected).map(|b| b.source);
    let mut new_source = current_source;
    let mut label = editor.draft.targets[index]
        .label
        .clone()
        .unwrap_or_default();
    let mut geometry = editor.draft.targets[index].geometry;
    let mut delete = false;

    ui.horizontal(|ui| {
        ui.label("Name");
        if ui.text_edit_singleline(&mut label).changed() {
            // Named as it is typed rather than on a confirm, because a text
            // box that quietly discards what was typed is worse than one
            // extra undo step.
        }
    });

    ui.horizontal(|ui| {
        ui.label("Input");
        let shown = current_source
            .map(source_name)
            .unwrap_or_else(|| "None".to_string());
        egui::ComboBox::from_id_salt("control-source")
            .selected_text(shown)
            .show_ui(ui, |ui| match kind {
                TargetKind::Touch => {
                    for &button in BUTTONS {
                        let source = Source::Button(button);
                        if ui
                            .selectable_label(current_source == Some(source), button_name(button))
                            .clicked()
                        {
                            new_source = Some(source);
                        }
                    }
                }
                TargetKind::StickZone => {
                    for source in [Source::LeftStick, Source::Dpad] {
                        if ui
                            .selectable_label(current_source == Some(source), source_name(source))
                            .clicked()
                        {
                            new_source = Some(source);
                        }
                    }
                }
            });
    });

    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Position")
            .small()
            .color(theme::LIGHT.text_dim),
    );
    let (guest_w, guest_h) = editor.screen;
    ui.horizontal(|ui| {
        ui.label("X");
        let mut px = geometry.x * guest_w;
        if ui
            .add(
                egui::DragValue::new(&mut px)
                    .speed(1.0)
                    .range(0.0..=guest_w),
            )
            .changed()
        {
            geometry.x = px / guest_w;
        }
        ui.label("Y");
        let mut py = geometry.y * guest_h;
        if ui
            .add(
                egui::DragValue::new(&mut py)
                    .speed(1.0)
                    .range(0.0..=guest_h),
            )
            .changed()
        {
            geometry.y = py / guest_h;
        }
    });
    if kind == TargetKind::StickZone {
        ui.horizontal(|ui| {
            ui.label("W");
            let mut pw = geometry.width * guest_w;
            if ui
                .add(
                    egui::DragValue::new(&mut pw)
                        .speed(1.0)
                        .range(0.0..=guest_w),
                )
                .changed()
            {
                geometry.width = pw / guest_w;
            }
            ui.label("H");
            let mut ph = geometry.height * guest_h;
            if ui
                .add(
                    egui::DragValue::new(&mut ph)
                        .speed(1.0)
                        .range(0.0..=guest_h),
                )
                .changed()
            {
                geometry.height = ph / guest_h;
            }
        });
    }
    ui.label(
        egui::RichText::new(format!(
            "Guest pixels, on a {guest_w:.0} by {guest_h:.0} screen."
        ))
        .small()
        .color(theme::LIGHT.text_dim),
    );

    ui.add_space(8.0);
    if ui.button("Delete").clicked() {
        delete = true;
    }

    // Apply after the interface is built, so nothing is borrowed twice.
    if delete {
        editor.checkpoint();
        editor.remove(&selected);
        editor.selected = None;
        return;
    }
    let target = &mut editor.draft.targets[index];
    let label_changed = target.label.clone().unwrap_or_default() != label;
    let geometry_changed = target.geometry != geometry;
    if label_changed || geometry_changed {
        target.label = (!label.trim().is_empty()).then(|| label.trim().to_string());
        target.geometry = geometry;
    }
    if new_source != current_source {
        if let Some(source) = new_source {
            editor.draft.bindings.retain(|b| b.target != selected);
            editor.draft.bindings.push(Binding {
                source,
                target: selected.clone(),
            });
        }
    }

    let duplicates = editor.duplicated_sources();
    if !duplicates.is_empty() {
        ui.add_space(6.0);
        let names: Vec<String> = duplicates.into_iter().map(source_name).collect();
        ui.label(
            egui::RichText::new(format!(
                "{} drives more than one control. That works — every one of \
                 them fires — but it is worth knowing.",
                names.join(", ")
            ))
            .small()
            .color(theme::LIGHT.warning),
        );
    }
}

/// A quiet line under a control.
fn caption_dim(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .small()
            .color(theme::LIGHT.text_dim),
    );
}
