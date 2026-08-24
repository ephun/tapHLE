/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSProcessInfo`.

use super::{NSTimeInterval, NSUInteger};
use crate::frameworks::foundation::ns_string;
use crate::libc::mach::host::PHYSICAL_MEMORY;
use crate::objc::{autorelease, id, msg, msg_class, objc_classes, ClassExports};
use crate::Environment;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Default)]
pub struct State {
    /// `NSProcessInfo*`
    process_info: Option<id>,
    next_unique_string: u64,
}

fn format_unique_string(timestamp_nanos: u128, process_id: u32, sequence: u64) -> String {
    format!("tapHLE-{timestamp_nanos:x}-{process_id:x}-{sequence:x}")
}

fn assert_process_info_singleton(env: &mut Environment, this: id) {
    assert_eq!(
        this,
        env.framework_state
            .foundation
            .ns_process_info
            .process_info
            .unwrap()
    );
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSProcessInfo: NSObject

+ (id)processInfo {
    if let Some(existing) = env.framework_state.foundation.ns_process_info.process_info {
        existing
    } else {
        let process_info: id = msg![env; this new];
        env.framework_state.foundation.ns_process_info.process_info = Some(process_info);
        process_info
    }
}

- (NSTimeInterval)systemUptime {
    assert_process_info_singleton(env, this); // TODO
    Instant::now().duration_since(env.startup_time).as_secs_f64()
}

- (u64)physicalMemory {
    assert_process_info_singleton(env, this); // TODO
    PHYSICAL_MEMORY.into()
}

// The devices tapHLE emulates are single-core, and its guest scheduler runs one
// guest thread at a time regardless, so reporting one is both the historically
// accurate answer and the one that matches what actually happens. Apps use this
// to size worker pools; a larger number would create threads that cannot run in
// parallel anyway.
- (NSUInteger)processorCount {
    assert_process_info_singleton(env, this); // TODO
    1
}
- (NSUInteger)activeProcessorCount {
    assert_process_info_singleton(env, this); // TODO
    1
}

- (i32)processIdentifier {
    assert_process_info_singleton(env, this); // TODO
    crate::libc::unistd::getpid(env)
}

- (id)environment {
    assert_process_info_singleton(env, this); // TODO
    msg_class![env; NSDictionary dictionary]
}

- (id)processName {
    // This function probably just needs to return a unique value
    // Testing on macOS appears CFBundleName is used
    assert_process_info_singleton(env, this); // TODO
    let main_bundle: id = msg_class![env; NSBundle mainBundle];
    let name_key: id = ns_string::get_static_str(env, "CFBundleName");
    msg![env; main_bundle objectForInfoDictionaryKey:name_key]
}

- (id)operatingSystemVersionString {
    assert_process_info_singleton(env, this); // TODO
    ns_string::get_static_str(env, crate::frameworks::OPERATING_SYSTEM_VERSION_STRING)
}

- (id)globallyUniqueString {
    assert_process_info_singleton(env, this); // TODO
    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("host clock is before the Unix epoch")
        .as_nanos();
    let sequence = env
        .framework_state
        .foundation
        .ns_process_info
        .next_unique_string;
    env.framework_state
        .foundation
        .ns_process_info
        .next_unique_string += 1;
    let string = format_unique_string(timestamp_nanos, std::process::id(), sequence);
    let string = ns_string::from_rust_string(env, string);
    autorelease(env, string)
}

@end

};

#[cfg(test)]
mod tests {
    use super::format_unique_string;

    #[test]
    fn globally_unique_strings_change_with_the_sequence() {
        let first = format_unique_string(1234, 42, 0);
        let second = format_unique_string(1234, 42, 1);

        assert_ne!(first, second);
        assert!(first.starts_with("tapHLE-"));
    }
}
