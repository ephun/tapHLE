/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Utilities for working with bundles. So far we are only interested in the
//! application bundle.
//!
//! Relevant Apple documentation:
//! * [Bundle Programming Guide](https://developer.apple.com/library/archive/documentation/CoreFoundation/Conceptual/CFBundles/Introduction/Introduction.html)
//!   * [Anatomy of an iOS Application Bundle](https://developer.apple.com/library/archive/documentation/CoreFoundation/Conceptual/CFBundles/BundleTypes/BundleTypes.html)
//! * [Bundle Resources](https://developer.apple.com/documentation/bundleresources?language=objc)

use crate::fs::{BundleData, Fs, GuestPath, GuestPathBuf};
use crate::image::Image;
use crate::window::{DeviceFamily, DeviceOrientation};
use plist::dictionary::Dictionary;
use plist::Value;
use std::io::Cursor;

#[derive(Debug)]
pub struct Bundle {
    path: GuestPathBuf,
    plist: Dictionary,
}

impl Bundle {
    /// See [Fs::new] for meaning of `read_only_mode`.
    pub fn new_bundle_and_fs_from_host_path(
        mut bundle_data: BundleData,
        read_only_mode: bool,
    ) -> Result<(Bundle, Fs), String> {
        let plist_bytes = bundle_data.read_plist()?;

        let plist = Value::from_reader(Cursor::new(plist_bytes))
            .map_err(|_| "Could not deserialize plist data".to_string())?;

        let plist = plist
            .into_dictionary()
            .ok_or_else(|| "plist root value is not a dictionary".to_string())?;

        let bundle_name = format!(
            "{}.app",
            if let Some(canonical) = plist.get("CFBundleName") {
                canonical.as_string().unwrap()
            } else {
                bundle_data.bundle_name()
            }
        );
        let bundle_id = plist["CFBundleIdentifier"].as_string().unwrap();

        let (fs, guest_path) = Fs::new(bundle_data, bundle_name, bundle_id, read_only_mode);

        let bundle = Bundle {
            path: guest_path,
            plist,
        };

        Ok((bundle, fs))
    }

    pub fn bundle_path(&self) -> &GuestPath {
        &self.path
    }

    pub fn bundle_identifier(&self) -> &str {
        self.plist["CFBundleIdentifier"].as_string().unwrap()
    }

    pub fn bundle_version(&self) -> &str {
        self.plist["CFBundleVersion"].as_string().unwrap()
    }

    /// The marketing version string, if the app has one.
    ///
    /// This is the version a store listing and a compatibility record show,
    /// and it is frequently different from `CFBundleVersion`, which is a build
    /// number. Plenty of apps of this era omit it entirely.
    pub fn short_version(&self) -> Option<&str> {
        self.plist
            .get("CFBundleShortVersionString")
            .and_then(|v| v.as_string())
    }

    pub fn bundle_localizations(&self) -> &[Value] {
        static EMPTY_VAL: Vec<Value> = Vec::new();
        self.plist
            .get("CFBundleLocalizations")
            .and_then(|v| v.as_array())
            .unwrap_or(&EMPTY_VAL)
    }

    /// Canonical name for the bundle according to Info.plist
    pub fn canonical_bundle_name(&self) -> Option<&str> {
        self.plist
            .get("CFBundleName")
            .map(|name| name.as_string().unwrap())
    }

    /// Name for the bundle, either the canonical name or, if there isn't one,
    /// the name this bundle has in the filesystem.
    pub fn bundle_name(&self) -> &str {
        self.path.file_name().unwrap().strip_suffix(".app").unwrap()
    }

    pub fn display_name(&self) -> &str {
        if let Some(display_name) = self.plist.get("CFBundleDisplayName") {
            display_name.as_string().unwrap()
        } else {
            ""
        }
    }

    pub fn minimum_os_version(&self) -> Option<&str> {
        self.plist
            .get("MinimumOSVersion")
            .map(|v| v.as_string().unwrap())
    }

    pub fn required_device_capabilities(&self) -> Vec<&str> {
        self.plist
            .get("UIRequiredDeviceCapabilities")
            .map(|v| {
                if let Some(dict) = v.as_dictionary() {
                    // TODO: support undesired capabilities
                    assert!(dict.values().all(|x| x.as_boolean().unwrap()));
                    dict.keys().map(|o| o.as_str()).collect()
                } else {
                    v.as_array()
                        .unwrap()
                        .iter()
                        .map(|o| o.as_string().unwrap())
                        .collect()
                }
            })
            .unwrap_or_default()
    }

    pub fn executable_path(&self) -> GuestPathBuf {
        // FIXME: Is this key optional? All iPhone apps seem to have it.
        self.path
            .join(self.plist["CFBundleExecutable"].as_string().unwrap())
    }

    /// Launch image file names to try, most specific first.
    ///
    /// An app that launches in landscape ships its launch image under an
    /// orientation-qualified name — `Default-Landscape.png`, or
    /// `Default-LandscapeLeft.png` — because that is what the real launcher
    /// looks for, and leaves plain `Default.png` portrait. tapHLE only ever
    /// looked for the unqualified name, so every landscape game opened on a
    /// portrait image stretched across a landscape window and turned on its
    /// side.
    ///
    /// `orientation_suffixes` are the modifiers to try, most specific first.
    /// The unqualified name is always the last resort, since it is the only
    /// one guaranteed to exist — and not even that.
    pub fn launch_image_paths(&self, orientation_suffixes: &[&str]) -> Vec<GuestPathBuf> {
        let base_name = match self.plist.get("UILaunchImageFile") {
            Some(value) => value.as_string().unwrap(),
            None => "Default",
        };
        launch_image_names(base_name, orientation_suffixes)
            .into_iter()
            .map(|name| self.path.join(name))
            .collect()
    }

    pub fn status_bar_hidden(&self) -> bool {
        self.plist
            .get("UIStatusBarHidden")
            .and_then(|v| v.as_boolean())
            .unwrap_or(false)
    }

    fn find_icon(icon_files: &[plist::Value]) -> Option<&plist::Value> {
        ["Icon", "Icon-72"]
            .iter()
            .find_map(|icon| {
                icon_files.iter().find(|found_icon| {
                    found_icon
                        .as_string()
                        .is_some_and(|s| s.trim_end_matches(".png") == *icon)
                })
            })
            .or_else(|| icon_files.first())
    }

    fn icon_path(&self) -> GuestPathBuf {
        // TODO: Fix this to what an actual iOS device does,
        // including Retina icons and such when we get there.
        // Reference: https://developer.apple.com/library/archive/qa/qa1686/_index.html
        // We check for the icon in the following order:
        // 1. CFBundleIconFile,
        // 2. CFBundleIconFiles for Icon, Icon-72,
        // 3. First in CFBundleIconFiles,
        // 4. Failsafe Icon.png
        if let Some(filename) = self.plist.get("CFBundleIconFile").or_else(|| {
            self.plist
                .get("CFBundleIconFiles")
                .and_then(|v| v.as_array())
                .and_then(|a| Self::find_icon(a))
        }) {
            if filename
                .as_string()
                .unwrap()
                .to_lowercase()
                .ends_with(".png")
            {
                self.path.join(filename.as_string().unwrap())
            } else {
                let filename_with_extension = format!("{}.png", filename.as_string().unwrap());
                self.path.join(filename_with_extension)
            }
        } else {
            self.path.join("Icon.png")
        }
    }

    /// Load icon and round off its corners (and add sheen if needed) for
    /// display.
    pub fn load_icon(&self, fs: &Fs) -> Result<Image, String> {
        let bytes = fs
            .read(self.icon_path())
            .map_err(|_| "Could not read icon file".to_string())?;
        let mut image =
            Image::from_bytes(&bytes).map_err(|e| format!("Could not parse icon image: {e}"))?;
        // UIPrerenderedIcon is used to avoid iOS applying a sheen effect,
        // should be boolean, but some apps use a string, so we check both.
        // See https://developer.apple.com/library/archive/qa/qa1614/_index.html
        // Default if it does not exist is NO/false.
        let add_sheen = !self
            .plist
            .get("UIPrerenderedIcon")
            .and_then(|v| v.as_boolean().or(v.as_string().map(|s| s == "YES")))
            .unwrap_or(false);
        // iPhone OS icons are 57px by 57px and the OS always applies a
        // 10px radius rounded corner (see e.g. documentation of
        // UIPrerenderedIcon). If the icon is larger for some reason,
        // let's scale to match.
        let corner_radius = (10.0 / 57.0) * (image.dimensions().0 as f32);
        image.round_corners(corner_radius, /* four_corners: */ true, add_sheen);
        Ok(image)
    }

    pub fn main_nib_filename(&self, device_family: Option<DeviceFamily>) -> Option<&str> {
        // TODO: extend this logic for all device-specific keys
        if let Some(device_family) = device_family {
            if device_family == DeviceFamily::iPad && self.plist.get("NSMainNibFile~ipad").is_some()
            {
                return self
                    .plist
                    .get("NSMainNibFile~ipad")
                    .map(|v| v.as_string().unwrap());
            }
        }
        self.plist
            .get("NSMainNibFile")
            .map(|v| v.as_string().unwrap())
    }

    /// The interface orientation the app declares it *launches* in, if it says.
    ///
    /// `UIInterfaceOrientation` and `UISupportedInterfaceOrientations` answer
    /// different questions, and conflating them loses this one. The array says
    /// which orientations the app can rotate to; the older singular key says
    /// which one it starts in. An app that lists all four but launches
    /// landscape is ordinary, and reading only the array leaves it in
    /// portrait, drawing its landscape
    /// artwork sideways in a portrait window.
    ///
    /// When the older key holds a comma-separated list, the first entry is the
    /// one the app launches in.
    pub fn initial_interface_orientation(&self) -> Option<&str> {
        let value = self.plist.get("UIInterfaceOrientation")?;
        let Some(string) = value.as_string() else {
            log!("UIInterfaceOrientation is not a string, ignoring");
            return None;
        };
        string.split(',').next().filter(|s| !s.is_empty())
    }

    pub fn supported_interface_orientations(&self) -> Vec<&str> {
        // UIInterfaceOrientation (iPhone OS 2.0) is a single string
        // (or a comma separated list of strings).
        // UISupportedInterfaceOrientations (iOS 3.2) is an array of strings and
        // takes precedence.
        // The newer key is documented as an array, and mostly is, but shipped
        // Info.plists disagree: some write a bare string under it, the older
        // key's spelling with the newer key's name, and some write a
        // dictionary of "Item 0", "Item 1" entries, which is what an array
        // edited in the wrong pane of Xcode's plist editor becomes.
        //
        // A string is read the way the older key is read. Anything else that is
        // not an array of strings is reported and treated as absent, which
        // falls through to the older key and then to the portrait default -
        // the same place a device ends up when it cannot read this key either.
        self.plist
            .get("UISupportedInterfaceOrientations")
            .and_then(|v| match v.as_array() {
                Some(array) => {
                    let orientations: Vec<&str> =
                        array.iter().filter_map(|o| o.as_string()).collect();
                    if orientations.is_empty() {
                        log!("UISupportedInterfaceOrientations names no orientations, ignoring");
                    }
                    (!orientations.is_empty()).then_some(orientations)
                }
                None => match v.as_string() {
                    Some(s) => Some(s.split(',').collect()),
                    None => {
                        log!("UISupportedInterfaceOrientations is not a list of strings, ignoring");
                        None
                    }
                },
            })
            .unwrap_or_else(|| {
                if let Some(v) = self
                    .plist
                    .get("UIInterfaceOrientation") {
                    let str = v.as_string().unwrap();
                    if str.contains(',') {
                        log!("UIInterfaceOrientation is a comma separated list of strings ({}), splitting!", str);
                    }
                    str.split(',').collect()
                } else {
                    vec!["UIInterfaceOrientationPortrait"]
                }
            })
    }

    pub fn device_family_array(&self) -> Vec<DeviceFamily> {
        self.plist
            .get("UIDeviceFamily")
            .map(|v| {
                v.as_array()
                    .unwrap()
                    .iter()
                    .map(|o| {
                        DeviceFamily::try_from(match o {
                            Value::Integer(i) => i.as_unsigned().unwrap(),
                            Value::String(s) => s.parse().unwrap(),
                            _ => unreachable!(),
                        })
                        .unwrap()
                    })
                    .collect()
            })
            .unwrap_or_else(|| vec![DeviceFamily::iPhone])
    }
}

/// The device orientation that shows `interface_orientation` upright, or [None]
/// for a name that is not one of iPhone OS's.
///
/// Landscape is inverted between the two vocabularies: interface
/// `LandscapeLeft` is device `LandscapeRight`, because the interface rotates
/// against the way the device is turned. The two portrait values agree.
/// Getting this backwards leaves an app upside down rather than failing, so it
/// is pinned by tests.
pub fn device_orientation_for_interface_orientation(
    interface_orientation: &str,
) -> Option<DeviceOrientation> {
    Some(match interface_orientation {
        "UIInterfaceOrientationPortrait" | "UIDeviceOrientationPortrait" => {
            DeviceOrientation::Portrait
        }
        "UIInterfaceOrientationPortraitUpsideDown" => DeviceOrientation::PortraitUpsideDown,
        "UIInterfaceOrientationLandscapeLeft" => DeviceOrientation::LandscapeRight,
        "UIInterfaceOrientationLandscapeRight" => DeviceOrientation::LandscapeLeft,
        // An older way to spell it. From testing, it corresponds to left.
        "UIInterfaceOrientationLandscape" => DeviceOrientation::LandscapeLeft,
        _ => return None,
    })
}

/// Launch image name modifiers to try for a device orientation, most specific
/// first.
///
/// The modifier in a launch image's file name names the *interface*
/// orientation, and for landscape that is the opposite of the *device*
/// orientation: `UIInterfaceOrientationLandscapeLeft` is
/// `UIDeviceOrientationLandscapeRight`, because the UI has to rotate against
/// the way the device was turned. [DeviceOrientation] is the device one, so
/// the two landscape arms below read backwards on purpose.
///
/// Getting this backwards does not fail loudly: an app that ships both
/// `Default-LandscapeLeft.png` and `Default-LandscapeRight.png` — the two are
/// 180° apart — simply comes up upside down, which is how it was found.
pub fn launch_image_suffixes(orientation: DeviceOrientation) -> &'static [&'static str] {
    match orientation {
        DeviceOrientation::Portrait => &["-Portrait"],
        DeviceOrientation::PortraitUpsideDown => &["-PortraitUpsideDown", "-Portrait"],
        DeviceOrientation::LandscapeLeft => &["-LandscapeRight", "-Landscape"],
        DeviceOrientation::LandscapeRight => &["-LandscapeLeft", "-Landscape"],
    }
}

/// Launch image file names for `base_name`, most specific first, ending with
/// the unqualified name.
///
/// Split out from [Bundle::launch_image_paths] because the ordering is the
/// part worth pinning, and it does not need a bundle on disk to check.
fn launch_image_names(base_name: &str, orientation_suffixes: &[&str]) -> Vec<String> {
    orientation_suffixes
        .iter()
        .map(|suffix| format!("{base_name}{suffix}.png"))
        .chain(std::iter::once(format!("{base_name}.png")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        device_orientation_for_interface_orientation, launch_image_names, launch_image_suffixes,
    };
    use crate::window::DeviceOrientation;

    /// The landscape pair is inverted between the interface and device
    /// vocabularies. It reads like a transcription error, so it is pinned.
    #[test]
    fn landscape_interface_orientations_map_to_the_opposite_device_orientation() {
        assert_eq!(
            device_orientation_for_interface_orientation("UIInterfaceOrientationLandscapeLeft"),
            Some(DeviceOrientation::LandscapeRight)
        );
        assert_eq!(
            device_orientation_for_interface_orientation("UIInterfaceOrientationLandscapeRight"),
            Some(DeviceOrientation::LandscapeLeft)
        );
    }

    /// Portrait is where the two vocabularies agree, so it must not pick up the
    /// inversion. `UIDeviceOrientationPortrait` appears in shipped plists that
    /// use the wrong prefix.
    #[test]
    fn portrait_interface_orientations_are_not_inverted() {
        assert_eq!(
            device_orientation_for_interface_orientation("UIInterfaceOrientationPortrait"),
            Some(DeviceOrientation::Portrait)
        );
        assert_eq!(
            device_orientation_for_interface_orientation("UIDeviceOrientationPortrait"),
            Some(DeviceOrientation::Portrait)
        );
        assert_eq!(
            device_orientation_for_interface_orientation(
                "UIInterfaceOrientationPortraitUpsideDown"
            ),
            Some(DeviceOrientation::PortraitUpsideDown)
        );
    }

    /// A name from outside iPhone OS's vocabulary is reported rather than
    /// crashing the app over a string in its Info.plist.
    #[test]
    fn an_unrecognised_orientation_is_not_an_orientation() {
        assert_eq!(
            device_orientation_for_interface_orientation("Sideways"),
            None
        );
        assert_eq!(device_orientation_for_interface_orientation(""), None);
    }

    /// The unqualified name must come last. It is the one every app has, so
    /// putting it anywhere else would mean a landscape app never reaches the
    /// landscape image it also ships — which is the bug this ordering fixes.
    #[test]
    fn the_unqualified_launch_image_is_the_last_resort() {
        assert_eq!(
            launch_image_names("Default", &["-LandscapeLeft", "-Landscape"]),
            [
                "Default-LandscapeLeft.png",
                "Default-Landscape.png",
                "Default.png"
            ]
        );
    }

    /// `UILaunchImageFile` renames the whole family, not just the unqualified
    /// file, so the modifiers hang off the app's own base name.
    #[test]
    fn a_custom_base_name_carries_the_orientation_modifiers() {
        assert_eq!(
            launch_image_names("Splash", &["-Landscape"]),
            ["Splash-Landscape.png", "Splash.png"]
        );
    }

    /// An app launching portrait has nothing more specific to ask for, and
    /// must still get a candidate list rather than an empty one.
    #[test]
    fn no_suffixes_still_yields_the_unqualified_name() {
        assert_eq!(launch_image_names("Default", &[]), ["Default.png"]);
    }

    /// The landscape modifier names the interface orientation, which is the
    /// opposite of the device orientation. An app asking for
    /// `UIInterfaceOrientationLandscapeLeft` gets a `LandscapeRight` device
    /// orientation from the environment; it must still be given the
    /// `-LandscapeLeft` image, or it launches upside down.
    #[test]
    fn the_landscape_modifier_names_the_interface_not_the_device() {
        assert_eq!(
            launch_image_suffixes(DeviceOrientation::LandscapeRight),
            ["-LandscapeLeft", "-Landscape"]
        );
        assert_eq!(
            launch_image_suffixes(DeviceOrientation::LandscapeLeft),
            ["-LandscapeRight", "-Landscape"]
        );
    }

    /// Portrait is the case where the two orientation kinds agree, so it must
    /// *not* pick up the inversion the landscape arms need.
    #[test]
    fn portrait_is_not_inverted() {
        assert_eq!(
            launch_image_suffixes(DeviceOrientation::Portrait),
            ["-Portrait"]
        );
        assert_eq!(
            launch_image_suffixes(DeviceOrientation::PortraitUpsideDown),
            ["-PortraitUpsideDown", "-Portrait"]
        );
    }

    /// The unqualified `-Landscape` modifier has to stay reachable from either
    /// landscape orientation: most landscape apps ship only that one, and it is
    /// what keeps them working when neither hand-specific name exists.
    #[test]
    fn plain_landscape_is_reachable_from_both_sides() {
        for orientation in [
            DeviceOrientation::LandscapeLeft,
            DeviceOrientation::LandscapeRight,
        ] {
            assert!(launch_image_suffixes(orientation).contains(&"-Landscape"));
        }
    }
}
