/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CALayer`.

use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::core_animation::ca_transaction;
use crate::frameworks::core_foundation::time::CFTimeInterval;
use crate::frameworks::core_graphics::cg_affine_transform::{
    CGAffineTransform, CGAffineTransformIdentity,
};
use crate::frameworks::core_graphics::cg_bitmap_context::{
    self, CGBitmapContextCreate, CGBitmapContextGetHeight, CGBitmapContextGetWidth,
};
use crate::frameworks::core_graphics::cg_color::{CGColorHostObject, CGColorRef};
use crate::frameworks::core_graphics::cg_color_space::CGColorSpaceCreateDeviceRGB;
use crate::frameworks::core_graphics::cg_context::{
    CGContextClearRect, CGContextDrawImage, CGContextFillRect, CGContextRef, CGContextRelease,
    CGContextSetRGBFillColor, CGContextTranslateCTM,
};
use crate::frameworks::core_graphics::cg_image::{
    kCGImageAlphaPremultipliedLast, kCGImageByteOrder32Big,
};
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::{self, get_static_str, to_rust_string};
use crate::mem::{GuestUSize, Ptr};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, todo_objc_setter,
    ClassExports, HostObject, ObjC,
};
use crate::Environment;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub(super) struct CALayerHostObject {
    /// Possibly nil, usually a UIView. This is a weak reference.
    delegate: id,
    /// Sublayers in back-to-front order. These are strong references.
    pub(super) sublayers: Vec<id>,
    /// The superlayer. This is a weak reference.
    superlayer: id,
    pub(super) bounds: CGRect,
    pub(super) position: CGPoint,
    pub(super) anchor_point: CGPoint,
    pub(super) affine_transform: CGAffineTransform,
    pub(super) hidden: bool,
    pub(super) opaque: bool,
    pub(super) opacity: f32,
    pub(super) background_color: Option<CGColorHostObject>,
    pub(super) corner_radius: CGFloat,
    /// Stored and reported back, but not yet honoured when compositing: see the
    /// clipping TODO in [super::composition].
    pub(super) masks_to_bounds: bool,
    /// Stored and reported back, but the compositor does not stroke a border
    /// yet.
    pub(super) border_width: CGFloat,
    /// Stored and reported back, but the compositor does not stroke a border
    /// yet.
    pub(super) border_color: Option<CGColorHostObject>,
    pub(super) needs_display: bool,
    pub(super) needs_display_on_bounds_change: bool,
    /// `CGImageRef*`
    pub(super) contents: id,
    /// `NSString*`, one of the `kCAGravity*` names, or nil while it has never
    /// been set. Stored and reported back, but not honoured when compositing:
    /// contents are always stretched to the bounds. A strong reference.
    pub(super) contents_gravity: id,
    /// For CAEAGLLayer only
    pub(super) drawable_properties: id,
    /// For CAEAGLLayer only (internal state for compositor)
    pub(super) presented_pixels: Option<(Vec<u8>, u32, u32)>,
    /// Internal, only exposed when calling `drawLayer:inContext:`
    pub(super) cg_context: Option<CGContextRef>,
    /// Internal state for compositor
    pub(super) gles_texture: Option<crate::gles::gles11_raw::types::GLuint>,
    /// Internal state for compositor
    pub(super) gles_texture_is_up_to_date: bool,
    pub(super) animations: HashMap<String, id>, // CAAnimation*
    pub(super) anonymous_animations: HashSet<id>, // CAAnimation*
}
impl HostObject for CALayerHostObject {}

impl CALayerHostObject {
    /// Internal helper method: generate a transformation matrix to transform
    /// from the superlayer's co-ordinate space (the space that the layer's
    /// position is specified in) to the layer's internal co-ordinate space
    /// (the space that the layer's bounds and its sublayers' positions are
    /// specified in).
    pub(super) fn superlayer_to_layer_transform(&self) -> CGAffineTransform {
        CGAffineTransform::make_translation(-self.bounds.origin.x, -self.bounds.origin.y)
            .concat(CGAffineTransform::make_translation(
                -self.bounds.size.width * self.anchor_point.x,
                -self.bounds.size.height * self.anchor_point.y,
            ))
            .concat(self.affine_transform)
            .concat(CGAffineTransform::make_translation(
                self.position.x,
                self.position.y,
            ))
    }
}

pub const kCAFilterLinear: &str = "kCAFilterLinear";
pub const kCAFilterNearest: &str = "kCAFilterNearest";
pub const kCAFilterTrilinear: &str = "kCAFilterTrilinear";

/// `contentsGravity` values. tapHLE does not honour the gravity when drawing a
/// layer's contents — they are always stretched to the bounds — but an app sets
/// it by reading one of these constants, and an unbound one is a null pointer
/// it dereferences to do so.
pub const kCAGravityCenter: &str = "center";
pub const kCAGravityTop: &str = "top";
pub const kCAGravityBottom: &str = "bottom";
pub const kCAGravityLeft: &str = "left";
pub const kCAGravityRight: &str = "right";
pub const kCAGravityTopLeft: &str = "topLeft";
pub const kCAGravityTopRight: &str = "topRight";
pub const kCAGravityBottomLeft: &str = "bottomLeft";
pub const kCAGravityBottomRight: &str = "bottomRight";
pub const kCAGravityResize: &str = "resize";
pub const kCAGravityResizeAspect: &str = "resizeAspect";
pub const kCAGravityResizeAspectFill: &str = "resizeAspectFill";

/// The action key a layer looks up when it is about to be replaced on screen.
/// It is an ordinary string like the gravity and filter names, and apps read it
/// to install or ask for a transition rather than to compare against.
pub const kCATransition: &str = "transition";

pub const CONSTANTS: ConstantExports = &[
    ("_kCATransition", HostConstant::NSString(kCATransition)),
    ("_kCAFilterLinear", HostConstant::NSString(kCAFilterLinear)),
    (
        "_kCAFilterNearest",
        HostConstant::NSString(kCAFilterNearest),
    ),
    (
        "_kCAFilterTrilinear",
        HostConstant::NSString(kCAFilterTrilinear),
    ),
    (
        "_kCAGravityCenter",
        HostConstant::NSString(kCAGravityCenter),
    ),
    ("_kCAGravityTop", HostConstant::NSString(kCAGravityTop)),
    (
        "_kCAGravityBottom",
        HostConstant::NSString(kCAGravityBottom),
    ),
    ("_kCAGravityLeft", HostConstant::NSString(kCAGravityLeft)),
    ("_kCAGravityRight", HostConstant::NSString(kCAGravityRight)),
    (
        "_kCAGravityTopLeft",
        HostConstant::NSString(kCAGravityTopLeft),
    ),
    (
        "_kCAGravityTopRight",
        HostConstant::NSString(kCAGravityTopRight),
    ),
    (
        "_kCAGravityBottomLeft",
        HostConstant::NSString(kCAGravityBottomLeft),
    ),
    (
        "_kCAGravityBottomRight",
        HostConstant::NSString(kCAGravityBottomRight),
    ),
    (
        "_kCAGravityResize",
        HostConstant::NSString(kCAGravityResize),
    ),
    (
        "_kCAGravityResizeAspect",
        HostConstant::NSString(kCAGravityResizeAspect),
    ),
    (
        "_kCAGravityResizeAspectFill",
        HostConstant::NSString(kCAGravityResizeAspectFill),
    ),
];

/// Recursive body of `-[CALayer renderInContext:]`; see the note there for the
/// deliberate limits.
fn render_in_context_inner(env: &mut Environment, layer: id, context: CGContextRef) {
    let (hidden, bounds, background_color, contents, sublayers) = {
        let host_object = env.objc.borrow::<CALayerHostObject>(layer);
        (
            host_object.hidden,
            host_object.bounds,
            host_object.background_color,
            host_object.contents,
            host_object.sublayers.clone(),
        )
    };
    if hidden {
        return;
    }

    // The layer's own coordinate space starts at its bounds origin.
    let rect = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: bounds.size,
    };

    if let Some(color) = background_color {
        CGContextSetRGBFillColor(env, context, color.r, color.g, color.b, color.a);
        CGContextFillRect(env, context, rect);
    }
    if contents != nil {
        CGContextDrawImage(env, context, rect, contents);
    }

    for sublayer in sublayers {
        // Translation only. A sublayer's origin in this layer's space is its
        // position minus its anchor point scaled by its own size.
        let (position, anchor_point, sub_bounds) = {
            let host_object = env.objc.borrow::<CALayerHostObject>(sublayer);
            (
                host_object.position,
                host_object.anchor_point,
                host_object.bounds,
            )
        };
        let dx = position.x - anchor_point.x * sub_bounds.size.width;
        let dy = position.y - anchor_point.y * sub_bounds.size.height;

        // There is no CGContextSaveGState here, so undo the translation
        // afterwards instead. That is exact for a pure translation.
        CGContextTranslateCTM(env, context, dx, dy);
        render_in_context_inner(env, sublayer, context);
        CGContextTranslateCTM(env, context, -dx, -dy);
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation CALayer: NSObject

+ (id)alloc {
    let host_object = Box::new(CALayerHostObject {
        delegate: nil,
        sublayers: Vec::new(),
        superlayer: nil,
        bounds: CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize { width: 0.0, height: 0.0 }
        },
        position: CGPoint { x: 0.0, y: 0.0 },
        anchor_point: CGPoint { x: 0.5, y: 0.5 },
        affine_transform: CGAffineTransformIdentity,
        hidden: false,
        opaque: false,
        opacity: 1.0,
        background_color: None, // transparency
        corner_radius: 0.0,
        masks_to_bounds: false,
        border_width: 0.0,
        border_color: None,
        needs_display: false,
        needs_display_on_bounds_change: false,
        contents: nil,
        contents_gravity: nil,
        drawable_properties: nil,
        presented_pixels: None,
        cg_context: None,
        gles_texture: None,
        gles_texture_is_up_to_date: false,
        animations: HashMap::new(),
        anonymous_animations: HashSet::new(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)layer {
    let new_layer: id = msg![env; this alloc];
    msg![env; new_layer init]
}

- (())dealloc {
    let &mut CALayerHostObject {
        drawable_properties,
        contents,
        contents_gravity,
        superlayer,
        cg_context,
        ref mut sublayers,
        ..
    } = env.objc.borrow_mut(this);
    let sublayers = std::mem::take(sublayers);

    if drawable_properties != nil {
        release(env, drawable_properties);
    }

    if contents != nil {
        release(env, contents);
    }

    if contents_gravity != nil {
        release(env, contents_gravity);
    }

    if let Some(cg_context) = cg_context {
        CGContextRelease(env, cg_context);
    }

    assert!(superlayer == nil);
    for sublayer in sublayers {
        env.objc.borrow_mut::<CALayerHostObject>(sublayer).superlayer = nil;
        release(env, sublayer);
    }

    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)delegate {
    env.objc.borrow::<CALayerHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<CALayerHostObject>(this).delegate = delegate;
}

- (id)superlayer {
    env.objc.borrow::<CALayerHostObject>(this).superlayer
}
// TODO: sublayers accessors

- (())addSublayer:(id)layer {
    if env.objc.borrow::<CALayerHostObject>(layer).superlayer == this {
        () = msg![env; this bringSublayerToFront:layer];
    } else {
        retain(env, layer);
        () = msg![env; layer removeFromSuperlayer];
        env.objc.borrow_mut::<CALayerHostObject>(layer).superlayer = this;
        env.objc.borrow_mut::<CALayerHostObject>(this).sublayers.push(layer);
    }
}

- (())insertSublayer:(id)layer atIndex:(u32)idx {
    retain(env, layer);
    () = msg![env; layer removeFromSuperlayer];
    env.objc.borrow_mut::<CALayerHostObject>(layer).superlayer = this;

    let CALayerHostObject { ref mut sublayers, .. } = env.objc.borrow_mut(this);
    // Past the end is the top, the same as for a view: this is where the view
    // side lands, and a layer tree built directly gets the same treatment.
    let idx = (idx as usize).min(sublayers.len());
    sublayers.insert(idx, layer);
}

- (())insertSublayer:(id)layer below:(id)sibling {
    retain(env, layer);
    () = msg![env; layer removeFromSuperlayer];
    env.objc.borrow_mut::<CALayerHostObject>(layer).superlayer = this;

    let CALayerHostObject { ref mut sublayers, .. } = env.objc.borrow_mut(this);
    let idx = sublayers.iter().position(|&sublayer| sublayer == sibling).unwrap();
    sublayers.insert(idx, layer);
}

- (())removeFromSuperlayer {
    let CALayerHostObject { ref mut superlayer, .. } = env.objc.borrow_mut(this);
    let superlayer = std::mem::take(superlayer);
    if superlayer == nil {
        return;
    }

    let CALayerHostObject { ref mut sublayers, .. } = env.objc.borrow_mut(superlayer);
    let idx = sublayers.iter().position(|&sublayer| sublayer == this).unwrap();
    let sublayer = sublayers.remove(idx);
    assert!(sublayer == this);
    release(env, this);
}

- (CGRect)bounds {
    env.objc.borrow::<CALayerHostObject>(this).bounds
}
- (())setBounds:(CGRect)bounds {
    let host_object = env.objc.borrow_mut::<CALayerHostObject>(this);
    let old_bounds = std::mem::replace(&mut host_object.bounds, bounds);
    if is_implicit_animation_enabled(env, this) && old_bounds != bounds {
        let old_bounds: id = msg_class![env; NSValue valueWithCGRect:old_bounds];
        let bounds: id = msg_class![env; NSValue valueWithCGRect:bounds];
        add_default_implied_basic_animation(env, this, "bounds", old_bounds, bounds);
    }
    if env.objc.borrow::<CALayerHostObject>(this).needs_display_on_bounds_change {
        () = msg![env; this setNeedsDisplay];
    }
}

- (CGPoint)position {
    env.objc.borrow::<CALayerHostObject>(this).position
}
- (())setPosition:(CGPoint)position {
    let host_object = env.objc.borrow_mut::<CALayerHostObject>(this);
    let old_position = std::mem::replace(&mut host_object.position, position);
    if is_implicit_animation_enabled(env, this) && old_position != position {
        let old_position: id = msg_class![env; NSValue valueWithCGPoint:old_position];
        let position: id = msg_class![env; NSValue valueWithCGPoint:position];
        add_default_implied_basic_animation(env, this, "position", old_position, position);
    }
}

- (CGPoint)anchorPoint {
    env.objc.borrow::<CALayerHostObject>(this).anchor_point
}
- (())setAnchorPoint:(CGPoint)anchor_point {
    let host_object = env.objc.borrow_mut::<CALayerHostObject>(this);
    let old_anchor_point = std::mem::replace(&mut host_object.anchor_point, anchor_point);
    if is_implicit_animation_enabled(env, this) && old_anchor_point != anchor_point {
        let old_anchor_point: id = msg_class![env; NSValue valueWithCGPoint:old_anchor_point];
        let anchor_point: id = msg_class![env; NSValue valueWithCGPoint:anchor_point];
        add_default_implied_basic_animation(env, this, "anchorPoint", old_anchor_point, anchor_point);
    }
}

- (CGAffineTransform)affineTransform {
    env.objc.borrow::<CALayerHostObject>(this).affine_transform
}
- (())setAffineTransform:(CGAffineTransform)affine_transform {
    let host_object = env.objc.borrow_mut::<CALayerHostObject>(this);
    let old_affine_transform = std::mem::replace(&mut host_object.affine_transform, affine_transform);
    if is_implicit_animation_enabled(env, this) && old_affine_transform != affine_transform {
        log!("TODO: Implicit animation for affineTransform change from {old_affine_transform:?} to {affine_transform:?}");
    }
}

- (CGRect)frame {
    let host_obj @ &CALayerHostObject {
        bounds,
        ..
    } = env.objc.borrow(this);
    host_obj.superlayer_to_layer_transform().apply_to_rect(CGRect {
        origin: CGPoint { x: bounds.origin.x, y: bounds.origin.y },
        size: bounds.size,
    })
}
- (())setFrame:(CGRect)frame {
    let CALayerHostObject {
        anchor_point,
        affine_transform,
        ..
    } = env.objc.borrow_mut(this);

    let inverse_transform = CGAffineTransform::make_translation(
        -frame.size.width * anchor_point.x,
        -frame.size.height * anchor_point.y,
    )
    .concat(*affine_transform).invert();

    // Not the same as ::apply_to_size() as this does not ignore translation.
    let transformed_size = inverse_transform.apply_to_rect(CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: frame.size
    }).size;
    let transformed_offset = inverse_transform.apply_to_point(CGPoint { x: 0.0, y: 0.0 });

    let new_position = CGPoint {
        x: frame.origin.x + transformed_offset.x,
        y: frame.origin.y + transformed_offset.y,
    };
    () = msg![env; this setPosition:new_position];
    let new_bounds = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: transformed_size,
    };
    () = msg![env; this setBounds:new_bounds];
}

- (bool)isHidden {
    env.objc.borrow::<CALayerHostObject>(this).hidden
}
- (())setHidden:(bool)hidden {
    let host_object = env.objc.borrow_mut::<CALayerHostObject>(this);
    let old_hidden = std::mem::replace(&mut host_object.hidden, hidden);
    if is_implicit_animation_enabled(env, this) && old_hidden != hidden {
        // i kinda hate this
        let old_hidden: id = msg_class![env; NSNumber numberWithBool:old_hidden];
        let hidden: id = msg_class![env; NSNumber numberWithBool:hidden];
        add_default_implied_basic_animation(env, this, "hidden", old_hidden, hidden);
    }
}

- (bool)isOpaque {
    env.objc.borrow::<CALayerHostObject>(this).opaque
}
- (())setOpaque:(bool)opaque {
    env.objc.borrow_mut::<CALayerHostObject>(this).opaque = opaque;
}

// Stored and reported back, so a guest that sets it and reads it back sees
// what it wrote, but the compositor does not clip sublayers to the layer's
// bounds yet. See the clipping TODO in the composition module.
// Draw this layer and its sublayers into a CoreGraphics context.
//
// Deliberately partial, and the boundary is worth stating precisely because
// the full behaviour is large: this draws each layer's **background colour**
// and its **contents image**, then recurses into sublayers positioned by
// translation. It does **not** apply affine transforms, opacity, corner radius,
// masking, or a delegate's -drawLayer:inContext:, and it does not consult any
// running animation's presentation values.
//
// That covers the common use — snapshotting a tree of image-backed layers into
// a UIGraphics image context — and nothing else. A layer relying on any of the
// omitted features renders wrong rather than not at all, so this logs once to
// say so.
//
// The layer tree is drawn with tapHLE's OpenGL compositor everywhere else, so
// there is no existing path to share; a faithful implementation would either
// grow this into a real CoreGraphics renderer or render through GL and read
// back.
- (())renderInContext:(CGContextRef)context {
    log_once!("TODO: -[CALayer renderInContext:] draws only background colour and contents; transforms, opacity, masking and drawLayer:inContext: are ignored");
    render_in_context_inner(env, this, context);
}

- (bool)masksToBounds {
    env.objc.borrow::<CALayerHostObject>(this).masks_to_bounds
}
- (())setMasksToBounds:(bool)masks_to_bounds {
    if masks_to_bounds {
        log_once!(
            "[CALayer setMasksToBounds:true] is stored but sublayers are not \
             clipped yet"
        );
    }
    env.objc.borrow_mut::<CALayerHostObject>(this).masks_to_bounds = masks_to_bounds;
}

- (f32)opacity {
    env.objc.borrow::<CALayerHostObject>(this).opacity
}
- (())setOpacity:(f32)opacity {
    let host_object = env.objc.borrow_mut::<CALayerHostObject>(this);
    let old_opacity = std::mem::replace(&mut host_object.opacity, opacity);
    if is_implicit_animation_enabled(env, this) && old_opacity != opacity {
        let old_opacity: id = msg_class![env; NSNumber numberWithFloat:old_opacity];
        let opacity: id = msg_class![env; NSNumber numberWithFloat:opacity];
        add_default_implied_basic_animation(env, this, "opacity", old_opacity, opacity);
    }
}

- (CGColorRef)backgroundColor {
    if let Some(bg_color) = env.objc.borrow::<CALayerHostObject>(this).background_color {
        let class = env.objc.get_known_class("_tapHLE_CGColor", &mut env.mem);
        let obj = env.objc.alloc_object(class, Box::new(bg_color), &mut env.mem);
        autorelease(env, obj)
    } else {
        nil
    }
}
- (())setBackgroundColor:(CGColorRef)new_color_ref {
    let old_color_ref = msg![env; this backgroundColor];
    let new_color = if new_color_ref == nil {
        None
    } else {
        Some(*env.objc.borrow::<CGColorHostObject>(new_color_ref))
    };
    let host_object = env.objc.borrow_mut::<CALayerHostObject>(this);
    host_object.background_color = new_color;
    if is_implicit_animation_enabled(env, this) && old_color_ref != nil && new_color_ref != nil {
        add_default_implied_basic_animation(env, this, "backgroundColor", old_color_ref, new_color_ref);
    }
}

// Stored and reported back so a guest reads back what it wrote, but the
// compositor does not stroke a border yet.
- (CGFloat)borderWidth {
    env.objc.borrow::<CALayerHostObject>(this).border_width
}
- (())setBorderWidth:(CGFloat)border_width {
    if border_width != 0.0 {
        log_once!("[CALayer setBorderWidth:] is stored but no border is drawn yet");
    }
    env.objc.borrow_mut::<CALayerHostObject>(this).border_width = border_width;
}

- (CGColorRef)borderColor {
    if let Some(border_color) = env.objc.borrow::<CALayerHostObject>(this).border_color {
        let class = env.objc.get_known_class("_tapHLE_CGColor", &mut env.mem);
        let obj = env.objc.alloc_object(class, Box::new(border_color), &mut env.mem);
        autorelease(env, obj)
    } else {
        nil
    }
}
- (())setBorderColor:(CGColorRef)new_color_ref {
    let new_color = if new_color_ref == nil {
        None
    } else {
        Some(*env.objc.borrow::<CGColorHostObject>(new_color_ref))
    };
    env.objc.borrow_mut::<CALayerHostObject>(this).border_color = new_color;
}

- (CGFloat)cornerRadius {
    env.objc.borrow::<CALayerHostObject>(this).corner_radius
}
- (())setCornerRadius:(CGFloat)corner_radius {
    let host_object = env.objc.borrow_mut::<CALayerHostObject>(this);
    let old_corner_radius = std::mem::replace(&mut host_object.corner_radius, corner_radius);
    if is_implicit_animation_enabled(env, this) && old_corner_radius != corner_radius {
        let old_corner_radius: id = msg_class![env; NSNumber numberWithFloat:old_corner_radius];
        let corner_radius: id = msg_class![env; NSNumber numberWithFloat:corner_radius];
        add_default_implied_basic_animation(env, this, "cornerRadius", old_corner_radius, corner_radius);
    }
}

- (bool)needsDisplay {
    env.objc.borrow::<CALayerHostObject>(this).needs_display
}
- (())setNeedsDisplay {
    env.objc.borrow_mut::<CALayerHostObject>(this).needs_display = true;
}

- (bool)needsDisplayOnBoundsChange {
    env.objc.borrow::<CALayerHostObject>(this).needs_display_on_bounds_change
}
- (())setNeedsDisplayOnBoundsChange:(bool)value {
    env.objc.borrow_mut::<CALayerHostObject>(this).needs_display_on_bounds_change = value;
}

// TODO: support setNeedsDisplayInRect:
- (())displayIfNeeded {
    let &mut CALayerHostObject {
        ref mut needs_display,
        delegate,
        ..
    } = env.objc.borrow_mut(this);
    if !std::mem::take(needs_display) {
        return;
    }

    if delegate == nil {
        return;
    }

    let delegate_class = ObjC::read_isa(delegate, &env.mem);

    // According to the Core Animation Programming Guide, a layer delegate must
    // provide either displayLayer: or drawLayer:inContext:, and the former is
    // called if both are defined.

    if env.objc.class_has_method_named(delegate_class, "displayLayer:") {
        () = msg![env; delegate displayLayer:this];
        return;
    }

    let &mut CALayerHostObject {
        cg_context,
        ref mut gles_texture_is_up_to_date,
        bounds: CGRect { origin, size },
        ..
    } = env.objc.borrow_mut(this);

    *gles_texture_is_up_to_date = false;

    // TODO: more correctly handle non-integer sizes?
    let int_width = size.width.round() as GuestUSize;
    let int_height = size.height.round() as GuestUSize;

    let need_new_context = cg_context.is_none_or(|existing|
            CGBitmapContextGetWidth(env, existing) != int_width ||
            CGBitmapContextGetHeight(env, existing) != int_height
    );
    let cg_context = if need_new_context {
        if let Some(old_context) = cg_context {
            CGContextRelease(env, old_context);
        }

        // Make sure this is in sync with the code in composition.rs that
        // uploads the texture!
        // TODO: is this the right color space?
        let color_space = CGColorSpaceCreateDeviceRGB(env);
        let cg_context = CGBitmapContextCreate(
            env,
            Ptr::null(),
            int_width,
            int_height,
            8, // bpp
            int_width.checked_mul(4).unwrap(),
            color_space,
            kCGImageByteOrder32Big | kCGImageAlphaPremultipliedLast
        );
        // The compositor draws this bitmap with its vertical texture
        // coordinate inverted (see composition.rs), so anything drawn into it
        // that has a handedness has to be written the other way up. Drawing
        // cannot work that out from the transform, so it is recorded here,
        // where it is known.
        cg_bitmap_context::mark_flipped_on_presentation(env, cg_context);
        env.objc.borrow_mut::<CALayerHostObject>(this).cg_context = Some(cg_context);
        cg_context
    } else {
        cg_context.unwrap()
    };

    CGContextTranslateCTM(env, cg_context, -origin.x, -origin.y);
    // TODO: move clearing to UIKit (clearsContextBeforeDrawing)?
    CGContextClearRect(env, cg_context, CGRect { origin, size });
    () = msg![env; delegate drawLayer:this inContext:cg_context];
    CGContextTranslateCTM(env, cg_context, origin.x, origin.y);
}

// CGImageRef*
- (id)contents {
    env.objc.borrow::<CALayerHostObject>(this).contents
}
- (())setContents:(id)new_contents {
    let host_obj = env.objc.borrow_mut::<CALayerHostObject>(this);
    host_obj.gles_texture_is_up_to_date = false;
    let old_contents = std::mem::replace(&mut host_obj.contents, new_contents);
    retain(env, new_contents);
    release(env, old_contents);
}

// How the contents are placed when they do not fill the layer's bounds. tapHLE
// always stretches them, so this is kept and reported back rather than obeyed —
// but an app sets it while building its layers, long before anything is drawn,
// and not understanding the message at all cost the whole app. The gravity
// names it passes are already exported as constants above.
- (id)contentsGravity {
    env.objc.borrow::<CALayerHostObject>(this).contents_gravity
}
- (())setContentsGravity:(id)gravity { // NSString*
    log_once!("[CALayer setContentsGravity:] is stored but contents are always stretched to the bounds");
    let host_obj = env.objc.borrow_mut::<CALayerHostObject>(this);
    let old_gravity = std::mem::replace(&mut host_obj.contents_gravity, gravity);
    retain(env, gravity);
    release(env, old_gravity);
}

- (())setEdgeAntialiasingMask:(u32)mask {
    todo_objc_setter!(this, mask);
}

- (())setMagnificationFilter:(id)filter {
    todo_objc_setter!(this, ns_string::to_rust_string(env, filter));
}

- (())setMinificationFilter:(id)filter {
    todo_objc_setter!(this, ns_string::to_rust_string(env, filter));
}

- (bool)containsPoint:(CGPoint)point {
    let bounds: CGRect = msg![env; this bounds];
    let x_range = bounds.origin.x..(bounds.origin.x + bounds.size.width);
    let y_range = bounds.origin.y..(bounds.origin.y + bounds.size.height);
    let CGPoint {x, y} = point;
    x_range.contains(&x) && y_range.contains(&y)
}

- (CGPoint)convertPoint:(CGPoint)point
              fromLayer:(id)other { // CALayer*

    if this == other {
        return point;
    }

    let res = transform_for_conversion(env, this, other).apply_to_point(point);
    log_dbg!("Converted {point:?} from {other:?} to {this:?}: {res:?}");
    res
}
- (CGPoint)convertPoint:(CGPoint)point
                toLayer:(id)other { // CALayer*
    if this == other {
        return point;
    }

    let res = transform_for_conversion(env, other, this).apply_to_point(point);
    log_dbg!("Converted {point:?} from {this:?} to {other:?}: {res:?}");
    res
}
- (CGRect)convertRect:(CGRect)rect
            fromLayer:(id)other { // CALayer*

    if this == other {
        return rect;
    }

    let res = transform_for_conversion(env, this, other).apply_to_rect(rect);
    log_dbg!("Converted {rect:?} from {other:?} to {this:?}: {res:?}");
    res
}
- (CGRect)convertRect:(CGRect)rect
              toLayer:(id)other { // CALayer*
    if this == other {
        return rect;
    }

    let res = transform_for_conversion(env, other, this).apply_to_rect(rect);
    log_dbg!("Converted {rect:?} from {this:?} to {other:?}: {res:?}");
    res
}

- (())addAnimation:(id)anim // CAAnimation*
            forKey:(id)key { // NSString*
    let duration: CFTimeInterval = msg![env; anim duration];
    if duration == 0.0 {
        // From the docs:
        //  If the duration property of the animation is zero or negative, the
        //  duration is changed to the current value of the
        //  kCATransactionAnimationDuration transaction property (if set) or to
        //  the default value of 0.25 seconds.
        let duration: CFTimeInterval = msg_class![env; CATransaction animationDuration];
        () = msg![env; anim setDuration:duration];
    }

    if key == nil {
        log_dbg!("[(CALayer*){:?} addAnimation:{:?} forKey:{:?}]", this, anim, key);
        let inserted = env.objc.borrow_mut::<CALayerHostObject>(this).anonymous_animations.insert(anim);
        assert!(inserted);
    } else {
        let key_string = to_rust_string(env, key);
        log_dbg!("[(CALayer*){:?} addAnimation:{:?} forKey:{:?} ({:?})]", this, anim, key, key_string);
        env.objc.borrow_mut::<CALayerHostObject>(this).animations.insert(key_string.to_string(), anim);
    }
    retain(env, anim);
}

- (())removeAnimationForKey:(id)key { // NSString*
    let key_string = to_rust_string(env, key);
    log_dbg!("[(CALayer*){:?} removeAnimationForKey:{:?} ({:?})]", this, key, key_string);
    if let Some(anim) = env.objc.borrow_mut::<CALayerHostObject>(this).animations.remove(&*key_string) {
        release(env, anim);
    };
}

// TODO: more

@end

};

pub fn remove_anonymous_animation(env: &mut Environment, layer: id, animation: id) {
    let removed = env
        .objc
        .borrow_mut::<CALayerHostObject>(layer)
        .anonymous_animations
        .remove(&animation);
    assert!(removed);
    release(env, animation);
}

fn transform_for_conversion(env: &mut Environment, this: id, other: id) -> CGAffineTransform {
    // The convertPoint methods can be used in two ways:
    // - If two layers are provided (one as the receiver, one as a parameter),
    //   then the layers are required to have a common ancestor, and it will be
    //   used to provide a reference for converting the point/rect.
    // - If one layer is provided, and the other layer is nil, then the layer
    //   is resolved to the co-ordinate space of the origin of the layer at the
    //   top of the hierarchy. This is effectively the same as screen space, or
    //   the co-ordinate space that windows live in.
    let need_common_ancestor = this != nil && other != nil;
    assert!(!(this == nil && other == nil));

    // This algorithm attempts to efficiently find the common ancestor of the
    // two layers by walking up each layer's superlayer chain, one at a time,
    // alternating between layers until it finds a match.
    // For the single-layer case, it of course only walks its superlayer chain.

    // Maps of layer pointers to transforms that map that layer's co-ordinate
    // space to that of the starting layer for the iteration.
    let mut this_map = HashMap::from([(this, CGAffineTransformIdentity)]);
    let mut other_map = HashMap::from([(other, CGAffineTransformIdentity)]);
    // Current iteration state.
    let mut this_superlayer = this;
    let mut this_transform = CGAffineTransformIdentity;
    let mut other_superlayer = other;
    let mut other_transform = CGAffineTransformIdentity;
    let (common_ancestor, this_transform, other_transform) = loop {
        if this_superlayer != nil {
            let this_hostobj: &CALayerHostObject = env.objc.borrow(this_superlayer);
            let next = this_hostobj.superlayer;
            let next_transform =
                this_transform.concat(this_hostobj.superlayer_to_layer_transform());
            if need_common_ancestor && next != nil {
                if let Some(&other_transform) = other_map.get(&next) {
                    break (next, next_transform, other_transform);
                }
                this_map.insert(next, next_transform);
            }
            this_superlayer = next;
            this_transform = next_transform;
        }

        if other_superlayer != nil {
            let other_hostobj: &CALayerHostObject = env.objc.borrow(other_superlayer);
            let next = other_hostobj.superlayer;
            let next_transform =
                other_transform.concat(other_hostobj.superlayer_to_layer_transform());
            if need_common_ancestor && next != nil {
                if let Some(&this_transform) = this_map.get(&next) {
                    break (next, this_transform, next_transform);
                }
                other_map.insert(next, next_transform);
            }
            other_superlayer = next;
            other_transform = next_transform;
        }

        if this_superlayer == nil && other_superlayer == nil {
            if need_common_ancestor {
                panic!("Layers {this:?} and {other:?} have no common ancestor!");
            } else {
                break (nil, this_transform, other_transform);
            }
        }
    };

    assert!((common_ancestor == nil) != need_common_ancestor);
    if need_common_ancestor {
        log_dbg!("{this:?} and {other:?}'s common ancestor: {common_ancestor:?}",);
    }
    log_dbg!("{this:?}'s transform in {common_ancestor:?}: {this_transform:?}");
    log_dbg!("{other:?}'s transform in {common_ancestor:?}: {other_transform:?}");
    let other_to_this = other_transform.concat(this_transform.invert());
    log_dbg!("Transform from {other:?} to {this:?}: {other_to_this:?}");
    other_to_this
}

fn add_default_implied_basic_animation(
    env: &mut Environment,
    layer: id,
    key_path: &'static str,
    from_value: id,
    to_value: id,
) {
    let key_path = get_static_str(env, key_path);
    let animation = msg_class![env; CABasicAnimation animationWithKeyPath:key_path];
    () = msg![env; animation setFromValue: from_value];
    () = msg![env; animation setToValue: to_value];
    ca_transaction::ThreadLocalState::add_animation(env, layer, animation);
}

// TODO: Remove once CAActions are implemented
fn is_implicit_animation_enabled(env: &mut Environment, layer: id) -> bool {
    // CALayers have implicit animations enabled by default, but UIKit doesn't
    // unless there's an active UIView animation block.
    let delegate = msg![env; layer delegate];
    let uiview_class = env.objc.get_known_class("UIView", &mut env.mem);
    let delegate_is_uiview: bool = msg![env; delegate isKindOfClass:uiview_class];
    !delegate_is_uiview || env.framework_state.uikit.ui_view.animation_block_count > 0
}
