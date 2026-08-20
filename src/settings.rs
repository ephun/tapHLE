/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Settings as data: what somebody chose, separately from what the emulator
//! resolved it to.
//!
//! [Options] is the settled configuration one run uses. This module is the
//! sparse thing a person edits, where every field is an [Option] and `None`
//! means "not decided at this level". That distinction is what lets settings
//! layer: a level that decides nothing lets the next one down decide, so an
//! unset setting emits no argument at all rather than emitting the emulator's
//! own default, which would silently countermand the per-app entry that makes
//! some app work.
//!
//! It lives in the emulator rather than the frontend because both programs
//! have to agree about it. The frontend writes these settings and the emulator
//! reads them, and a second implementation of the same type in the other
//! program is how the two drift apart.
//!
//! Nothing here invents a setting. Each field maps to an option the emulator
//! actually parses, and [EmulatorSettings::validate] proves it by feeding the
//! generated arguments back through [Options::parse_argument].
//!
//! [Options]: crate::options::Options
//! [Options::parse_argument]: crate::options::Options::parse_argument

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The orientation the virtual device starts in.
///
/// Landscape-native is one of the choices rather than a separate switch
/// because the emulator documents it as mutually exclusive with the other
/// orientations; making it a checkbox would let someone ask for a
/// combination that cannot happen.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum OrientationPref {
    Portrait,
    UpsideDown,
    LandscapeLeft,
    LandscapeRight,
    LandscapeNative,
}

impl OrientationPref {
    pub const ALL: &'static [OrientationPref] = &[
        OrientationPref::Portrait,
        OrientationPref::UpsideDown,
        OrientationPref::LandscapeLeft,
        OrientationPref::LandscapeRight,
        OrientationPref::LandscapeNative,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OrientationPref::Portrait => "Portrait",
            OrientationPref::UpsideDown => "Portrait, upside down",
            OrientationPref::LandscapeLeft => "Landscape, rotated left",
            OrientationPref::LandscapeRight => "Landscape, rotated right",
            OrientationPref::LandscapeNative => "Landscape (native)",
        }
    }

    fn args(self) -> Vec<String> {
        let mut args = Vec::new();
        match self {
            OrientationPref::Portrait => args.push("--portrait".to_string()),
            OrientationPref::UpsideDown => args.push("--upside-down".to_string()),
            OrientationPref::LandscapeLeft => args.push("--landscape-left".to_string()),
            OrientationPref::LandscapeRight => args.push("--landscape-right".to_string()),
            OrientationPref::LandscapeNative => {
                args.push("--landscape-native".to_string());
                return args;
            }
        }
        // The other four are only meaningful with landscape-native off, and
        // a lower layer may have turned it on.
        args.push("--no-landscape-native".to_string());
        args
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum DeviceFamilyPref {
    IPhone,
    IPad,
}

impl DeviceFamilyPref {
    pub const ALL: &'static [DeviceFamilyPref] =
        &[DeviceFamilyPref::IPhone, DeviceFamilyPref::IPad];

    pub fn label(self) -> &'static str {
        match self {
            DeviceFamilyPref::IPhone => "iPhone / iPod touch",
            DeviceFamilyPref::IPad => "iPad",
        }
    }

    fn value(self) -> &'static str {
        match self {
            DeviceFamilyPref::IPhone => "iphone",
            DeviceFamilyPref::IPad => "ipad",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Gles1Pref {
    Native,
    OnGl2,
}

impl Gles1Pref {
    pub const ALL: &'static [Gles1Pref] = &[Gles1Pref::Native, Gles1Pref::OnGl2];

    pub fn label(self) -> &'static str {
        match self {
            Gles1Pref::Native => "Native OpenGL ES 1.1",
            Gles1Pref::OnGl2 => "Translate to OpenGL 2.1",
        }
    }

    fn value(self) -> &'static str {
        match self {
            Gles1Pref::Native => "gles1_native",
            Gles1Pref::OnGl2 => "gles1_on_gl2",
        }
    }
}

/// The frame rate ceiling. `Off` is a real emulator setting, not an absence
/// of one, so it is a variant rather than [None].
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum FrameRateLimit {
    Off,
    Fps(f64),
}

impl FrameRateLimit {
    fn value(self) -> String {
        match self {
            FrameRateLimit::Off => "off".to_string(),
            FrameRateLimit::Fps(fps) => format!("{fps}"),
        }
    }
}

/// Everything somebody has chosen, at both scopes, as it is stored on disk.
///
/// One file, two scopes. `global` applies to every app; `apps` holds
/// overrides keyed by the app's bundle identifier, the same key
/// `tapHLE_default_options.txt` uses. Both the emulator and the frontend read
/// and write this, which is the point: a setting means the same thing to both
/// programs because there is only one of it.
#[derive(Clone, Default, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsFile {
    /// Applies to every app.
    pub global: EmulatorSettings,
    /// Overrides for one app, by bundle identifier.
    pub apps: BTreeMap<String, EmulatorSettings>,
}

impl SettingsFile {
    /// The key for settings that apply to every version of an app.
    pub fn app_key(bundle_identifier: &str) -> String {
        bundle_identifier.to_string()
    }

    /// The key for settings that apply to one exact version of an app.
    ///
    /// Two versions of one app are separate records in the compatibility
    /// database and can need different settings, so the frontend keys its
    /// library this way and the settings file has to be able to express it.
    pub fn app_version_key(bundle_identifier: &str, bundle_version: &str) -> String {
        format!("{bundle_identifier}@{bundle_version}")
    }

    /// The user's settings for one app: their overrides layered over their
    /// global defaults, most specific last.
    ///
    /// An entry keyed by bundle identifier applies to every version of the
    /// app; one keyed `identifier@version` applies to that version only and
    /// is layered over the first, so a setting made for the app as a whole is
    /// still inherited by a version that overrides something else.
    ///
    /// This is only the user's half. The shipped per-app defaults in
    /// `tapHLE_default_options.txt` and the command line are applied around it
    /// by the emulator; see [USER_SETTINGS_WIN_OVER_SHIPPED_DEFAULTS] for the
    /// order and why it is that way.
    pub fn resolve(&self, bundle_identifier: &str, bundle_version: &str) -> EmulatorSettings {
        let mut resolved = self.global.clone();
        for key in [
            Self::app_key(bundle_identifier),
            Self::app_version_key(bundle_identifier, bundle_version),
        ] {
            if let Some(over) = self.apps.get(&key) {
                resolved = EmulatorSettings::inherit(&resolved, over);
            }
        }
        resolved
    }

    pub fn from_json(text: &str) -> Result<SettingsFile, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    pub fn to_json(&self) -> String {
        // Indented because somebody is expected to be able to open and read
        // it, the same as the frontend's own files.
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Whether the user's own settings are applied after the per-app defaults
/// tapHLE ships, and so win over them.
///
/// This is the one ordering choice in the whole chain that is a judgement
/// call rather than a fact, so it is stated once, here, instead of being
/// implied by the order of some statements.
///
/// `true` keeps the behaviour tapHLE has always had: whatever the user set
/// beats the shipped default for an app.
///
/// `false` would put the shipped per-app defaults last, so that a fix tapHLE
/// ships for one app survives a blanket preference the user set for
/// everything. That matters because 57 shipped defaults lock an orientation,
/// and a single global orientation preference silently breaks all of them.
/// The user's per-app override still wins either way.
///
/// Changing this changes behaviour for existing installs, so it is a
/// maintainer decision and not something to flip while passing.
pub const USER_SETTINGS_WIN_OVER_SHIPPED_DEFAULTS: bool = true;

/// Settings that become emulator options. Used both as the global defaults
/// and as a per-app override.
#[derive(Clone, Default, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EmulatorSettings {
    pub fullscreen: Option<bool>,
    pub device_family: Option<DeviceFamilyPref>,
    pub orientation: Option<OrientationPref>,
    pub scale_hack: Option<u32>,
    pub gles1: Option<Gles1Pref>,
    pub force_composition: Option<bool>,
    pub ignore_gl_errors: Option<bool>,
    pub frame_rate_limit: Option<FrameRateLimit>,
    pub print_fps: Option<bool>,
    pub analog_stick_tilt: Option<bool>,
    pub deadzone: Option<f32>,
    pub x_tilt_range: Option<f32>,
    pub y_tilt_range: Option<f32>,
    pub x_tilt_offset: Option<f32>,
    pub y_tilt_offset: Option<f32>,
    pub network_access: Option<bool>,
    pub direct_memory_access: Option<bool>,
    pub error_popup: Option<bool>,
    pub preferred_languages: Option<String>,
    /// Whether to draw an app's font with this computer's copy of it when the
    /// names match, rather than with a substitute.
    pub use_host_fonts: Option<bool>,
    /// What to draw in place of a particular iPhone font, keyed by the font's
    /// family name. The value is either the id of a family tapHLE ships or the
    /// path of a font file.
    ///
    /// A map rather than an `Option`, because these are a set of independent
    /// choices: an app that picks a font for Futura should still get the
    /// general choice for Helvetica. `inherit` overlays rather than replaces
    /// for that reason.
    #[serde(default)]
    pub font_choices: BTreeMap<String, String>,
    /// Modules to enable verbose tracing for. This is the `TAPHLE_LOG_MODULES`
    /// environment variable rather than an option, because that is how the
    /// emulator reads it.
    pub log_modules: Option<String>,
    /// Anything else, written as it would be typed on the command line. Every
    /// emulator option stays reachable without the frontend having to grow a
    /// control for each one, and it is checked by the emulator's own parser
    /// before a run starts.
    pub extra_arguments: Option<String>,
    /// What the controller does on this app's touchscreen.
    ///
    /// Not an argument like the rest: its coordinates are fractions of the
    /// guest screen, and the screen is not known until the device family and
    /// orientation are settled, so the emulator applies it separately rather
    /// than through [EmulatorSettings::to_args]. A per-app layout replaces an
    /// inherited one wholesale rather than merging target by target, because
    /// half of one layout mixed with half of another is not a layout anybody
    /// designed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controls: Option<crate::controls::ControlLayout>,
}

impl EmulatorSettings {
    /// Whether anything at all is set at this level.
    pub fn is_empty(&self) -> bool {
        *self == EmulatorSettings::default()
    }

    /// The settings that apply when `self` is layered over `base`.
    pub fn inherit(base: &EmulatorSettings, over: &EmulatorSettings) -> EmulatorSettings {
        macro_rules! pick {
            ($($field:ident),+ $(,)?) => {
                EmulatorSettings {
                    $($field: over.$field.clone().or_else(|| base.$field.clone()),)+
                    // Font choices merge instead of one level replacing the
                    // other: picking a font for Futura in one app must not
                    // discard the general choice made for Helvetica.
                    font_choices: {
                        let mut merged = base.font_choices.clone();
                        merged.extend(over.font_choices.clone());
                        merged
                    },
                }
            };
        }
        pick!(
            fullscreen,
            device_family,
            orientation,
            scale_hack,
            gles1,
            force_composition,
            ignore_gl_errors,
            frame_rate_limit,
            print_fps,
            analog_stick_tilt,
            deadzone,
            x_tilt_range,
            y_tilt_range,
            x_tilt_offset,
            y_tilt_offset,
            network_access,
            direct_memory_access,
            error_popup,
            preferred_languages,
            log_modules,
            extra_arguments,
            controls,
            use_host_fonts,
        )
    }

    /// The command-line arguments these settings ask for.
    pub fn to_args(&self) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        fn flag(args: &mut Vec<String>, value: Option<bool>, on: &str, off: &str) {
            match value {
                Some(true) => args.push(on.to_string()),
                Some(false) => args.push(off.to_string()),
                None => (),
            }
        }

        flag(&mut args, self.fullscreen, "--fullscreen", "--windowed");
        if let Some(orientation) = self.orientation {
            args.extend(orientation.args());
        }
        if let Some(family) = self.device_family {
            args.push(format!("--device-family={}", family.value()));
        }
        if let Some(scale) = self.scale_hack {
            args.push(format!("--scale-hack={}", scale.max(1)));
        }
        if let Some(gles1) = self.gles1 {
            args.push(format!("--gles1={}", gles1.value()));
        }
        flag(
            &mut args,
            self.force_composition,
            "--force-composition",
            "--no-force-composition",
        );
        flag(
            &mut args,
            self.ignore_gl_errors,
            "--ignore-gl-errors",
            "--report-gl-errors",
        );
        if let Some(limit) = self.frame_rate_limit {
            args.push(format!("--fps-limit={}", limit.value()));
        }
        flag(&mut args, self.print_fps, "--print-fps", "--no-print-fps");
        flag(
            &mut args,
            self.analog_stick_tilt,
            "--enable-analog-stick-tilt-controls",
            "--disable-analog-stick-tilt-controls",
        );
        for (value, name) in [
            (self.deadzone, "deadzone"),
            (self.x_tilt_range, "x-tilt-range"),
            (self.y_tilt_range, "y-tilt-range"),
            (self.x_tilt_offset, "x-tilt-offset"),
            (self.y_tilt_offset, "y-tilt-offset"),
        ] {
            if let Some(value) = value {
                args.push(format!("--{name}={value}"));
            }
        }
        flag(
            &mut args,
            self.network_access,
            "--allow-network-access",
            "--deny-network-access",
        );
        flag(
            &mut args,
            self.direct_memory_access,
            "--enable-direct-memory-access",
            "--disable-direct-memory-access",
        );
        flag(
            &mut args,
            self.error_popup,
            "--error-popup",
            "--no-error-popup",
        );
        if let Some(languages) = self
            .preferred_languages
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())
        {
            args.push(format!("--preferred-languages={languages}"));
        }
        flag(
            &mut args,
            self.use_host_fonts,
            "--host-fonts",
            "--no-host-fonts",
        );
        for (family, choice) in &self.font_choices {
            args.push(format!("--font={family}={choice}"));
        }
        if let Some(extra) = self.extra_arguments.as_deref() {
            args.extend(extra.split_whitespace().map(str::to_string));
        }
        args
    }

    /// Environment variables these settings ask for.
    pub fn to_env(&self) -> Vec<(String, String)> {
        let mut env = Vec::new();
        if let Some(modules) = self
            .log_modules
            .as_deref()
            .map(str::trim)
            .filter(|m| !m.is_empty())
        {
            env.push((
                crate::log::LOG_MODULES_ENV_VAR.to_string(),
                modules.to_string(),
            ));
        }
        env
    }

    /// Arguments the emulator would reject, checked with the emulator's own
    /// parser so the frontend cannot drift from what it accepts.
    ///
    /// This is what stops a typo in the free-text argument box from producing
    /// a launch that fails with a usage message and no explanation.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let mut options = crate::options::Options::default();
        for arg in self.to_args() {
            match options.parse_argument(&arg) {
                Ok(true) => (),
                Ok(false) => problems.push(format!("{arg} is not a tapHLE option")),
                Err(e) => problems.push(format!("{arg}: {e}")),
            }
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn everything_set() -> EmulatorSettings {
        EmulatorSettings {
            fullscreen: Some(true),
            device_family: Some(DeviceFamilyPref::IPad),
            orientation: Some(OrientationPref::LandscapeLeft),
            scale_hack: Some(2),
            gles1: Some(Gles1Pref::OnGl2),
            force_composition: Some(true),
            ignore_gl_errors: Some(true),
            frame_rate_limit: Some(FrameRateLimit::Fps(120.0)),
            print_fps: Some(true),
            analog_stick_tilt: Some(false),
            deadzone: Some(0.2),
            x_tilt_range: Some(45.0),
            y_tilt_range: Some(45.0),
            x_tilt_offset: Some(5.0),
            y_tilt_offset: Some(-5.0),
            network_access: Some(true),
            direct_memory_access: Some(false),
            error_popup: Some(false),
            preferred_languages: Some("en,fr".to_string()),
            log_modules: Some("tapHLE::mem".to_string()),
            extra_arguments: Some("--headless".to_string()),
            use_host_fonts: Some(false),
            font_choices: BTreeMap::from([("Helvetica".to_string(), "DejaVuSans".to_string())]),
            controls: Some(crate::controls::ControlLayout {
                targets: vec![crate::controls::Target {
                    id: "jump".to_string(),
                    kind: crate::controls::TargetKind::Touch,
                    geometry: crate::controls::Geometry {
                        x: 0.88,
                        y: 0.79,
                        width: 0.0,
                        height: 0.0,
                    },
                    label: Some("Jump".to_string()),
                }],
                bindings: vec![crate::controls::Binding {
                    source: crate::controls::Source::Button(crate::options::Button::A),
                    target: "jump".to_string(),
                }],
            }),
        }
    }

    /// A per-app override layers over the global settings rather than
    /// replacing them, so an app that decides one thing still inherits the
    /// rest. This is the same rule as [EmulatorSettings::inherit]; the test
    /// exists because [SettingsFile::resolve] is what the emulator actually
    /// calls, and a resolve that replaced instead of layering would discard
    /// every global setting the moment an app had any override at all.
    #[test]
    fn an_app_override_layers_over_the_global_settings() {
        let mut file = SettingsFile::default();
        file.global.fullscreen = Some(true);
        file.global.deadzone = Some(0.2);
        file.apps.insert(
            "com.example.game".to_string(),
            EmulatorSettings {
                deadzone: Some(0.5),
                ..EmulatorSettings::default()
            },
        );

        let resolved = file.resolve("com.example.game", "1.0");
        assert_eq!(
            resolved.deadzone,
            Some(0.5),
            "the app's own value should win"
        );
        assert_eq!(
            resolved.fullscreen,
            Some(true),
            "the rest should be inherited"
        );

        let other = file.resolve("com.example.other", "1.0");
        assert_eq!(
            other.deadzone,
            Some(0.2),
            "an app with no entry gets the global"
        );
    }

    /// A setting made for one version has to layer over the setting made for
    /// the app as a whole, not replace it. Keying the library by
    /// identifier-and-version is deliberate — two versions of an app can need
    /// different settings — so both keys have to be able to coexist.
    #[test]
    fn a_version_specific_entry_layers_over_the_whole_app_entry() {
        let mut file = SettingsFile::default();
        file.global.print_fps = Some(true);
        file.apps.insert(
            "com.example.game".to_string(),
            EmulatorSettings {
                fullscreen: Some(true),
                deadzone: Some(0.3),
                ..EmulatorSettings::default()
            },
        );
        file.apps.insert(
            "com.example.game@2.0".to_string(),
            EmulatorSettings {
                deadzone: Some(0.9),
                ..EmulatorSettings::default()
            },
        );

        let v2 = file.resolve("com.example.game", "2.0");
        assert_eq!(v2.deadzone, Some(0.9), "the version's own value wins");
        assert_eq!(v2.fullscreen, Some(true), "the app-wide value is inherited");
        assert_eq!(v2.print_fps, Some(true), "so is the global one");

        let v1 = file.resolve("com.example.game", "1.0");
        assert_eq!(v1.deadzone, Some(0.3), "another version is unaffected");
    }

    #[test]
    fn an_app_with_no_entry_gets_the_global_settings() {
        let mut file = SettingsFile::default();
        file.global.print_fps = Some(true);
        assert_eq!(file.resolve("anything", "1.0").print_fps, Some(true));
    }

    /// The frontend writes this file and the emulator reads it, so a value
    /// that does not survive the trip is a setting that silently stops
    /// applying the moment somebody restarts.
    #[test]
    fn settings_survive_a_json_round_trip() {
        let mut file = SettingsFile {
            global: everything_set(),
            apps: Default::default(),
        };
        file.apps
            .insert("com.example.game".to_string(), everything_set());

        let json = file.to_json();
        let back = SettingsFile::from_json(&json).expect("should parse what it wrote");
        assert_eq!(file, back);
    }

    /// An empty file, a file with only one scope, and a file with unknown
    /// keys all have to load. The first two are ordinary states; the third is
    /// what happens when somebody opens a newer tapHLE's settings in an older
    /// one, and refusing to load would lose every other setting in the file.
    #[test]
    fn a_partial_or_unfamiliar_settings_file_still_loads() {
        for text in [
            "{}",
            r#"{"global": {}}"#,
            r#"{"apps": {}}"#,
            r#"{"global": {"fullscreen": true}, "unknown_section": 1}"#,
            r#"{"global": {"fullscreen": true, "not_a_setting": "x"}}"#,
        ] {
            assert!(
                SettingsFile::from_json(text).is_ok(),
                "should have loaded: {text}"
            );
        }
        let parsed = SettingsFile::from_json(r#"{"global": {"fullscreen": true}}"#).unwrap();
        assert_eq!(parsed.global.fullscreen, Some(true));
    }

    #[test]
    fn a_malformed_settings_file_is_an_error_rather_than_a_panic() {
        assert!(SettingsFile::from_json("not json at all").is_err());
        assert!(SettingsFile::from_json("").is_err());
    }

    /// The whole point of this module is that it emits real emulator
    /// options. The emulator's own parser is the only authority on that, so
    /// it is the thing consulted.
    #[test]
    fn every_generated_argument_is_a_real_emulator_option() {
        assert!(everything_set().validate().is_empty());
    }

    /// Both settings of every switch have to be expressible, or a per-app
    /// override could turn something on and never turn it off again.
    #[test]
    fn switches_emit_something_in_both_directions() {
        for value in [Some(true), Some(false)] {
            let settings = EmulatorSettings {
                fullscreen: value,
                force_composition: value,
                network_access: value,
                ..Default::default()
            };
            assert_eq!(settings.to_args().len(), 3);
            assert!(settings.validate().is_empty());
        }
    }

    /// An unset setting must emit nothing, so that the options files keep
    /// deciding. Emitting the emulator's default instead would quietly
    /// override the per-app entries those files exist for.
    #[test]
    fn nothing_set_means_no_arguments() {
        let settings = EmulatorSettings::default();
        assert!(settings.to_args().is_empty());
        assert!(settings.to_env().is_empty());
        assert!(settings.is_empty());
    }

    #[test]
    fn an_override_wins_and_the_rest_is_inherited() {
        let global = EmulatorSettings {
            fullscreen: Some(true),
            scale_hack: Some(2),
            ..Default::default()
        };
        let over = EmulatorSettings {
            fullscreen: Some(false),
            ..Default::default()
        };
        let merged = EmulatorSettings::inherit(&global, &over);
        assert_eq!(merged.fullscreen, Some(false));
        assert_eq!(merged.scale_hack, Some(2));
    }

    /// Landscape-native is exclusive with the other orientations, so asking
    /// for an ordinary orientation has to switch it off as well — a lower
    /// layer may have turned it on for this app.
    #[test]
    fn an_ordinary_orientation_switches_landscape_native_off() {
        let settings = EmulatorSettings {
            orientation: Some(OrientationPref::LandscapeRight),
            ..Default::default()
        };
        assert_eq!(
            settings.to_args(),
            ["--landscape-right", "--no-landscape-native"]
        );
        let native = EmulatorSettings {
            orientation: Some(OrientationPref::LandscapeNative),
            ..Default::default()
        };
        assert_eq!(native.to_args(), ["--landscape-native"]);
    }

    /// The free-text box is checked against the real parser, so a typo is
    /// reported in the dialog rather than becoming a failed launch.
    #[test]
    fn a_bad_extra_argument_is_reported() {
        let settings = EmulatorSettings {
            extra_arguments: Some("--not-an-option".to_string()),
            ..Default::default()
        };
        let problems = settings.validate();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("--not-an-option"));
    }

    /// Verbose tracing is an environment variable, not an option; passing it
    /// as an argument would fail the launch.
    #[test]
    fn log_modules_becomes_an_environment_variable() {
        let settings = EmulatorSettings {
            log_modules: Some("tapHLE::mem".to_string()),
            ..Default::default()
        };
        assert!(settings.to_args().is_empty());
        assert_eq!(
            settings.to_env(),
            [("TAPHLE_LOG_MODULES".to_string(), "tapHLE::mem".to_string())]
        );
    }
}
