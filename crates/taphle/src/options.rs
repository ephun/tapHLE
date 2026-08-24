/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Parsing and management of user-configurable options, e.g. for input methods.

use crate::gles::GLESImplementation;
use crate::window::{DeviceFamily, DeviceOrientation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::net::{SocketAddr, ToSocketAddrs};
use std::num::NonZeroU32;
use std::path::PathBuf;

pub const OPTIONS_HELP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../runtime/OPTIONS_HELP.txt"
));

/// Game controller button for `--button-to-touch=` option.
///
/// Serialised by name rather than by index, so a stored control layout means
/// the same button on a controller that enumerates its buttons differently.
#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Button {
    DPadLeft,
    DPadUp,
    DPadRight,
    DPadDown,
    Start,
    A,
    B,
    X,
    Y,
    LeftShoulder,
}

/// A key on this computer's own keyboard, for `--key-to-touch=`.
///
/// Its own list rather than SDL's whole keycode space, for two reasons. A
/// stored layout should be text somebody can read and edit, and an editor
/// should be able to offer exactly the keys that can be bound instead of
/// several hundred codes including ones no keyboard has.
///
/// The two modifier keys of a pair are one key here. Nobody means "the right
/// Shift specifically", and a layout that said so would quietly do nothing on
/// a keyboard with one Shift.
macro_rules! keys {
    ($($variant:ident => $($keycode:ident)|+ ,)*) => {
        #[derive(Copy, Clone, Hash, PartialEq, Eq, Debug, Serialize, Deserialize)]
        pub enum Key {
            $($variant,)*
        }

        impl Key {
            /// Every key that can be bound, in the order a list should offer
            /// them.
            pub const ALL: &'static [Key] = &[$(Key::$variant,)*];

            /// What this key is called in options files and settings.
            pub fn name(self) -> &'static str {
                match self {
                    $(Key::$variant => stringify!($variant),)*
                }
            }

            pub fn from_name(name: &str) -> Option<Key> {
                match name {
                    $(stringify!($variant) => Some(Key::$variant),)*
                    _ => None,
                }
            }

            /// The key SDL has just reported, if it is one that can be bound.
            pub fn from_keycode(keycode: sdl2::keyboard::Keycode) -> Option<Key> {
                use sdl2::keyboard::Keycode as K;
                match keycode {
                    $($(K::$keycode)|+ => Some(Key::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

keys! {
    A => A, B => B, C => C, D => D, E => E, F => F, G => G, H => H, I => I,
    J => J, K => K, L => L, M => M, N => N, O => O, P => P, Q => Q, R => R,
    S => S, T => T, U => U, V => V, W => W, X => X, Y => Y, Z => Z,
    Num0 => Num0, Num1 => Num1, Num2 => Num2, Num3 => Num3, Num4 => Num4,
    Num5 => Num5, Num6 => Num6, Num7 => Num7, Num8 => Num8, Num9 => Num9,
    Left => Left, Right => Right, Up => Up, Down => Down,
    Space => Space, Return => Return, Escape => Escape, Tab => Tab,
    Backspace => Backspace,
    Shift => LShift | RShift,
    Ctrl => LCtrl | RCtrl,
    Alt => LAlt | RAlt,
    Comma => Comma, Period => Period, Slash => Slash, Semicolon => Semicolon,
    Quote => Quote, LeftBracket => LeftBracket, RightBracket => RightBracket,
    Backslash => Backslash, Minus => Minus, Equals => Equals,
    Backquote => Backquote,
}

/// Four keys standing in for a D-pad.
///
/// Grouped rather than four separate options because they only mean anything
/// together: three of the four leaves a control somebody cannot steer.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct KeyDpad {
    pub up: Key,
    pub down: Key,
    pub left: Key,
    pub right: Key,
}

/// Struct containing all user-configurable options.
/// What somebody chose to draw in place of one of the iPhone's fonts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontChoice {
    /// A family tapHLE ships, by its catalogue id.
    Bundled(String),
    /// A font file on this computer.
    File(std::path::PathBuf),
}

#[derive(Clone)]
pub struct Options {
    pub fullscreen: bool,
    pub device_family: Option<DeviceFamily>,
    pub initial_orientation: DeviceOrientation,
    /// The device's natural orientation is landscape and the app renders
    /// landscape-native (its OpenGL viewport is the landscape size) rather than
    /// drawing rotated content into a portrait framebuffer. In this mode the
    /// emulated screen is landscape-shaped and no presentation rotation is
    /// applied. Use it for landscape apps that neglect UIKit auto-rotation and
    /// come out sideways/clipped with `--landscape-left`/`--landscape-right`.
    pub landscape_native: bool,
    pub scale_hack: NonZeroU32,
    pub deadzone: f32,
    pub analog_stick_tilt_controls: bool,
    pub x_tilt_range: f32,
    pub y_tilt_range: f32,
    pub x_tilt_offset: f32,
    pub y_tilt_offset: f32,
    pub button_to_touch: HashMap<Button, (f32, f32)>,
    pub dpad_to_touch: Option<(f32, f32, f32, f32)>,
    pub stick_to_touch: Option<(f32, f32, f32, f32)>,
    /// Keys on this computer's keyboard mapped to points on the guest screen,
    /// the same way `button_to_touch` maps a controller's buttons. Somebody
    /// without a controller has a keyboard.
    pub key_to_touch: HashMap<Key, (f32, f32)>,
    /// Four keys driving a touch around a region, the keyboard's answer to
    /// `dpad_to_touch`, with the region it moves in.
    pub key_dpad_to_touch: Option<(KeyDpad, (f32, f32, f32, f32))>,
    pub stabilize_virtual_cursor: Option<(f32, f32)>,
    pub gles1_implementation: Option<GLESImplementation>,
    pub direct_memory_access: bool,
    pub gdb_listen_addrs: Option<Vec<SocketAddr>>,
    pub preferred_languages: Option<Vec<String>>,
    /// What to draw when an app asks for one of the iPhone's fonts, for the
    /// fonts somebody has chosen for themselves. Keyed by the normalised
    /// family name, so that a choice made for "Helvetica" also answers
    /// `Helvetica-BoldOblique`.
    pub font_overrides: HashMap<String, FontChoice>,
    /// Whether to use a font this computer already has when its name matches
    /// what the app asked for. On by default: someone who owns the real font
    /// should get it without having to say so.
    pub use_host_fonts: bool,
    /// Clickmap to replay once the app is running, driving it without any
    /// host-level input synthesis. See [crate::replay].
    pub replay: Option<std::path::PathBuf>,
    /// Quit once the replay's last step has settled, so an unattended run ends
    /// on its own instead of waiting to be killed.
    pub replay_quit: bool,
    pub headless: bool,
    pub print_fps: bool,
    pub fps_limit: Option<f64>,
    pub force_composition: bool,
    pub network_access: bool,
    pub popup_errors: bool,
    pub dumping_options: DumpingOptions,
    pub dumping_file: PathBuf,
    pub ignore_gl_errors: bool,
    pub zero_stack_after_guest_to_host_call: Option<u32>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            fullscreen: false,
            device_family: None,
            initial_orientation: DeviceOrientation::Portrait,
            landscape_native: false,
            scale_hack: NonZeroU32::new(1).unwrap(),
            analog_stick_tilt_controls: true,
            deadzone: 0.1,
            x_tilt_range: 60.0,
            y_tilt_range: 60.0,
            x_tilt_offset: 0.0,
            y_tilt_offset: 0.0,
            button_to_touch: HashMap::new(),
            dpad_to_touch: None,
            stick_to_touch: None,
            key_to_touch: HashMap::new(),
            key_dpad_to_touch: None,
            stabilize_virtual_cursor: None,
            gles1_implementation: None,
            direct_memory_access: true,
            gdb_listen_addrs: None,
            preferred_languages: None,
            font_overrides: HashMap::new(),
            use_host_fonts: true,
            replay: None,
            replay_quit: false,
            headless: false,
            print_fps: false,
            fps_limit: Some(60.0), // Original iPhone is 60Hz and uses v-sync,
            force_composition: false,
            network_access: false,
            popup_errors: true,
            dumping_options: Default::default(),
            dumping_file: crate::paths::user_data_base_path().join("DUMP.txt"),
            ignore_gl_errors: false,
            zero_stack_after_guest_to_host_call: None,
        }
    }
}

impl Options {
    /// Parse the command-line argument syntax for an option. Returns `Ok(true)`
    /// if the option was valid and has been applied, or `Ok(false)` if the
    /// option was not recognized.
    pub fn parse_argument(&mut self, arg: &str) -> Result<bool, String> {
        fn parse_degrees(arg: &str, name: &str) -> Result<f32, String> {
            let arg: f32 = arg
                .parse()
                .map_err(|_| format!("Value for {name} is invalid"))?;
            if !arg.is_finite() || !(-360.0..=360.0).contains(&arg) {
                return Err(format!("Value for {name} is out of range"));
            }
            Ok(arg)
        }

        // A fraction of an axis's travel, not an angle. This needs its own
        // range check: the deadzone used to be parsed as if it were degrees,
        // which accepted anything from -360 to 360. A negative one reached
        // `assert!(deadzone >= 0.0)` in `window::convert_axis` and panicked on
        // an ordinary command line, and one above 1 silently swallowed the
        // stick's whole range instead of being rejected.
        fn parse_fraction(arg: &str, name: &str) -> Result<f32, String> {
            let arg: f32 = arg
                .parse()
                .map_err(|_| format!("Value for {name} is invalid"))?;
            if !arg.is_finite() || !(0.0..=1.0).contains(&arg) {
                return Err(format!("Value for {name} must be between 0 and 1"));
            }
            Ok(arg)
        }

        // Every boolean option below has both an on and an off spelling. A
        // one-way flag cannot be turned back off by a later argument, and
        // options arrive in layers — the bundled defaults file, the user's
        // options file, then the command line, then the frontend's per-app
        // override — so without the off spelling a default set for an app
        // could never be countermanded.
        if arg == "--fullscreen" {
            self.fullscreen = true;
        } else if arg == "--windowed" {
            self.fullscreen = false;
        } else if arg == "--portrait" {
            self.initial_orientation = DeviceOrientation::Portrait;
        } else if arg == "--upside-down" {
            self.initial_orientation = DeviceOrientation::PortraitUpsideDown;
        } else if arg == "--landscape-left" {
            self.initial_orientation = DeviceOrientation::LandscapeLeft;
        } else if arg == "--landscape-right" {
            self.initial_orientation = DeviceOrientation::LandscapeRight;
        } else if arg == "--host-fonts" {
            self.use_host_fonts = true;
        } else if arg == "--no-host-fonts" {
            self.use_host_fonts = false;
        } else if let Some(value) = arg.strip_prefix("--font=") {
            // `--font=<iPhone family>=<what to draw instead>`, where the second
            // half is either the id of a family tapHLE ships or the path of a
            // font file on this computer. Repeating the option adds another
            // font rather than replacing the first, because these are a set of
            // independent choices, not one setting.
            let (family, choice) = value
                .split_once('=')
                .ok_or_else(|| "--font needs <family>=<substitute>".to_string())?;
            if family.trim().is_empty() || choice.trim().is_empty() {
                return Err("--font needs <family>=<substitute>".to_string());
            }
            let choice = if crate::font::catalogue::bundled(choice).is_some() {
                FontChoice::Bundled(choice.to_string())
            } else {
                FontChoice::File(std::path::PathBuf::from(choice))
            };
            self.font_overrides
                .insert(crate::font::catalogue::normalise(family), choice);
        } else if arg == "--landscape-native" {
            self.landscape_native = true;
        } else if arg == "--no-landscape-native" {
            self.landscape_native = false;
        } else if let Some(value) = arg.strip_prefix("--device-family=") {
            let parsed =
                DeviceFamily::try_from(value).map_err(|_| "Invalid device family".to_string())?;
            self.device_family = Some(parsed);
        } else if let Some(value) = arg.strip_prefix("--scale-hack=") {
            self.scale_hack = value
                .parse()
                .map_err(|_| "Invalid scale hack factor".to_string())?;
        } else if arg == "--disable-analog-stick-tilt-controls" {
            self.analog_stick_tilt_controls = false;
        } else if arg == "--enable-analog-stick-tilt-controls" {
            self.analog_stick_tilt_controls = true;
        } else if let Some(value) = arg.strip_prefix("--deadzone=") {
            self.deadzone = parse_fraction(value, "deadzone")?;
        } else if let Some(value) = arg.strip_prefix("--x-tilt-range=") {
            self.x_tilt_range = parse_degrees(value, "X tilt range")?;
        } else if let Some(value) = arg.strip_prefix("--y-tilt-range=") {
            self.y_tilt_range = parse_degrees(value, "Y tilt range")?;
        } else if let Some(value) = arg.strip_prefix("--x-tilt-offset=") {
            self.x_tilt_offset = parse_degrees(value, "X tilt offset")?;
        } else if let Some(value) = arg.strip_prefix("--y-tilt-offset=") {
            self.y_tilt_offset = parse_degrees(value, "Y tilt offset")?;
        } else if let Some(values) = arg.strip_prefix("--button-to-touch=") {
            let (button, coords) = values
                .split_once(',')
                .ok_or_else(|| "--button-to-touch= requires three values".to_string())?;
            let (x, y) = coords
                .split_once(',')
                .ok_or_else(|| "--button-to-touch= requires three values".to_string())?;
            let button = match button {
                "DPadLeft" => Ok(Button::DPadLeft),
                "DPadUp" => Ok(Button::DPadUp),
                "DPadRight" => Ok(Button::DPadRight),
                "DPadDown" => Ok(Button::DPadDown),
                "Start" => Ok(Button::Start),
                "A" => Ok(Button::A),
                "B" => Ok(Button::B),
                "X" => Ok(Button::X),
                "Y" => Ok(Button::Y),
                "LeftShoulder" => Ok(Button::LeftShoulder),
                _ => Err("Invalid button for --button-to-touch=".to_string()),
            }?;
            let x: f32 = x
                .parse()
                .map_err(|_| "Invalid X co-ordinate for --button-to-touch=".to_string())?;
            let y: f32 = y
                .parse()
                .map_err(|_| "Invalid Y co-ordinate for --button-to-touch=".to_string())?;
            self.button_to_touch.insert(button, (x, y));
        } else if let Some(values) = arg.strip_prefix("--key-to-touch=") {
            let (key, coords) = values
                .split_once(',')
                .ok_or_else(|| "--key-to-touch= requires three values".to_string())?;
            let (x, y) = coords
                .split_once(',')
                .ok_or_else(|| "--key-to-touch= requires three values".to_string())?;
            let key = Key::from_name(key)
                .ok_or_else(|| format!("Invalid key for --key-to-touch=: {key}"))?;
            let x: f32 = x
                .parse()
                .map_err(|_| "Invalid X co-ordinate for --key-to-touch=".to_string())?;
            let y: f32 = y
                .parse()
                .map_err(|_| "Invalid Y co-ordinate for --key-to-touch=".to_string())?;
            self.key_to_touch.insert(key, (x, y));
        } else if let Some(values) = arg.strip_prefix("--key-dpad-to-touch=") {
            let parts: Vec<&str> = values.split(',').collect();
            let parts: [&str; 8] = parts.try_into().map_err(|_| {
                "--key-dpad-to-touch= requires four keys and four numbers".to_string()
            })?;
            let mut keys = [Key::Up; 4];
            for (key, name) in keys.iter_mut().zip(&parts[..4]) {
                *key = Key::from_name(name)
                    .ok_or_else(|| format!("Invalid key for --key-dpad-to-touch=: {name}"))?;
            }
            let mut nums = [0.0f32; 4];
            for (num, text) in nums.iter_mut().zip(&parts[4..]) {
                *num = text
                    .parse()
                    .map_err(|_| "invalid --key-dpad-to-touch".to_string())?;
            }
            self.key_dpad_to_touch = Some((
                KeyDpad {
                    up: keys[0],
                    down: keys[1],
                    left: keys[2],
                    right: keys[3],
                },
                (nums[0], nums[1], nums[2], nums[3]),
            ));
        } else if let Some(values) = arg.strip_prefix("--stick-to-touch=") {
            let nums: [f32; 4] = values
                .split(',')
                .map(|s| s.parse::<f32>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "invalid --stick-to-touch".to_string())?
                .try_into()
                .map_err(|_| "--stick-to-touch= requires four values".to_string())?;

            self.stick_to_touch = Some((nums[0], nums[1], nums[2], nums[3]));
        } else if let Some(values) = arg.strip_prefix("--dpad-to-touch=") {
            let nums: [f32; 4] = values
                .split(',')
                .map(|s| s.parse::<f32>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "invalid --dpad-to-touch".to_string())?
                .try_into()
                .map_err(|_| "--dpad-to-touch= requires four values".to_string())?;

            self.dpad_to_touch = Some((nums[0], nums[1], nums[2], nums[3]));
        } else if let Some(value) = arg.strip_prefix("--stabilize-virtual-cursor=") {
            let (smoothing_strength, sticky_radius) = value
                .split_once(',')
                .ok_or_else(|| "--stabilize-virtual-cursor= requires two values".to_string())?;
            let smoothing_strength: f32 = smoothing_strength
                .parse()
                .ok()
                .filter(|&s| s >= 0.0)
                .ok_or_else(|| {
                    "Invalid smoothing strength for --stabilize-virtual-cursor=".to_string()
                })?;
            let sticky_radius: f32 = sticky_radius
                .parse()
                .ok()
                .filter(|&s| s >= 0.0)
                .ok_or_else(|| {
                    "Invalid sticky radius for --stabilize-virtual-cursor=".to_string()
                })?;
            self.stabilize_virtual_cursor = Some((smoothing_strength, sticky_radius));
        } else if arg == "--no-stabilize-virtual-cursor" {
            // The off spelling exists for the same reason every boolean option
            // has one: seven apps switch stabilisation on in
            // runtime/default_options.txt, and without this nothing a later
            // layer said could turn it back off again.
            self.stabilize_virtual_cursor = None;
        } else if let Some(value) = arg.strip_prefix("--gles1=") {
            self.gles1_implementation = Some(
                GLESImplementation::from_short_name(value)
                    .map_err(|_| "Unrecognized --gles1= value".to_string())?,
            );
        } else if arg == "--disable-direct-memory-access" {
            self.direct_memory_access = false;
        } else if arg == "--enable-direct-memory-access" {
            self.direct_memory_access = true;
        } else if let Some(address) = arg.strip_prefix("--gdb=") {
            let addrs = address
                .to_socket_addrs()
                .map_err(|e| format!("Could not resolve GDB server listen address: {e}"))?
                .collect();
            self.gdb_listen_addrs = Some(addrs);
        } else if let Some(value) = arg.strip_prefix("--preferred-languages=") {
            self.preferred_languages = Some(value.split(',').map(ToOwned::to_owned).collect());
        } else if let Some(value) = arg.strip_prefix("--replay=") {
            self.replay = Some(value.into());
        } else if arg == "--replay-quit" {
            self.replay_quit = true;
        } else if arg == "--no-replay-quit" {
            self.replay_quit = false;
        } else if arg == "--headless" {
            self.headless = true;
            // Can't show the dialog box when headless!
            self.popup_errors = false;
        } else if arg == "--print-fps" {
            self.print_fps = true;
        } else if arg == "--no-print-fps" {
            self.print_fps = false;
        } else if let Some(value) = arg.strip_prefix("--fps-limit=") {
            if value == "off" {
                self.fps_limit = None;
            } else {
                let limit: f64 = value
                    .parse()
                    .ok()
                    .filter(|&v| v > 0.0)
                    .ok_or_else(|| "Invalid value for --fps-limit=".to_string())?;
                self.fps_limit = Some(limit);
            }
        } else if arg == "--force-composition" {
            self.force_composition = true;
        } else if arg == "--no-force-composition" {
            self.force_composition = false;
        } else if arg == "--allow-network-access" {
            self.network_access = true;
        } else if arg == "--deny-network-access" {
            self.network_access = false;
        } else if arg == "--no-error-popup" {
            self.popup_errors = false;
        } else if arg == "--error-popup" {
            self.popup_errors = true;
        } else if let Some(values) = arg.strip_prefix("--dump=") {
            self.dumping_options = parse_dump_options(values)?;
        } else if let Some(path) = arg.strip_prefix("--dump-file=") {
            self.dumping_file = crate::paths::user_data_base_path().join(path);
        } else if arg == "--ignore-gl-errors" {
            self.ignore_gl_errors = true;
        } else if arg == "--report-gl-errors" {
            self.ignore_gl_errors = false;
        } else if let Some(value) = arg.strip_prefix("--zero-stack-after-guest-to-host-call=") {
            self.zero_stack_after_guest_to_host_call = Some(value.parse().map_err(|_| {
                "Invalid value for --zero-stack-after-guest-to-host-call=".to_string()
            })?);
        } else {
            return Ok(false);
        };
        Ok(true)
    }
}

/// Try to get app-specific options from a file.
///
/// Returns [Ok] if there is no error when reading the file, otherwise [Err].
/// The [Ok] value is a [Some] with the options if they could be found, or
/// [None] if no options were found for this app.
pub fn get_options_from_file<F: Read>(file: F, app_id: &str) -> Result<Option<String>, String> {
    let file = BufReader::new(file);
    for (line_no, line) in BufRead::lines(file).enumerate() {
        // Line numbering usually starts from 1
        let line_no = line_no + 1;

        let line = line.map_err(|e| format!("Error while reading line {line_no}: {e}"))?;

        // # for single-line comments
        let line = if let Some((rest, _)) = line.split_once('#') {
            rest
        } else {
            &line
        };

        // Empty/all-comment lines ignored
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (line_app_id, line_options) = line.split_once(':').ok_or_else(|| format!("Line {line_no} is not a comment and is missing a colon (:) to separate the app ID from the options"))?;
        let line_app_id = line_app_id.trim();

        if line_app_id != app_id {
            continue;
        }

        let line_options = line_options.trim();
        if line_options.is_empty() {
            return Ok(None);
        } else {
            return Ok(Some(line_options.to_string()));
        }
    }
    Ok(None)
}

#[derive(Default, Clone)]
pub struct DumpingOptions {
    pub linking_info: bool,
    pub symbols: bool,
}

impl DumpingOptions {
    /// Check if any of the dumping options are active.
    pub fn any(&self) -> bool {
        self.linking_info || self.symbols
    }
}

/// Every boolean option's on spelling paired with its off spelling, and a way
/// to read the field each pair controls.
///
/// The desktop frontend layers a per-app override on top of a global default
/// on top of the options files, so each of these must be expressible in both
/// directions from an argument list. Keeping the pairs in one table lets the
/// tests below check that without restating them.
#[cfg(test)]
type BooleanOptionPair = (&'static str, &'static str, fn(&Options) -> bool);

#[cfg(test)]
const BOOLEAN_OPTION_PAIRS: &[BooleanOptionPair] = &[
    ("--fullscreen", "--windowed", |o| o.fullscreen),
    ("--landscape-native", "--no-landscape-native", |o| {
        o.landscape_native
    }),
    (
        "--enable-analog-stick-tilt-controls",
        "--disable-analog-stick-tilt-controls",
        |o| o.analog_stick_tilt_controls,
    ),
    (
        "--enable-direct-memory-access",
        "--disable-direct-memory-access",
        |o| o.direct_memory_access,
    ),
    ("--print-fps", "--no-print-fps", |o| o.print_fps),
    ("--force-composition", "--no-force-composition", |o| {
        o.force_composition
    }),
    ("--allow-network-access", "--deny-network-access", |o| {
        o.network_access
    }),
    ("--error-popup", "--no-error-popup", |o| o.popup_errors),
    ("--ignore-gl-errors", "--report-gl-errors", |o| {
        o.ignore_gl_errors
    }),
];

fn parse_dump_options(options: &str) -> Result<DumpingOptions, String> {
    let mut dumping_options = DumpingOptions::default();
    for opt in options.split(",") {
        if opt == "linking-info" {
            // Dumps linked symbols, classes and selectors for the given app
            dumping_options.linking_info = true;
        } else if opt == "symbols" {
            // Dumps tapHLE provided symbols and exits
            dumping_options.symbols = true;
        } else {
            return Err(format!("Unrecognized option {opt} for --dump=..."));
        }
    }
    Ok(dumping_options)
}

#[cfg(test)]
mod tests {
    use super::{Key, KeyDpad, Options, BOOLEAN_OPTION_PAIRS};
    use crate::window::DeviceOrientation;

    fn parse_all(args: &[&str]) -> Options {
        let mut options = Options::default();
        for arg in args {
            assert_eq!(
                options.parse_argument(arg),
                Ok(true),
                "option {arg:?} was not recognized"
            );
        }
        options
    }

    /// Each boolean option's two spellings must actually set and clear the
    /// field, in either order. A pair where both spellings set the same value
    /// would silently make one layer of configuration unable to countermand
    /// another.
    #[test]
    fn boolean_options_can_be_switched_both_ways() {
        for &(on, off, read) in BOOLEAN_OPTION_PAIRS {
            assert!(read(&parse_all(&[off, on])), "{on:?} did not switch on");
            assert!(!read(&parse_all(&[on, off])), "{off:?} did not switch off");
        }
    }

    /// The deadzone is a fraction of the stick's travel, and the emulator
    /// asserts it is not negative once a stick moves. Parsing it as if it were
    /// an angle accepted -360 to 360, so `--deadzone=-1` parsed cleanly and
    /// then panicked in `window::convert_axis` the first time somebody touched
    /// the stick.
    #[test]
    fn a_deadzone_outside_zero_to_one_is_rejected() {
        for bad in ["-1", "-0.5", "1.5", "200", "inf", "NaN"] {
            let mut options = Options::default();
            assert!(
                options
                    .parse_argument(&format!("--deadzone={bad}"))
                    .is_err(),
                "--deadzone={bad} should have been rejected"
            );
        }
    }

    #[test]
    fn a_deadzone_inside_zero_to_one_is_accepted() {
        for good in ["0", "0.1", "0.25", "1"] {
            let options = parse_all(&[&format!("--deadzone={good}")]);
            assert_eq!(options.deadzone, good.parse::<f32>().unwrap());
        }
    }

    /// Portrait is the default orientation, so it had no spelling of its own
    /// and could not be asked for once another layer had chosen landscape.
    #[test]
    fn portrait_can_be_asked_for_explicitly() {
        let options = parse_all(&["--landscape-left", "--portrait"]);
        assert_eq!(options.initial_orientation, DeviceOrientation::Portrait);
    }

    /// An unrecognized option is reported as unrecognized rather than as an
    /// error, because that is how a bundle path is told apart from a flag.
    #[test]
    fn an_unknown_option_is_not_an_error() {
        let mut options = Options::default();
        assert_eq!(options.parse_argument("--no-such-option"), Ok(false));
    }

    /// Every key has to survive being written to a settings file and read
    /// back, and no two may share a name — a collision would silently rebind
    /// one of them to the other.
    #[test]
    fn every_key_name_is_unique_and_round_trips() {
        let mut seen = std::collections::HashSet::new();
        for &key in Key::ALL {
            assert_eq!(Key::from_name(key.name()), Some(key), "{key:?}");
            assert!(seen.insert(key.name()), "two keys called {}", key.name());
        }
    }

    /// Both modifiers of a pair are the same key here, because a layout that
    /// insisted on the right Shift would do nothing on a keyboard with one.
    #[test]
    fn either_of_a_modifier_pair_is_the_same_key() {
        use sdl2::keyboard::Keycode as K;
        for (left, right, key) in [
            (K::LShift, K::RShift, Key::Shift),
            (K::LCtrl, K::RCtrl, Key::Ctrl),
            (K::LAlt, K::RAlt, Key::Alt),
        ] {
            assert_eq!(Key::from_keycode(left), Some(key));
            assert_eq!(Key::from_keycode(right), Some(key));
        }
    }

    /// F12 is the debugger. If it were bindable, mapping it would take the
    /// only way to break into a stuck app.
    #[test]
    fn the_debugger_key_cannot_be_bound() {
        assert_eq!(Key::from_keycode(sdl2::keyboard::Keycode::F12), None);
    }

    #[test]
    fn keyboard_mappings_parse() {
        let options = parse_all(&[
            "--key-to-touch=Space,470,310",
            "--key-to-touch=Num1,10,20",
            "--key-dpad-to-touch=W,S,A,D,10,10,50,50",
        ]);
        assert_eq!(options.key_to_touch[&Key::Space], (470.0, 310.0));
        assert_eq!(options.key_to_touch[&Key::Num1], (10.0, 20.0));
        assert_eq!(
            options.key_dpad_to_touch,
            Some((
                KeyDpad {
                    up: Key::W,
                    down: Key::S,
                    left: Key::A,
                    right: Key::D,
                },
                (10.0, 10.0, 50.0, 50.0)
            ))
        );
    }

    /// A misspelled key has to be refused rather than ignored: a mapping that
    /// silently does nothing is harder to notice than one that complains.
    #[test]
    fn a_keyboard_mapping_that_is_wrong_is_refused() {
        for bad in [
            "--key-to-touch=Spacebar,1,2",
            "--key-to-touch=Space,1",
            "--key-to-touch=Space,1,over-there",
            "--key-dpad-to-touch=W,S,A,D,1,2,3",
            "--key-dpad-to-touch=W,S,A,Nope,1,2,3,4",
        ] {
            let mut options = Options::default();
            assert!(options.parse_argument(bad).is_err(), "{bad} was accepted");
        }
    }
}
