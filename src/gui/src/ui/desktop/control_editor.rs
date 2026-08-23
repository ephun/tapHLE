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

use crate::ui::theme;
use egui::{Color32, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use tapHLE::controls::{Binding, ControlLayout, Geometry, Source, Target, TargetKind};
use tapHLE::options::{Button, Key, KeyDpad};

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
        Source::Key(key) => format!("{} key", key_name(key)),
        Source::KeyDpad(dpad) => match dpad {
            ARROW_KEYS => "Arrow keys".to_string(),
            WASD => "W A S D".to_string(),
            other => format!(
                "{} {} {} {}",
                key_name(other.up),
                key_name(other.left),
                key_name(other.down),
                key_name(other.right),
            ),
        },
    }
}

/// What a key is called on the keyboard somebody is looking at, rather than
/// what it is called in a settings file.
fn key_name(key: Key) -> String {
    match key {
        Key::Left => "←".to_string(),
        Key::Right => "→".to_string(),
        Key::Up => "↑".to_string(),
        Key::Down => "↓".to_string(),
        Key::Return => "Enter".to_string(),
        Key::Backquote => "`".to_string(),
        Key::LeftBracket => "[".to_string(),
        Key::RightBracket => "]".to_string(),
        Key::Backslash => "\\".to_string(),
        Key::Semicolon => ";".to_string(),
        Key::Quote => "'".to_string(),
        Key::Comma => ",".to_string(),
        Key::Period => ".".to_string(),
        Key::Slash => "/".to_string(),
        Key::Minus => "-".to_string(),
        Key::Equals => "=".to_string(),
        // The digits are `Num0`..`Num9` so they can be enum variants; nobody
        // calls the 1 key "Num1".
        other => other.name().trim_start_matches("Num").to_string(),
    }
}

/// The two arrangements every game with keyboard movement uses. Offered as
/// whole choices because that is how somebody thinks of them — "the arrow
/// keys", not four separate bindings they have to get right in order.
const ARROW_KEYS: KeyDpad = KeyDpad {
    up: Key::Up,
    down: Key::Down,
    left: Key::Left,
    right: Key::Right,
};
const WASD: KeyDpad = KeyDpad {
    up: Key::W,
    down: Key::S,
    left: Key::A,
    right: Key::D,
};

/// The key somebody just pressed, if the editor is listening for one.
///
/// egui reports the modifiers separately from the keys, because for its own
/// widgets they are modifiers rather than keys. Here they are keys like any
/// other — a game that wants Shift to fire wants Shift to fire — so they are
/// read back out of the modifier state.
fn pressed_key(ctx: &egui::Context) -> Option<Key> {
    ctx.input(|i| {
        for event in &i.events {
            if let egui::Event::Key {
                key, pressed: true, ..
            } = event
            {
                if let Some(key) = from_egui_key(*key) {
                    return Some(key);
                }
            }
        }
        if i.modifiers.shift {
            return Some(Key::Shift);
        }
        if i.modifiers.ctrl {
            return Some(Key::Ctrl);
        }
        if i.modifiers.alt {
            return Some(Key::Alt);
        }
        None
    })
}

fn from_egui_key(key: egui::Key) -> Option<Key> {
    use egui::Key as E;
    Some(match key {
        E::A => Key::A,
        E::B => Key::B,
        E::C => Key::C,
        E::D => Key::D,
        E::E => Key::E,
        E::F => Key::F,
        E::G => Key::G,
        E::H => Key::H,
        E::I => Key::I,
        E::J => Key::J,
        E::K => Key::K,
        E::L => Key::L,
        E::M => Key::M,
        E::N => Key::N,
        E::O => Key::O,
        E::P => Key::P,
        E::Q => Key::Q,
        E::R => Key::R,
        E::S => Key::S,
        E::T => Key::T,
        E::U => Key::U,
        E::V => Key::V,
        E::W => Key::W,
        E::X => Key::X,
        E::Y => Key::Y,
        E::Z => Key::Z,
        E::Num0 => Key::Num0,
        E::Num1 => Key::Num1,
        E::Num2 => Key::Num2,
        E::Num3 => Key::Num3,
        E::Num4 => Key::Num4,
        E::Num5 => Key::Num5,
        E::Num6 => Key::Num6,
        E::Num7 => Key::Num7,
        E::Num8 => Key::Num8,
        E::Num9 => Key::Num9,
        E::ArrowLeft => Key::Left,
        E::ArrowRight => Key::Right,
        E::ArrowUp => Key::Up,
        E::ArrowDown => Key::Down,
        E::Space => Key::Space,
        E::Enter => Key::Return,
        E::Tab => Key::Tab,
        E::Backspace => Key::Backspace,
        E::Comma => Key::Comma,
        E::Period => Key::Period,
        E::Slash => Key::Slash,
        E::Semicolon => Key::Semicolon,
        E::Quote => Key::Quote,
        E::OpenBracket => Key::LeftBracket,
        E::CloseBracket => Key::RightBracket,
        E::Backslash => Key::Backslash,
        E::Minus => Key::Minus,
        E::Equals => Key::Equals,
        E::Backtick => Key::Backquote,
        // Escape cancels the listening rather than binding, so it is not
        // here, and neither is anything the emulator cannot map.
        _ => return None,
    })
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
    capture: Option<crate::run::capture::Capture>,
    capture_error: Option<String>,
    /// Set while the editor is waiting for a key press to bind.
    listening: bool,
    /// True only on the frame listening began, so a modifier somebody happened
    /// to be holding when they clicked is not taken as the key they meant.
    listening_started: bool,
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
            listening: false,
            listening_started: false,
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
    pub fn capturing(&mut self, capture: crate::run::capture::Capture) {
        self.capture = Some(capture);
        self.capture_error = None;
    }

    fn poll_capture(&mut self, ctx: &egui::Context) {
        let Some(capture) = &mut self.capture else {
            return;
        };
        match capture.poll() {
            crate::run::capture::Progress::Running | crate::run::capture::Progress::Taking => {
                // Nothing else would wake the interface: the app closing, or
                // the frame landing, are both changes on disk rather than
                // input events.
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
            crate::run::capture::Progress::Ready(frame) => {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [frame.width, frame.height],
                    &frame.rgba,
                );
                self.backdrop =
                    Some(ctx.load_texture("control-editor-backdrop", image, Default::default()));
                self.capture = None;
            }
            crate::run::capture::Progress::Failed(e) => {
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

/// Where a newly placed control sits, as a fraction of the guest screen.
///
/// A touch is a point, so it is simply where the click landed. A stick zone
/// is a circle centred on it: the thing a zone stands for is a thumb sweeping
/// a stick, which reaches every direction equally, so a shape with a long
/// axis says something about the control that is not true.
///
/// Round has to mean round in guest pixels, not in fractions. A quarter of
/// each axis — what this used to be — is a circle only on a square screen; on
/// a 320 by 480 phone it came out as an 80 by 120 oval. So the diameter is a
/// quarter of the shorter side, converted into each axis separately.
///
/// Split out of [canvas] so the shape can be checked without a window.
fn placed_geometry(placing: Placing, at: (f32, f32), screen: (f32, f32)) -> Geometry {
    let (guest_w, guest_h) = screen;
    let (width, height) = match placing {
        Placing::StickZone => {
            let diameter = 0.25 * guest_w.min(guest_h);
            (diameter / guest_w, diameter / guest_h)
        }
        _ => (0.0, 0.0),
    };
    // Clamped so a zone dropped near an edge stays on the screen. A target
    // outside it can never be touched, and it is what the layout stores.
    Geometry {
        x: (at.0 - width / 2.0).clamp(0.0, 1.0 - width),
        y: (at.1 - height / 2.0).clamp(0.0, 1.0 - height),
        width,
        height,
    }
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
                painter.add(egui::Shape::ellipse_stroke(
                    zone.center(),
                    zone.size() / 2.0,
                    Stroke::new(1.0_f32, Color32::from_gray(90)),
                ));
            }
        }
    }

    // Placing a new control: the next click on the canvas puts it there.
    if editor.placing != Placing::Nothing {
        if let Some(at) = response.interact_pointer_pos() {
            if response.clicked() {
                let g = placed_geometry(
                    editor.placing,
                    (
                        ((at.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
                        ((at.y - rect.top()) / rect.height()).clamp(0.0, 1.0),
                    ),
                    editor.screen,
                );
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
                painter.add(egui::Shape::ellipse_filled(
                    hit.center(),
                    hit.size() / 2.0,
                    accent.linear_multiply(0.22),
                ));
                painter.add(egui::Shape::ellipse_stroke(
                    hit.center(),
                    hit.size() / 2.0,
                    Stroke::new(if selected { 2.0_f32 } else { 1.0_f32 }, accent),
                ));
            }
        }

        let caption = target.label.clone().unwrap_or_else(|| {
            match editor.draft.bindings.iter().find(|b| b.target == target.id) {
                Some(binding) => source_name(binding.source),
                None => target.id.clone(),
            }
        });
        // Some captions are long — "Left shoulder", "Arrow keys", or whatever
        // somebody types into Name — and a touch marker is thirty pixels
        // across. Laid out as one unwrapped line the caption ran out of its
        // marker and off the edge of the canvas, so it is wrapped to a width
        // the marker can carry and then held inside the screen it labels.
        let wrap = match target.kind {
            TargetKind::Touch => 84.0,
            TargetKind::StickZone => hit.width().max(60.0),
        };
        let galley = painter.layout(
            caption,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
            wrap,
        );
        let size = galley.size();
        // Under a touch marker rather than across it: a circle that small
        // cannot hold two lines, and hiding the app's own button is the one
        // thing this caption must not do. A zone is big enough to hold it.
        let top = match target.kind {
            TargetKind::Touch => hit.bottom(),
            TargetKind::StickZone => hit.center().y - size.y / 2.0,
        };
        let caption_at = Pos2::new(
            (hit.center().x - size.x / 2.0).clamp(
                rect.left() + 2.0,
                (rect.right() - size.x - 2.0).max(rect.left() + 2.0),
            ),
            top.clamp(
                rect.top() + 2.0,
                (rect.bottom() - size.y - 2.0).max(rect.top() + 2.0),
            ),
        );
        // The caption now sits on the app's own screenshot rather than on the
        // marker's tint, and white on whatever is underneath is not always
        // readable.
        painter.rect_filled(
            Rect::from_min_size(caption_at, size).expand2(Vec2::new(3.0, 1.0)),
            3.0,
            Color32::from_black_alpha(150),
        );
        painter.galley(caption_at, galley, Color32::WHITE);

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
    crate::ui::widgets::section(ui, "The app's screen");
    // Three states, and only the middle one is new: nothing running, the app
    // running while somebody gets it to the screen they want, and the wait
    // for the frame after they have asked for it.
    let running = editor.capture.as_ref().is_some_and(|c| c.can_take());
    let taking = editor.capture.as_ref().is_some_and(|c| !c.can_take());
    let mut take = false;
    let mut stop = false;
    if running {
        caption_dim(
            ui,
            "The app is running in its own window. Play it to the screen you \
             want to map, then take the picture.",
        );
        ui.horizontal(|ui| {
            take = ui
                .button("Take the picture")
                .on_hover_text("Uses the next frame the app draws.")
                .clicked();
            stop = ui
                .button("Stop")
                .on_hover_text("Closes the app without taking anything.")
                .clicked();
        });
    } else if taking {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(
                egui::RichText::new("Taking the picture…")
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
                    "Runs the app in its own window until you take a \
                     picture. This does not count as playing it.",
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
    if take {
        if let Some(capture) = &mut editor.capture {
            if let Err(e) = capture.take_now() {
                editor.capture_error = Some(e);
                editor.capture = None;
            }
        }
    }
    if stop {
        // Dropping it is what closes the app: the capture owns the process.
        editor.capture = None;
        editor.capture_error = None;
    }
    if let Some(error) = &editor.capture_error {
        ui.label(
            egui::RichText::new(error)
                .small()
                .color(theme::LIGHT.warning),
        );
    }

    ui.add_space(8.0);
    crate::ui::widgets::section(ui, "Add");
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
    crate::ui::widgets::section(ui, "Selected");

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
            .show_ui(ui, |ui| {
                let mut group = |ui: &mut Ui, name: &str, sources: &[Source]| {
                    caption_dim(ui, name);
                    for &source in sources {
                        if ui
                            .selectable_label(current_source == Some(source), source_name(source))
                            .clicked()
                        {
                            new_source = Some(source);
                        }
                    }
                };
                match kind {
                    TargetKind::Touch => {
                        let buttons: Vec<Source> =
                            BUTTONS.iter().map(|&b| Source::Button(b)).collect();
                        group(ui, "Controller", &buttons);
                        // A key that is already bound stays on the list, so
                        // the combo shows what this control does rather than
                        // showing nothing chosen for a binding that exists.
                        // Any other key is bound by pressing it.
                        if let Some(source @ Source::Key(_)) = current_source {
                            group(ui, "Keyboard", &[source]);
                        }
                    }
                    TargetKind::StickZone => {
                        group(ui, "Controller", &[Source::LeftStick, Source::Dpad]);
                        group(
                            ui,
                            "Keyboard",
                            &[Source::KeyDpad(ARROW_KEYS), Source::KeyDpad(WASD)],
                        );
                    }
                }
            });
        if kind == TargetKind::Touch {
            let label = if editor.listening {
                "Press a key…"
            } else {
                "Press a key"
            };
            if ui
                .add(egui::Button::selectable(editor.listening, label))
                .on_hover_text("Binds this control to a key on your keyboard.")
                .clicked()
            {
                editor.listening = !editor.listening;
                editor.listening_started = editor.listening;
            }
        }
    });

    if editor.listening {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            editor.listening = false;
        } else if !editor.listening_started {
            if let Some(key) = pressed_key(ui.ctx()) {
                editor.listening = false;
                new_source = Some(Source::Key(key));
            }
        }
        editor.listening_started = false;
    }
    if editor.listening {
        ui.label(
            egui::RichText::new("Listening. Press a key, or Escape to give up.")
                .small()
                .color(theme::LIGHT.accent),
        );
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The picker shows these names, so two keys sharing one would put two
    /// identical entries on the list with no way to tell them apart.
    #[test]
    fn every_key_has_a_name_of_its_own() {
        let mut seen = std::collections::HashSet::new();
        for &key in Key::ALL {
            let name = key_name(key);
            assert!(!name.is_empty(), "{key:?} has no name");
            assert!(seen.insert(name.clone()), "two keys called {name}");
        }
    }

    /// The number row is `Num0`..`Num9` because those have to be enum
    /// variants. Showing that spelling to somebody would be nonsense.
    #[test]
    fn the_number_row_is_shown_as_digits() {
        assert_eq!(key_name(Key::Num1), "1");
        assert_eq!(key_name(Key::Num0), "0");
    }

    /// The two familiar arrangements are named rather than spelled out, and
    /// anything else falls back to listing its four keys.
    #[test]
    fn a_key_dpad_is_named_when_it_is_one_somebody_recognises() {
        assert_eq!(source_name(Source::KeyDpad(ARROW_KEYS)), "Arrow keys");
        assert_eq!(source_name(Source::KeyDpad(WASD)), "W A S D");
        let odd = KeyDpad {
            up: Key::I,
            down: Key::K,
            left: Key::J,
            right: Key::L,
        };
        assert_eq!(source_name(Source::KeyDpad(odd)), "I J K L");
    }

    /// Every key the editor can bind has to be one the person can actually
    /// press, or the button would be offering something unreachable.
    #[test]
    fn the_keys_egui_reports_are_keys_the_emulator_knows() {
        for (from, expect) in [
            (egui::Key::Space, Key::Space),
            (egui::Key::Enter, Key::Return),
            (egui::Key::ArrowLeft, Key::Left),
            (egui::Key::OpenBracket, Key::LeftBracket),
            (egui::Key::Backtick, Key::Backquote),
            (egui::Key::Num7, Key::Num7),
            (egui::Key::W, Key::W),
        ] {
            assert_eq!(from_egui_key(from), Some(expect), "{from:?}");
        }
        // Escape gives up listening rather than binding, and a function key
        // is not something the emulator can map.
        assert_eq!(from_egui_key(egui::Key::Escape), None);
        assert_eq!(from_egui_key(egui::Key::F12), None);
    }

    /// Only a press starts a touch. The rest of the editor assumes a source
    /// fits its target, so the two lists it offers have to agree with what
    /// the emulator will actually do with them.
    #[test]
    fn the_offered_sources_fit_the_target_they_are_offered_for() {
        for &button in BUTTONS {
            assert_eq!(Source::Button(button).drives(), TargetKind::Touch);
        }
        for source in [
            Source::LeftStick,
            Source::Dpad,
            Source::KeyDpad(ARROW_KEYS),
            Source::KeyDpad(WASD),
        ] {
            assert_eq!(source.drives(), TargetKind::StickZone);
        }
        assert_eq!(Source::Key(Key::Space).drives(), TargetKind::Touch);
    }

    /// A stick zone stands for a thumb sweeping a circle, so a new one has to
    /// be round on whatever shape of screen the app has. Equal fractions of
    /// the two axes are not round: on a 320 by 480 screen they are an 80 by
    /// 120 oval, and on an iPad in landscape a different oval again.
    #[test]
    fn a_new_stick_zone_is_a_circle_on_any_screen() {
        for screen in [(320.0, 480.0), (480.0, 320.0), (768.0, 1024.0)] {
            let g = placed_geometry(Placing::StickZone, (0.5, 0.5), screen);
            let (w, h) = (g.width * screen.0, g.height * screen.1);
            assert!((w - h).abs() < 0.01, "{screen:?}: {w} by {h} pixels");
            // Centred on the click rather than hanging below and right of it,
            // which is what a corner-anchored rectangle did.
            assert!((g.x + g.width / 2.0 - 0.5).abs() < 0.001, "{g:?}");
            assert!((g.y + g.height / 2.0 - 0.5).abs() < 0.001, "{g:?}");
        }
    }

    /// Centring a zone on the click puts half of it past the edge when the
    /// click is at one. A target off the screen can never be touched.
    #[test]
    fn a_stick_zone_placed_at_an_edge_stays_on_the_screen() {
        for at in [(0.0, 0.0), (1.0, 1.0), (0.0, 1.0), (1.0, 0.0)] {
            let g = placed_geometry(Placing::StickZone, at, (320.0, 480.0));
            assert!(g.is_on_screen(), "{at:?}: {g:?}");
        }
    }

    /// A touch is a point, so it goes exactly where it was put and carries no
    /// region at all.
    #[test]
    fn a_new_touch_is_the_point_that_was_clicked() {
        let g = placed_geometry(Placing::Touch, (0.25, 0.75), (320.0, 480.0));
        assert_eq!((g.x, g.y, g.width, g.height), (0.25, 0.75, 0.0, 0.0));
    }
}
