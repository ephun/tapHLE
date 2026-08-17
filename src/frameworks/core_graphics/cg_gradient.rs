/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CGGradient.h`, and the two `CGContext` functions that draw one.
//!
//! Gradients are how apps of this era drew almost every non-image
//! background, button and title bar, so the demand is high: of the 1192
//! distinct apps in the import-demand catalogue, 439 import
//! `CGContextDrawLinearGradient`, 417 `CGGradientRelease` and 362
//! `CGGradientCreateWithColorComponents`. Calling an unimplemented one ends
//! the app, so a game that drew a gradient anywhere in its start-up path
//! could not run at all.
//!
//! Useful resources:
//! - Apple's [Gradients chapter](https://developer.apple.com/library/archive/documentation/GraphicsImaging/Conceptual/drawingwithquartz2d/dq_shadings/dq_shadings.html)
//!   of the Quartz 2D Programming Guide, which defines what the extend options
//!   do and how a radial gradient interpolates between two circles.

use super::cg_bitmap_context::CGBitmapContextDrawer;
use super::cg_color::{to_rgba, CGColorRef};
use super::cg_color_space::CGColorSpaceRef;
use super::cg_context::CGContextRef;
use super::{CGFloat, CGPoint};
use crate::frameworks::core_foundation::{CFRelease, CFRetain, CFTypeRef};
use crate::frameworks::foundation::NSUInteger;
use crate::mem::{ConstPtr, GuestUSize};
use crate::objc::{msg, objc_classes, ClassExports, HostObject};
use crate::Environment;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// Like the other CFType-based Core Graphics types here, a gradient is modelled
// as an Objective-C object whose class name is not visible to the guest.
@implementation _tapHLE_CGGradient: NSObject
@end

};

pub type CGGradientRef = CFTypeRef;

pub type CGGradientDrawingOptions = u32;
/// Paint the start colour over everything before the gradient's start.
pub const kCGGradientDrawsBeforeStartLocation: CGGradientDrawingOptions = 1 << 0;
/// Paint the end colour over everything after the gradient's end.
pub const kCGGradientDrawsAfterEndLocation: CGGradientDrawingOptions = 1 << 1;

/// A colour stop: where it sits on the gradient's 0..1 axis, and its RGBA.
type Stop = (CGFloat, (CGFloat, CGFloat, CGFloat, CGFloat));

pub(super) struct CGGradientHostObject {
    /// Stops in ascending order of location. Never empty: a gradient with no
    /// colours cannot be created.
    stops: Vec<Stop>,
}
impl HostObject for CGGradientHostObject {}

/// Build the stop list from colours and optional locations.
///
/// A null `locations` means "spread them evenly from 0 to 1", which is what the
/// documentation specifies and what the overwhelming majority of callers pass.
/// Locations are sorted because the sampler assumes ascending order, and
/// Core Graphics itself does not require the caller to sort them.
fn make_stops(
    colors: Vec<(CGFloat, CGFloat, CGFloat, CGFloat)>,
    locations: Option<Vec<CGFloat>>,
) -> Vec<Stop> {
    assert!(!colors.is_empty());
    let count = colors.len();
    let locations = locations.unwrap_or_else(|| {
        if count == 1 {
            vec![0.0]
        } else {
            (0..count)
                .map(|i| i as CGFloat / (count - 1) as CGFloat)
                .collect()
        }
    });
    let mut stops: Vec<Stop> = locations.into_iter().zip(colors).collect();
    stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    stops
}

/// The colour at position `t` along the gradient axis.
///
/// `t` outside 0..1 is clamped to the end stops. Whether such a position should
/// be painted at all is the caller's decision, driven by the drawing options;
/// this only answers what colour it would be.
fn sample(stops: &[Stop], t: CGFloat) -> (CGFloat, CGFloat, CGFloat, CGFloat) {
    let first = stops[0];
    if t <= first.0 {
        return first.1;
    }
    let last = stops[stops.len() - 1];
    if t >= last.0 {
        return last.1;
    }
    for pair in stops.windows(2) {
        let (from_at, from) = pair[0];
        let (to_at, to) = pair[1];
        if t > to_at {
            continue;
        }
        // Two stops at the same location are a hard colour break, and the one
        // later in the list wins from that point on.
        let span = to_at - from_at;
        let mix = if span <= 0.0 {
            1.0
        } else {
            (t - from_at) / span
        };
        return (
            from.0 + (to.0 - from.0) * mix,
            from.1 + (to.1 - from.1) * mix,
            from.2 + (to.2 - from.2) * mix,
            from.3 + (to.3 - from.3) * mix,
        );
    }
    last.1
}

fn new_gradient(env: &mut Environment, stops: Vec<Stop>) -> CGGradientRef {
    let isa = env.objc.get_known_class("_tapHLE_CGGradient", &mut env.mem);
    env.objc
        .alloc_object(isa, Box::new(CGGradientHostObject { stops }), &mut env.mem)
}

/// Read `count` locations from the guest, or [None] if the pointer is null.
fn read_locations(
    env: &mut Environment,
    locations: ConstPtr<CGFloat>,
    count: GuestUSize,
) -> Option<Vec<CGFloat>> {
    if locations.is_null() {
        return None;
    }
    Some((0..count).map(|i| env.mem.read(locations + i)).collect())
}

/// `CGGradientCreateWithColorComponents`.
///
/// The components array holds one colour after another, each with as many
/// entries as the colour space has components plus alpha. Every colour space
/// tapHLE models is four-component RGBA (see
/// [super::cg_color::CGColorHostObject]), so the stride is four; a monochrome
/// space would be two, and reading it as four would silently shift every colour
/// after the first.
fn CGGradientCreateWithColorComponents(
    env: &mut Environment,
    _space: CGColorSpaceRef,
    components: ConstPtr<CGFloat>,
    locations: ConstPtr<CGFloat>,
    count: GuestUSize,
) -> CGGradientRef {
    assert!(count > 0);
    let colors = (0..count)
        .map(|i| {
            let base = components + i * 4;
            (
                env.mem.read(base),
                env.mem.read(base + 1),
                env.mem.read(base + 2),
                env.mem.read(base + 3),
            )
        })
        .collect();
    let locations = read_locations(env, locations, count);
    let stops = make_stops(colors, locations);
    log_dbg!("CGGradientCreateWithColorComponents() => {:?}", stops);
    new_gradient(env, stops)
}

/// `CGGradientCreateWithColors`, taking a `CFArray` of `CGColorRef`.
fn CGGradientCreateWithColors(
    env: &mut Environment,
    _space: CGColorSpaceRef,
    colors: CFTypeRef,
    locations: ConstPtr<CGFloat>,
) -> CGGradientRef {
    // A CFArray is an NSArray here, so this is an ordinary message send.
    let count: NSUInteger = msg![env; colors count];
    assert!(count > 0);
    let colors: Vec<_> = (0..count)
        .map(|i| {
            let color: CGColorRef = msg![env; colors objectAtIndex:i];
            to_rgba(&env.objc, color)
        })
        .collect();
    let locations = read_locations(env, locations, count);
    new_gradient(env, make_stops(colors, locations))
}

pub fn CGGradientRetain(env: &mut Environment, gradient: CGGradientRef) -> CGGradientRef {
    if !gradient.is_null() {
        CFRetain(env, gradient)
    } else {
        gradient
    }
}

pub fn CGGradientRelease(env: &mut Environment, gradient: CGGradientRef) {
    if !gradient.is_null() {
        CFRelease(env, gradient);
    }
}

/// Whether a position outside 0..1 should be painted, given the options.
fn extends(t: CGFloat, options: CGGradientDrawingOptions) -> bool {
    if t < 0.0 {
        options & kCGGradientDrawsBeforeStartLocation != 0
    } else if t > 1.0 {
        options & kCGGradientDrawsAfterEndLocation != 0
    } else {
        true
    }
}

/// Paint an axial gradient. Implementation of `CGContextDrawLinearGradient`.
///
/// The gradient's position is an affine function of the device pixel, because
/// the CTM is affine and so is the projection onto the axis. That is why this
/// samples `t` at three device points and interpolates rather than inverting
/// the transform per pixel: the result is exact, not an approximation.
///
/// **What is painted is bounded along the axis but not across it.** Core
/// Graphics fills the current clip region, and tapHLE has no clip state — see
/// `CGContextClipToRect` — so a gradient drawn into a clipped sub-rectangle
/// spreads sideways across the whole bitmap. Along the axis the extend options
/// do bound it, which is why the common case of a gradient whose start and end
/// are the edges of the rectangle being filled comes out right.
pub(super) fn draw_linear_gradient(
    env: &mut Environment,
    context: CGContextRef,
    gradient: CGGradientRef,
    start: CGPoint,
    end: CGPoint,
    options: CGGradientDrawingOptions,
) {
    let stops = env
        .objc
        .borrow::<CGGradientHostObject>(gradient)
        .stops
        .clone();
    let mut drawer = CGBitmapContextDrawer::new(&env.objc, &mut env.mem, context);

    let axis = (end.x - start.x, end.y - start.y);
    let length_squared = axis.0 * axis.0 + axis.1 * axis.1;
    if length_squared == 0.0 {
        // A zero-length axis paints nothing unless the caller asked for the
        // after-end extension, in which case the whole area is the end colour.
        if options & kCGGradientDrawsAfterEndLocation != 0 {
            let color = drawer.prepare_color(sample(&stops, 1.0));
            for y in 0..drawer.height() as i32 {
                for x in 0..drawer.width() as i32 {
                    drawer.put_pixel((x, y), color, /* blend: */ true);
                }
            }
        }
        return;
    }

    // t as an affine function of the device pixel: sample it at three points
    // and read off the coefficients.
    let inverse = drawer.transform().invert();
    let t_at = |x: CGFloat, y: CGFloat| {
        let p = inverse.apply_to_point(CGPoint { x, y });
        ((p.x - start.x) * axis.0 + (p.y - start.y) * axis.1) / length_squared
    };
    let t_origin = t_at(0.5, 0.5);
    let t_per_x = t_at(1.5, 0.5) - t_origin;
    let t_per_y = t_at(0.5, 1.5) - t_origin;

    for y in 0..drawer.height() as i32 {
        let row_t = t_origin + t_per_y * y as CGFloat;
        for x in 0..drawer.width() as i32 {
            let t = row_t + t_per_x * x as CGFloat;
            if !extends(t, options) {
                continue;
            }
            let color = drawer.prepare_color(sample(&stops, t));
            drawer.put_pixel((x, y), color, /* blend: */ true);
        }
    }
}

/// Solve for the gradient position of a point between two circles.
///
/// The parameter `s` is the one for which the point lies on the circle centred
/// at `start + s * (end - start)`, with radius `start_radius` plus `s` times
/// the radius difference. Expanding that equality gives a quadratic in `s`;
/// the larger root is the one to use, because the later circle is drawn over
/// the earlier one, and a root whose radius would be negative is not a real
/// solution.
///
/// Returns [None] when the point lies on neither circle for any `s`, which
/// happens outside the cone the two circles sweep out.
fn radial_position(
    point: CGPoint,
    start: CGPoint,
    start_radius: CGFloat,
    end: CGPoint,
    end_radius: CGFloat,
) -> Option<CGFloat> {
    let centre_delta = (end.x - start.x, end.y - start.y);
    let radius_delta = end_radius - start_radius;
    let offset = (point.x - start.x, point.y - start.y);

    let a = centre_delta.0 * centre_delta.0 + centre_delta.1 * centre_delta.1
        - radius_delta * radius_delta;
    let b = offset.0 * centre_delta.0 + offset.1 * centre_delta.1 + start_radius * radius_delta;
    let c = offset.0 * offset.0 + offset.1 * offset.1 - start_radius * start_radius;

    let valid = |s: CGFloat| (start_radius + s * radius_delta >= 0.0).then_some(s);

    if a.abs() < 1e-6 {
        // Degenerate case: the quadratic collapses to a linear equation. Two
        // circles of equal radius with the same centre give b == 0 as well,
        // and then no position exists.
        if b == 0.0 {
            return None;
        }
        return valid(c / (2.0 * b));
    }
    let discriminant = b * b - a * c;
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let (larger, smaller) = {
        let s1 = (b + root) / a;
        let s2 = (b - root) / a;
        if s1 >= s2 {
            (s1, s2)
        } else {
            (s2, s1)
        }
    };
    valid(larger).or_else(|| valid(smaller))
}

/// Paint a radial gradient. Implementation of `CGContextDrawRadialGradient`.
///
/// Unlike the axial case, the position is not an affine function of the device
/// pixel — an affine transform turns a circle into an ellipse — so the
/// transform is inverted per pixel. The same clipping caveat as
/// [draw_linear_gradient] applies.
pub(super) fn draw_radial_gradient(
    env: &mut Environment,
    context: CGContextRef,
    gradient: CGGradientRef,
    start: CGPoint,
    start_radius: CGFloat,
    end: CGPoint,
    end_radius: CGFloat,
    options: CGGradientDrawingOptions,
) {
    let stops = env
        .objc
        .borrow::<CGGradientHostObject>(gradient)
        .stops
        .clone();
    let mut drawer = CGBitmapContextDrawer::new(&env.objc, &mut env.mem, context);
    let inverse = drawer.transform().invert();

    for y in 0..drawer.height() as i32 {
        for x in 0..drawer.width() as i32 {
            let point = inverse.apply_to_point(CGPoint {
                x: x as CGFloat + 0.5,
                y: y as CGFloat + 0.5,
            });
            let Some(t) = radial_position(point, start, start_radius, end, end_radius) else {
                continue;
            };
            if !extends(t, options) {
                continue;
            }
            let color = drawer.prepare_color(sample(&stops, t));
            drawer.put_pixel((x, y), color, /* blend: */ true);
        }
    }
}

pub const FUNCTIONS: crate::dyld::FunctionExports = &[
    crate::export_c_func!(CGGradientCreateWithColorComponents(_, _, _, _)),
    crate::export_c_func!(CGGradientCreateWithColors(_, _, _)),
    crate::export_c_func!(CGGradientRetain(_)),
    crate::export_c_func!(CGGradientRelease(_)),
];

#[cfg(test)]
mod tests {
    use super::{
        extends, kCGGradientDrawsAfterEndLocation, kCGGradientDrawsBeforeStartLocation, make_stops,
        radial_position, sample, CGPoint,
    };

    const BLACK: (f32, f32, f32, f32) = (0.0, 0.0, 0.0, 1.0);
    const WHITE: (f32, f32, f32, f32) = (1.0, 1.0, 1.0, 1.0);

    #[test]
    fn absent_locations_spread_the_stops_evenly() {
        let stops = make_stops(vec![BLACK, WHITE, BLACK], None);
        assert_eq!(stops[0].0, 0.0);
        assert_eq!(stops[1].0, 0.5);
        assert_eq!(stops[2].0, 1.0);
    }

    #[test]
    fn a_single_colour_sits_at_the_start() {
        let stops = make_stops(vec![WHITE], None);
        assert_eq!(stops.len(), 1);
        assert_eq!(stops[0].0, 0.0);
        // Every position is that colour, in both directions.
        assert_eq!(sample(&stops, -5.0), WHITE);
        assert_eq!(sample(&stops, 0.5), WHITE);
        assert_eq!(sample(&stops, 5.0), WHITE);
    }

    #[test]
    fn unsorted_locations_are_put_in_order() {
        let stops = make_stops(vec![BLACK, WHITE], Some(vec![1.0, 0.0]));
        assert_eq!(stops[0], (0.0, WHITE));
        assert_eq!(stops[1], (1.0, BLACK));
    }

    #[test]
    fn sampling_interpolates_between_the_surrounding_stops() {
        let stops = make_stops(vec![BLACK, WHITE], None);
        assert_eq!(sample(&stops, 0.0), BLACK);
        assert_eq!(sample(&stops, 1.0), WHITE);
        assert_eq!(sample(&stops, 0.25).0, 0.25);
        assert_eq!(sample(&stops, 0.75).0, 0.75);
    }

    #[test]
    fn sampling_clamps_outside_the_axis() {
        let stops = make_stops(vec![BLACK, WHITE], None);
        assert_eq!(sample(&stops, -1.0), BLACK);
        assert_eq!(sample(&stops, 2.0), WHITE);
    }

    #[test]
    fn two_stops_at_one_location_are_a_hard_break() {
        let stops = make_stops(vec![BLACK, BLACK, WHITE], Some(vec![0.0, 0.5, 0.5]));
        // Just before the break it is still the first colour; at and after it,
        // the later stop wins.
        assert_eq!(sample(&stops, 0.49).0, 0.0);
        assert_eq!(sample(&stops, 0.5), WHITE);
    }

    #[test]
    fn the_extend_options_gate_only_the_outside() {
        assert!(extends(0.5, 0));
        assert!(!extends(-0.1, 0));
        assert!(!extends(1.1, 0));
        assert!(extends(-0.1, kCGGradientDrawsBeforeStartLocation));
        assert!(!extends(1.1, kCGGradientDrawsBeforeStartLocation));
        assert!(extends(1.1, kCGGradientDrawsAfterEndLocation));
        assert!(!extends(-0.1, kCGGradientDrawsAfterEndLocation));
    }

    #[test]
    fn a_concentric_radial_gradient_grows_with_the_distance() {
        // The common case by far: one centre, radius 0 to 10.
        let centre = CGPoint { x: 0.0, y: 0.0 };
        let at = |x: f32| radial_position(CGPoint { x, y: 0.0 }, centre, 0.0, centre, 10.0);
        assert_eq!(at(0.0), Some(0.0));
        assert_eq!(at(5.0), Some(0.5));
        assert_eq!(at(10.0), Some(1.0));
        // Outside the end circle is still a position; the options decide
        // whether it is painted.
        assert_eq!(at(20.0), Some(2.0));
    }

    #[test]
    fn a_concentric_gradient_with_an_inner_radius_continues_inwards() {
        let centre = CGPoint { x: 0.0, y: 0.0 };
        // Radius 5 to radius 10. The inside of the start circle is not a hole:
        // the family of circles shrinks to a point at position -1, so every
        // point in there has a position, and it is the extend options rather
        // than the geometry that decide whether it is painted.
        let at = |x: f32| radial_position(CGPoint { x, y: 0.0 }, centre, 5.0, centre, 10.0);
        assert_eq!(at(7.5), Some(0.5));
        assert_eq!(at(5.0), Some(0.0));
        assert_eq!(at(2.5), Some(-0.5));
        assert_eq!(at(0.0), Some(-1.0));
    }

    #[test]
    fn a_point_outside_the_cone_of_two_circles_has_no_position() {
        // Equal radii and different centres sweep out a strip one radius wide;
        // a point well off to the side lies on none of the circles.
        let start = CGPoint { x: 0.0, y: 0.0 };
        let end = CGPoint { x: 10.0, y: 0.0 };
        assert_eq!(
            radial_position(CGPoint { x: 0.0, y: 5.0 }, start, 1.0, end, 1.0),
            None
        );
        // Directly on the strip, it does.
        assert_eq!(
            radial_position(CGPoint { x: 5.0, y: 1.0 }, start, 1.0, end, 1.0),
            Some(0.5)
        );
    }

    #[test]
    fn two_identical_circles_have_no_gradient_at_all() {
        let centre = CGPoint { x: 1.0, y: 2.0 };
        assert_eq!(
            radial_position(CGPoint { x: 3.0, y: 4.0 }, centre, 4.0, centre, 4.0),
            None
        );
    }
}
