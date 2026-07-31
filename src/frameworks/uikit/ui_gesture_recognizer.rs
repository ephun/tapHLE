/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Gesture recognizers.

use crate::frameworks::core_graphics::CGPoint;
use crate::frameworks::foundation::NSUInteger;
use crate::objc::{id, objc_classes, release, retain, ClassExports, HostObject, SEL};
use crate::Environment;

#[derive(Default)]
struct UIGestureRecognizerHostObject {
    target: id,
    action: Option<SEL>,
    delegate: id,
    view: id,
    enabled: bool,
}
impl HostObject for UIGestureRecognizerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIGestureRecognizer: NSObject

+ (id)alloc {
    let host_object = Box::new(UIGestureRecognizerHostObject {
        enabled: true,
        ..Default::default()
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithTarget:(id)target action:(SEL)action {
    retain(env, target);
    let host = env.objc.borrow_mut::<UIGestureRecognizerHostObject>(this);
    host.target = target;
    host.action = Some(action);
    this
}

- (())dealloc {
    let target = env.objc.borrow::<UIGestureRecognizerHostObject>(this).target;
    release(env, target);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)delegate { env.objc.borrow::<UIGestureRecognizerHostObject>(this).delegate }
- (())setDelegate:(id)delegate { env.objc.borrow_mut::<UIGestureRecognizerHostObject>(this).delegate = delegate; }
- (bool)isEnabled { env.objc.borrow::<UIGestureRecognizerHostObject>(this).enabled }
- (())setEnabled:(bool)enabled { env.objc.borrow_mut::<UIGestureRecognizerHostObject>(this).enabled = enabled; }
- (id)view { env.objc.borrow::<UIGestureRecognizerHostObject>(this).view }
- (NSUInteger)numberOfTouches { 0 }
- (CGPoint)locationInView:(id)_view { CGPoint::default() }

@end

@implementation UIPinchGestureRecognizer: UIGestureRecognizer

- (f32)scale { 1.0 }
- (())setScale:(f32)_scale {}
- (f32)velocity { 0.0 }

@end

};

pub fn set_view(env: &mut Environment, recognizer: id, view: id) {
    env.objc
        .borrow_mut::<UIGestureRecognizerHostObject>(recognizer)
        .view = view;
}
