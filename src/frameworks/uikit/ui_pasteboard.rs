/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIPasteboard`.

use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::foundation::{ns_string, NSInteger, NSUInteger};
use crate::objc::{
    id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
};
use crate::Environment;
use std::collections::HashMap;

// The pasteboard notification names and their userInfo keys.
//
// tapHLE does not post these yet: the pasteboard only changes when the guest
// changes it, so an app that also observes the change learns nothing it did not
// already know. They are exported because an app reads the name to subscribe,
// and an unbound one is a null pointer it dereferences without checking —
// which is how an app dies during startup, long before any pasteboard is
// involved.
const UIPasteboardChangedNotification: &str = "UIPasteboardChangedNotification";
const UIPasteboardRemovedNotification: &str = "UIPasteboardRemovedNotification";
const UIPasteboardChangedTypesAddedKey: &str = "UIPasteboardChangedTypesAddedKey";
const UIPasteboardChangedTypesRemovedKey: &str = "UIPasteboardChangedTypesRemovedKey";

pub const CONSTANTS: ConstantExports = &[
    (
        "_UIPasteboardChangedNotification",
        HostConstant::NSString(UIPasteboardChangedNotification),
    ),
    (
        "_UIPasteboardRemovedNotification",
        HostConstant::NSString(UIPasteboardRemovedNotification),
    ),
    (
        "_UIPasteboardChangedTypesAddedKey",
        HostConstant::NSString(UIPasteboardChangedTypesAddedKey),
    ),
    (
        "_UIPasteboardChangedTypesRemovedKey",
        HostConstant::NSString(UIPasteboardChangedTypesRemovedKey),
    ),
];

#[derive(Default)]
pub struct State {
    named_pasteboards: HashMap<String, id>,
    next_unique_name: u32,
}

struct UIPasteboardHostObject {
    name: String,
    persistent: bool,
    string: id,
    values: HashMap<String, id>,
    change_count: NSInteger,
}
impl HostObject for UIPasteboardHostObject {}

fn set_value(env: &mut Environment, pasteboard: id, type_: id, value: id) {
    let type_ = ns_string::to_rust_string(env, type_).into_owned();
    let value = retain(env, value);
    let old = {
        let host = env.objc.borrow_mut::<UIPasteboardHostObject>(pasteboard);
        host.change_count = host.change_count.wrapping_add(1);
        if value == nil {
            host.values.remove(&type_)
        } else {
            host.values.insert(type_, value)
        }
    };
    if let Some(old) = old {
        release(env, old);
    }
}

fn value_for_type(env: &mut Environment, pasteboard: id, type_: id) -> id {
    let type_ = ns_string::to_rust_string(env, type_).into_owned();
    env.objc
        .borrow::<UIPasteboardHostObject>(pasteboard)
        .values
        .get(&type_)
        .copied()
        .unwrap_or(nil)
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIPasteboard: NSObject

+ (id)generalPasteboard {
    let name = ns_string::get_static_str(env, "com.apple.UIKit.pboard.general");
    msg![env; this pasteboardWithName:name create:true]
}

+ (id)pasteboardWithName:(id)name create:(bool)create {
    let name = ns_string::to_rust_string(env, name).into_owned();
    if let Some(pasteboard) = env
        .framework_state
        .uikit
        .ui_pasteboard
        .named_pasteboards
        .get(&name)
        .copied()
    {
        return pasteboard;
    }
    if !create {
        return nil;
    }

    let pasteboard = env.objc.alloc_static_object(
        this,
        Box::new(UIPasteboardHostObject {
            name: name.clone(),
            persistent: false,
            string: nil,
            values: HashMap::new(),
            change_count: 0,
        }),
        &mut env.mem,
    );
    env.framework_state
        .uikit
        .ui_pasteboard
        .named_pasteboards
        .insert(name, pasteboard);
    pasteboard
}

+ (id)pasteboardWithUniqueName {
    let unique_number = env.framework_state.uikit.ui_pasteboard.next_unique_name;
    env.framework_state.uikit.ui_pasteboard.next_unique_name += 1;
    let name = ns_string::from_rust_string(
        env,
        format!("com.tapHLE.UIPasteboard.unique.{unique_number}"),
    );
    msg![env; this pasteboardWithName:name create:true]
}

+ (())removePasteboardWithName:(id)name {
    let name = ns_string::to_rust_string(env, name).into_owned();
    env.framework_state
        .uikit
        .ui_pasteboard
        .named_pasteboards
        .remove(&name);
}

- (id)retain { this }
- (())release {}
- (id)autorelease { this }

- (id)name {
    let name = env.objc.borrow::<UIPasteboardHostObject>(this).name.clone();
    ns_string::from_rust_string(env, name)
}

- (bool)isPersistent {
    env.objc.borrow::<UIPasteboardHostObject>(this).persistent
}

- (())setPersistent:(bool)persistent {
    env.objc.borrow_mut::<UIPasteboardHostObject>(this).persistent = persistent;
}

- (id)string {
    env.objc.borrow::<UIPasteboardHostObject>(this).string
}

- (())setString:(id)string {
    let copied = if string == nil {
        nil
    } else {
        msg![env; string copy]
    };
    let old = {
        let host = env.objc.borrow_mut::<UIPasteboardHostObject>(this);
        host.change_count = host.change_count.wrapping_add(1);
        std::mem::replace(&mut host.string, copied)
    };
    release(env, old);
}

- (NSInteger)changeCount {
    env.objc.borrow::<UIPasteboardHostObject>(this).change_count
}

- (id)pasteboardTypes {
    let types: Vec<String> = env
        .objc
        .borrow::<UIPasteboardHostObject>(this)
        .values
        .keys()
        .cloned()
        .collect();
    let capacity = types.len() as NSUInteger;
    let array: id = msg_class![env; NSMutableArray arrayWithCapacity:capacity];
    for type_ in types {
        let type_ = ns_string::from_rust_string(env, type_);
        () = msg![env; array addObject:type_];
    }
    array
}

- (bool)containsPasteboardTypes:(id)types {
    let count: NSUInteger = msg![env; types count];
    for index in 0..count {
        let type_: id = msg![env; types objectAtIndex:index];
        let type_ = ns_string::to_rust_string(env, type_).into_owned();
        if env
            .objc
            .borrow::<UIPasteboardHostObject>(this)
            .values
            .contains_key(&type_)
        {
            return true;
        }
    }
    false
}

- (id)valueForPasteboardType:(id)type_ {
    value_for_type(env, this, type_)
}

- (id)dataForPasteboardType:(id)type_ {
    value_for_type(env, this, type_)
}

- (())setValue:(id)value forPasteboardType:(id)type_ {
    set_value(env, this, type_, value);
}

- (())setData:(id)data forPasteboardType:(id)type_ {
    set_value(env, this, type_, data);
}

@end

};
