/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIColor`.

use super::ui_graphics::UIGraphicsGetCurrentContext;
use crate::frameworks::core_graphics::cg_color::{CGColorRef, CGColorRelease, CGColorRetain};
use crate::frameworks::core_graphics::cg_context::CGContextSetRGBFillColor;
use crate::frameworks::core_graphics::{cg_color, CGFloat};
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::frameworks::foundation::NSInteger;
use crate::mem::MutPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, ClassExports, HostObject, NSZonePtr, ObjC,
    SEL,
};
use crate::Environment;
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    standard_colors: HashMap<SEL, id>,
}

/// Convert HSB (as UIKit and nib archives use it, all components 0…1) to RGB.
///
/// This is the standard conversion: hue selects a face of the colour hex,
/// saturation mixes towards white and brightness scales the result.
fn hsb_to_rgb(
    hue: CGFloat,
    saturation: CGFloat,
    brightness: CGFloat,
) -> (CGFloat, CGFloat, CGFloat) {
    let saturation = saturation.clamp(0.0, 1.0);
    let brightness = brightness.clamp(0.0, 1.0);
    if saturation == 0.0 {
        return (brightness, brightness, brightness);
    }
    // A hue of exactly 1.0 is the same colour as 0.0, and must not select a
    // sixth sector that does not exist.
    let hue = hue.rem_euclid(1.0) * 6.0;
    let sector = hue.floor();
    let offset = hue - sector;
    let p = brightness * (1.0 - saturation);
    let q = brightness * (1.0 - saturation * offset);
    let t = brightness * (1.0 - saturation * (1.0 - offset));
    match sector as i32 {
        0 => (brightness, t, p),
        1 => (q, brightness, p),
        2 => (p, brightness, t),
        3 => (p, q, brightness),
        4 => (t, p, brightness),
        _ => (brightness, p, q),
    }
}

fn get_standard_color(
    env: &mut Environment,
    sel: SEL,
    r: CGFloat,
    g: CGFloat,
    b: CGFloat,
    a: CGFloat,
) -> id {
    if let Some(&existing) = env.framework_state.uikit.ui_color.standard_colors.get(&sel) {
        existing
    } else {
        let new: id = msg_class![env; _tapHLE_UIColor_Static alloc];
        let new: id = msg![env; new initWithRed:r green:g blue:b alpha:a];
        env.framework_state
            .uikit
            .ui_color
            .standard_colors
            .insert(sel, new);
        new
    }
}

struct UIColorHostObject {
    cg_color: CGColorRef,
}
impl HostObject for UIColorHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIColor: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIColorHostObject {
        cg_color: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)colorWithCGColor:(CGColorRef)cg_color {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCGColor:cg_color];
    autorelease(env, new)
}

+ (id)colorWithRed:(CGFloat)r
             green:(CGFloat)g
              blue:(CGFloat)b
             alpha:(CGFloat)a {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithRed:r green:g blue:b alpha:a];
    autorelease(env, new)
}

+ (id)colorWithWhite:(CGFloat)w alpha:(CGFloat)a {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithWhite:w alpha:a];
    autorelease(env, new)
}

// A colour that tiles an image. tapHLE's colours are a single RGBA quadruple,
// with no pattern to carry, so this returns a clear colour rather than a wrong
// solid one: the pattern simply does not draw, leaving whatever is behind it
// visible. Substituting an opaque colour would cover content the app expected
// to see through the texture, which is a worse and much harder to diagnose
// result than a missing background.
+ (id)colorWithPatternImage:(id)image { // UIImage*
    log!("TODO: [UIColor colorWithPatternImage:{:?}] - pattern colours are not supported; returning a clear colour", image);
    msg![env; this clearColor]
}

+ (id)clearColor    { get_standard_color(env, _cmd, 0.0, 0.0, 0.0, 0.0) }
+ (id)blackColor    { get_standard_color(env, _cmd, 0.0, 0.0, 0.0, 1.0) }
+ (id)whiteColor    { get_standard_color(env, _cmd, 1.0, 1.0, 1.0, 1.0) }
+ (id)darkGrayColor {
    get_standard_color(env, _cmd, 1.0/3.0, 1.0/3.0, 1.0/3.0, 1.0)
}
+ (id)grayColor {
    get_standard_color(env, _cmd, 1.0/2.0, 1.0/2.0, 1.0/2.0, 1.0)
}
+ (id)lightGrayColor {
    get_standard_color(env, _cmd, 2.0/3.0, 2.0/3.0, 2.0/3.0, 1.0)
}
+ (id)blueColor     { get_standard_color(env, _cmd, 0.0, 0.0, 1.0, 1.0) }
+ (id)brownColor    { get_standard_color(env, _cmd, 0.6, 0.4, 0.2, 1.0) }
+ (id)cyanColor     { get_standard_color(env, _cmd, 0.0, 1.0, 1.0, 1.0) }
+ (id)greenColor    { get_standard_color(env, _cmd, 0.0, 1.0, 0.0, 1.0) }
+ (id)magentaColor  { get_standard_color(env, _cmd, 1.0, 0.0, 1.0, 1.0) }
+ (id)orangeColor   { get_standard_color(env, _cmd, 1.0, 0.5, 0.0, 1.0) }
+ (id)purpleColor   { get_standard_color(env, _cmd, 0.5, 0.0, 1.5, 1.0) }
+ (id)redColor      { get_standard_color(env, _cmd, 1.0, 0.0, 0.0, 1.0) }
+ (id)yellowColor   { get_standard_color(env, _cmd, 1.0, 1.0, 0.0, 1.0) }

// TODO: more initializers, set methods, more accessors

- (id)initWithCGColor:(CGColorRef)cg_color {
    CGColorRetain(env, cg_color);
    env.objc.borrow_mut::<UIColorHostObject>(this).cg_color = cg_color;
    this
}

- (id)initWithWhite:(CGFloat)w alpha:(CGFloat)a {
    let w = w.clamp(0.0, 1.0);
    let a = a.clamp(0.0, 1.0);

    env.objc.borrow_mut::<UIColorHostObject>(this).cg_color = cg_color::from_rgba(env, (w, w, w, a));

    this
}

- (id)initWithRed:(CGFloat)r
            green:(CGFloat)g
             blue:(CGFloat)b
            alpha:(CGFloat)a {
    env.objc.borrow_mut::<UIColorHostObject>(this).cg_color = cg_color::from_rgba(env, (r, g, b, a));
    this
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    let key_ns_string = get_static_str(env, "UIAlpha");
    let a: CGFloat = msg![env; coder decodeFloatForKey:key_ns_string];

    let key_ns_string = get_static_str(env, "UIColorComponentCount");
    let count: NSInteger = msg![env; coder decodeIntegerForKey:key_ns_string];

    // Dispatch on which components are actually present rather than on the
    // count. The count does not identify the colour space — RGBA and HSBA both
    // have four — and archives in the wild carry counts this once rejected
    // outright, which aborted the app over a colour.
    let red_key = get_static_str(env, "UIRed");
    let white_key = get_static_str(env, "UIWhite");
    let hue_key = get_static_str(env, "UIHue");

    if msg![env; coder containsValueForKey:red_key] {
        let r: CGFloat = msg![env; coder decodeFloatForKey:red_key];
        let key_ns_string = get_static_str(env, "UIGreen");
        let g: CGFloat = msg![env; coder decodeFloatForKey:key_ns_string];
        let key_ns_string = get_static_str(env, "UIBlue");
        let b: CGFloat = msg![env; coder decodeFloatForKey:key_ns_string];
        log_dbg!(
            "[(UIColor*){:?} initWithCoder:{:?}] => count {}, r {}, g {}, b {}, a {}",
            this, coder, count, r, g, b, a
        );
        msg![env; this initWithRed:r green:g blue:b alpha:a]
    } else if msg![env; coder containsValueForKey:white_key] {
        let w: CGFloat = msg![env; coder decodeFloatForKey:white_key];
        log_dbg!(
            "[(UIColor*){:?} initWithCoder:{:?}] => count {}, w {}, a {}",
            this, coder, count, w, a
        );
        msg![env; this initWithWhite:w alpha:a]
    } else if msg![env; coder containsValueForKey:hue_key] {
        let h: CGFloat = msg![env; coder decodeFloatForKey:hue_key];
        let key_ns_string = get_static_str(env, "UISaturation");
        let s: CGFloat = msg![env; coder decodeFloatForKey:key_ns_string];
        let key_ns_string = get_static_str(env, "UIBrightness");
        let v: CGFloat = msg![env; coder decodeFloatForKey:key_ns_string];
        let (r, g, b) = hsb_to_rgb(h, s, v);
        log_dbg!(
            "[(UIColor*){:?} initWithCoder:{:?}] => count {}, h {}, s {}, b {}, a {}",
            this, coder, count, h, s, v, a
        );
        msg![env; this initWithRed:r green:g blue:b alpha:a]
    } else {
        // Pattern colours and any encoding not handled above land here. Opaque
        // black is wrong, but it is a colour: the archive this came from is a
        // whole view hierarchy, and refusing to decode one fill would throw the
        // rest of it away too.
        log!(
            "TODO: [(UIColor*){:?} initWithCoder:{:?}] has {} components in no recognised colour space; using black",
            this, coder, count
        );
        msg![env; this initWithRed:0.0 green:0.0 blue:0.0 alpha:a]
    }
}

- (bool)getRed:(MutPtr<CGFloat>)r
         green:(MutPtr<CGFloat>)g
          blue:(MutPtr<CGFloat>)b
         alpha:(MutPtr<CGFloat>)a {
    let color = env.objc.borrow::<UIColorHostObject>(this).cg_color;
    let (r_, g_, b_, a_) = cg_color::to_rgba(&env.objc, color);
    env.mem.write(r, r_);
    env.mem.write(g, g_);
    env.mem.write(b, b_);
    env.mem.write(a, a_);
    true
}

- (())set {
    msg![env; this setFill]
    // TODO: set stroke color as well
}

- (())setFill {
    let context = UIGraphicsGetCurrentContext(env);
    assert_ne!(context, nil);
    let (r, g, b, a) = get_rgba(&env.objc, this);
    CGContextSetRGBFillColor(env, context, r, g, b, a);
}

- (CGColorRef)CGColor {
    env.objc.borrow::<UIColorHostObject>(this).cg_color
}

- (())dealloc {
    let color = env.objc.borrow_mut::<UIColorHostObject>(this).cg_color;
    CGColorRelease(env, color);

    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)colorWithAlphaComponent:(CGFloat)a {
    let a = a.clamp(0.0, 1.0);
    let (r, g, b, _) = get_rgba(&env.objc, this);
    msg_class![env; UIColor colorWithRed:r green:g blue:b alpha:a]
}

@end

// Undocumented classes used in NIBs
@implementation UICGColor: UIColor
@end
@implementation UIDeviceRGBColor: UIColor
@end

// Special subclass for standard colors with a static lifetime.
// See `get_standard_color`.
@implementation _tapHLE_UIColor_Static: UIColor

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIColorHostObject {
        cg_color: nil,
    });
    env.objc.alloc_static_object(this, host_object, &mut env.mem)
}

- (id) retain { this }
- (()) release {}
- (id) autorelease { this }

@end

};

/// Shortcut for use in Core Animation's compositor: get the RGBA triple for a
/// `UIColor*`.
pub fn get_rgba(objc: &ObjC, ui_color: id) -> (CGFloat, CGFloat, CGFloat, CGFloat) {
    let color = objc.borrow::<UIColorHostObject>(ui_color).cg_color;
    cg_color::to_rgba(objc, color)
}
