/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFType` (type-generic functions etc).

use super::{CFHashCode, CFIndex};
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::foundation::{ns_value, NSUInteger};
use crate::objc::{nil, Class};
use crate::{msg, objc};
use crate::{msg_class, Environment};

pub type CFTypeRef = objc::id;

/// An opaque identifier for a Core Foundation type.
///
/// The values are tapHLE's own and mean nothing outside it. Apple assigns them
/// at runtime in registration order and documents that they change between
/// releases, so the only thing an app may do with one is compare it against the
/// result of the matching `CF<Type>GetTypeID()` — which is exactly what these
/// support.
pub type CFTypeID = NSUInteger;

/// The types tapHLE can name, and the class each one is in tapHLE's
/// Foundation-underneath-Core-Foundation arrangement.
///
/// Order matters: the first entry whose class the object is a kind of wins, so
/// anything that is a subclass of another entry has to come first. Nothing here
/// currently does, but a table read top to bottom is the kind that stays
/// correct when one is added.
const TYPES: &[(&str, CFTypeID)] = &[
    ("NSString", STRING),
    ("NSNumber", NUMBER),
    ("NSDictionary", DICTIONARY),
    ("NSArray", ARRAY),
    ("NSSet", SET),
    ("NSData", DATA),
    ("NSDate", DATE),
    ("NSURL", URL),
];

// The identifiers themselves. They live together rather than beside their own
// types because the only thing they have to be is distinct, and that is a
// property of the set rather than of any one of them.
//
/// The answer for something that is not one of the types above. Zero is not a
/// type ID any of them uses, so an app comparing against a real one gets false.
const NOT_A_CF_TYPE: CFTypeID = 0;
const STRING: CFTypeID = 1;
const NUMBER: CFTypeID = 2;
const BOOLEAN: CFTypeID = 3;
const DICTIONARY: CFTypeID = 4;
const ARRAY: CFTypeID = 5;
const SET: CFTypeID = 6;
const DATA: CFTypeID = 7;
const DATE: CFTypeID = 8;
const URL: CFTypeID = 9;

pub fn CFRetain(env: &mut Environment, object: CFTypeRef) -> CFTypeRef {
    assert!(!object.is_null()); // not allowed, unlike for normal objc objects
    objc::retain(env, object)
}
pub fn CFRelease(env: &mut Environment, object: CFTypeRef) {
    objc::release(env, object);
}

/// iOS does not use Objective-C garbage collection, so making a Core
/// Foundation object collectable has no effect on its ownership.
pub fn CFMakeCollectable(_env: &mut Environment, object: CFTypeRef) -> CFTypeRef {
    make_collectable(object)
}

fn make_collectable(object: CFTypeRef) -> CFTypeRef {
    object
}

pub fn CFGetRetainCount(env: &mut Environment, object: CFTypeRef) -> CFIndex {
    let count: NSUInteger = msg![env; object retainCount];
    count as CFIndex
}

pub fn CFEqual(env: &mut Environment, object1: CFTypeRef, object2: CFTypeRef) -> bool {
    if object1 == object2 {
        return true;
    }

    // Strings keep the special case: `isEqual:` on the NSString class cluster
    // is not trusted yet, which is why this was written with
    // `isEqualToString:` in the first place. What changes is that anything
    // else is now compared instead of being asserted away — a number, a date
    // or a dictionary reaching here is an ordinary comparison, and it used to
    // end the app.
    let str_class: Class = msg_class![env; NSString class];
    let object1_class: Class = msg![env; object1 class];
    if msg![env; object1_class isKindOfClass:str_class] {
        let object2_class: Class = msg![env; object2 class];
        // A string is never equal to something that is not a string, and
        // `isEqualToString:` is not the way to find that out.
        return msg![env; object2_class isKindOfClass:str_class]
            && msg![env; object1 isEqualToString:object2];
    }

    msg![env; object1 isEqual:object2]
}

pub fn CFHash(env: &mut Environment, object: CFTypeRef) -> CFHashCode {
    msg![env; object hash]
}

/// Which Core Foundation type an object is.
///
/// Apps ask this about a value they were handed rather than one they made —
/// out of a dictionary, a property list, or a system API — and then branch on
/// the answer. Three versions of Super Hexagon and Don't Look Back all ask it
/// about the system proxy settings during start-up.
pub fn CFGetTypeID(env: &mut Environment, object: CFTypeRef) -> CFTypeID {
    if object == nil {
        // Apple crashes on NULL here. tapHLE says "no type", which every
        // comparison an app makes against a real type ID gets right.
        log!("Warning: CFGetTypeID(NULL); returning the not-a-CF-object ID");
        return NOT_A_CF_TYPE;
    }

    for &(class_name, type_id) in TYPES {
        let class: Class = env.objc.get_known_class(class_name, &mut env.mem);
        let is_kind: bool = msg![env; object isKindOfClass:class];
        if !is_kind {
            continue;
        }
        // A boolean is its own Core Foundation type even though Foundation
        // spells it as an NSNumber.
        if type_id == NUMBER && ns_value::is_boolean(env, object) {
            return BOOLEAN;
        }
        return type_id;
    }

    let class: Class = objc::ObjC::read_isa(object, &env.mem);
    log!(
        "Warning: CFGetTypeID for class {:?}, which is not a Core Foundation type tapHLE can name; returning the not-a-CF-object ID",
        env.objc.get_class_name(class)
    );
    NOT_A_CF_TYPE
}

// What an app compares the result of CFGetTypeID against. Each is a function
// on Apple because the value is decided at runtime there; here they are
// constants wearing a function's clothes.
fn CFStringGetTypeID(_env: &mut Environment) -> CFTypeID {
    STRING
}
fn CFNumberGetTypeID(_env: &mut Environment) -> CFTypeID {
    NUMBER
}
fn CFBooleanGetTypeID(_env: &mut Environment) -> CFTypeID {
    BOOLEAN
}
fn CFDictionaryGetTypeID(_env: &mut Environment) -> CFTypeID {
    DICTIONARY
}
fn CFArrayGetTypeID(_env: &mut Environment) -> CFTypeID {
    ARRAY
}
fn CFSetGetTypeID(_env: &mut Environment) -> CFTypeID {
    SET
}
fn CFDataGetTypeID(_env: &mut Environment) -> CFTypeID {
    DATA
}
fn CFDateGetTypeID(_env: &mut Environment) -> CFTypeID {
    DATE
}
fn CFURLGetTypeID(_env: &mut Environment) -> CFTypeID {
    URL
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CFRetain(_)),
    export_c_func!(CFRelease(_)),
    export_c_func!(CFMakeCollectable(_)),
    export_c_func!(CFGetRetainCount(_)),
    export_c_func!(CFEqual(_, _)),
    export_c_func!(CFHash(_)),
    export_c_func!(CFGetTypeID(_)),
    export_c_func!(CFStringGetTypeID()),
    export_c_func!(CFNumberGetTypeID()),
    export_c_func!(CFBooleanGetTypeID()),
    export_c_func!(CFDictionaryGetTypeID()),
    export_c_func!(CFArrayGetTypeID()),
    export_c_func!(CFSetGetTypeID()),
    export_c_func!(CFDataGetTypeID()),
    export_c_func!(CFDateGetTypeID()),
    export_c_func!(CFURLGetTypeID()),
];

#[cfg(test)]
mod tests {
    use crate::mem::Ptr;

    #[test]
    fn make_collectable_is_an_identity_operation() {
        let object: super::CFTypeRef = Ptr::from_bits(0x1234);

        assert_eq!(super::make_collectable(object), object);
    }

    /// The one property the type IDs have to have. An app compares the result
    /// of CFGetTypeID against these, so two types sharing a number would make
    /// it take the wrong branch, and one of them colliding with the
    /// not-a-CF-object answer would make an unrecognised object look like it.
    #[test]
    fn every_type_id_is_distinct_and_none_of_them_is_zero() {
        let ids = [
            super::STRING,
            super::NUMBER,
            super::BOOLEAN,
            super::DICTIONARY,
            super::ARRAY,
            super::SET,
            super::DATA,
            super::DATE,
            super::URL,
        ];

        for (index, id) in ids.iter().enumerate() {
            assert_ne!(*id, super::NOT_A_CF_TYPE);
            assert!(!ids[..index].contains(id));
        }
    }
}
