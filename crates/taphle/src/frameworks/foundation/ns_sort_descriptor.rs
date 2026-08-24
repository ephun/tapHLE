/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSSortDescriptor`.
//!
//! Resources:
//! - Apple's [NSSortDescriptor](https://developer.apple.com/documentation/foundation/nssortdescriptor)

use crate::frameworks::foundation::NSComparisonResult;
use crate::objc::{
    autorelease, id, msg, msg_send, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr, SEL,
};
use crate::Environment;

struct NSSortDescriptorHostObject {
    /// `NSString*`, retained.
    key: id,
    ascending: bool,
    /// The comparison selector. Defaults to `compare:`.
    selector: Option<SEL>,
}
impl HostObject for NSSortDescriptorHostObject {}

/// Compare two objects with one descriptor, honouring its key, selector and
/// direction.
pub fn compare_with_descriptor(
    env: &mut Environment,
    descriptor: id,
    a: id,
    b: id,
) -> NSComparisonResult {
    let &NSSortDescriptorHostObject {
        key,
        ascending,
        selector,
    } = env.objc.borrow(descriptor);

    // A nil key means "compare the objects themselves", which is what
    // -sortedArrayUsingDescriptors: does for an array of plain values.
    let (a_value, b_value) = if key == nil {
        (a, b)
    } else {
        (msg![env; a valueForKey:key], msg![env; b valueForKey:key])
    };

    let selector = selector.unwrap_or_else(|| env.objc.lookup_selector("compare:").unwrap());
    let result: NSComparisonResult = msg_send(env, (a_value, selector, b_value));
    if ascending {
        result
    } else {
        -result
    }
}

/// Order two objects by a whole array of descriptors, earlier ones winning.
pub fn compare_with_descriptors(
    env: &mut Environment,
    descriptors: id,
    a: id,
    b: id,
) -> NSComparisonResult {
    let count: crate::frameworks::foundation::NSUInteger = msg![env; descriptors count];
    for i in 0..count {
        let descriptor: id = msg![env; descriptors objectAtIndex:i];
        let result = compare_with_descriptor(env, descriptor, a, b);
        if result != 0 {
            return result;
        }
    }
    0
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSSortDescriptor: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSSortDescriptorHostObject {
        key: nil,
        ascending: true,
        selector: None,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)sortDescriptorWithKey:(id)key // NSString*
                  ascending:(bool)ascending {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithKey:key ascending:ascending];
    autorelease(env, new)
}

- (id)initWithKey:(id)key // NSString*
        ascending:(bool)ascending {
    retain(env, key);
    let host_object = env.objc.borrow_mut::<NSSortDescriptorHostObject>(this);
    host_object.key = key;
    host_object.ascending = ascending;
    this
}

- (id)initWithKey:(id)key // NSString*
        ascending:(bool)ascending
         selector:(SEL)selector {
    let new: id = msg![env; this initWithKey:key ascending:ascending];
    env.objc.borrow_mut::<NSSortDescriptorHostObject>(new).selector = Some(selector);
    new
}

- (())dealloc {
    let key = env.objc.borrow::<NSSortDescriptorHostObject>(this).key;
    release(env, key);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)key {
    env.objc.borrow::<NSSortDescriptorHostObject>(this).key
}
- (bool)ascending {
    env.objc.borrow::<NSSortDescriptorHostObject>(this).ascending
}
- (SEL)selector {
    let selector = env.objc.borrow::<NSSortDescriptorHostObject>(this).selector;
    match selector {
        Some(selector) => selector,
        None => env.objc.lookup_selector("compare:").unwrap(),
    }
}

- (NSComparisonResult)compareObject:(id)a toObject:(id)b {
    compare_with_descriptor(env, this, a, b)
}

- (id)reversedSortDescriptor {
    let &NSSortDescriptorHostObject { key, ascending, selector } = env.objc.borrow(this);
    let class: crate::objc::Class = msg![env; this class];
    let new: id = msg![env; class alloc];
    let new: id = msg![env; new initWithKey:key ascending:(!ascending)];
    env.objc.borrow_mut::<NSSortDescriptorHostObject>(new).selector = selector;
    autorelease(env, new)
}

// NSCopying: descriptors are immutable, so a copy can be the same object.
- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

@end

};
