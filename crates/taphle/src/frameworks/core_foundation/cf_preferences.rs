/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFPreferences`.
//!
//! According to Apple's docs, it's not toll-free bridged to `NSUserDefaults`,
//! but we are still implementing one atop of another.

use super::cf_string::CFStringRef;
use super::CFTypeRef;
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::frameworks::foundation::ns_string;
use crate::objc::{id, msg, msg_class, nil};
use crate::Environment;

type CFPropertyListRef = CFTypeRef;

/// Whether `app_id` names the running app's own preferences domain.
///
/// `kCFPreferencesCurrentApplication` is the usual spelling, but naming your
/// own bundle identifier means the same thing and is just as common. Any other
/// domain belongs to a different app, which the sandbox puts out of reach on a
/// device too, so there is nothing to read or write and saying so is the
/// honest answer rather than a reason to stop.
fn is_current_application(env: &mut Environment, app_id: CFStringRef) -> bool {
    let current_app = ns_string::get_static_str(env, kCFPreferencesCurrentApplication);
    if msg![env; app_id isEqualToString:current_app] {
        return true;
    }
    let main_bundle: id = msg_class![env; NSBundle mainBundle];
    let bundle_id: id = msg![env; main_bundle bundleIdentifier];
    let matches: bool = bundle_id != nil && msg![env; app_id isEqualToString:bundle_id];
    if !matches {
        log!("TODO: CFPreferences for another application's domain, ignoring");
    }
    matches
}

fn CFPreferencesCopyAppValue(
    env: &mut Environment,
    key: CFStringRef,
    app_id: CFStringRef,
) -> CFPropertyListRef {
    if !is_current_application(env, app_id) {
        return nil;
    }
    let user_defaults: id = msg_class![env; NSUserDefaults standardUserDefaults];
    let value: id = msg![env; user_defaults objectForKey:key];
    msg![env; value copy]
}

fn CFPreferencesSetAppValue(
    env: &mut Environment,
    key: CFStringRef,
    value: CFPropertyListRef,
    app_id: CFStringRef,
) {
    assert!(!value.is_null()); // TODO
    if !is_current_application(env, app_id) {
        return;
    }
    let user_defaults: id = msg_class![env; NSUserDefaults standardUserDefaults];
    msg![env; user_defaults setObject:value forKey:key]
}

fn CFPreferencesAppSynchronize(env: &mut Environment, app_id: CFStringRef) -> bool {
    if !is_current_application(env, app_id) {
        // Nothing was written to that domain, so nothing failed to be flushed.
        return true;
    }
    let user_defaults: id = msg_class![env; NSUserDefaults standardUserDefaults];
    msg![env; user_defaults synchronize]
}

pub const kCFPreferencesCurrentApplication: &str = "kCFPreferencesCurrentApplication";

pub const CONSTANTS: ConstantExports = &[(
    "_kCFPreferencesCurrentApplication",
    HostConstant::NSString(kCFPreferencesCurrentApplication),
)];

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CFPreferencesCopyAppValue(_, _)),
    export_c_func!(CFPreferencesSetAppValue(_, _, _)),
    export_c_func!(CFPreferencesAppSynchronize(_)),
];
