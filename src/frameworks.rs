/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Our implementations of various frameworks.
//!
//! Each child module should be named after the framework it implements.
//! It can potentially have multiple child modules itself if it's a particularly
//! complex framework.
//!
//! See also `dyld/function_lists.rs` and `objc/classes/class_lists.rs`.
//!
//! Most modules in here are not going to link to documentation that should be
//! trivial to find by searching for the class or function name. For example,
//! the documentation of `NSArray` won't link to the main developer.apple.com
//! page documenting that class, but if there's something interesting in the
//! Documentation Archive relating to arrays, that might be linked.

#![allow(non_upper_case_globals)] // Lots of Apple constants begin with "k"
#![allow(clippy::enum_variant_names)] // Lots of Apple enums have the same prefix
#![allow(clippy::too_many_arguments)] // It's not our fault!

/// The iPhone OS version tapHLE reports to guest apps, as a literal.
///
/// A macro rather than a `const` so that it can also be `concat!`-ed into the
/// longer forms other frameworks report, which needs a literal.
macro_rules! system_version {
    () => {
        "2.0"
    };
}

/// The iPhone OS version tapHLE reports to guest apps.
///
/// Several frameworks answer this question — `-[UIDevice systemVersion]` and
/// `-[NSProcessInfo operatingSystemVersionString]` among them — and an app may
/// read more than one. They must agree: an app that gate-keeps a feature on one
/// and a workaround on the other would otherwise see a device that does not
/// exist. Kept here rather than in either framework because neither owns the
/// answer.
pub const SYSTEM_VERSION: &str = system_version!();

/// The same version in the longer form `-[NSProcessInfo
/// operatingSystemVersionString]` uses.
///
/// Real iPhone OS names a build too, as in "Version 4.3.3 (Build 8J2)". tapHLE
/// has no build to name, and inventing one would be a detail an app could go
/// looking for, so the version stands alone — still well-formed, and still what
/// a version parser reads.
pub const OPERATING_SYSTEM_VERSION_STRING: &str = concat!("Version ", system_version!());

pub mod audio_toolbox;
pub mod avfoundation;
pub mod carbon_core;
pub mod cfnetwork;
pub mod core_animation;
pub mod core_audio_types;
pub mod core_foundation;
pub mod core_graphics;
pub mod core_location;
pub mod core_motion;
pub mod core_telephony;
pub mod foundation;
pub mod game_kit;
pub mod iad;
pub mod media_player;
pub mod message_ui;
pub mod openal;
pub mod opengles;
pub mod security;
pub mod store_kit;
pub mod system_configuration;
pub mod uikit;

/// Container for state of various child modules
#[derive(Default)]
pub struct State {
    avfoundation: avfoundation::State,
    audio_toolbox: audio_toolbox::State,
    core_animation: core_animation::State,
    foundation: foundation::State,
    media_player: media_player::State,
    openal: openal::State,
    opengles: opengles::State,
    pub security: security::State,
    uikit: uikit::State,
}

impl State {
    pub(crate) fn take_triggered_frame_capture(&mut self) -> Option<opengles::FrameCaptureRequest> {
        self.opengles.take_triggered_frame_capture()
    }

    pub(crate) fn rearm_frame_capture(&mut self, request: opengles::FrameCaptureRequest) {
        self.opengles.rearm_frame_capture(request);
    }
}

/// Container for thread local state of various child modules
#[derive(Default)]
pub struct ThreadLocalState {
    foundation: foundation::ThreadLocalState,
    core_animation: core_animation::ThreadLocalState,
}
