/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CGGeometry.h` (`CGPoint`, `CGSize`, `CGRect`, etc)
//!
//! See also [crate::frameworks::uikit::ui_geometry].

use std::ops::{Add, Mul, Sub};

use super::CGFloat;
use crate::abi::{impl_GuestRet_for_large_struct, GuestArg};
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::mem::{MutPtr, SafeRead};
use crate::Environment;

fn parse_tuple(s: &str) -> Result<(f32, f32), ()> {
    let (a, b) = s.split_once(", ").ok_or(())?;
    Ok((a.parse().map_err(|_| ())?, b.parse().map_err(|_| ())?))
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C, packed)]
pub struct CGPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}
unsafe impl SafeRead for CGPoint {}
impl_GuestRet_for_large_struct!(CGPoint);
impl GuestArg for CGPoint {
    const REG_COUNT: usize = 2;

    fn from_regs(regs: &[u32]) -> Self {
        CGPoint {
            x: GuestArg::from_regs(&regs[0..1]),
            y: GuestArg::from_regs(&regs[1..2]),
        }
    }
    fn to_regs(self, regs: &mut [u32]) {
        self.x.to_regs(&mut regs[0..1]);
        self.y.to_regs(&mut regs[1..2]);
    }
}
impl std::str::FromStr for CGPoint {
    type Err = ();
    fn from_str(s: &str) -> Result<CGPoint, ()> {
        let s = s.strip_prefix('{').ok_or(())?.strip_suffix('}').ok_or(())?;
        let (x, y) = parse_tuple(s)?;
        Ok(CGPoint { x, y })
    }
}
impl std::fmt::Display for CGPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let &CGPoint { x, y } = self;
        write!(f, "{{{x}, {y}}}")
    }
}
// Implemented to aid animation code.
// Theres are the operations needed for the interpolation.
impl Mul<f32> for CGPoint {
    type Output = CGPoint;

    fn mul(self, rhs: f32) -> Self::Output {
        CGPoint {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}
impl Add<CGPoint> for CGPoint {
    type Output = CGPoint;

    fn add(self, rhs: CGPoint) -> Self::Output {
        CGPoint {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}
impl Sub<CGPoint> for CGPoint {
    type Output = CGPoint;

    fn sub(self, rhs: CGPoint) -> Self::Output {
        CGPoint {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}
// This function is rare because it is usually inlined.
fn CGPointEqualToPoint(_env: &mut Environment, a: CGPoint, b: CGPoint) -> bool {
    a == b
}

pub const CGPointZero: CGPoint = CGPoint { x: 0.0, y: 0.0 };

#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C, packed)]
pub struct CGSize {
    pub width: CGFloat,
    pub height: CGFloat,
}
unsafe impl SafeRead for CGSize {}
impl_GuestRet_for_large_struct!(CGSize);
impl GuestArg for CGSize {
    const REG_COUNT: usize = 2;

    fn from_regs(regs: &[u32]) -> Self {
        CGSize {
            width: GuestArg::from_regs(&regs[0..1]),
            height: GuestArg::from_regs(&regs[1..2]),
        }
    }
    fn to_regs(self, regs: &mut [u32]) {
        self.width.to_regs(&mut regs[0..1]);
        self.height.to_regs(&mut regs[1..2]);
    }
}
impl std::str::FromStr for CGSize {
    type Err = ();
    fn from_str(s: &str) -> Result<CGSize, ()> {
        let s = s.strip_prefix('{').ok_or(())?.strip_suffix('}').ok_or(())?;
        let (w, h) = parse_tuple(s)?;
        Ok(CGSize {
            width: w,
            height: h,
        })
    }
}
impl std::fmt::Display for CGSize {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let &CGSize { width, height } = self;
        write!(f, "{{{width}, {height}}}")
    }
}
// Implemented to aid animation code.
// Theres are the operations needed for the interpolation.
impl Mul<f32> for CGSize {
    type Output = CGSize;

    fn mul(self, rhs: f32) -> Self::Output {
        CGSize {
            width: self.width * rhs,
            height: self.height * rhs,
        }
    }
}
impl Add<CGSize> for CGSize {
    type Output = CGSize;

    fn add(self, rhs: CGSize) -> Self::Output {
        CGSize {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}
impl Sub<CGSize> for CGSize {
    type Output = CGSize;

    fn sub(self, rhs: CGSize) -> Self::Output {
        CGSize {
            width: self.width - rhs.width,
            height: self.height - rhs.height,
        }
    }
}
// This function is rare because it is usually inlined.
fn CGSizeEqualToSize(_env: &mut Environment, a: CGSize, b: CGSize) -> bool {
    a == b
}

pub const CGSizeZero: CGSize = CGSize {
    width: 0.0,
    height: 0.0,
};

#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C, packed)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}
unsafe impl SafeRead for CGRect {}
impl_GuestRet_for_large_struct!(CGRect);
impl GuestArg for CGRect {
    const REG_COUNT: usize = 4;

    fn from_regs(regs: &[u32]) -> Self {
        CGRect {
            origin: GuestArg::from_regs(&regs[0..2]),
            size: GuestArg::from_regs(&regs[2..4]),
        }
    }
    fn to_regs(self, regs: &mut [u32]) {
        self.origin.to_regs(&mut regs[0..2]);
        self.size.to_regs(&mut regs[2..4]);
    }
}
impl std::str::FromStr for CGRect {
    type Err = ();
    fn from_str(s: &str) -> Result<CGRect, ()> {
        let s = s
            .strip_prefix("{{")
            .ok_or(())?
            .strip_suffix("}}")
            .ok_or(())?;
        let (a, b) = s.split_once("}, {").ok_or(())?;
        let (x, y) = parse_tuple(a)?;
        let (width, height) = parse_tuple(b)?;
        Ok(CGRect {
            origin: CGPoint { x, y },
            size: CGSize { width, height },
        })
    }
}
impl std::fmt::Display for CGRect {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let &CGRect { origin, size } = self;
        write!(f, "{{{origin}, {size}}}")
    }
}
// Implemented to aid animation code.
// Theres are the operations needed for the interpolation.
impl Mul<f32> for CGRect {
    type Output = CGRect;

    fn mul(self, rhs: f32) -> Self::Output {
        CGRect {
            origin: self.origin * rhs,
            size: self.size * rhs,
        }
    }
}
impl Add<CGRect> for CGRect {
    type Output = CGRect;

    fn add(self, rhs: CGRect) -> Self::Output {
        CGRect {
            origin: self.origin + rhs.origin,
            size: self.size + rhs.size,
        }
    }
}
impl Sub<CGRect> for CGRect {
    type Output = CGRect;

    fn sub(self, rhs: CGRect) -> Self::Output {
        CGRect {
            origin: self.origin - rhs.origin,
            size: self.size - rhs.size,
        }
    }
}
// This function is rare because it is usually inlined.
fn CGRectEqualToRect(_env: &mut Environment, a: CGRect, b: CGRect) -> bool {
    a == b
}

pub const CGRectZero: CGRect = CGRect {
    origin: CGPointZero,
    size: CGSizeZero,
};

fn CGRectContainsPoint(_env: &mut Environment, rect: CGRect, point: CGPoint) -> bool {
    rect.origin.x <= point.x
        && rect.origin.x + rect.size.width > point.x
        && rect.origin.y <= point.y
        && rect.origin.y + rect.size.height > point.y
}

/// Whether `rect2` lies entirely within `rect1`.
///
/// Core Graphics documents an empty rectangle as contained by any rectangle,
/// and nothing as containing an empty one except via that rule, so the empty
/// case is answered before the edge comparisons.
fn CGRectContainsRect(_env: &mut Environment, rect1: CGRect, rect2: CGRect) -> bool {
    if rect2.size.width <= 0.0 || rect2.size.height <= 0.0 {
        return true;
    }
    if rect1.size.width <= 0.0 || rect1.size.height <= 0.0 {
        return false;
    }
    rect1.origin.x <= rect2.origin.x
        && rect1.origin.y <= rect2.origin.y
        && rect1.origin.x + rect1.size.width >= rect2.origin.x + rect2.size.width
        && rect1.origin.y + rect1.size.height >= rect2.origin.y + rect2.size.height
}

fn CGRectIntersectsRect(_env: &mut Environment, rect1: CGRect, rect2: CGRect) -> bool {
    rect1.origin.x.max(rect2.origin.x)
        <= (rect1.origin.x + rect1.size.width).min(rect2.origin.x + rect2.size.width)
        && rect1.origin.y.max(rect2.origin.y)
            <= (rect1.origin.y + rect1.size.height).min(rect2.origin.y + rect2.size.height)
}

/// The overlap of two rectangles, or the null rectangle when there is none.
///
/// An empty rectangle — one with no width or no height — overlaps nothing, so
/// it produces the null rectangle rather than being rejected. This used to
/// assert instead, on both the inputs and a zero-area result, which turned
/// ordinary geometry into a crash: an empty rectangle is what a view that has
/// not been laid out yet has, and code that clips one thing against another
/// hands it straight to this function without checking, because on a device
/// there is nothing to check for.
///
/// The Jim and Frank Mysteries HD ended here while opening its first scene.
pub(super) fn CGRectIntersection(_env: &mut Environment, rect1: CGRect, rect2: CGRect) -> CGRect {
    rect_intersection(rect1, rect2)
}

fn rect_intersection(rect1: CGRect, rect2: CGRect) -> CGRect {
    if rect1 == CGRectNull || rect2 == CGRectNull {
        return CGRectNull;
    }
    if rect1.size.width <= 0.0
        || rect1.size.height <= 0.0
        || rect2.size.width <= 0.0
        || rect2.size.height <= 0.0
    {
        return CGRectNull;
    }
    let x = rect1.origin.x.max(rect2.origin.x);
    let y = rect1.origin.y.max(rect2.origin.y);
    let width = (rect1.origin.x + rect1.size.width).min(rect2.origin.x + rect2.size.width) - x;
    let height = (rect1.origin.y + rect1.size.height).min(rect2.origin.y + rect2.size.height) - y;
    // Rectangles that merely touch along an edge produce no area, and Core
    // Graphics counts that as not intersecting: `CGRectIntersectsRect` is false
    // for them too.
    if width <= 0.0 || height <= 0.0 {
        return CGRectNull;
    }
    CGRect {
        origin: CGPoint { x, y },
        size: CGSize { width, height },
    }
}

fn CGRectGetMinX(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.origin.x
}

fn CGRectGetMidX(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.origin.x + rect.size.width / 2.0
}

fn CGRectGetMaxX(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.origin.x + rect.size.width
}

fn CGRectGetMinY(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.origin.y
}

fn CGRectGetMidY(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.origin.y + rect.size.height / 2.0
}

fn CGRectGetMaxY(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.origin.y + rect.size.height
}

fn CGRectGetHeight(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.size.height
}

fn CGRectGetWidth(_env: &mut Environment, rect: CGRect) -> CGFloat {
    rect.size.width
}

fn CGRectMake(
    _env: &mut Environment,
    x: CGFloat,
    y: CGFloat,
    width: CGFloat,
    height: CGFloat,
) -> CGRect {
    CGRect {
        origin: CGPoint { x, y },
        size: CGSize { width, height },
    }
}

pub const CGRectNull: CGRect = CGRect {
    origin: CGPoint {
        x: f32::INFINITY,
        y: f32::INFINITY,
    },
    size: CGSizeZero,
};

fn CGRectIsNull(_env: &mut Environment, rect: CGRect) -> bool {
    rect == CGRectNull
}

fn CGRectOffset(_env: &mut Environment, rect: CGRect, dx: CGFloat, dy: CGFloat) -> CGRect {
    assert!(rect != CGRectNull); // TODO
    CGRect {
        origin: CGPoint {
            x: rect.origin.x + dx,
            y: rect.origin.y + dy,
        },
        size: rect.size,
    }
}

fn CGRectInset(_env: &mut Environment, rect: CGRect, dx: CGFloat, dy: CGFloat) -> CGRect {
    let res = CGRect {
        origin: CGPoint {
            x: rect.origin.x + dx,
            y: rect.origin.y + dy,
        },
        size: CGSize {
            width: rect.size.width - 2.0 * dx,
            height: rect.size.height - 2.0 * dy,
        },
    };
    assert!(res.size.width >= 0.0); // TODO return a null rectangle
    assert!(res.size.height >= 0.0); // TODO return a null rectangle

    // center invariant
    assert!(rect.origin.x + rect.size.width / 2.0 == res.origin.x + res.size.width / 2.0);
    assert!(rect.origin.y + rect.size.height / 2.0 == res.origin.y + res.size.height / 2.0);
    res
}

/// Whether a rectangle encloses no area.
///
/// A zero *or negative* width or height is empty, and so is the null rectangle,
/// which this reports through the same size test rather than as a special case.
/// The distinction from `CGRectIsNull` is worth keeping straight: null is one
/// specific rectangle, empty is a property many rectangles have.
fn CGRectIsEmpty(_env: &mut Environment, rect: CGRect) -> bool {
    rect_is_empty(rect)
}

fn rect_is_empty(rect: CGRect) -> bool {
    // Tested for being positive rather than for being non-positive so that a NaN
    // extent counts as empty. A rectangle whose size has picked up a NaN from
    // guest arithmetic encloses nothing anyone can draw, and calling it
    // non-empty would send it on to code that then divides by it.
    let positive = |v: CGFloat| v.partial_cmp(&0.0) == Some(std::cmp::Ordering::Greater);
    !(positive(rect.size.width) && positive(rect.size.height))
}

/// Turn a rectangle with a negative width or height into the equivalent one with
/// positive extents.
///
/// Guest code produces these constantly by subtracting two points in whichever
/// order they arrived, and most of Core Graphics is specified in terms of the
/// standardised form, so this is what makes such a rectangle usable rather than
/// a special case at every call site.
fn CGRectStandardize(_env: &mut Environment, rect: CGRect) -> CGRect {
    standardize(rect)
}

fn standardize(rect: CGRect) -> CGRect {
    let (x, width) = if rect.size.width < 0.0 {
        (rect.origin.x + rect.size.width, -rect.size.width)
    } else {
        (rect.origin.x, rect.size.width)
    };
    let (y, height) = if rect.size.height < 0.0 {
        (rect.origin.y + rect.size.height, -rect.size.height)
    } else {
        (rect.origin.y, rect.size.height)
    };
    CGRect {
        origin: CGPoint { x, y },
        size: CGSize { width, height },
    }
}

/// The smallest rectangle containing both arguments.
///
/// An empty rectangle contributes nothing, which is the specified behaviour and
/// not a shortcut: unioning with `CGRectZero` would otherwise drag the result
/// out to include the origin, which is a classic source of views sized to reach
/// the top-left corner of the screen.
fn CGRectUnion(_env: &mut Environment, rect1: CGRect, rect2: CGRect) -> CGRect {
    if rect_is_empty(rect1) {
        return standardize(rect2);
    }
    if rect_is_empty(rect2) {
        return standardize(rect1);
    }
    let (a, b) = (standardize(rect1), standardize(rect2));
    let min_x = a.origin.x.min(b.origin.x);
    let min_y = a.origin.y.min(b.origin.y);
    let max_x = (a.origin.x + a.size.width).max(b.origin.x + b.size.width);
    let max_y = (a.origin.y + a.size.height).max(b.origin.y + b.size.height);
    CGRect {
        origin: CGPoint { x: min_x, y: min_y },
        size: CGSize {
            width: max_x - min_x,
            height: max_y - min_y,
        },
    }
}

/// Which edge `CGRectDivide` cuts from.
type CGRectEdge = u32;
const CGRectMinXEdge: CGRectEdge = 0;
const CGRectMinYEdge: CGRectEdge = 1;
const CGRectMaxXEdge: CGRectEdge = 2;
const CGRectMaxYEdge: CGRectEdge = 3;

/// Split a rectangle into a slice of the given thickness taken off one edge, and
/// the remainder.
///
/// The two out-parameters are what makes this awkward to use and easy to get
/// wrong: `slice` is the piece cut off, `remainder` is what is left, and either
/// pointer may be null. An amount larger than the rectangle gives the whole
/// rectangle as the slice and an empty remainder pinned to the far edge, which
/// is the specified result rather than an error.
fn CGRectDivide(
    env: &mut Environment,
    rect: CGRect,
    slice: MutPtr<CGRect>,
    remainder: MutPtr<CGRect>,
    amount: CGFloat,
    edge: CGRectEdge,
) {
    let (slice_rect, remainder_rect) = divide(rect, amount, edge);
    if !slice.is_null() {
        env.mem.write(slice, slice_rect);
    }
    if !remainder.is_null() {
        env.mem.write(remainder, remainder_rect);
    }
}

fn divide(rect: CGRect, amount: CGFloat, edge: CGRectEdge) -> (CGRect, CGRect) {
    let rect = standardize(rect);
    let CGRect { origin, size } = rect;
    // A negative amount cuts nothing; the specification clamps into the
    // rectangle at both ends. Written as max-then-min rather than with `clamp`
    // because `clamp` panics on a NaN bound, and the extent comes from the
    // guest.
    let along_x = edge == CGRectMinXEdge || edge == CGRectMaxXEdge;
    let extent = if along_x { size.width } else { size.height };
    let cut = amount.max(0.0).min(extent);
    let rest = extent - cut;

    let sized = |x: CGFloat, y: CGFloat, width: CGFloat, height: CGFloat| CGRect {
        origin: CGPoint { x, y },
        size: CGSize { width, height },
    };
    match edge {
        CGRectMinXEdge => (
            sized(origin.x, origin.y, cut, size.height),
            sized(origin.x + cut, origin.y, rest, size.height),
        ),
        CGRectMaxXEdge => (
            sized(origin.x + rest, origin.y, cut, size.height),
            sized(origin.x, origin.y, rest, size.height),
        ),
        CGRectMinYEdge => (
            sized(origin.x, origin.y, size.width, cut),
            sized(origin.x, origin.y + cut, size.width, rest),
        ),
        CGRectMaxYEdge => (
            sized(origin.x, origin.y + rest, size.width, cut),
            sized(origin.x, origin.y, size.width, rest),
        ),
        // Not one of the four edges. Core Graphics has no defined answer and an
        // app that gets here has passed uninitialised memory, so the whole
        // rectangle comes back as the remainder and nothing is cut.
        _ => {
            log!("CGRectDivide() with unknown edge {}; cutting nothing", edge);
            (CGRectZero, rect)
        }
    }
}

pub(super) fn CGRectIntegral(_env: &mut Environment, rect: CGRect) -> CGRect {
    if rect == CGRectNull {
        return rect;
    }
    assert!(
        rect.size.width >= 0.0 && rect.size.height >= 0.0,
        "unexpected {}",
        rect
    );
    let new_x = rect.origin.x.floor();
    let new_y = rect.origin.y.floor();
    let new_width = (rect.origin.x + rect.size.width).ceil() - new_x;
    let new_height = (rect.origin.y + rect.size.height).ceil() - new_y;
    CGRect {
        origin: CGPoint { x: new_x, y: new_y },
        size: CGSize {
            width: new_width,
            height: new_height,
        },
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGPointEqualToPoint(_, _)),
    export_c_func!(CGSizeEqualToSize(_, _)),
    export_c_func!(CGRectEqualToRect(_, _)),
    export_c_func!(CGRectContainsPoint(_, _)),
    export_c_func!(CGRectContainsRect(_, _)),
    export_c_func!(CGRectIntersectsRect(_, _)),
    export_c_func!(CGRectIntersection(_, _)),
    export_c_func!(CGRectGetMinX(_)),
    export_c_func!(CGRectGetMidX(_)),
    export_c_func!(CGRectGetMaxX(_)),
    export_c_func!(CGRectGetMinY(_)),
    export_c_func!(CGRectGetMidY(_)),
    export_c_func!(CGRectGetMaxY(_)),
    export_c_func!(CGRectGetHeight(_)),
    export_c_func!(CGRectGetWidth(_)),
    export_c_func!(CGRectMake(_, _, _, _)),
    export_c_func!(CGRectIsNull(_)),
    export_c_func!(CGRectIsEmpty(_)),
    export_c_func!(CGRectStandardize(_)),
    export_c_func!(CGRectUnion(_, _)),
    export_c_func!(CGRectDivide(_, _, _, _, _)),
    export_c_func!(CGRectOffset(_, _, _)),
    export_c_func!(CGRectInset(_, _, _)),
    export_c_func!(CGRectIntegral(_)),
];

pub const CONSTANTS: ConstantExports = &[
    (
        "_CGSizeZero",
        HostConstant::Custom(|env| env.mem.alloc_and_write(CGSizeZero).cast().cast_const()),
    ),
    (
        "_CGPointZero",
        HostConstant::Custom(|env| env.mem.alloc_and_write(CGPointZero).cast().cast_const()),
    ),
    (
        "_CGRectZero",
        HostConstant::Custom(|env| env.mem.alloc_and_write(CGRectZero).cast().cast_const()),
    ),
    (
        "_CGRectNull",
        HostConstant::Custom(|env| env.mem.alloc_and_write(CGRectNull).cast().cast_const()),
    ),
];

#[cfg(test)]
mod tests {
    use super::{
        divide, rect_intersection, rect_is_empty, standardize, CGPoint, CGRect, CGRectMaxXEdge,
        CGRectMaxYEdge, CGRectMinXEdge, CGRectMinYEdge, CGRectNull, CGRectZero, CGSize,
    };

    /// The ordinary case, so the null answers below are not vacuously right.
    #[test]
    fn overlapping_rectangles_intersect() {
        assert_eq!(
            rect_intersection(rect(0.0, 0.0, 10.0, 10.0), rect(5.0, 5.0, 10.0, 10.0)),
            rect(5.0, 5.0, 5.0, 5.0)
        );
    }

    /// An empty rectangle overlaps nothing. This used to assert, and an
    /// unlaid-out view is empty, so games reached it on ordinary paths.
    #[test]
    fn an_empty_rectangle_intersects_nothing() {
        assert_eq!(
            rect_intersection(CGRectZero, rect(0.0, 0.0, 10.0, 10.0)),
            CGRectNull
        );
        assert_eq!(
            rect_intersection(rect(0.0, 0.0, 10.0, 0.0), rect(0.0, 0.0, 10.0, 10.0)),
            CGRectNull
        );
        assert_eq!(
            rect_intersection(rect(0.0, 0.0, 10.0, 10.0), rect(2.0, 2.0, 0.0, 5.0)),
            CGRectNull
        );
    }

    /// Touching along an edge is not intersecting, which is also what
    /// `CGRectIntersectsRect` says.
    #[test]
    fn edge_contact_is_not_an_intersection() {
        assert_eq!(
            rect_intersection(rect(0.0, 0.0, 10.0, 10.0), rect(10.0, 0.0, 10.0, 10.0)),
            CGRectNull
        );
    }

    #[test]
    fn separated_rectangles_do_not_intersect() {
        assert_eq!(
            rect_intersection(rect(0.0, 0.0, 5.0, 5.0), rect(20.0, 20.0, 5.0, 5.0)),
            CGRectNull
        );
    }

    fn rect(x: f32, y: f32, width: f32, height: f32) -> CGRect {
        CGRect {
            origin: CGPoint { x, y },
            size: CGSize { width, height },
        }
    }

    #[test]
    fn emptiness_covers_zero_negative_and_null() {
        assert!(!rect_is_empty(rect(0.0, 0.0, 1.0, 1.0)));
        assert!(rect_is_empty(CGRectZero));
        assert!(rect_is_empty(rect(5.0, 5.0, 0.0, 10.0)));
        assert!(rect_is_empty(rect(5.0, 5.0, 10.0, 0.0)));
        assert!(rect_is_empty(rect(5.0, 5.0, -10.0, 10.0)));
        // The null rectangle has a zero size, so it falls out of the same test
        // rather than needing its own.
        assert!(rect_is_empty(CGRectNull));
    }

    #[test]
    fn standardizing_moves_the_origin_to_the_smaller_corner() {
        assert_eq!(
            standardize(rect(10.0, 20.0, -4.0, -6.0)),
            rect(6.0, 14.0, 4.0, 6.0)
        );
        // Already positive: unchanged.
        assert_eq!(
            standardize(rect(1.0, 2.0, 3.0, 4.0)),
            rect(1.0, 2.0, 3.0, 4.0)
        );
    }

    #[test]
    fn dividing_off_the_low_edge_puts_the_slice_first() {
        let (slice, remainder) = divide(rect(0.0, 0.0, 100.0, 50.0), 30.0, CGRectMinXEdge);
        assert_eq!(slice, rect(0.0, 0.0, 30.0, 50.0));
        assert_eq!(remainder, rect(30.0, 0.0, 70.0, 50.0));

        let (slice, remainder) = divide(rect(0.0, 0.0, 100.0, 50.0), 20.0, CGRectMinYEdge);
        assert_eq!(slice, rect(0.0, 0.0, 100.0, 20.0));
        assert_eq!(remainder, rect(0.0, 20.0, 100.0, 30.0));
    }

    #[test]
    fn dividing_off_the_high_edge_puts_the_slice_at_the_far_end() {
        let (slice, remainder) = divide(rect(0.0, 0.0, 100.0, 50.0), 30.0, CGRectMaxXEdge);
        assert_eq!(slice, rect(70.0, 0.0, 30.0, 50.0));
        assert_eq!(remainder, rect(0.0, 0.0, 70.0, 50.0));

        let (slice, remainder) = divide(rect(0.0, 0.0, 100.0, 50.0), 20.0, CGRectMaxYEdge);
        assert_eq!(slice, rect(0.0, 30.0, 100.0, 20.0));
        assert_eq!(remainder, rect(0.0, 0.0, 100.0, 30.0));
    }

    #[test]
    fn dividing_by_more_than_the_rectangle_holds_gives_it_all_away() {
        let (slice, remainder) = divide(rect(0.0, 0.0, 100.0, 50.0), 500.0, CGRectMinXEdge);
        assert_eq!(slice, rect(0.0, 0.0, 100.0, 50.0));
        assert_eq!(remainder, rect(100.0, 0.0, 0.0, 50.0));
        // And a negative amount cuts nothing at all.
        let (slice, remainder) = divide(rect(0.0, 0.0, 100.0, 50.0), -5.0, CGRectMinXEdge);
        assert_eq!(slice, rect(0.0, 0.0, 0.0, 50.0));
        assert_eq!(remainder, rect(0.0, 0.0, 100.0, 50.0));
    }
}
