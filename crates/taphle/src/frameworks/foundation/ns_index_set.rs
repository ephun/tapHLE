/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSIndexSet` and `NSMutableIndexSet`.
//!
//! A set of unsigned integers, stored as **sorted, disjoint, non-adjacent
//! ranges** rather than as individual indexes. That is how Apple's is built
//! too, and here it is a correctness requirement rather than an optimisation:
//! `+indexSetWithIndexesInRange:` over a whole collection is the ordinary way
//! one of these is made, and a set that stored a million separate integers for
//! `NSMakeRange(0, 1000000)` would exhaust the guest heap answering a question
//! about a range.
//!
//! Resources:
//! - Apple's [NSIndexSet](https://developer.apple.com/documentation/foundation/nsindexset)

use super::{NSNotFound, NSRange, NSUInteger};
use crate::mem::MutPtr;
use crate::objc::{
    autorelease, id, msg, objc_classes, Class, ClassExports, HostObject, NSZonePtr,
};
use crate::Environment;

/// The ranges, sorted by location, disjoint, and never adjacent — two ranges
/// that touch are always merged into one, so "is this one range?" is answered
/// by the length of this list.
#[derive(Default, Clone)]
struct IndexSet {
    ranges: Vec<(NSUInteger, NSUInteger)>, // (location, length), length > 0
}

impl IndexSet {
    fn count(&self) -> NSUInteger {
        self.ranges.iter().map(|&(_, len)| len).sum()
    }

    fn contains(&self, index: NSUInteger) -> bool {
        self.ranges
            .iter()
            .any(|&(loc, len)| index >= loc && index - loc < len)
    }

    fn first(&self) -> Option<NSUInteger> {
        self.ranges.first().map(|&(loc, _)| loc)
    }

    fn last(&self) -> Option<NSUInteger> {
        self.ranges.last().map(|&(loc, len)| loc + len - 1)
    }

    /// Add a range, merging it with anything it touches or overlaps. An empty
    /// range adds nothing, which is what Apple's does.
    fn add_range(&mut self, location: NSUInteger, length: NSUInteger) {
        if length == 0 {
            return;
        }
        let mut start = location;
        let mut end = location + length; // exclusive
        let mut merged: Vec<(NSUInteger, NSUInteger)> = Vec::with_capacity(self.ranges.len() + 1);
        let mut inserted = false;
        for &(loc, len) in &self.ranges {
            let (r_start, r_end) = (loc, loc + len);
            if r_end < start {
                // Entirely before, and not adjacent.
                merged.push((loc, len));
            } else if r_start > end {
                // Entirely after: the new range's final position is known.
                if !inserted {
                    merged.push((start, end - start));
                    inserted = true;
                }
                merged.push((loc, len));
            } else {
                // Touching or overlapping: absorb it.
                start = start.min(r_start);
                end = end.max(r_end);
            }
        }
        if !inserted {
            merged.push((start, end - start));
        }
        self.ranges = merged;
    }

    /// Remove a range, splitting whatever it cuts through.
    fn remove_range(&mut self, location: NSUInteger, length: NSUInteger) {
        if length == 0 {
            return;
        }
        let (start, end) = (location, location + length); // exclusive
        let mut kept: Vec<(NSUInteger, NSUInteger)> = Vec::with_capacity(self.ranges.len() + 1);
        for &(loc, len) in &self.ranges {
            let (r_start, r_end) = (loc, loc + len);
            if r_end <= start || r_start >= end {
                kept.push((loc, len));
                continue;
            }
            if r_start < start {
                kept.push((r_start, start - r_start));
            }
            if r_end > end {
                kept.push((end, r_end - end));
            }
        }
        self.ranges = kept;
    }

    /// The lowest index greater than `index`, or `None`.
    fn index_greater_than(&self, index: NSUInteger) -> Option<NSUInteger> {
        for &(loc, len) in &self.ranges {
            if loc > index {
                return Some(loc);
            }
            if index < loc + len - 1 {
                return Some(index + 1);
            }
        }
        None
    }

    /// The highest index less than `index`, or `None`.
    fn index_less_than(&self, index: NSUInteger) -> Option<NSUInteger> {
        let mut best = None;
        for &(loc, len) in &self.ranges {
            if loc >= index {
                break;
            }
            best = Some((loc + len - 1).min(index - 1));
        }
        best
    }

    fn iter(&self) -> impl Iterator<Item = NSUInteger> + '_ {
        self.ranges
            .iter()
            .flat_map(|&(loc, len)| loc..(loc + len))
    }
}

#[derive(Default)]
struct NSIndexSetHostObject {
    set: IndexSet,
}
impl HostObject for NSIndexSetHostObject {}

/// `NSNotFound` as the unsigned value a caller compares against. Foundation's
/// constant is the signed spelling of the same bit pattern.
fn not_found() -> NSUInteger {
    NSNotFound as NSUInteger
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSIndexSet: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<NSIndexSetHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)indexSet {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new init];
    autorelease(env, new)
}

+ (id)indexSetWithIndex:(NSUInteger)index {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithIndex:index];
    autorelease(env, new)
}

+ (id)indexSetWithIndexesInRange:(NSRange)range {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithIndexesInRange:range];
    autorelease(env, new)
}

- (id)initWithIndex:(NSUInteger)index {
    env.objc.borrow_mut::<NSIndexSetHostObject>(this).set.add_range(index, 1);
    this
}

- (id)initWithIndexesInRange:(NSRange)range {
    env.objc.borrow_mut::<NSIndexSetHostObject>(this)
        .set.add_range(range.location, range.length);
    this
}

- (id)initWithIndexSet:(id)other { // NSIndexSet*
    let set = env.objc.borrow::<NSIndexSetHostObject>(other).set.clone();
    env.objc.borrow_mut::<NSIndexSetHostObject>(this).set = set;
    this
}

- (NSUInteger)count {
    env.objc.borrow::<NSIndexSetHostObject>(this).set.count()
}

- (NSUInteger)firstIndex {
    env.objc.borrow::<NSIndexSetHostObject>(this).set.first().unwrap_or_else(not_found)
}

- (NSUInteger)lastIndex {
    env.objc.borrow::<NSIndexSetHostObject>(this).set.last().unwrap_or_else(not_found)
}

- (bool)containsIndex:(NSUInteger)index {
    env.objc.borrow::<NSIndexSetHostObject>(this).set.contains(index)
}

- (bool)containsIndexesInRange:(NSRange)range {
    let set = &env.objc.borrow::<NSIndexSetHostObject>(this).set;
    (range.location..range.location + range.length).all(|i| set.contains(i))
}

- (bool)intersectsIndexesInRange:(NSRange)range {
    let set = &env.objc.borrow::<NSIndexSetHostObject>(this).set;
    (range.location..range.location + range.length).any(|i| set.contains(i))
}

- (NSUInteger)indexGreaterThanIndex:(NSUInteger)index {
    env.objc.borrow::<NSIndexSetHostObject>(this)
        .set.index_greater_than(index).unwrap_or_else(not_found)
}

- (NSUInteger)indexLessThanIndex:(NSUInteger)index {
    env.objc.borrow::<NSIndexSetHostObject>(this)
        .set.index_less_than(index).unwrap_or_else(not_found)
}

- (NSUInteger)indexGreaterThanOrEqualToIndex:(NSUInteger)index {
    let set = &env.objc.borrow::<NSIndexSetHostObject>(this).set;
    if set.contains(index) {
        index
    } else {
        set.index_greater_than(index).unwrap_or_else(not_found)
    }
}

- (NSUInteger)indexLessThanOrEqualToIndex:(NSUInteger)index {
    let set = &env.objc.borrow::<NSIndexSetHostObject>(this).set;
    if set.contains(index) {
        index
    } else {
        set.index_less_than(index).unwrap_or_else(not_found)
    }
}

// The C-array accessor, which is how an app of this era walks a set: blocks
// did not exist yet on the OS versions tapHLE targets.
- (NSUInteger)getIndexes:(MutPtr<NSUInteger>)buffer
                maxCount:(NSUInteger)max_count
            inIndexRange:(MutPtr<NSRange>)range_ptr { // NSRangePointer, may be NULL
    let (start, end) = if range_ptr.is_null() {
        (0, not_found())
    } else {
        let range = env.mem.read(range_ptr);
        (range.location, range.location.saturating_add(range.length))
    };

    let indexes: Vec<NSUInteger> = env.objc.borrow::<NSIndexSetHostObject>(this)
        .set.iter()
        .filter(|&i| i >= start && i < end)
        .take(max_count as usize)
        .collect();

    for (offset, &index) in indexes.iter().enumerate() {
        env.mem.write(buffer + offset as NSUInteger, index);
    }

    // The range is an in-out parameter: on return it describes what is left to
    // fetch, so a caller looping on this terminates.
    if !range_ptr.is_null() {
        let consumed_to = indexes.last().map(|&i| i + 1).unwrap_or(end);
        let remaining = end.saturating_sub(consumed_to);
        env.mem.write(range_ptr, NSRange { location: consumed_to, length: remaining });
    }

    indexes.len() as NSUInteger
}

- (bool)isEqualToIndexSet:(id)other { // NSIndexSet*
    let mine: Vec<_> = env.objc.borrow::<NSIndexSetHostObject>(this).set.ranges.clone();
    let theirs: Vec<_> = env.objc.borrow::<NSIndexSetHostObject>(other).set.ranges.clone();
    mine == theirs
}

- (id)copyWithZone:(NSZonePtr)_zone {
    let set = env.objc.borrow::<NSIndexSetHostObject>(this).set.clone();
    let class = env.objc.get_known_class("NSIndexSet", &mut env.mem);
    env.objc.alloc_object(class, Box::new(NSIndexSetHostObject { set }), &mut env.mem)
}

- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    let set = env.objc.borrow::<NSIndexSetHostObject>(this).set.clone();
    let class = env.objc.get_known_class("NSMutableIndexSet", &mut env.mem);
    env.objc.alloc_object(class, Box::new(NSIndexSetHostObject { set }), &mut env.mem)
}

@end

@implementation NSMutableIndexSet: NSIndexSet

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<NSIndexSetHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (())addIndex:(NSUInteger)index {
    env.objc.borrow_mut::<NSIndexSetHostObject>(this).set.add_range(index, 1);
}

- (())removeIndex:(NSUInteger)index {
    env.objc.borrow_mut::<NSIndexSetHostObject>(this).set.remove_range(index, 1);
}

- (())addIndexesInRange:(NSRange)range {
    env.objc.borrow_mut::<NSIndexSetHostObject>(this)
        .set.add_range(range.location, range.length);
}

- (())removeIndexesInRange:(NSRange)range {
    env.objc.borrow_mut::<NSIndexSetHostObject>(this)
        .set.remove_range(range.location, range.length);
}

- (())addIndexes:(id)other { // NSIndexSet*
    let ranges = env.objc.borrow::<NSIndexSetHostObject>(other).set.ranges.clone();
    let set = &mut env.objc.borrow_mut::<NSIndexSetHostObject>(this).set;
    for (loc, len) in ranges {
        set.add_range(loc, len);
    }
}

- (())removeIndexes:(id)other { // NSIndexSet*
    let ranges = env.objc.borrow::<NSIndexSetHostObject>(other).set.ranges.clone();
    let set = &mut env.objc.borrow_mut::<NSIndexSetHostObject>(this).set;
    for (loc, len) in ranges {
        set.remove_range(loc, len);
    }
}

- (())removeAllIndexes {
    env.objc.borrow_mut::<NSIndexSetHostObject>(this).set.ranges.clear();
}

- (id)copyWithZone:(NSZonePtr)_zone {
    let set = env.objc.borrow::<NSIndexSetHostObject>(this).set.clone();
    let class: Class = env.objc.get_known_class("NSIndexSet", &mut env.mem);
    env.objc.alloc_object(class, Box::new(NSIndexSetHostObject { set }), &mut env.mem)
}

@end

};

#[cfg(test)]
mod tests {
    use super::IndexSet;

    /// Adjacent ranges have to coalesce, or every question about the set
    /// depends on how it happened to be built.
    #[test]
    fn adjacent_ranges_merge() {
        let mut set = IndexSet::default();
        set.add_range(0, 3);
        set.add_range(3, 2);
        assert_eq!(set.ranges, vec![(0, 5)]);
        assert_eq!(set.count(), 5);
    }

    #[test]
    fn overlapping_and_separate_ranges() {
        let mut set = IndexSet::default();
        set.add_range(10, 5);
        set.add_range(0, 2);
        set.add_range(12, 5);
        assert_eq!(set.ranges, vec![(0, 2), (10, 7)]);
        assert!(set.contains(16));
        assert!(!set.contains(9));
        assert_eq!(set.first(), Some(0));
        assert_eq!(set.last(), Some(16));
    }

    /// Removing from the middle splits a range in two.
    #[test]
    fn removing_the_middle_splits() {
        let mut set = IndexSet::default();
        set.add_range(0, 10);
        set.remove_range(4, 2);
        assert_eq!(set.ranges, vec![(0, 4), (6, 4)]);
        assert!(!set.contains(5));
    }

    #[test]
    fn stepping_through_the_set() {
        let mut set = IndexSet::default();
        set.add_range(0, 2);
        set.add_range(5, 1);
        assert_eq!(set.index_greater_than(0), Some(1));
        assert_eq!(set.index_greater_than(1), Some(5));
        assert_eq!(set.index_greater_than(5), None);
        assert_eq!(set.index_less_than(5), Some(1));
        assert_eq!(set.index_less_than(0), None);
        assert_eq!(set.iter().collect::<Vec<_>>(), vec![0, 1, 5]);
    }

    /// An empty range is not an error and adds nothing, as Apple's does.
    #[test]
    fn an_empty_range_is_ignored() {
        let mut set = IndexSet::default();
        set.add_range(7, 0);
        assert!(set.ranges.is_empty());
        assert_eq!(set.count(), 0);
        assert_eq!(set.first(), None);
    }
}
