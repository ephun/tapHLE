/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The `NSSet` class cluster, including `NSMutableSet` and `NSCountedSet`.

use super::ns_array;
use super::ns_dictionary::DictionaryHostObject;
use super::ns_enumerator::{fast_enumeration_helper, NSFastEnumerationState};
use super::NSUInteger;
use crate::abi::{CallFromHost, DotDotDot};
use crate::environment::Environment;
use crate::mem::MutPtr;
use crate::objc::{
    autorelease, block_invoke_function, id, msg, msg_class, nil, objc_classes, release, retain,
    ClassExports, HostObject, NSZonePtr,
};

/// Belongs to _tapHLE_NSSet
#[derive(Debug, Default)]
struct SetHostObject {
    dict: DictionaryHostObject,
}
impl HostObject for SetHostObject {}

fn copy_set(env: &mut Environment, set: id, mutable: bool) -> id {
    let objects: Vec<id> = env
        .objc
        .borrow::<SetHostObject>(set)
        .dict
        .iter_keys()
        .collect();
    let copy: id = if mutable {
        msg_class![env; NSMutableSet alloc]
    } else {
        msg_class![env; NSSet alloc]
    };
    let null: id = msg_class![env; NSNull null];
    for object in objects {
        let mut host_object: SetHostObject = std::mem::take(env.objc.borrow_mut(copy));
        host_object
            .dict
            .insert(env, object, null, /* copy_key: */ false);
        *env.objc.borrow_mut(copy) = host_object;
    }
    copy
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// NSSet is an abstract class. A subclass must provide:
// - (NSUInteger)count;
// - (id)member:(id)object;   (NSSet below implements this in terms of the
//                             enumerator, so a subclass need not)
// - (NSEnumerator*)objectEnumerator;
// We can pick whichever subclass we want for the various alloc methods.
// For the time being, that will always be _tapHLE_NSSet.
@implementation NSSet: NSObject

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSSet might be subclassed by something which needs allocWithZone:
    // to have the normal behaviour. Unimplemented: call superclass alloc then.
    assert!(this == env.objc.get_known_class("NSSet", &mut env.mem));
    msg_class![env; _tapHLE_NSSet allocWithZone:zone]
}

+ (id)set {
    let set: id = msg![env; this new];
    autorelease(env, set)
}

+ (id)setWithArray:(id)array { // NSArray *
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithArray:array];
    autorelease(env, new)
}

+ (id)setWithObject:(id)object {
    assert!(object != nil);
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithObject:object];
    autorelease(env, new)
}

+ (id)setWithObjects:(id)first_obj, ...args {
    assert!(this == env.objc.get_known_class("NSSet", &mut env.mem));
    let new: id = msg![env; this alloc];
    env.objc.borrow_mut::<SetHostObject>(new).dict = set_from_objects(env, first_obj, args);
    autorelease(env, new)
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

// NSMutableCopying implementation
- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    copy_set(env, this, true)
}

// The set's *own* object equal to the argument, or nil. That is a different
// question from containsObject:, and the difference is the point of the
// method: a set used to intern values is asked for the instance it already
// holds, so the caller can use that one instead of its own copy.
//
// It is listed above as a primitive a subclass provides, and neither concrete
// subclass provided it, so asking was fatal. Implementing it here in terms of
// the enumerator gives it to both, and to any guest subclass that supplies
// only the three primitives.
- (id)member:(id)object {
    let enumerator: id = msg![env; this objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            return nil;
        }
        if msg![env; next isEqual:object] {
            return next;
        }
    }
}

- (bool)containsObject:(id)object {
    // Apple defines this as whether member: found anything. Answering the same
    // question a second way is how the two answers come to disagree, so it is
    // asked once.
    let member: id = msg![env; this member:object];
    member != nil
}

- (bool)intersectsSet:(id)other { // NSSet *
    let enumerator: id = msg![env; this objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            return false;
        }
        if msg![env; other containsObject:next] {
            return true;
        }
    }
}

- (bool)isSubsetOfSet:(id)other { // NSSet *
    let enumerator: id = msg![env; this objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            return true;
        }
        if !msg![env; other containsObject:next] {
            return false;
        }
    }
}

- (bool)isEqualToSet:(id)other { // NSSet *
    // Sets hold no duplicates, so equal counts plus one-way containment is
    // equality.
    let this_count: NSUInteger = msg![env; this count];
    let other_count: NSUInteger = msg![env; other count];
    if this_count != other_count {
        return false;
    }
    msg![env; this isSubsetOfSet:other]
}

// Returns a new set of the objects for which the predicate block returns YES.
// The block is `BOOL (^)(id obj, BOOL *stop)`; setting *stop ends enumeration.
- (id)objectsPassingTest:(id)predicate { // block
    let result: id = msg_class![env; NSMutableSet set];
    let stop: MutPtr<u8> = env.mem.alloc_and_write(0u8);
    let invoke = block_invoke_function(env, predicate);
    let enumerator: id = msg![env; this objectEnumerator];
    loop {
        let obj: id = msg![env; enumerator nextObject];
        if obj == nil {
            break;
        }
        let passed: bool = invoke.call_from_host(env, (predicate, obj, stop));
        if passed {
            () = msg![env; result addObject:obj];
        }
        if env.mem.read(stop.cast_const()) != 0 {
            break;
        }
    }
    env.mem.free(stop.cast());
    // objectsPassingTest: is documented to return an (immutable) NSSet.
    let immutable: id = msg![env; result copy];
    autorelease(env, immutable)
}

@end

// NSMutableSet is an abstract class. A subclass must provide everything
// NSSet provides, plus:
// - (void)addObject:(id)object;
// - (void)removeObject:(id)object;
// Note that it inherits from NSSet, so we must ensure we override any default
// methods that would be inappropriate for mutability.
@implementation NSMutableSet: NSSet

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSSet might be subclassed by something which needs allocWithZone:
    // to have the normal behaviour. Unimplemented: call superclass alloc then.
    assert!(this == env.objc.get_known_class("NSMutableSet", &mut env.mem));
    msg_class![env; _tapHLE_NSMutableSet allocWithZone:zone]
}

+ (id)setWithObjects:(id)first_obj, ...args {
    assert!(this == env.objc.get_known_class("NSMutableSet", &mut env.mem));
    let new: id = msg![env; this alloc];
    env.objc.borrow_mut::<SetHostObject>(new).dict = set_from_objects(env, first_obj, args);
    autorelease(env, new)
}

+ (id)setWithCapacity:(NSUInteger)capacity {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCapacity:capacity];
    autorelease(env, new)
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    copy_set(env, this, false)
}

// NSMutableCopying implementation
- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    copy_set(env, this, true)
}

@end

// Our private subclass that is the single implementation of NSSet for the
// time being.
@implementation _tapHLE_NSSet: NSSet

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(SetHostObject {
        dict: Default::default(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithObject:(id)object {
    let null: id = msg_class![env; NSNull null];

    let mut dict = <DictionaryHostObject as Default>::default();
    dict.insert(env, object, null, /* copy_key: */ false);

    env.objc.borrow_mut::<SetHostObject>(this).dict = dict;

    this
}

- (id)initWithObjects:(id)first_obj, ...args {
    env.objc.borrow_mut::<SetHostObject>(this).dict = set_from_objects(env, first_obj, args);
    this
}

- (id)initWithArray:(id)array {
    env.objc.borrow_mut::<SetHostObject>(this).dict = set_from_array(env, array);
    this
}

- (())dealloc {
    std::mem::take(&mut env.objc.borrow_mut::<SetHostObject>(this).dict).release(env);
    env.objc.dealloc_object(this, &mut env.mem)
}

// TODO: more init methods, etc

// TODO: accessors
- (NSUInteger)count {
    env.objc.borrow_mut::<SetHostObject>(this).dict.count
}

- (id)anyObject {
    let object_or_none = env.objc.borrow_mut::<SetHostObject>(this).dict.iter_keys().next();
    match object_or_none {
        Some(object) => object,
        None => nil
    }
}

- (id)allObjects {
    let objects = env.objc.borrow_mut::<SetHostObject>(this).dict.iter_keys().collect();
    ns_array::from_vec(env, objects)
}

- (id)objectEnumerator { // NSEnumerator*
    let array: id = msg![env; this allObjects];
    msg![env; array objectEnumerator]
}

// NSFastEnumeration implementation
- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    // We assume that order in which objects are reported is consistent
    // between calls!
    let objects: id = msg![env; this allObjects];
    let count: NSUInteger = msg![env; objects count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; objects objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

@end

// Our private subclass that is the single implementation of NSMutableSet for
// the time being.
@implementation _tapHLE_NSMutableSet: NSMutableSet

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(SetHostObject {
        dict: Default::default(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithObject:(id)object {
    let null: id = msg_class![env; NSNull null];

    let mut dict = <DictionaryHostObject as Default>::default();
    dict.insert(env, object, null, /* copy_key: */ false);

    env.objc.borrow_mut::<SetHostObject>(this).dict = dict;

    this
}

- (id)initWithObjects:(id)first_obj, ...args {
    env.objc.borrow_mut::<SetHostObject>(this).dict = set_from_objects(env, first_obj, args);
    this
}

- (id)initWithArray:(id)array {
    env.objc.borrow_mut::<SetHostObject>(this).dict = set_from_array(env, array);
    this
}

- (id)initWithCapacity:(NSUInteger)_capacity {
    // TODO: capacity
    msg![env; this init]
}

- (())dealloc {
    std::mem::take(&mut env.objc.borrow_mut::<SetHostObject>(this).dict).release(env);
    env.objc.dealloc_object(this, &mut env.mem)
}

// TODO: init methods etc

- (NSUInteger)count {
    env.objc.borrow_mut::<SetHostObject>(this).dict.count
}

- (id)anyObject {
    let object_or_none = env.objc.borrow_mut::<SetHostObject>(this).dict.iter_keys().next();
    match object_or_none {
        Some(object) => object,
        None => nil
    }
}

- (id)allObjects {
    let objects = env.objc.borrow_mut::<SetHostObject>(this).dict.iter_keys().collect();
    ns_array::from_vec(env, objects)
}

- (id)objectEnumerator { // NSEnumerator*
    let array: id = msg![env; this allObjects];
    msg![env; array objectEnumerator]
}

// NSFastEnumeration implementation
- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    // TODO: check that set wasn't mutated!
    // We assume that order in which objects are reported is consistent
    // between calls!
    let objects: id = msg![env; this allObjects];
    let count: NSUInteger = msg![env; objects count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; objects objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

// TODO: more mutation methods

- (())addObject:(id)object {
    let null: id = msg_class![env; NSNull null];
    let mut host_obj: SetHostObject = std::mem::take(env.objc.borrow_mut(this));
    host_obj.dict.insert(env, object, null, /* copy_key: */ false);
    *env.objc.borrow_mut(this) = host_obj;
}

- (())removeObject:(id)object {
    let mut host_obj: SetHostObject = std::mem::take(env.objc.borrow_mut(this));
    host_obj.dict.remove(env, object);
    *env.objc.borrow_mut(this) = host_obj;
}

- (())removeAllObjects {
    let mut old_host_obj = std::mem::replace(
        env.objc.borrow_mut(this),
        SetHostObject {
            dict: Default::default(),
        },
    );
    old_host_obj.dict.release(env);
}

- (())unionSet:(id)other { // NSSet *
    let enumerator: id = msg![env; other objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        () = msg![env; this addObject:next];
    }
}

- (())minusSet:(id)other { // NSSet *
    let enumerator: id = msg![env; other objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        () = msg![env; this removeObject:next];
    }
}

- (())intersectSet:(id)other { // NSSet *
    // Iterate a snapshot, because the receiver is mutated in the loop.
    let members: id = msg![env; this allObjects];
    let members: id = msg![env; members copy];
    let enumerator: id = msg![env; members objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        let keep: bool = msg![env; other containsObject:next];
        if !keep {
            () = msg![env; this removeObject:next];
        }
    }
    release(env, members);
}

- (())addObjectsFromArray:(id)array { // NSArray *
    let enumerator: id = msg![env; array objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        () = msg![env; this addObject:next];
    }
}

- (())setSet:(id)other { // NSSet *
    // `other` may be this very set, so its members have to be read out before
    // anything is removed.
    let members: id = msg![env; other allObjects];
    let members: id = msg![env; members copy];
    () = msg![env; this removeAllObjects];
    let enumerator: id = msg![env; members objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        () = msg![env; this addObject:next];
    }
    release(env, members);
}

@end

};

/// Helper method shared between `initWithObjects:` of `_tapHLE_NSSet` and
/// `_tapHLE_NSMutableSet`
fn set_from_objects(env: &mut Environment, first_obj: id, args: DotDotDot) -> DictionaryHostObject {
    let null: id = msg_class![env; NSNull null];

    let mut dict = <DictionaryHostObject as Default>::default();
    dict.insert(env, first_obj, null, /* copy_key: */ false);
    let mut varargs = args.start();
    loop {
        let next_arg: id = varargs.next(env);
        if next_arg == nil {
            break;
        }
        dict.insert(env, next_arg, null, /* copy_key: */ false);
    }
    dict
}

/// Helper method shared between `initWithArray:` of `_tapHLE_NSSet` and
/// `_tapHLE_NSMutableSet`
fn set_from_array(env: &mut Environment, array: id) -> DictionaryHostObject {
    let null: id = msg_class![env; NSNull null];

    let mut dict = <DictionaryHostObject as Default>::default();
    let count: NSUInteger = msg![env; array count];
    for i in 0..count {
        let next: id = msg![env; array objectAtIndex:i];
        dict.insert(env, next, null, /* copy_key: */ false);
    }
    dict
}
