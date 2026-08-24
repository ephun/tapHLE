/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CGPath.h`, and the path a `CGContext` builds up in its own right.
//!
//! Curves are **flattened to line segments as they are added**, rather than
//! kept as curves and flattened at draw time. That loses the ability to
//! re-flatten at a different scale if the path is drawn under a larger
//! transform than it was built under, which is a real if unusual limitation.
//! What it buys is that every consumer — fill, stroke, clip — sees one simple
//! representation, and a polygon is something this can rasterise correctly
//! today rather than approximately.
//!
//! Resources:
//! - Apple's [CGPath reference](https://developer.apple.com/documentation/coregraphics/cgpath)

use super::cg_affine_transform::CGAffineTransform;
use super::{CGFloat, CGPoint, CGRect};
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_foundation::{CFRelease, CFRetain, CFTypeRef};
use crate::objc::{objc_classes, ClassExports, HostObject};
use crate::Environment;

pub type CGPathRef = CFTypeRef;
pub type CGMutablePathRef = CGPathRef;

/// One connected run of points. `closed` decides whether the last point joins
/// back to the first, which matters for stroking; a fill closes every subpath
/// implicitly either way, as CoreGraphics does.
#[derive(Clone, Debug, Default)]
pub struct Subpath {
    pub points: Vec<CGPoint>,
    pub closed: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Path {
    pub subpaths: Vec<Subpath>,
}

/// How finely an arc is flattened. Arcs in this era's UI are corner rounding a
/// few pixels across, where sixteen segments per quarter turn is already past
/// the point of being visible.
const ARC_SEGMENTS_PER_QUARTER_TURN: usize = 16;

impl Path {
    pub fn is_empty(&self) -> bool {
        self.subpaths.iter().all(|s| s.points.len() < 2)
    }

    pub fn clear(&mut self) {
        self.subpaths.clear();
    }

    pub fn current_point(&self) -> Option<CGPoint> {
        self.subpaths.last().and_then(|s| s.points.last()).copied()
    }

    pub fn move_to(&mut self, point: CGPoint) {
        self.subpaths.push(Subpath {
            points: vec![point],
            closed: false,
        });
    }

    pub fn line_to(&mut self, point: CGPoint) {
        // A line with no preceding move starts a subpath at that point, which
        // is what CoreGraphics does rather than treating it as an error.
        match self.subpaths.last_mut() {
            Some(subpath) if !subpath.closed => subpath.points.push(point),
            _ => self.move_to(point),
        }
    }

    pub fn close(&mut self) {
        if let Some(subpath) = self.subpaths.last_mut() {
            subpath.closed = true;
        }
    }

    /// Append a whole other path, transformed.
    pub fn append(&mut self, other: &Path, transform: CGAffineTransform) {
        for subpath in &other.subpaths {
            self.subpaths.push(Subpath {
                points: subpath
                    .points
                    .iter()
                    .map(|&p| transform.apply_to_point(p))
                    .collect(),
                closed: subpath.closed,
            });
        }
    }

    pub fn add_rect(&mut self, rect: CGRect) {
        let CGRect { origin, size } = rect;
        self.move_to(origin);
        self.line_to(CGPoint {
            x: origin.x + size.width,
            y: origin.y,
        });
        self.line_to(CGPoint {
            x: origin.x + size.width,
            y: origin.y + size.height,
        });
        self.line_to(CGPoint {
            x: origin.x,
            y: origin.y + size.height,
        });
        self.close();
    }

    /// The ellipse inscribed in `rect`, as a closed subpath.
    ///
    /// Sampled parametrically rather than built from the four Bézier arcs the
    /// real implementation uses, because everything here is flattened to line
    /// segments anyway — going via curves would only add a conversion whose
    /// output is the same polygon.
    ///
    /// A negative width or height is fine: the ellipse fits the rectangle the
    /// numbers describe, and normalising a rectangle is not this function's
    /// job.
    pub fn add_ellipse_in_rect(&mut self, rect: CGRect) {
        let radius_x = rect.size.width / 2.0;
        let radius_y = rect.size.height / 2.0;
        let center = CGPoint {
            x: rect.origin.x + radius_x,
            y: rect.origin.y + radius_y,
        };
        let steps = ARC_SEGMENTS_PER_QUARTER_TURN * 4;
        for i in 0..steps {
            let angle = std::f32::consts::TAU * (i as f32 / steps as f32);
            let point = CGPoint {
                x: center.x + radius_x * angle.cos(),
                y: center.y + radius_y * angle.sin(),
            };
            if i == 0 {
                self.move_to(point);
            } else {
                self.line_to(point);
            }
        }
        self.close();
    }

    /// A polyline: a move to the first point, then a line to each of the rest.
    ///
    /// An empty slice adds nothing at all rather than an empty subpath, which
    /// keeps [Self::is_empty] honest.
    pub fn add_lines(&mut self, points: &[CGPoint]) {
        let Some((&first, rest)) = points.split_first() else {
            return;
        };
        self.move_to(first);
        for &point in rest {
            self.line_to(point);
        }
    }

    /// A centred arc, flattened.
    pub fn add_arc(
        &mut self,
        center: CGPoint,
        radius: CGFloat,
        start_angle: CGFloat,
        end_angle: CGFloat,
        clockwise: bool,
    ) {
        let mut sweep = end_angle - start_angle;
        // Normalise into the direction asked for, so that "clockwise from 0 to
        // pi/2" is three quarters of a turn rather than a negative quarter.
        if clockwise {
            while sweep > 0.0 {
                sweep -= std::f32::consts::TAU;
            }
        } else {
            while sweep < 0.0 {
                sweep += std::f32::consts::TAU;
            }
        }

        let quarters = (sweep.abs() / std::f32::consts::FRAC_PI_2).max(f32::EPSILON);
        let steps = ((quarters * ARC_SEGMENTS_PER_QUARTER_TURN as f32).ceil() as usize).max(1);
        for i in 0..=steps {
            let angle = start_angle + sweep * (i as f32 / steps as f32);
            let point = CGPoint {
                x: center.x + radius * angle.cos(),
                y: center.y + radius * angle.sin(),
            };
            if i == 0 && self.current_point().is_none() {
                self.move_to(point);
            } else {
                self.line_to(point);
            }
        }
    }

    /// The tangent-arc form: round off the corner at `corner` between the line
    /// coming from the current point and the line heading towards `next`.
    ///
    /// This is what rounded rectangles are actually built from, which is why it
    /// is here at all — it is the only arc call an app doing corner rounding
    /// makes.
    pub fn add_arc_to_point(&mut self, corner: CGPoint, next: CGPoint, radius: CGFloat) {
        let Some(current) = self.current_point() else {
            // No current point means no incoming tangent to round against.
            // CoreGraphics treats this as a move.
            self.move_to(corner);
            return;
        };

        let (v1x, v1y) = (current.x - corner.x, current.y - corner.y);
        let (v2x, v2y) = (next.x - corner.x, next.y - corner.y);
        let len1 = (v1x * v1x + v1y * v1y).sqrt();
        let len2 = (v2x * v2x + v2y * v2y).sqrt();
        if len1 == 0.0 || len2 == 0.0 || radius <= 0.0 {
            // Degenerate: a zero radius, or a corner on top of one of its
            // neighbours, is just a corner.
            self.line_to(corner);
            return;
        }

        let (u1x, u1y) = (v1x / len1, v1y / len1);
        let (u2x, u2y) = (v2x / len2, v2y / len2);

        let cos_angle = (u1x * u2x + u1y * u2y).clamp(-1.0, 1.0);
        let angle = cos_angle.acos();
        if angle <= f32::EPSILON || (std::f32::consts::PI - angle) <= f32::EPSILON {
            // Collinear: there is no corner to round.
            self.line_to(corner);
            return;
        }

        // Distance from the corner to each tangent point.
        let tangent_distance = radius / (angle / 2.0).tan();
        // A radius too large for the available edge lengths is clamped, as
        // CoreGraphics does, rather than producing a loop.
        let tangent_distance = tangent_distance.min(len1).min(len2);
        let effective_radius = tangent_distance * (angle / 2.0).tan();

        let t1 = CGPoint {
            x: corner.x + u1x * tangent_distance,
            y: corner.y + u1y * tangent_distance,
        };
        let t2 = CGPoint {
            x: corner.x + u2x * tangent_distance,
            y: corner.y + u2y * tangent_distance,
        };

        // The centre lies along the angle bisector, at the distance that puts
        // it `effective_radius` from both tangent points.
        let (bx, by) = (u1x + u2x, u1y + u2y);
        let b_len = (bx * bx + by * by).sqrt();
        if b_len == 0.0 {
            self.line_to(corner);
            return;
        }
        let center_distance =
            (effective_radius * effective_radius + tangent_distance * tangent_distance).sqrt();
        let center = CGPoint {
            x: corner.x + (bx / b_len) * center_distance,
            y: corner.y + (by / b_len) * center_distance,
        };

        self.line_to(t1);
        let start = (t1.y - center.y).atan2(t1.x - center.x);
        let end = (t2.y - center.y).atan2(t2.x - center.x);
        // Pick the short way round; the rounded corner is always the minor arc.
        let mut sweep = end - start;
        while sweep > std::f32::consts::PI {
            sweep -= std::f32::consts::TAU;
        }
        while sweep < -std::f32::consts::PI {
            sweep += std::f32::consts::TAU;
        }
        let steps = ((sweep.abs() / std::f32::consts::FRAC_PI_2
            * ARC_SEGMENTS_PER_QUARTER_TURN as f32)
            .ceil() as usize)
            .max(1);
        for i in 1..=steps {
            let angle = start + sweep * (i as f32 / steps as f32);
            self.line_to(CGPoint {
                x: center.x + effective_radius * angle.cos(),
                y: center.y + effective_radius * angle.sin(),
            });
        }
    }
}

pub(super) struct CGPathHostObject {
    pub(super) path: Path,
}
impl HostObject for CGPathHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// As with _tapHLE_CGContext: CGPath is a CFType, and tapHLE's CFTypes are
// Objective-C objects, so it needs a class whose name is never visible.
@implementation _tapHLE_CGPath: NSObject

- (())dealloc {
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

};

fn CGPathCreateMutable(env: &mut Environment) -> CGMutablePathRef {
    let host_object = Box::new(CGPathHostObject {
        path: Path::default(),
    });
    let class = env.objc.get_known_class("_tapHLE_CGPath", &mut env.mem);
    env.objc.alloc_object(class, host_object, &mut env.mem)
}

fn CGPathRelease(env: &mut Environment, path: CGPathRef) {
    if !path.is_null() {
        CFRelease(env, path);
    }
}

fn CGPathRetain(env: &mut Environment, path: CGPathRef) -> CGPathRef {
    if path.is_null() {
        path
    } else {
        CFRetain(env, path)
    }
}

/// The `transform` argument of every mutation function is an optional pointer
/// to a transform applied to the points being added — not to the path as a
/// whole. A null pointer means identity, which is what every caller here uses.
fn transform_or_identity(
    env: &mut Environment,
    transform: crate::mem::ConstPtr<CGAffineTransform>,
) -> CGAffineTransform {
    if transform.is_null() {
        super::cg_affine_transform::CGAffineTransformIdentity
    } else {
        env.mem.read(transform)
    }
}

fn CGPathMoveToPoint(
    env: &mut Environment,
    path: CGMutablePathRef,
    transform: crate::mem::ConstPtr<CGAffineTransform>,
    x: CGFloat,
    y: CGFloat,
) {
    let transform = transform_or_identity(env, transform);
    let point = transform.apply_to_point(CGPoint { x, y });
    env.objc
        .borrow_mut::<CGPathHostObject>(path)
        .path
        .move_to(point);
}

fn CGPathAddLineToPoint(
    env: &mut Environment,
    path: CGMutablePathRef,
    transform: crate::mem::ConstPtr<CGAffineTransform>,
    x: CGFloat,
    y: CGFloat,
) {
    let transform = transform_or_identity(env, transform);
    let point = transform.apply_to_point(CGPoint { x, y });
    env.objc
        .borrow_mut::<CGPathHostObject>(path)
        .path
        .line_to(point);
}

fn CGPathAddArcToPoint(
    env: &mut Environment,
    path: CGMutablePathRef,
    transform: crate::mem::ConstPtr<CGAffineTransform>,
    x1: CGFloat,
    y1: CGFloat,
    x2: CGFloat,
    y2: CGFloat,
    radius: CGFloat,
) {
    let transform = transform_or_identity(env, transform);
    let corner = transform.apply_to_point(CGPoint { x: x1, y: y1 });
    let next = transform.apply_to_point(CGPoint { x: x2, y: y2 });
    env.objc
        .borrow_mut::<CGPathHostObject>(path)
        .path
        .add_arc_to_point(corner, next, radius);
}

fn CGPathAddRect(
    env: &mut Environment,
    path: CGMutablePathRef,
    transform: crate::mem::ConstPtr<CGAffineTransform>,
    rect: CGRect,
) {
    let transform = transform_or_identity(env, transform);
    let rect = transform.apply_to_rect(rect);
    env.objc
        .borrow_mut::<CGPathHostObject>(path)
        .path
        .add_rect(rect);
}

fn CGPathCloseSubpath(env: &mut Environment, path: CGMutablePathRef) {
    env.objc.borrow_mut::<CGPathHostObject>(path).path.close();
}

fn CGPathIsEmpty(env: &mut Environment, path: CGPathRef) -> bool {
    path.is_null() || env.objc.borrow::<CGPathHostObject>(path).path.is_empty()
}

/// Read a path out of a `CGPathRef` for a consumer such as
/// `CGContextAddPath`.
pub(super) fn borrow_path(env: &Environment, path: CGPathRef) -> Path {
    env.objc.borrow::<CGPathHostObject>(path).path.clone()
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGPathCreateMutable()),
    export_c_func!(CGPathRelease(_)),
    export_c_func!(CGPathRetain(_)),
    export_c_func!(CGPathMoveToPoint(_, _, _, _)),
    export_c_func!(CGPathAddLineToPoint(_, _, _, _)),
    export_c_func!(CGPathAddArcToPoint(_, _, _, _, _, _, _)),
    export_c_func!(CGPathAddRect(_, _, _)),
    export_c_func!(CGPathCloseSubpath(_)),
    export_c_func!(CGPathIsEmpty(_)),
];

#[cfg(test)]
mod tests {
    use super::{CGPoint, CGRect, Path};
    use crate::frameworks::core_graphics::CGSize;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> CGRect {
        CGRect {
            origin: CGPoint { x, y },
            size: CGSize { width, height },
        }
    }

    #[test]
    fn an_ellipse_is_one_closed_subpath_inside_its_rectangle() {
        let mut path = Path::default();
        path.add_ellipse_in_rect(rect(10.0, 20.0, 100.0, 50.0));
        assert_eq!(path.subpaths.len(), 1);
        assert!(path.subpaths[0].closed);
        for point in &path.subpaths[0].points {
            // Allowing a hair of floating-point slack at the extremes.
            assert!(point.x >= 9.99 && point.x <= 110.01, "{:?}", point);
            assert!(point.y >= 19.99 && point.y <= 70.01, "{:?}", point);
        }
    }

    #[test]
    fn an_ellipse_reaches_all_four_extremes_of_its_rectangle() {
        let mut path = Path::default();
        path.add_ellipse_in_rect(rect(0.0, 0.0, 100.0, 100.0));
        let points = &path.subpaths[0].points;
        let max_x = points.iter().fold(f32::MIN, |a, p| a.max(p.x));
        let min_x = points.iter().fold(f32::MAX, |a, p| a.min(p.x));
        let max_y = points.iter().fold(f32::MIN, |a, p| a.max(p.y));
        let min_y = points.iter().fold(f32::MAX, |a, p| a.min(p.y));
        assert!(max_x > 99.9 && min_x < 0.1);
        assert!(max_y > 99.9 && min_y < 0.1);
    }

    #[test]
    fn adding_no_lines_leaves_no_subpath() {
        let mut path = Path::default();
        path.add_lines(&[]);
        assert!(path.subpaths.is_empty());
        assert!(path.is_empty());
    }

    #[test]
    fn adding_lines_makes_one_open_polyline() {
        let mut path = Path::default();
        path.add_lines(&[
            CGPoint { x: 0.0, y: 0.0 },
            CGPoint { x: 1.0, y: 1.0 },
            CGPoint { x: 2.0, y: 0.0 },
        ]);
        assert_eq!(path.subpaths.len(), 1);
        assert_eq!(path.subpaths[0].points.len(), 3);
        assert!(!path.subpaths[0].closed);
    }
}
