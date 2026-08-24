/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSCountedSet`.
//!
//! A mutable set that remembers how many times each object was added. Adding an
//! object already present raises its count instead of being a no-op, and
//! removing it lowers the count, the object leaving the set only when the count
//! reaches zero.
//!
//! Resources:
//! - Apple's [NSCountedSet](https://developer.apple.com/documentation/foundation/nscountedset)

use crate::frameworks::foundation::{ns_array, NSUInteger};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use crate::Environment;

#[derive(Default)]
struct NSCountedSetHostObject {
    /// Each distinct object and how many times it has been added. A vector
    /// rather than a hash map because membership is defined by `isEqual:`,
    /// which has to run guest code and so cannot be used to hash.
    entries: Vec<(id, NSUInteger)>,
}
impl HostObject for NSCountedSetHostObject {}

/// Index of the entry equal to `object`, by `isEqual:`.
fn index_of(env: &mut Environment, set: id, object: id) -> Option<usize> {
    let entries: Vec<id> = env
        .objc
        .borrow::<NSCountedSetHostObject>(set)
        .entries
        .iter()
        .map(|&(member, _)| member)
        .collect();
    for (index, member) in entries.into_iter().enumerate() {
        let equal: bool = msg![env; object isEqual:member];
        if equal {
            return Some(index);
        }
    }
    None
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSCountedSet: NSMutableSet

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<NSCountedSetHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    this
}

- (id)initWithCapacity:(NSUInteger)capacity {
    env.objc
        .borrow_mut::<NSCountedSetHostObject>(this)
        .entries
        .reserve(capacity as usize);
    this
}

- (id)initWithArray:(id)array { // NSArray*
    let count: NSUInteger = msg![env; array count];
    for i in 0..count {
        let object: id = msg![env; array objectAtIndex:i];
        () = msg![env; this addObject:object];
    }
    this
}

- (id)initWithSet:(id)set { // NSSet*
    let objects: id = msg![env; set allObjects];
    msg![env; this initWithArray:objects]
}

- (())dealloc {
    let entries = std::mem::take(&mut env.objc.borrow_mut::<NSCountedSetHostObject>(this).entries);
    for (object, _) in entries {
        release(env, object);
    }
    env.objc.dealloc_object(this, &mut env.mem)
}

- (NSUInteger)count {
    // The number of distinct objects, not the sum of the counts.
    env.objc.borrow::<NSCountedSetHostObject>(this).entries.len() as NSUInteger
}

- (NSUInteger)countForObject:(id)object {
    match index_of(env, this, object) {
        Some(index) => env.objc.borrow::<NSCountedSetHostObject>(this).entries[index].1,
        None => 0,
    }
}

- (())addObject:(id)object {
    if let Some(index) = index_of(env, this, object) {
        env.objc.borrow_mut::<NSCountedSetHostObject>(this).entries[index].1 += 1;
        return;
    }
    retain(env, object);
    env.objc
        .borrow_mut::<NSCountedSetHostObject>(this)
        .entries
        .push((object, 1));
}

- (())removeObject:(id)object {
    let Some(index) = index_of(env, this, object) else {
        return;
    };
    let entry = &mut env.objc.borrow_mut::<NSCountedSetHostObject>(this).entries[index];
    entry.1 -= 1;
    if entry.1 > 0 {
        return;
    }
    let (member, _) = env
        .objc
        .borrow_mut::<NSCountedSetHostObject>(this)
        .entries
        .remove(index);
    release(env, member);
}

- (())removeAllObjects {
    let entries = std::mem::take(&mut env.objc.borrow_mut::<NSCountedSetHostObject>(this).entries);
    for (object, _) in entries {
        release(env, object);
    }
}

- (id)member:(id)object {
    match index_of(env, this, object) {
        Some(index) => env.objc.borrow::<NSCountedSetHostObject>(this).entries[index].0,
        None => nil,
    }
}

- (bool)containsObject:(id)object {
    index_of(env, this, object).is_some()
}

- (id)anyObject {
    let entries = &env.objc.borrow::<NSCountedSetHostObject>(this).entries;
    entries.first().map_or(nil, |&(object, _)| object)
}

// Each distinct object appears once, which is what NSSet's accessors report;
// the multiplicity is only visible through -countForObject:.
- (id)allObjects {
    let objects: Vec<id> = env
        .objc
        .borrow::<NSCountedSetHostObject>(this)
        .entries
        .iter()
        .map(|&(object, _)| object)
        .collect();
    ns_array::from_vec(env, objects)
}

- (id)objectEnumerator { // NSEnumerator*
    let array: id = msg![env; this allObjects];
    msg![env; array objectEnumerator]
}

- (id)copyWithZone:(NSZonePtr)_zone {
    let objects: id = msg![env; this allObjects];
    let new: id = msg_class![env; NSSet alloc];
    msg![env; new initWithArray:objects]
}

- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    let objects: id = msg![env; this allObjects];
    let new: id = msg_class![env; NSMutableSet alloc];
    msg![env; new initWithArray:objects]
}

- (id)description {
    let objects: id = msg![env; this allObjects];
    let description: id = msg![env; objects description];
    autorelease(env, description);
    description
}

@end

};
