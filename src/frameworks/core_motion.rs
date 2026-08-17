/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The Core Motion framework.

use crate::dyld::HostDylib;
use crate::frameworks::foundation::NSTimeInterval;
use crate::objc::{id, nil, objc_classes, ClassExports};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/CoreMotion.framework/CoreMotion",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[],
    function_exports: &[],
};

const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation CMMotionManager: NSObject

- (bool)isGyroAvailable {
    // It could make sense to implement gyroscope support at least for Android.
    log!("TODO: [(CMMotionManager *){:?} isGyroAvailable] -> false", this);
    false
}
- (bool)isDeviceMotionAvailable {
    log!("TODO: [(CMMotionManager *){:?} isDeviceMotionAvailable] -> false", this);
    // According to docs, this is functionally equivalent to `isGyroAvailable`
    // method. (All devices have accelerometer, but only some do have gyro).
    false
}
- (bool)isAccelerometerAvailable {
    // According to https://developer.apple.com/documentation/coremotion/getting-raw-accelerometer-events?language=objc,
    // every iOS device has an accelerometer, but on real hardware this method
    // can still return false if the device isn't ready to produce data yet.
    // Here we always return true since we don't model that readiness state.
    true
}

// Reading the sensor data is separate from asking whether the sensor exists,
// and a caller is entitled to do the second without the first. Unity does
// exactly that: it checks availability once at startup, then reads `gyroData`
// and `deviceMotion` unconditionally every frame and branches on whether the
// returned object is nil. Apple specifies nil as the no-data answer for both
// ("If no gyroscope data is available, the value of this property is nil"), so
// a device with no gyroscope is fully described by the methods below. This is
// how an iPhone 3GS behaves, not a stub standing in for something better.
//
// Producing real data would mean more than filling these in: the caller starts
// updates through `startAccelerometerUpdatesToQueue:withHandler:`, which needs
// block dispatch onto an NSOperationQueue. tapHLE has no Core Motion data
// source yet, so the start/stop methods below are honest no-ops rather than
// pretending to begin something.

- (bool)isGyroActive {
    // Updates never start on a device with no gyroscope, so this stays false
    // even after `startGyroUpdates`.
    false
}

- (id)gyroData {
    nil
}
- (id)deviceMotion {
    nil
}

- (())setGyroUpdateInterval:(NSTimeInterval)_interval {}
- (())setDeviceMotionUpdateInterval:(NSTimeInterval)_interval {}
- (())setAccelerometerUpdateInterval:(NSTimeInterval)_interval {}

- (())startGyroUpdates {}
- (())stopGyroUpdates {}
- (())startDeviceMotionUpdates {}
- (())stopDeviceMotionUpdates {}
- (())startAccelerometerUpdatesToQueue:(id)_queue
                          withHandler:(id)_handler {
    log_once!(
        "TODO: [CMMotionManager startAccelerometerUpdatesToQueue:withHandler:] \
         is a no-op; tapHLE delivers accelerometer data through UIAccelerometer \
         only, so an app reading it through Core Motion will see no motion."
    );
}
- (())stopAccelerometerUpdates {}

@end

};
