/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIEvent`.

use super::ui_touch::UITouchHostObject;
use crate::frameworks::foundation::{NSTimeInterval, NSUInteger};
use crate::mem::MutVoidPtr;
use crate::objc::{
    id, msg, msg_class, nil, objc_classes, release, ClassExports, HostObject, NSZonePtr,
};
use crate::Environment;

pub(super) struct UIEventHostObject {
    /// `NSSet<UITouch*>*`
    touches: id,
    timestamp: NSTimeInterval,
}
impl HostObject for UIEventHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIEvent: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIEventHostObject {
        touches: nil,
        timestamp: 0.0,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (())dealloc {
    let &UIEventHostObject { touches, .. } = env.objc.borrow(this);
    release(env, touches);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (NSTimeInterval)timestamp {
    env.objc.borrow::<UIEventHostObject>(this).timestamp
}

- (id)touchesForView:(id)view_ {
    let &UIEventHostObject { touches, .. } = env.objc.borrow(this);

    let touches_for_view: id = msg_class![env; NSMutableSet allocWithZone:(MutVoidPtr::null())];

    let touches_arr: id = msg![env; touches allObjects];
    let touches_count: NSUInteger = msg![env; touches_arr count];
    for i in 0..touches_count {
        let touch: id = msg![env; touches_arr objectAtIndex:i];
        let &UITouchHostObject { view, .. } = env.objc.borrow(touch);
        if view_ == view {
            let _: () = msg![env; touches_for_view addObject:touch];
            if !msg![env; view isMultipleTouchEnabled] {
                break;
            }
        }
    }

    touches_for_view
}

- (id)allTouches {
    let &UIEventHostObject { touches, .. } = env.objc.borrow(this);
    touches
}

// TODO: more accessors

@end

};

/// For use by [super::ui_touch]: create a `UIEvent` with a set of `UITouch*`.
///
/// Ownership of `touches` is transferred to the event. UIKit keeps one event
/// object for the lifetime of an active touch sequence, so callers update that
/// object as the set of active touches changes instead of allocating a new
/// event for every phase.
pub(super) fn new_event(env: &mut Environment, touches: id) -> id {
    let event: id = msg_class![env; UIEvent alloc];
    let timestamp: NSTimeInterval = {
        let process_info = msg_class![env; NSProcessInfo processInfo];
        msg![env; process_info systemUptime]
    };
    let borrow = env.objc.borrow_mut::<UIEventHostObject>(event);
    borrow.touches = touches;
    borrow.timestamp = timestamp;
    event
}

/// Replace the event's active touch snapshot while preserving its identity.
/// Ownership of `touches` is transferred to `event`.
pub(super) fn update_event(env: &mut Environment, event: id, touches: id) {
    let timestamp: NSTimeInterval = {
        let process_info = msg_class![env; NSProcessInfo processInfo];
        msg![env; process_info systemUptime]
    };
    let old_touches = {
        let host_object = env.objc.borrow_mut::<UIEventHostObject>(event);
        let old_touches = std::mem::replace(&mut host_object.touches, touches);
        host_object.timestamp = timestamp;
        old_touches
    };
    release(env, old_touches);
}
