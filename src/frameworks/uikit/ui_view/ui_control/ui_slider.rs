/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UISlider`.

use crate::frameworks::core_graphics::CGRect;
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_super, objc_classes, todo_objc_setter,
    ClassExports, NSZonePtr,
};

#[derive(Default)]
struct UISliderHostObject {
    superclass: super::UIControlHostObject,
    value: f32,
    minimum_value: f32,
    maximum_value: f32,
}
impl_HostObject_with_superclass!(UISliderHostObject);

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UISlider: UIControl

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UISliderHostObject {
        // UIKit's documented defaults.
        maximum_value: 1.0,
        ..Default::default()
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithFrame:(CGRect)frame {
    log!("[(UISlider*){:?} initWithFrame:{:?}] TODO: Implement UISlider. The control won't be rendered.", this, frame);
    msg_super![env; this initWithFrame:frame]
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    log!("[(UISlider*){:?} initWithCoder:{:?}] TODO: Implement UISlider. The control won't be rendered.", this, coder);
    msg_super![env; this initWithCoder:coder]
}

// The slider is not drawn, but its value is real state: an app sets it (from a
// saved setting, say) and reads it back, and clamping to the configured range
// is what makes that round-trip behave. Only the rendering is missing.
- (f32)value {
    env.objc.borrow::<UISliderHostObject>(this).value
}
- (())setValue:(f32)value {
    let host_object = env.objc.borrow_mut::<UISliderHostObject>(this);
    host_object.value = value.clamp(host_object.minimum_value, host_object.maximum_value);
}
- (())setValue:(f32)value animated:(bool)_animated {
    () = msg![env; this setValue:value];
}

- (f32)minimumValue {
    env.objc.borrow::<UISliderHostObject>(this).minimum_value
}
- (())setMinimumValue:(f32)minimum {
    let host_object = env.objc.borrow_mut::<UISliderHostObject>(this);
    host_object.minimum_value = minimum;
    let value = host_object.value.max(minimum);
    host_object.value = value;
}

- (f32)maximumValue {
    env.objc.borrow::<UISliderHostObject>(this).maximum_value
}
- (())setMaximumValue:(f32)maximum {
    let host_object = env.objc.borrow_mut::<UISliderHostObject>(this);
    host_object.maximum_value = maximum;
    let value = host_object.value.min(maximum);
    host_object.value = value;
}

- (())setMinimumValueImage:(id)img { // UIImage *
    todo_objc_setter!(this, img);
}
- (())setMaximumValueImage:(id)img { // UIImage *
    todo_objc_setter!(this, img);
}

// TODO: all of it

@end

};
