/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSIndexPath`, and UIKit's `row`/`section` category on it.
//!
//! Foundation's version is a general list of indexes into nested collections.
//! UIKit adds a two-element interpretation — section then row — and in practice
//! that is the only one apps of this era use, because the class exists here to
//! address a table view cell.
//!
//! Both are provided, and `-row` and `-section` are defined in terms of the
//! general indexes rather than stored alongside them, so an index path built
//! one way and read back the other agrees.
//!
//! Resources:
//! - Apple's [NSIndexPath](https://developer.apple.com/documentation/foundation/nsindexpath)

use super::{NSComparisonResult, NSNotFound, NSUInteger};
use crate::mem::{ConstPtr, MutPtr};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, retain, Class, ClassExports, HostObject,
    NSZonePtr,
};
use crate::Environment;

#[derive(Default)]
struct NSIndexPathHostObject {
    indexes: Vec<NSUInteger>,
}
impl HostObject for NSIndexPathHostObject {}

/// `(section, row)`, the UIKit interpretation, with 0 for anything absent.
pub fn section_and_row(env: &Environment, index_path: id) -> (NSUInteger, NSUInteger) {
    let indexes = &env.objc.borrow::<NSIndexPathHostObject>(index_path).indexes;
    (
        indexes.first().copied().unwrap_or(0),
        indexes.get(1).copied().unwrap_or(0),
    )
}

/// Build an index path from UIKit's `(row, section)` pair, for UIKit's own use.
pub fn for_row_in_section(env: &mut Environment, row: NSUInteger, section: NSUInteger) -> id {
    let new: id = msg_class![env; NSIndexPath alloc];
    env.objc
        .borrow_mut::<NSIndexPathHostObject>(new)
        .indexes
        .extend_from_slice(&[section, row]);
    autorelease(env, new)
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSIndexPath: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<NSIndexPathHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)indexPathWithIndex:(NSUInteger)index {
    let new: id = msg![env; this alloc];
    env.objc.borrow_mut::<NSIndexPathHostObject>(new).indexes.push(index);
    autorelease(env, new)
}

+ (id)indexPathWithIndexes:(ConstPtr<NSUInteger>)indexes
                    length:(NSUInteger)length {
    let new: id = msg![env; this alloc];
    let mut collected = Vec::with_capacity(length as usize);
    for i in 0..length {
        collected.push(env.mem.read(indexes + i));
    }
    env.objc.borrow_mut::<NSIndexPathHostObject>(new).indexes = collected;
    autorelease(env, new)
}

// UIKit's category. The argument order is row-then-section but the stored order
// is section-then-row, because an index path reads outermost first.
+ (id)indexPathForRow:(NSUInteger)row
            inSection:(NSUInteger)section {
    let new: id = msg![env; this alloc];
    env.objc.borrow_mut::<NSIndexPathHostObject>(new).indexes.extend_from_slice(&[section, row]);
    autorelease(env, new)
}

- (id)initWithIndex:(NSUInteger)index {
    env.objc.borrow_mut::<NSIndexPathHostObject>(this).indexes = vec![index];
    this
}

- (NSUInteger)length {
    env.objc.borrow::<NSIndexPathHostObject>(this).indexes.len().try_into().unwrap()
}

// NSNotFound for an out-of-range position, which is what Apple documents rather
// than an exception, and what callers check for.
- (NSUInteger)indexAtPosition:(NSUInteger)position {
    env.objc
        .borrow::<NSIndexPathHostObject>(this)
        .indexes
        .get(position as usize)
        .copied()
        .unwrap_or(NSNotFound as NSUInteger)
}

- (())getIndexes:(MutPtr<NSUInteger>)out {
    let indexes = env.objc.borrow::<NSIndexPathHostObject>(this).indexes.clone();
    for (i, index) in indexes.into_iter().enumerate() {
        env.mem.write(out + TryInto::<u32>::try_into(i).unwrap(), index);
    }
}

- (NSUInteger)section {
    section_and_row(env, this).0
}
- (NSUInteger)row {
    section_and_row(env, this).1
}

- (id)indexPathByAddingIndex:(NSUInteger)index {
    let mut indexes = env.objc.borrow::<NSIndexPathHostObject>(this).indexes.clone();
    indexes.push(index);
    let class: Class = msg![env; this class];
    let new: id = msg![env; class alloc];
    env.objc.borrow_mut::<NSIndexPathHostObject>(new).indexes = indexes;
    autorelease(env, new)
}

- (id)indexPathByRemovingLastIndex {
    let mut indexes = env.objc.borrow::<NSIndexPathHostObject>(this).indexes.clone();
    indexes.pop();
    let class: Class = msg![env; this class];
    let new: id = msg![env; class alloc];
    env.objc.borrow_mut::<NSIndexPathHostObject>(new).indexes = indexes;
    autorelease(env, new)
}

// Value semantics. Two index paths with the same indexes are equal and hash
// alike, which is what lets one be used as a dictionary key — apps keep
// per-cell state that way.
- (bool)isEqual:(id)other {
    if other == nil {
        return false;
    }
    let other_class: Class = msg![env; other class];
    let this_class: Class = msg![env; this class];
    if other_class != this_class {
        return false;
    }
    let a = env.objc.borrow::<NSIndexPathHostObject>(this).indexes.clone();
    let b = env.objc.borrow::<NSIndexPathHostObject>(other).indexes.clone();
    a == b
}

// Order-sensitive, which is the point: (0,1) must not collide with (1,0).
- (NSUInteger)hash {
    let indexes = &env.objc.borrow::<NSIndexPathHostObject>(this).indexes;
    indexes.iter().fold(0u32, |acc, &i| acc.wrapping_mul(31).wrapping_add(i))
}

- (NSComparisonResult)compare:(id)other {
    let a = env.objc.borrow::<NSIndexPathHostObject>(this).indexes.clone();
    let b = env.objc.borrow::<NSIndexPathHostObject>(other).indexes.clone();
    match a.cmp(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

// Immutable, so a copy can be the same object.
- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

@end

};
