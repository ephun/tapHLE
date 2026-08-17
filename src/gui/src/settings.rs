/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Global defaults, per-app overrides, and how both become emulator options.
//!
//! Every emulator setting here is [Option]: `None` means "not set at this
//! level", and a level that does not set something lets the next one down
//! decide. The chain runs
//!
//! ```text
//! per-app override → global default → tapHLE_options.txt
//!                  → tapHLE_default_options.txt → the emulator's own default
//! ```
//!
//! which is why an unset setting emits no argument at all rather than
//! emitting the emulator's default value: passing a value would silently
//! countermand the per-app entry someone added to
//! `tapHLE_default_options.txt`, and those entries are what makes several
//! games work.
//!
//! Nothing here invents a setting. Each field maps to an option the emulator
//! actually parses, and [EmulatorSettings::validate] proves it by feeding the
//! generated arguments back through the emulator's own parser.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    /// Modules to enable verbose tracing for. This is the `TAPHLE_LOG_MODULES`
    /// environment variable rather than an option, because that is how the
    /// emulator reads it.
    pub log_modules: Option<String>,
    /// Anything else, written as it would be typed on the command line. Every
    /// emulator option stays reachable without the frontend having to grow a
    /// control for each one, and it is checked by the emulator's own parser
    /// before a run starts.
    pub extra_arguments: Option<String>,
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
                tapHLE::log::LOG_MODULES_ENV_VAR.to_string(),
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
        let mut options = tapHLE::options::Options::default();
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

/// How the library is laid out. A compact list is the alternative to the
/// grid, so the library code is written against the entry order rather than
/// against a grid.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum IconSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl IconSize {
    pub const ALL: &'static [IconSize] = &[IconSize::Small, IconSize::Medium, IconSize::Large];

    pub fn label(self) -> &'static str {
        match self {
            IconSize::Small => "Small",
            IconSize::Medium => "Medium",
            IconSize::Large => "Large",
        }
    }

    /// Points, before display scaling. 57 is the size of an iPhone OS icon,
    /// and 72 the iPad's, so the three sizes bracket the originals.
    pub fn points(self) -> f32 {
        match self {
            IconSize::Small => 48.0,
            IconSize::Medium => 64.0,
            IconSize::Large => 88.0,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SortOrder {
    #[default]
    Title,
    Publisher,
    RecentlyPlayed,
    Compatibility,
    DateAdded,
}

impl SortOrder {
    pub const ALL: &'static [SortOrder] = &[
        SortOrder::Title,
        SortOrder::Publisher,
        SortOrder::RecentlyPlayed,
        SortOrder::Compatibility,
        SortOrder::DateAdded,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SortOrder::Title => "Title",
            SortOrder::Publisher => "Publisher",
            SortOrder::RecentlyPlayed => "Recently played",
            SortOrder::Compatibility => "Compatibility",
            SortOrder::DateAdded => "Date added",
        }
    }
}

/// Everything in `settings.json`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FrontendSettings {
    /// Emulator defaults for every app in the library.
    pub emulator: EmulatorSettings,
    /// Folders scanned for apps. Empty means the standard `tapHLE_apps`.
    pub library_folders: Vec<PathBuf>,
    /// An explicitly chosen emulator executable, for an unusual layout.
    pub emulator_path: Option<PathBuf>,
    /// Open the log panel by itself when a run ends badly.
    pub reveal_log_on_crash: bool,
    /// Keep the log panel open and show the developer-facing extras.
    pub developer_mode: bool,
    /// Ask GitHub for a newer release at startup.
    pub check_for_updates: bool,
    /// Ask before taking an app out of the library.
    pub confirm_remove: bool,
    pub log_capacity: usize,
    pub log_show_timestamps: bool,
    /// Interface zoom on top of the display's own scaling.
    pub ui_zoom: f32,
}

impl Default for FrontendSettings {
    fn default() -> Self {
        FrontendSettings {
            emulator: EmulatorSettings::default(),
            library_folders: Vec::new(),
            emulator_path: None,
            reveal_log_on_crash: true,
            developer_mode: false,
            check_for_updates: true,
            confirm_remove: true,
            log_capacity: crate::logstore::DEFAULT_CAPACITY,
            log_show_timestamps: true,
            ui_zoom: 1.0,
        }
    }
}

/// Everything in `state.json`: where the window was and what was on screen,
/// kept apart from settings because it changes constantly and means nothing
/// on another machine.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct UiState {
    pub window_size: Option<[f32; 2]>,
    pub window_position: Option<[f32; 2]>,
    pub maximized: bool,
    pub log_panel_visible: bool,
    pub log_panel_height: f32,
    pub details_panel_width: f32,
    pub selected_app: Option<String>,
    pub view_mode: ViewMode,
    pub icon_size: IconSize,
    pub sort_order: SortOrder,
    pub sort_descending: bool,
    pub favorites_only: bool,
    /// Which version of an app the library shows, keyed by bundle identifier.
    /// Absent means the newest one, which is what a fresh library shows.
    pub chosen_versions: std::collections::HashMap<String, String>,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            window_size: None,
            window_position: None,
            maximized: false,
            log_panel_visible: false,
            log_panel_height: 220.0,
            details_panel_width: 320.0,
            selected_app: None,
            view_mode: ViewMode::default(),
            icon_size: IconSize::default(),
            sort_order: SortOrder::default(),
            sort_descending: false,
            favorites_only: false,
            chosen_versions: std::collections::HashMap::new(),
        }
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
        }
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
