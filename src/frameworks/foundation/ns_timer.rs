/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSTimer`.

use super::ns_run_loop::NSDefaultRunLoopMode;
use super::NSTimeInterval;
use super::{ns_run_loop, ns_string};
use crate::objc::{
    autorelease, id, msg, msg_class, msg_send, nil, objc_classes, release, retain, ClassExports,
    HostObject, NSZonePtr, SEL,
};
use crate::Environment;
use std::time::{Duration, Instant};

struct NSTimerHostObject {
    ns_interval: NSTimeInterval,
    /// Copy of `ns_interval` in Rust's type for time intervals. Keep in sync!
    rust_interval: Duration,
    /// Strong reference
    target: id,
    selector: SEL,
    /// Strong reference
    user_info: id,
    repeats: bool,
    due_by: Option<Instant>,
    /// If the timer is currently running its callback, this is set so that the
    /// re-entering the run loop from inside the callback doesn't cause an
    /// infinite loop.
    is_running_callback: bool,
    /// Weak reference
    run_loop: id,
}
impl HostObject for NSTimerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// NSTimer doesn't seem to be an abstract class?
@implementation NSTimer: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    // Only needed for the alloc/init route below; the convenience constructors
    // build their host object directly.
    let host_object = Box::new(NSTimerHostObject {
        ns_interval: 0.0,
        rust_interval: Duration::ZERO,
        target: nil,
        selector: env.objc.lookup_selector("fire").unwrap(),
        user_info: nil,
        repeats: false,
        due_by: None,
        run_loop: nil,
        is_running_callback: false,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)timerWithTimeInterval:(NSTimeInterval)ns_interval
                     target:(id)target
                   selector:(SEL)selector
                   userInfo:(id)user_info
                    repeats:(bool)repeats {
    let ns_interval = ns_interval.max(0.0001);
    let rust_interval = Duration::from_secs_f64(ns_interval);

    retain(env, target);
    retain(env, user_info);

    let host_object = Box::new(NSTimerHostObject {
        ns_interval,
        rust_interval,
        target,
        selector,
        user_info,
        repeats,
        due_by: Some(Instant::now().checked_add(rust_interval).unwrap()),
        run_loop: nil,
        is_running_callback: false
    });
    let new = env.objc.alloc_object(this, host_object, &mut env.mem);

    log_dbg!(
        "New {} timer {:?}, interval {}s, target [{:?} {}], user info {:?}",
        if repeats { "repeating" } else { "single-use" },
        new,
        ns_interval,
        target,
        selector.as_str(&env.mem),
        user_info,
    );

    autorelease(env, new)
}

+ (id)scheduledTimerWithTimeInterval:(NSTimeInterval)interval
                              target:(id)target
                            selector:(SEL)selector
                            userInfo:(id)user_info
                             repeats:(bool)repeats {
    let timer = msg![env; this timerWithTimeInterval:interval
                                              target:target
                                            selector:selector
                                            userInfo:user_info
                                             repeats:repeats];

    let run_loop: id = msg_class![env; NSRunLoop currentRunLoop];
    let mode: id = ns_string::get_static_str(env, NSDefaultRunLoopMode);
    let _: () = msg![env; run_loop addTimer:timer forMode:mode];

    timer
}

// The designated initializer, and the only route that lets a caller say *when*
// the timer first fires rather than "one interval from now".
//
// A timer made this way is not scheduled; the caller adds it to a run loop.
// That is the whole reason to use this instead of the convenience constructors,
// and it is why the fire date matters: the delay until the first fire is
// measured from now, so a date already in the past makes the timer due
// immediately, as it does on Apple's.
- (id)initWithFireDate:(id)fire_date // NSDate*
              interval:(NSTimeInterval)ns_interval
                target:(id)target
              selector:(SEL)selector
              userInfo:(id)user_info
               repeats:(bool)repeats {
    let seconds_from_now: NSTimeInterval = if fire_date == nil {
        0.0
    } else {
        msg![env; fire_date timeIntervalSinceNow]
    };

    // The repeat interval is a separate thing from the first fire, and is only
    // meaningful for a repeating timer. Clamping matches the convenience
    // constructors: a zero or negative interval would otherwise fire in a tight
    // loop.
    let ns_interval = ns_interval.max(0.0001);
    let rust_interval = Duration::from_secs_f64(ns_interval);

    retain(env, target);
    retain(env, user_info);

    let due_by = Instant::now().checked_add(Duration::from_secs_f64(seconds_from_now.max(0.0)));

    let host_object = env.objc.borrow_mut::<NSTimerHostObject>(this);
    host_object.ns_interval = ns_interval;
    host_object.rust_interval = rust_interval;
    host_object.target = target;
    host_object.selector = selector;
    host_object.user_info = user_info;
    host_object.repeats = repeats;
    host_object.due_by = due_by;

    log_dbg!(
        "New unscheduled {} timer {:?}, first fire in {}s, interval {}s, target [{:?} {}]",
        if repeats { "repeating" } else { "single-use" },
        this,
        seconds_from_now,
        ns_interval,
        target,
        selector.as_str(&env.mem),
    );

    this
}

- (())dealloc {
    let &NSTimerHostObject {
        target,
        user_info,
        ..
    } = env.objc.borrow(this);
    release(env, target);
    release(env, user_info);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (NSTimeInterval)timeInterval {
    let host_object = env.objc.borrow::<NSTimerHostObject>(this);
    if host_object.repeats {
        host_object.ns_interval
    } else {
        0.0 // this is the documented behaviour!
    }
}
- (id)userInfo {
    env.objc.borrow::<NSTimerHostObject>(this).user_info
}
- (bool)isValid {
    env.objc.borrow::<NSTimerHostObject>(this).due_by.is_some()
}

- (())invalidate {
    let timer = env.objc.borrow_mut::<NSTimerHostObject>(this);
    // Timer might already be invalid, don't try to remove it twice.
    if timer.due_by.take().is_some() {
        let run_loop = timer.run_loop;
        ns_run_loop::remove_timer(env, run_loop, this);
    }
}

- (())fire {
    let &NSTimerHostObject {
        target,
        selector,
        repeats,
        ..
    } = env.objc.borrow(this);

    let pool: id = msg_class![env; NSAutoreleasePool new];

    // Signature should be `- (void)timerDidFire:(NSTimer *)which`.
    let _: () = msg_send(env, (target, selector, this));

    release(env, pool);

    if !repeats {
        () = msg![env; this invalidate];
    }
}

// TODO: more constructors
// TODO: more accessors

@end

};

/// For use by `CADisplayLink`
pub fn set_time_interval(env: &mut Environment, timer: id, interval: NSTimeInterval) {
    let host_object = env.objc.borrow_mut::<NSTimerHostObject>(timer);
    host_object.ns_interval = interval;
    host_object.rust_interval = Duration::from_secs_f64(interval);
}

/// For use by `NSRunLoop`. Returns whether this run loop now owns the timer;
/// `false` means another one already does.
///
/// A timer may be scheduled on more than one run loop and fires on each, but
/// only one is modelled here. The first registration is the one that stands:
/// the timer is already firing, which is what the app asked for, and taking it
/// over would move it to a loop that may be run less often or not at all.
pub(super) fn set_run_loop(env: &mut Environment, timer: id, run_loop: id) -> bool {
    let host_object = env.objc.borrow_mut::<NSTimerHostObject>(timer);
    if host_object.run_loop != nil && host_object.run_loop != run_loop {
        return false;
    }
    host_object.run_loop = run_loop;
    true
}

/// For use by `NSRunLoop`: check if a timer is due to fire and fire it if
/// necessary.
///
/// Returns the next firing time, if any.
pub(super) fn handle_timer(env: &mut Environment, timer: id) -> Option<Instant> {
    let &NSTimerHostObject {
        ns_interval,
        rust_interval,
        target,
        selector,
        repeats,
        due_by,
        is_running_callback,
        run_loop,
        ..
    } = env.objc.borrow(timer);

    // If a timer is already running its callback, we don't want to re-enter it
    // and cause an infinite loop.
    if is_running_callback {
        return None;
    }

    // Invalidated timers should be removed from the run loop, but if a timer
    // is invalidated from another timer earlier in the current tick of the
    // run loop it might still be run.
    let due_by = due_by?;

    let now = Instant::now();

    if due_by > now {
        return Some(due_by);
    }

    let overdue_by = now.duration_since(due_by);

    // Timer may be released when it's invalidated, so we need to retain it so
    // it's still around to pass to the timer target.
    retain(env, timer);

    // Advancing the timer before sending its message seems like a good idea
    // considering this function is potentially re-entrant.
    let new_due_by = if repeats {
        // When rescheduling a repeating timer, the next firing should be based
        // on when the timer should have fired, not when it actually fired, so
        // that there is no drift over time.
        //
        // For example, if a timer has an interval of 60s and starts at 00:00,
        // the first firing would be scheduled for 01:00, and the second firing
        // should be scheduled for 02:00, even if the first firing was at 01:01.
        //
        // However: if the timer handling is delayed past a whole interval, it
        // should not try to catch up. For example, if the first firing is
        // scheduled for 01:00 but happens at 02:30, then the next firing should
        // be scheduled for 03:00.
        // TODO: Use `.div_duration_f64()` once that is stabilized.
        let advance_by = (overdue_by.as_secs_f64() / ns_interval).max(1.0).ceil();
        assert!(advance_by == (advance_by as u32) as f64);
        let advance_by = advance_by as u32;
        if advance_by > 1 {
            log_dbg!("Warning: Timer {:?} is lagging. It is overdue by {}s and has missed {} interval(s)!", timer, overdue_by.as_secs_f64(), advance_by - 1);
        }
        let advance_by = rust_interval.checked_mul(advance_by).unwrap();
        Some(due_by.checked_add(advance_by).unwrap())
    } else {
        ns_run_loop::remove_timer(env, run_loop, timer);
        None
    };
    env.objc.borrow_mut::<NSTimerHostObject>(timer).due_by = new_due_by;
    env.objc
        .borrow_mut::<NSTimerHostObject>(timer)
        .is_running_callback = true;

    log_dbg!(
        "Timer {:?} fired, sending {:?} message to {:?}",
        timer,
        selector.as_str(&env.mem),
        target
    );

    let pool: id = msg_class![env; NSAutoreleasePool new];

    // Signature should be `- (void)timerDidFire:(NSTimer *)which`.
    let _: () = msg_send(env, (target, selector, timer));

    env.objc
        .borrow_mut::<NSTimerHostObject>(timer)
        .is_running_callback = false;

    release(env, timer);
    release(env, pool);

    new_due_by
}
