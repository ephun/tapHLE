/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::objc::{autorelease, id, msg, objc_classes, ClassExports};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// NSMachPort backs the main-thread run-loop integration used by older
// networking libraries. Port delivery is not implemented yet, but callers
// still need a valid port object to register with their run loop.
@implementation NSMachPort: NSObject

+ (id)port {
    let port: id = msg![env; this new];
    autorelease(env, port)
}

@end

};
