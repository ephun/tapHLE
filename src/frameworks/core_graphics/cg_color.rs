/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CGColor.h`

use std::ops::{Add, Mul, Sub};

use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_foundation::{CFRelease, CFRetain, CFTypeRef};
use crate::frameworks::core_graphics::cg_color_space::{
    self as cg_color_space, kCGColorSpaceGenericRGB, CGColorSpaceHostObject, CGColorSpaceRef,
};
use crate::frameworks::core_graphics::CGFloat;
use crate::mem::{guest_size_of, ConstPtr, GuestUSize, MutPtr, Ptr};
use crate::objc::{objc_classes, ClassExports, HostObject, ObjC};
use crate::Environment;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// CGColor seems to be a CFType-based type, but in our implementation
// those are just Objective-C types, so we need a class for it, but its name is
// not visible anywhere.
@implementation _tapHLE_CGColor: NSObject

- (())dealloc {
    // Free the component array CGColorGetComponents may have materialised. It
    // is owned by this colour, so its lifetime ends here; without this the
    // array would outlive the only thing that could ever free it.
    let components = env.objc.borrow::<CGColorHostObject>(this).components;
    if !components.is_null() {
        env.mem.free(components.cast());
    }
    // Same reasoning for the colour space CGColorGetColorSpace may have
    // materialised: the colour owns it, so its last reference goes here.
    let color_space = env.objc.borrow::<CGColorHostObject>(this).color_space;
    if !color_space.is_null() {
        crate::frameworks::core_foundation::CFRelease(env, color_space);
    }
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

};

#[derive(Copy, Clone)]
pub struct CGColorHostObject {
    pub color_space_name: &'static str,
    /// Guest-visible component array, created on demand by
    /// `CGColorGetComponents` and owned by this colour. Null until asked for.
    pub components: MutPtr<CGFloat>,
    /// Guest-visible colour space object, created on demand by
    /// `CGColorGetColorSpace` and owned by this colour, for the same reason.
    pub color_space: CGColorSpaceRef,
    // this assumes usage of CGColorSpaceGenericRGB
    // TODO: support other color spaces
    pub r: CGFloat,
    pub g: CGFloat,
    pub b: CGFloat,
    pub a: CGFloat,
}
impl HostObject for CGColorHostObject {}
// Implemented to aid animation code.
// Theres are the operations needed for the interpolation.
impl Mul<f32> for CGColorHostObject {
    type Output = CGColorHostObject;

    fn mul(self, rhs: f32) -> Self::Output {
        CGColorHostObject {
            components: Ptr::null(),
            color_space: Ptr::null(),
            color_space_name: self.color_space_name,
            r: self.r * rhs,
            g: self.g * rhs,
            b: self.b * rhs,
            a: self.a * rhs,
        }
    }
}
impl Add<CGColorHostObject> for CGColorHostObject {
    type Output = CGColorHostObject;

    fn add(self, rhs: CGColorHostObject) -> Self::Output {
        CGColorHostObject {
            components: Ptr::null(),
            color_space: Ptr::null(),
            color_space_name: self.color_space_name,
            r: self.r + rhs.r,
            g: self.g + rhs.g,
            b: self.b + rhs.b,
            a: self.a + rhs.a,
        }
    }
}
impl Sub<CGColorHostObject> for CGColorHostObject {
    type Output = CGColorHostObject;

    fn sub(self, rhs: CGColorHostObject) -> Self::Output {
        CGColorHostObject {
            components: Ptr::null(),
            color_space: Ptr::null(),
            color_space_name: self.color_space_name,
            r: self.r - rhs.r,
            g: self.g - rhs.g,
            b: self.b - rhs.b,
            a: self.a - rhs.a,
        }
    }
}

pub type CGColorRef = CFTypeRef;
pub fn CGColorRelease(env: &mut Environment, c: CGColorRef) {
    if !c.is_null() {
        CFRelease(env, c);
    }
}
pub fn CGColorRetain(env: &mut Environment, c: CGColorRef) -> CGColorRef {
    if !c.is_null() {
        CFRetain(env, c)
    } else {
        c
    }
}

fn CGColorCreate(
    env: &mut Environment,
    space: CGColorSpaceRef,
    components: MutPtr<CGFloat>,
) -> CGColorRef {
    let color_space = env.objc.borrow::<CGColorSpaceHostObject>(space).name;
    assert_eq!(color_space, kCGColorSpaceGenericRGB);
    let r = env.mem.read(components);
    let g = env.mem.read(components + 1);
    let b = env.mem.read(components + 2);
    let a = env.mem.read(components + 3);
    from_rgba(env, (r, g, b, a))
}

fn CGColorCreateGenericRGB(
    env: &mut Environment,
    r: CGFloat,
    g: CGFloat,
    b: CGFloat,
    a: CGFloat,
) -> CGColorRef {
    from_rgba(env, (r, g, b, a))
}

fn CGColorEqualToColor(env: &mut Environment, a: CGColorRef, b: CGColorRef) -> bool {
    to_rgba(&env.objc, a) == to_rgba(&env.objc, b)
}

/// `CGColorGetComponents` — the colour's components as a C array.
///
/// The real function hands back a pointer into storage the colour owns, valid
/// for as long as the colour is. tapHLE keeps its components as Rust fields
/// with no guest-visible array behind them, so one is materialised on demand
/// and cached on the colour, which reproduces that lifetime: the same pointer
/// every time, alive as long as the colour, and freed with it.
///
/// Returning a fresh allocation per call would leak, and a stack temporary
/// would dangle the moment this returned.
fn CGColorGetComponents(env: &mut Environment, color: CGColorRef) -> ConstPtr<CGFloat> {
    if color.is_null() {
        return Ptr::null();
    }
    let existing = env.objc.borrow::<CGColorHostObject>(color).components;
    if !existing.is_null() {
        return existing.cast_const();
    }
    let &CGColorHostObject { r, g, b, a, .. } = env.objc.borrow(color);
    let components: MutPtr<CGFloat> = env.mem.alloc(guest_size_of::<CGFloat>() * 4).cast();
    for (index, value) in [r, g, b, a].into_iter().enumerate() {
        env.mem.write(components + index as GuestUSize, value);
    }
    env.objc.borrow_mut::<CGColorHostObject>(color).components = components;
    components.cast_const()
}

/// `CGColorGetColorSpace` — the colour space the colour was created in.
///
/// The real function returns a space the colour owns; the caller does not
/// retain it and must not release it. tapHLE's colours store their space as a
/// name rather than as an object, so one is created on first ask and cached on
/// the colour, exactly as [CGColorGetComponents] handles the same problem: the
/// same pointer every time, alive as long as the colour, released with it.
fn CGColorGetColorSpace(env: &mut Environment, color: CGColorRef) -> CGColorSpaceRef {
    if color.is_null() {
        return Ptr::null();
    }
    let existing = env.objc.borrow::<CGColorHostObject>(color).color_space;
    if !existing.is_null() {
        return existing;
    }
    let name = env.objc.borrow::<CGColorHostObject>(color).color_space_name;
    let space = cg_color_space::from_name(env, name);
    env.objc.borrow_mut::<CGColorHostObject>(color).color_space = space;
    space
}

/// The number of components, which is four for every colour space tapHLE
/// models.
///
/// Note that this counts alpha, unlike `CGColorSpaceGetNumberOfComponents`,
/// which does not.
fn CGColorGetNumberOfComponents(_env: &mut Environment, color: CGColorRef) -> GuestUSize {
    if color.is_null() {
        0
    } else {
        4
    }
}

fn CGColorGetAlpha(env: &mut Environment, color: CGColorRef) -> CGFloat {
    if color.is_null() {
        return 0.0;
    }
    env.objc.borrow::<CGColorHostObject>(color).a
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGColorGetComponents(_)),
    export_c_func!(CGColorGetColorSpace(_)),
    export_c_func!(CGColorGetNumberOfComponents(_)),
    export_c_func!(CGColorGetAlpha(_)),
    export_c_func!(CGColorRetain(_)),
    export_c_func!(CGColorRelease(_)),
    export_c_func!(CGColorCreate(_, _)),
    export_c_func!(CGColorCreateGenericRGB(_, _, _, _)),
    export_c_func!(CGColorEqualToColor(_, _)),
];

/// Shortcut for use by `UIColor`: directly construct a `CGColor` instance from
/// an rgba tuple of CGFloats.
pub fn from_rgba(env: &mut Environment, rgba: (CGFloat, CGFloat, CGFloat, CGFloat)) -> CGColorRef {
    let (r, g, b, a) = rgba;
    let host_obj = Box::new(CGColorHostObject {
        components: Ptr::null(),
        color_space: Ptr::null(),
        color_space_name: kCGColorSpaceGenericRGB,
        r,
        g,
        b,
        a,
    });
    let class = env.objc.get_known_class("_tapHLE_CGColor", &mut env.mem);
    env.objc.alloc_object(class, host_obj, &mut env.mem)
}

/// Shortcut for use by `UIColor`
pub fn to_rgba(objc: &ObjC, color: CGColorRef) -> (CGFloat, CGFloat, CGFloat, CGFloat) {
    let &CGColorHostObject {
        color_space_name,
        r,
        g,
        b,
        a,
        ..
    } = objc.borrow(color);
    assert_eq!(color_space_name, kCGColorSpaceGenericRGB);
    (r, g, b, a)
}
