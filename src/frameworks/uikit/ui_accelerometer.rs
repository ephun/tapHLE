/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIAccelerometer`.
//!
//! Useful resources:
//! - [Apple's documentation for UIAcceleration](https://developer.apple.com/documentation/uikit/uiacceleration) has a really nice diagram of how the accelerometer axes relate to an iPhone.

use crate::frameworks::foundation::NSTimeInterval;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, ClassExports, HostObject,
    NSZonePtr, TrivialHostObject, SEL,
};
use crate::Environment;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct State {
    /// [UIAccelerometer sharedAccelerometer]
    shared_accelerometer: Option<id>,
    /// Something implementing UIAccelerometerDelegate, weak reference
    delegate: Option<id>,
    update_interval: Option<NSTimeInterval>,
    due_by: Option<Instant>,
}

type UIAccelerationValue = f64;

const DEFAULT_UPDATE_INTERVAL: f64 = 1.0 / 60.0;
/// An hour between accelerometer updates is already far outside anything a
/// game means by "slow"; beyond it the value is not a request, it is garbage.
const MAX_UPDATE_INTERVAL: f64 = 60.0 * 60.0;

struct UIAccelerationHostObject {
    x: UIAccelerationValue,
    y: UIAccelerationValue,
    z: UIAccelerationValue,
    timestamp: NSTimeInterval,
}
impl HostObject for UIAccelerationHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// This is a singleton.
@implementation UIAccelerometer: NSObject

+ (id)sharedAccelerometer {
    if let Some(accelerometer) =
        env.framework_state.uikit.ui_accelerometer.shared_accelerometer {
        accelerometer
    } else {
        let new = env.objc.alloc_static_object(
            this,
            Box::new(TrivialHostObject),
            &mut env.mem
        );
        env.framework_state.uikit.ui_accelerometer.shared_accelerometer = Some(new);
        new
   }
}
- (id)retain { this }
- (())release {}
- (id)autorelease { this }

// TODO: more accessors

- (id)delegate {
    env.framework_state.uikit.ui_accelerometer.delegate.unwrap_or(nil)
}
- (())setDelegate:(id)delegate {
    if delegate == nil {
        env.framework_state.uikit.ui_accelerometer.delegate = None;
    } else {
        if env.framework_state.uikit.ui_accelerometer.delegate != Some(delegate) {
            env.window().print_accelerometer_notice(&env.options);
        }
        env.framework_state.uikit.ui_accelerometer.delegate = Some(delegate);
    }
}

- (NSTimeInterval)updateInterval {
    env.framework_state.uikit.ui_accelerometer.update_interval.unwrap_or(DEFAULT_UPDATE_INTERVAL)
}
- (())setUpdateInterval:(NSTimeInterval)interval {
    // The system can limit this value, and must (some apps pass 0 and this can
    // cause a division-by-zero. 60Hz has been chosen here to match 60fps.
    //
    // The upper bound is here for the same reason as the lower one, and was
    // added for the same kind of app: a value too large to be a Duration at all
    // reached `Duration::from_secs_f64` in the run loop and ended the app on
    // the first accelerometer tick. Doodle Jump v3.1.1 and Doodle Jump HD both
    // pass one during start-up. A device clamps such a request to what its
    // hardware can do; tapHLE cannot guess what was meant, so it uses its
    // default and says which value it declined.
    let usable = usable_update_interval(interval);
    if usable != interval {
        log!(
            "Warning: -[UIAccelerometer setUpdateInterval:{}] is not an interval this device would produce; using {}s",
            interval, usable
        );
    }
    env.framework_state.uikit.ui_accelerometer.update_interval = Some(usable);
}

@end

@implementation UIAcceleration: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIAccelerationHostObject {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        timestamp: 0.0,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (UIAccelerationValue)x {
    env.objc.borrow::<UIAccelerationHostObject>(this).x
}
- (UIAccelerationValue)y {
    env.objc.borrow::<UIAccelerationHostObject>(this).y
}
- (UIAccelerationValue)z {
    env.objc.borrow::<UIAccelerationHostObject>(this).z
}
- (NSTimeInterval)timestamp {
    env.objc.borrow::<UIAccelerationHostObject>(this).timestamp
}

@end

};

/// The interval an accelerometer will actually be run at, given what an app
/// asked for.
///
/// Anything outside the range tapHLE can act on becomes the default rather
/// than being clamped to an edge, because a value like infinity is not a
/// request that was rounded off — it is one that says nothing about what the
/// app wanted.
fn usable_update_interval(interval: NSTimeInterval) -> NSTimeInterval {
    if interval.is_finite() && interval <= MAX_UPDATE_INTERVAL {
        interval.max(DEFAULT_UPDATE_INTERVAL)
    } else {
        DEFAULT_UPDATE_INTERVAL
    }
}

/// For use by `NSRunLoop` via [super::handle_events]: check if an accelerometer
/// update is due and send one if appropriate.
///
/// Returns the time an accelerometer update is due, if any.
pub(super) fn handle_accelerometer(env: &mut Environment) -> Option<Instant> {
    let state = &mut env.framework_state.uikit.ui_accelerometer;

    let delegate = state.delegate?;

    let ns_interval = state.update_interval.unwrap_or(DEFAULT_UPDATE_INTERVAL);
    let rust_interval = Duration::from_secs_f64(ns_interval);

    let now = Instant::now();
    if let Some(due_by) = state.due_by {
        if due_by > now {
            return Some(due_by);
        }

        // See NSTimer implementation for a discussion of what this does.
        // I don't know if iPhone OS uses this approach for accelerometer
        // updates, but there's no obvious reason not to.
        let overdue_by = now.duration_since(due_by);
        // TODO: Use `.div_duration_f64()` once that is stabilized.
        let advance_by = (overdue_by.as_secs_f64() / ns_interval).max(1.0).ceil();
        assert!(advance_by == (advance_by as u32) as f64);
        let advance_by = advance_by as u32;
        if advance_by > 1 {
            log_dbg!("Warning: Accelerometer is lagging. It is overdue by {}s and has missed {} interval(s)!", overdue_by.as_secs_f64(), advance_by - 1);
        }
        let advance_by = rust_interval.checked_mul(advance_by).unwrap();
        let new_due_by = due_by.checked_add(advance_by).unwrap();
        state.due_by = Some(new_due_by);
    } else {
        // In Resident Evil 4 the delegate is set before it fully initializes.
        // If the first message is sent immediately, it crashes.
        // This change prevents it by not sending the first message until the
        // time interval first passes
        let new_due_by = now.checked_add(rust_interval).unwrap();
        state.due_by = Some(new_due_by);
        if new_due_by > now {
            return Some(new_due_by);
        }
    };

    // UIKit creates and drains autorelease pools when handling events.
    let pool: id = msg_class![env; NSAutoreleasePool new];

    let (x, y, z) = env.window().get_acceleration(&env.options);
    let timestamp: NSTimeInterval = {
        let process_info = msg_class![env; NSProcessInfo processInfo];
        msg![env; process_info systemUptime]
    };
    let acceleration: id = msg_class![env; UIAcceleration alloc];
    *env.objc.borrow_mut(acceleration) = UIAccelerationHostObject {
        x: x.into(),
        y: y.into(),
        z: z.into(),
        timestamp,
    };
    autorelease(env, acceleration);

    let accelerometer: id = msg_class![env; UIAccelerometer sharedAccelerometer];

    log_dbg!(
        "Sending [{:?} accelerometer:{:?} didAccelerate:{:?}]",
        delegate,
        accelerometer,
        acceleration,
    );
    let sel: SEL = env
        .objc
        .register_host_selector("accelerometer:didAccelerate:".to_string(), &mut env.mem);
    let responds: bool = msg![env; delegate respondsToSelector:sel];
    if responds {
        let _: () = msg![env; delegate accelerometer:accelerometer
                                       didAccelerate:acceleration];
    }

    release(env, pool);

    env.framework_state.uikit.ui_accelerometer.due_by
}

#[cfg(test)]
mod tests {
    use super::{usable_update_interval, DEFAULT_UPDATE_INTERVAL, MAX_UPDATE_INTERVAL};

    #[test]
    fn an_ordinary_interval_is_left_alone() {
        assert_eq!(usable_update_interval(1.0 / 30.0), 1.0 / 30.0);
        assert_eq!(usable_update_interval(1.0), 1.0);
    }

    #[test]
    fn a_faster_interval_than_the_screen_is_slowed_to_it() {
        assert_eq!(usable_update_interval(0.0), DEFAULT_UPDATE_INTERVAL);
        assert_eq!(usable_update_interval(-1.0), DEFAULT_UPDATE_INTERVAL);
        assert_eq!(
            usable_update_interval(1.0 / 1000.0),
            DEFAULT_UPDATE_INTERVAL
        );
    }

    /// The one that ended two apps: `Duration::from_secs_f64` panics on any of
    /// these, and the run loop reaches it on the first accelerometer tick.
    #[test]
    fn an_interval_that_is_not_a_duration_becomes_the_default() {
        assert_eq!(
            usable_update_interval(f64::INFINITY),
            DEFAULT_UPDATE_INTERVAL
        );
        assert_eq!(
            usable_update_interval(f64::NEG_INFINITY),
            DEFAULT_UPDATE_INTERVAL
        );
        assert_eq!(usable_update_interval(f64::NAN), DEFAULT_UPDATE_INTERVAL);
        assert_eq!(usable_update_interval(f64::MAX), DEFAULT_UPDATE_INTERVAL);
        assert_eq!(
            usable_update_interval(MAX_UPDATE_INTERVAL + 1.0),
            DEFAULT_UPDATE_INTERVAL
        );
    }
}
