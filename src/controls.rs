/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Control mappings as data: what a physical input does on the guest screen.
//!
//! The emulator has always been able to map a controller onto the touchscreen
//! — `--button-to-touch`, `--dpad-to-touch`, `--stick-to-touch` — but only as
//! command-line parameters carrying absolute guest pixels. That is enough for
//! a person who knows the app is 480 by 320, and useless to a graphical
//! editor, which needs to say "here, on this picture" without knowing what
//! device the app will end up on.
//!
//! So a mapping is stored as two halves that can be edited separately:
//!
//! ```text
//! Target                        Binding
//!   what happens on the guest     what the person presses
//!   screen, and where             to make it happen
//! ```
//!
//! Splitting them is what keeps the vocabulary small. A D-pad and an analog
//! stick are not two kinds of guest behaviour, they are two [Source]s for one
//! [TargetKind::StickZone]. Adding a keyboard binding later adds a source, not
//! a new kind of target, and a target can carry more than one binding so the
//! same on-screen control can be reachable several ways.
//!
//! # Geometry is normalised
//!
//! Every coordinate here is a fraction of the guest screen, `0.0` to `1.0`,
//! rather than a pixel. A layout drawn for an iPhone-shaped app therefore
//! still means something on an iPad-shaped one, and a layout does not silently
//! move when an app is run in a different orientation. The absolute pixels the
//! emulator's input code wants are worked out at the last moment, by
//! [ControlLayout::apply_to], once the device family and orientation are
//! actually settled.
//!
//! Absolute coordinates were not a mistake at the time — they are exactly what
//! a command line should take — but they cannot be the stored form.

use crate::options::{Button, Options};
use crate::window::{DeviceFamily, DeviceOrientation};
use serde::{Deserialize, Serialize};

/// What a mapping does on the guest screen.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum TargetKind {
    /// One touch, held for as long as the physical input is held.
    Touch,
    /// A touch that moves inside a region, driven by a two-axis input.
    StickZone,
}

/// Where on the guest screen a target sits, as a fraction of the screen.
///
/// A [TargetKind::Touch] uses only `x` and `y`. A [TargetKind::StickZone] uses
/// all four, and the touch moves within that rectangle.
#[derive(Copy, Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Geometry {
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub width: f32,
    #[serde(default)]
    pub height: f32,
}

impl Geometry {
    /// Whether every value is inside the screen.
    ///
    /// A layout that fails this is not rejected — an editor should be able to
    /// hold a half-finished mapping — but nothing should be *stored* outside
    /// the screen, because a touch there can never be delivered.
    pub fn is_on_screen(&self) -> bool {
        let inside = |v: f32| (0.0..=1.0).contains(&v);
        inside(self.x)
            && inside(self.y)
            && inside(self.width)
            && inside(self.height)
            && inside(self.x + self.width)
            && inside(self.y + self.height)
    }

    fn to_pixels(self, (w, h): (f32, f32)) -> (f32, f32, f32, f32) {
        (self.x * w, self.y * h, self.width * w, self.height * h)
    }

    fn from_pixels((x, y, width, height): (f32, f32, f32, f32), (w, h): (f32, f32)) -> Geometry {
        Geometry {
            x: x / w,
            y: y / h,
            width: width / w,
            height: height / h,
        }
    }
}

/// Something on the guest screen that a physical input can drive.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Target {
    /// Referred to by [Binding::target]. Unique within a layout.
    pub id: String,
    pub kind: TargetKind,
    pub geometry: Geometry,
    /// What a person calls it — "Jump", "Move". Optional because tapHLE
    /// cannot know it: an app binary does not say that a point means "jump",
    /// so only whoever made the layout can.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// A physical input.
///
/// Deliberately logical rather than an SDL button number, so a layout means
/// the same thing on a controller that enumerates its buttons differently, and
/// so it can be shown with the glyph that controller actually uses.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Source {
    /// A single button.
    Button(Button),
    /// The D-pad as a whole, as a two-axis input.
    Dpad,
    /// The left analog stick.
    LeftStick,
}

/// What drives a target.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Binding {
    pub source: Source,
    /// The [Target::id] this drives.
    pub target: String,
}

/// A whole set of control mappings for one app.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ControlLayout {
    pub targets: Vec<Target>,
    pub bindings: Vec<Binding>,
}

/// The guest screen a layout's fractions are measured against.
///
/// This is the unrotated, unscaled size the touch coordinates use, which is
/// what `--button-to-touch` has always been documented in: 320 by 480 for an
/// iPhone-shaped app in portrait, 480 by 320 in landscape.
pub fn guest_screen(
    family: DeviceFamily,
    orientation: DeviceOrientation,
    landscape_native: bool,
) -> (f32, f32) {
    let (width, height) = family.portrait_size();
    let (width, height) = if landscape_native {
        (height, width)
    } else {
        (width, height)
    };
    match orientation {
        DeviceOrientation::Portrait | DeviceOrientation::PortraitUpsideDown => {
            (width as f32, height as f32)
        }
        DeviceOrientation::LandscapeLeft | DeviceOrientation::LandscapeRight => {
            (height as f32, width as f32)
        }
    }
}

impl ControlLayout {
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty() && self.bindings.is_empty()
    }

    fn target(&self, id: &str) -> Option<&Target> {
        self.targets.iter().find(|t| t.id == id)
    }

    /// Write this layout into the options the input code reads, converting
    /// fractions into the guest pixels it expects.
    ///
    /// Called once the device family and orientation are settled, because
    /// those are what decide the screen a fraction refers to. A binding naming
    /// a target that does not exist is skipped rather than being an error: a
    /// layout is user data, and one dangling reference should not cost
    /// somebody every other mapping they made.
    pub fn apply_to(&self, options: &mut Options, screen: (f32, f32)) {
        for binding in &self.bindings {
            let Some(target) = self.target(&binding.target) else {
                continue;
            };
            let (x, y, width, height) = target.geometry.to_pixels(screen);
            match (binding.source, target.kind) {
                (Source::Button(button), TargetKind::Touch) => {
                    options.button_to_touch.insert(button, (x, y));
                }
                (Source::Dpad, TargetKind::StickZone) => {
                    options.dpad_to_touch = Some((x, y, width, height));
                }
                (Source::LeftStick, TargetKind::StickZone) => {
                    options.stick_to_touch = Some((x, y, width, height));
                }
                // A button pointed at a region, or a stick pointed at a
                // point, is a layout that says something the input code has
                // no way to do. Skipped rather than approximated, because
                // guessing which half was meant would be worse than the
                // mapping visibly not working.
                _ => (),
            }
        }
    }

    /// Read the equivalent layout back out of already-resolved options.
    ///
    /// This is how mappings that arrived as command-line options or from
    /// `tapHLE_default_options.txt` become visible to an editor: they are the
    /// same mappings, and somebody looking at the guest screen should see them
    /// whichever way they were set.
    pub fn from_options(options: &Options, screen: (f32, f32)) -> ControlLayout {
        let mut layout = ControlLayout::default();

        // Sorted so the result does not depend on the hash map's order, which
        // would make the layout shuffle every run and any test of it flaky.
        let mut buttons: Vec<_> = options.button_to_touch.iter().collect();
        buttons.sort_by_key(|(button, _)| format!("{button:?}"));
        for (button, &(x, y)) in buttons {
            let id = format!("{button:?}");
            layout.targets.push(Target {
                id: id.clone(),
                kind: TargetKind::Touch,
                geometry: Geometry::from_pixels((x, y, 0.0, 0.0), screen),
                label: None,
            });
            layout.bindings.push(Binding {
                source: Source::Button(*button),
                target: id,
            });
        }

        for (id, source, region) in [
            ("dpad", Source::Dpad, options.dpad_to_touch),
            ("left-stick", Source::LeftStick, options.stick_to_touch),
        ] {
            if let Some(region) = region {
                layout.targets.push(Target {
                    id: id.to_string(),
                    kind: TargetKind::StickZone,
                    geometry: Geometry::from_pixels(region, screen),
                    label: None,
                });
                layout.bindings.push(Binding {
                    source,
                    target: id.to_string(),
                });
            }
        }

        layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANDSCAPE_IPHONE: (f32, f32) = (480.0, 320.0);

    fn touch(id: &str, button: Button, x: f32, y: f32) -> (Target, Binding) {
        (
            Target {
                id: id.to_string(),
                kind: TargetKind::Touch,
                geometry: Geometry {
                    x,
                    y,
                    width: 0.0,
                    height: 0.0,
                },
                label: None,
            },
            Binding {
                source: Source::Button(button),
                target: id.to_string(),
            },
        )
    }

    /// The whole reason geometry is stored as fractions: the mappings that
    /// already exist have to come out of the conversion as the same pixels
    /// they went in as. Every shipped default is an absolute coordinate, so a
    /// conversion that drifted would move all of them at once.
    #[test]
    fn absolute_coordinates_survive_a_round_trip() {
        let mut before = Options::default();
        before.button_to_touch.insert(Button::A, (470.0, 310.0));
        before.button_to_touch.insert(Button::Start, (240.0, 10.0));
        before.stick_to_touch = Some((25.0, 200.0, 95.0, 100.0));
        before.dpad_to_touch = Some((10.0, 10.0, 50.0, 50.0));

        let layout = ControlLayout::from_options(&before, LANDSCAPE_IPHONE);
        let mut after = Options::default();
        layout.apply_to(&mut after, LANDSCAPE_IPHONE);

        assert_eq!(after.button_to_touch, before.button_to_touch);
        assert_eq!(after.stick_to_touch, before.stick_to_touch);
        assert_eq!(after.dpad_to_touch, before.dpad_to_touch);
    }

    /// A fraction has to mean the same place on a bigger screen. This is what
    /// absolute coordinates could not do, and the reason for the change.
    #[test]
    fn a_layout_scales_to_a_different_guest_screen() {
        let (target, binding) = touch("jump", Button::A, 0.5, 0.25);
        let layout = ControlLayout {
            targets: vec![target],
            bindings: vec![binding],
        };

        let mut iphone = Options::default();
        layout.apply_to(&mut iphone, (480.0, 320.0));
        assert_eq!(iphone.button_to_touch[&Button::A], (240.0, 80.0));

        let mut ipad = Options::default();
        layout.apply_to(&mut ipad, (1024.0, 768.0));
        assert_eq!(ipad.button_to_touch[&Button::A], (512.0, 192.0));
    }

    /// The screen a fraction is measured against depends on the orientation
    /// and on landscape-native, and getting that wrong would misplace every
    /// mapping in an app rather than failing visibly.
    #[test]
    fn the_guest_screen_follows_orientation_and_landscape_native() {
        use DeviceOrientation::*;
        let iphone = DeviceFamily::iPhone;
        assert_eq!(guest_screen(iphone, Portrait, false), (320.0, 480.0));
        assert_eq!(
            guest_screen(iphone, PortraitUpsideDown, false),
            (320.0, 480.0)
        );
        assert_eq!(guest_screen(iphone, LandscapeLeft, false), (480.0, 320.0));
        assert_eq!(guest_screen(iphone, LandscapeRight, false), (480.0, 320.0));
        // Landscape-native makes the device's natural shape landscape, and
        // forces the orientation to portrait, so the screen is landscape
        // without any rotation being applied.
        assert_eq!(guest_screen(iphone, Portrait, true), (480.0, 320.0));
        assert_eq!(
            guest_screen(DeviceFamily::iPad, Portrait, false),
            (768.0, 1024.0)
        );
        assert_eq!(
            guest_screen(DeviceFamily::iPad, LandscapeLeft, false),
            (1024.0, 768.0)
        );
    }

    #[test]
    fn a_binding_naming_a_missing_target_is_skipped_not_fatal() {
        let (target, binding) = touch("jump", Button::A, 0.5, 0.5);
        let layout = ControlLayout {
            targets: vec![target],
            bindings: vec![
                binding,
                Binding {
                    source: Source::Button(Button::B),
                    target: "deleted".to_string(),
                },
            ],
        };
        let mut options = Options::default();
        layout.apply_to(&mut options, LANDSCAPE_IPHONE);
        assert!(options.button_to_touch.contains_key(&Button::A));
        assert!(!options.button_to_touch.contains_key(&Button::B));
    }

    /// A button cannot drive a region and a stick cannot drive a point. The
    /// input code has no behaviour for either, so the mapping is skipped
    /// rather than approximated into something the person did not ask for.
    #[test]
    fn a_source_and_target_that_do_not_fit_are_skipped() {
        let layout = ControlLayout {
            targets: vec![Target {
                id: "zone".to_string(),
                kind: TargetKind::StickZone,
                geometry: Geometry {
                    x: 0.1,
                    y: 0.1,
                    width: 0.2,
                    height: 0.2,
                },
                label: None,
            }],
            bindings: vec![Binding {
                source: Source::Button(Button::A),
                target: "zone".to_string(),
            }],
        };
        let mut options = Options::default();
        layout.apply_to(&mut options, LANDSCAPE_IPHONE);
        assert!(options.button_to_touch.is_empty());
        assert_eq!(options.stick_to_touch, None);
    }

    #[test]
    fn geometry_knows_when_it_leaves_the_screen() {
        let on = Geometry {
            x: 0.1,
            y: 0.1,
            width: 0.5,
            height: 0.5,
        };
        assert!(on.is_on_screen());
        let off = Geometry {
            x: 0.8,
            y: 0.1,
            width: 0.5,
            height: 0.1,
        };
        assert!(!off.is_on_screen(), "runs off the right-hand edge");
        let negative = Geometry {
            x: -0.1,
            y: 0.1,
            width: 0.1,
            height: 0.1,
        };
        assert!(!negative.is_on_screen());
    }

    /// Every control mapping tapHLE ships, converted to fractions and back,
    /// has to land on exactly the pixel it started on.
    ///
    /// `tapHLE_default_options.txt` carries 179 button mappings and a
    /// scattering of stick and D-pad regions across 68 apps, all of them
    /// absolute coordinates that somebody arrived at by looking at the screen.
    /// Normalising them is the one change that could move all of them at once,
    /// and moving them would not fail loudly — the app would run, the button
    /// would land somewhere slightly wrong, and it would look like a
    /// compatibility regression rather than a units bug.
    ///
    /// The file is read at compile time so this cannot pass by finding
    /// nothing: the assertion at the end fails if the mappings disappear.
    #[test]
    fn every_shipped_mapping_survives_normalisation() {
        let shipped = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tapHLE_default_options.txt"
        ));

        let mut apps_checked = 0;
        let mut mappings_checked = 0;

        for line in shipped.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((app_id, args)) = line.split_once(':') else {
                continue;
            };

            let mut before = Options::default();
            for arg in args.split_ascii_whitespace() {
                // The file is allowed to contain options this test does not
                // care about; only a malformed one should fail here.
                assert!(
                    before.parse_argument(arg).is_ok(),
                    "{app_id}: {arg} did not parse"
                );
            }

            let mappings = before.button_to_touch.len()
                + before.dpad_to_touch.is_some() as usize
                + before.stick_to_touch.is_some() as usize;
            if mappings == 0 {
                continue;
            }

            // The orientation the app's own defaults asked for is what decides
            // the screen its coordinates were written against.
            for family in [DeviceFamily::iPhone, DeviceFamily::iPad] {
                let screen =
                    guest_screen(family, before.initial_orientation, before.landscape_native);
                let layout = ControlLayout::from_options(&before, screen);
                let mut after = Options::default();
                layout.apply_to(&mut after, screen);

                assert_eq!(
                    after.button_to_touch, before.button_to_touch,
                    "{app_id}: button mappings moved on {family:?}"
                );
                assert_eq!(
                    after.dpad_to_touch, before.dpad_to_touch,
                    "{app_id}: D-pad region moved on {family:?}"
                );
                assert_eq!(
                    after.stick_to_touch, before.stick_to_touch,
                    "{app_id}: stick region moved on {family:?}"
                );
            }

            apps_checked += 1;
            mappings_checked += mappings;
        }

        // Measured, not guessed: 33 of the 68 shipped lines carry control
        // mappings, and they hold 194 between them. The floor is a little
        // under both so that ordinary tuning does not trip it, while deleting
        // the mappings wholesale still would.
        assert!(
            apps_checked >= 30,
            "only {apps_checked} apps had mappings to check"
        );
        assert!(
            mappings_checked >= 180,
            "only {mappings_checked} mappings were checked"
        );
    }

    #[test]
    fn a_layout_survives_a_json_round_trip() {
        let (target, binding) = touch("jump", Button::A, 0.88, 0.79);
        let layout = ControlLayout {
            targets: vec![Target {
                label: Some("Jump".to_string()),
                ..target
            }],
            bindings: vec![binding],
        };
        let json = serde_json::to_string(&layout).unwrap();
        let back: ControlLayout = serde_json::from_str(&json).unwrap();
        assert_eq!(layout, back);
    }
}
