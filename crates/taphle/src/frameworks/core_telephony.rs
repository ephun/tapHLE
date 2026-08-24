/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! CoreTelephony device-information classes.

use crate::objc::{id, nil, objc_classes, ClassExports};

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/CoreTelephony.framework/CoreTelephony",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[],
    function_exports: &[],
};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// Windows hosts do not expose an iPhone cellular provider. A nil provider is
// the documented no-service result and lets apps use their existing fallback.
@implementation CTTelephonyNetworkInfo: NSObject

- (id)subscriberCellularProvider {
    nil
}

@end

};
