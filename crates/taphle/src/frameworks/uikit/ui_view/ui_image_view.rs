/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIImageView`.

use crate::frameworks::core_graphics::cg_image::CGImageRef;
use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::frameworks::foundation::{NSInteger, NSTimeInterval, NSUInteger};
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_super, nil, objc_classes, release, retain,
    ClassExports, NSZonePtr,
};
use std::time::Instant;

struct UIImageViewHostObject {
    superclass: super::UIViewHostObject,
    /// `UIImage*`
    image: id,
    /// `NSArray<UIImage *> *`
    animation_images: id,
    animation_duration: NSTimeInterval,
    /// A value of zero means repeat indefinitely.
    animation_repeat_count: NSInteger,
    animation_started_at: Option<Instant>,
}

fn effective_animation_duration(
    configured_duration: NSTimeInterval,
    image_count: NSUInteger,
) -> NSTimeInterval {
    if configured_duration == 0.0 {
        // UIKit uses 30 frames per second when no explicit duration is set.
        NSTimeInterval::from(image_count) / 30.0
    } else {
        configured_duration
    }
}

impl Default for UIImageViewHostObject {
    fn default() -> Self {
        // UIKit image views are decorative by default. An archived override
        // may opt them back into hit testing in UIView::initWithCoder:.
        let superclass = super::UIViewHostObject {
            user_interaction_enabled: false,
            ..Default::default()
        };

        Self {
            superclass,
            image: nil,
            animation_images: nil,
            animation_duration: 0.0,
            animation_repeat_count: 0,
            animation_started_at: None,
        }
    }
}
impl_HostObject_with_superclass!(UIImageViewHostObject);

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIImageView: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIImageViewHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithFrame:(CGRect)frame {
    let this: id = msg_super![env; this initWithFrame:frame];
    // Not sure if UIImageView does this unconditionally, or only for images
    // with alpha channels.
    () = msg![env; this setOpaque:false];
    this
}

- (())dealloc {
    let &UIImageViewHostObject {
        superclass: _,
        image,
        animation_images,
        animation_duration: _,
        animation_repeat_count: _,
        animation_started_at: _,
    } = env.objc.borrow(this);
    release(env, image);
    release(env, animation_images);
    msg_super![env; this dealloc]
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];

    let key_ns_string = get_static_str(env, "UIImage");
    let image: id = msg![env; coder decodeObjectForKey:key_ns_string];

    () = msg![env; this setImage:image];

    this
}

- (id)initWithImage:(id)image { // UIImage*
    let size: CGSize = msg![env; image size];
    let frame = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size
    };
    let this = msg_super![env; this initWithFrame:frame];
    () = msg![env; this setImage:image];
    // Not sure if UIImageView does this unconditionally, or only for images
    // with alpha channels.
    () = msg![env; this setOpaque:false];
    this
}

- (id)image {
    env.objc.borrow::<UIImageViewHostObject>(this).image
}

- (())setImage:(id)new_image { // UIImage*
    let host_obj = env.objc.borrow_mut::<UIImageViewHostObject>(this);
    let old_image = std::mem::replace(&mut host_obj.image, new_image);
    retain(env, new_image);
    release(env, old_image);

    let layer: id = msg![env; this layer];
    let cg_image: CGImageRef = msg![env; new_image CGImage];
    () = msg![env; layer setContents:cg_image];
}

- (())setAnimationImages:(id)images { // NSArray<UIImage *>*
    let old_images = {
        let host_object = env.objc.borrow_mut::<UIImageViewHostObject>(this);
        std::mem::replace(&mut host_object.animation_images, images)
    };
    retain(env, images);
    release(env, old_images);

    let count: NSUInteger = if images == nil {
        0
    } else {
        msg![env; images count]
    };
    if count == 0 {
        env.objc
            .borrow_mut::<UIImageViewHostObject>(this)
            .animation_started_at = None;
    } else {
        // TODO: Use all images in the array instead of just the first one.
        let first_image: id = msg![env; images objectAtIndex:0u32];
        () = msg![env; this setImage:first_image];
    }
}

- (id)animationImages {
    env.objc
        .borrow::<UIImageViewHostObject>(this)
        .animation_images
}

- (())setAnimationDuration:(NSTimeInterval)duration { // NSArray<UIImage *>*
    env.objc
        .borrow_mut::<UIImageViewHostObject>(this)
        .animation_duration = duration;
}

- (NSTimeInterval)animationDuration {
    env.objc
        .borrow::<UIImageViewHostObject>(this)
        .animation_duration
}

- (NSInteger)animationRepeatCount {
    env.objc
        .borrow::<UIImageViewHostObject>(this)
        .animation_repeat_count
}

- (())setAnimationRepeatCount:(NSInteger)repeat_count {
    env.objc
        .borrow_mut::<UIImageViewHostObject>(this)
        .animation_repeat_count = repeat_count;
}

- (())startAnimating {
    let animation_images = env
        .objc
        .borrow::<UIImageViewHostObject>(this)
        .animation_images;
    if animation_images != nil {
        let count: NSUInteger = msg![env; animation_images count];
        if count != 0 {
            env.objc
                .borrow_mut::<UIImageViewHostObject>(this)
                .animation_started_at = Some(Instant::now());
        }
    }
}

- (())stopAnimating {
    env.objc
        .borrow_mut::<UIImageViewHostObject>(this)
        .animation_started_at = None;
}

- (bool)isAnimating {
    let (started_at, repeat_count, configured_duration, animation_images) = {
        let host_object = env.objc.borrow::<UIImageViewHostObject>(this);
        (
            host_object.animation_started_at,
            host_object.animation_repeat_count,
            host_object.animation_duration,
            host_object.animation_images,
        )
    };
    let Some(started_at) = started_at else {
        return false;
    };

    let image_count: NSUInteger = if animation_images == nil {
        0
    } else {
        msg![env; animation_images count]
    };
    if image_count == 0 {
        env.objc
            .borrow_mut::<UIImageViewHostObject>(this)
            .animation_started_at = None;
        return false;
    }

    if repeat_count == 0 {
        return true;
    }

    let total_duration = effective_animation_duration(configured_duration, image_count)
        * NSTimeInterval::from(repeat_count);
    if total_duration > 0.0 && started_at.elapsed().as_secs_f64() < total_duration {
        true
    } else {
        env.objc
            .borrow_mut::<UIImageViewHostObject>(this)
            .animation_started_at = None;
        false
    }
}

@end

};

#[cfg(test)]
mod tests {
    use super::{effective_animation_duration, UIImageViewHostObject};

    #[test]
    fn image_views_ignore_touches_by_default() {
        assert!(
            !UIImageViewHostObject::default()
                .superclass
                .user_interaction_enabled
        );
    }

    #[test]
    fn zero_animation_duration_defaults_to_thirty_frames_per_second() {
        assert_eq!(effective_animation_duration(0.0, 60), 2.0);
        assert_eq!(effective_animation_duration(1.25, 60), 1.25);
    }
}
