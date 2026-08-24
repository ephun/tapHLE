/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CATransform3D.h`
//!
//! The 4-by-4 matrix Core Animation uses for layer transforms. tapHLE had no
//! definition of it at all, so an app that so much as read
//! `CATransform3DIdentity` — 249 of the 1192 distinct apps in the import-demand
//! catalogue import that constant — could not get the value, and every builder
//! function was a hard stop when called.
//!
//! **This is the type and its arithmetic, not layer support.** `CALayer` here
//! has no 3-D transform property, so a transform an app builds and hands to a
//! layer is still not applied to what is drawn. That is deliberate: the
//! arithmetic is what apps call, it is self-contained and checkable, and
//! separating it means the compositing work can be done later without also
//! having to get the matrix maths right at the same time.
//!
//! Useful resources:
//! - Apple's [CATransform3D reference](https://developer.apple.com/documentation/quartzcore/catransform3d),
//!   which specifies each function as a matrix product in a definite order.

use crate::abi::{impl_GuestRet_for_large_struct, GuestArg};
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::frameworks::core_graphics::cg_affine_transform::CGAffineTransform;
use crate::frameworks::core_graphics::CGFloat;
use crate::mem::SafeRead;
use crate::Environment;

/// Apple's convention is that a point is a **row** vector post-multiplied by
/// the matrix: `p' = p × M`. That is the same convention [CGAffineTransform]
/// uses, which is why the translation lives in `m41`–`m43` rather than in the
/// last column.
///
/// Getting this backwards transposes every rotation, and a transposed rotation
/// is still a valid rotation — by the opposite angle — so it produces plausible
/// output rather than an obvious failure. The tests pin the z-axis case against
/// `CGAffineTransformMakeRotation` for exactly that reason.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C, packed)]
#[allow(non_snake_case)]
pub struct CATransform3D {
    pub m11: CGFloat,
    pub m12: CGFloat,
    pub m13: CGFloat,
    pub m14: CGFloat,
    pub m21: CGFloat,
    pub m22: CGFloat,
    pub m23: CGFloat,
    pub m24: CGFloat,
    pub m31: CGFloat,
    pub m32: CGFloat,
    pub m33: CGFloat,
    pub m34: CGFloat,
    pub m41: CGFloat,
    pub m42: CGFloat,
    pub m43: CGFloat,
    pub m44: CGFloat,
}
unsafe impl SafeRead for CATransform3D {}
impl_GuestRet_for_large_struct!(CATransform3D);

impl GuestArg for CATransform3D {
    // Sixteen words is the largest argument the ABI layer can currently read:
    // read_next_arg's scratch array is exactly that size. This type is right at
    // that limit, not comfortably inside it.
    const REG_COUNT: usize = 16;

    fn from_regs(regs: &[u32]) -> Self {
        let mut values = [0f32; 16];
        for (value, reg) in values.iter_mut().zip(regs.iter()) {
            *value = f32::from_bits(*reg);
        }
        CATransform3D::from_rows([
            [values[0], values[1], values[2], values[3]],
            [values[4], values[5], values[6], values[7]],
            [values[8], values[9], values[10], values[11]],
            [values[12], values[13], values[14], values[15]],
        ])
    }
    fn to_regs(self, regs: &mut [u32]) {
        let rows = self.rows();
        for (index, reg) in regs.iter_mut().enumerate() {
            *reg = rows[index / 4][index % 4].to_bits();
        }
    }
}

impl CATransform3D {
    /// Row-major, so `rows[i][j]` is `m(i+1)(j+1)`: the same subscript order
    /// the field names use.
    fn rows(self) -> [[CGFloat; 4]; 4] {
        [
            [self.m11, self.m12, self.m13, self.m14],
            [self.m21, self.m22, self.m23, self.m24],
            [self.m31, self.m32, self.m33, self.m34],
            [self.m41, self.m42, self.m43, self.m44],
        ]
    }

    fn from_rows(rows: [[CGFloat; 4]; 4]) -> Self {
        CATransform3D {
            m11: rows[0][0],
            m12: rows[0][1],
            m13: rows[0][2],
            m14: rows[0][3],
            m21: rows[1][0],
            m22: rows[1][1],
            m23: rows[1][2],
            m24: rows[1][3],
            m31: rows[2][0],
            m32: rows[2][1],
            m33: rows[2][2],
            m34: rows[2][3],
            m41: rows[3][0],
            m42: rows[3][1],
            m43: rows[3][2],
            m44: rows[3][3],
        }
    }

    /// The matrix product `self × other`, in the row-vector sense: transforming
    /// by the result is transforming by `self` and then by `other`.
    fn multiply(self, other: Self) -> Self {
        let (a, b) = (self.rows(), other.rows());
        let mut result = [[0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    result[i][j] += a[i][k] * b[k][j];
                }
            }
        }
        Self::from_rows(result)
    }
}

pub const CATransform3DIdentity: CATransform3D = CATransform3D {
    m11: 1.0,
    m12: 0.0,
    m13: 0.0,
    m14: 0.0,
    m21: 0.0,
    m22: 1.0,
    m23: 0.0,
    m24: 0.0,
    m31: 0.0,
    m32: 0.0,
    m33: 1.0,
    m34: 0.0,
    m41: 0.0,
    m42: 0.0,
    m43: 0.0,
    m44: 1.0,
};

fn make_translation(tx: CGFloat, ty: CGFloat, tz: CGFloat) -> CATransform3D {
    CATransform3D {
        m41: tx,
        m42: ty,
        m43: tz,
        ..CATransform3DIdentity
    }
}

fn make_scale(sx: CGFloat, sy: CGFloat, sz: CGFloat) -> CATransform3D {
    CATransform3D {
        m11: sx,
        m22: sy,
        m33: sz,
        ..CATransform3DIdentity
    }
}

/// A rotation of `angle` radians about the axis `(x, y, z)`.
///
/// This is the row-vector form, which is the transpose of the Rodrigues matrix
/// as it is usually written for column vectors. A zero-length axis has no
/// rotation to describe, and Apple's documentation says the result is then the
/// identity.
fn make_rotation(angle: CGFloat, x: CGFloat, y: CGFloat, z: CGFloat) -> CATransform3D {
    let length = (x * x + y * y + z * z).sqrt();
    if length == 0.0 || !length.is_finite() {
        return CATransform3DIdentity;
    }
    let (x, y, z) = (x / length, y / length, z / length);
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    CATransform3D::from_rows([
        [c + x * x * t, y * x * t + z * s, z * x * t - y * s, 0.0],
        [x * y * t - z * s, c + y * y * t, z * y * t + x * s, 0.0],
        [x * z * t + y * s, y * z * t - x * s, c + z * z * t, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// Whether the transform does nothing outside the plane, and so can be written
/// as a [CGAffineTransform] without loss.
fn is_affine(t: CATransform3D) -> bool {
    t.m13 == 0.0
        && t.m14 == 0.0
        && t.m23 == 0.0
        && t.m24 == 0.0
        && t.m31 == 0.0
        && t.m32 == 0.0
        && t.m33 == 1.0
        && t.m34 == 0.0
        && t.m43 == 0.0
        && t.m44 == 1.0
}

fn to_affine(t: CATransform3D) -> CGAffineTransform {
    CGAffineTransform {
        a: t.m11,
        b: t.m12,
        c: t.m21,
        d: t.m22,
        tx: t.m41,
        ty: t.m42,
    }
}

fn from_affine(t: CGAffineTransform) -> CATransform3D {
    CATransform3D {
        m11: t.a,
        m12: t.b,
        m21: t.c,
        m22: t.d,
        m41: t.tx,
        m42: t.ty,
        ..CATransform3DIdentity
    }
}

// The guest-visible functions. Each is specified by Apple as a matrix product
// in a definite order, and the order is what makes a translate-then-rotate come
// out differently from a rotate-then-translate, so each one names it.

fn CATransform3DMakeTranslation(
    _env: &mut Environment,
    tx: CGFloat,
    ty: CGFloat,
    tz: CGFloat,
) -> CATransform3D {
    make_translation(tx, ty, tz)
}

fn CATransform3DMakeScale(
    _env: &mut Environment,
    sx: CGFloat,
    sy: CGFloat,
    sz: CGFloat,
) -> CATransform3D {
    make_scale(sx, sy, sz)
}

fn CATransform3DMakeRotation(
    _env: &mut Environment,
    angle: CGFloat,
    x: CGFloat,
    y: CGFloat,
    z: CGFloat,
) -> CATransform3D {
    make_rotation(angle, x, y, z)
}

/// `t' = translate(tx, ty, tz) × t` — the translation happens *before* whatever
/// `t` does, so translating a scaled transform moves by the unscaled amount.
fn CATransform3DTranslate(
    _env: &mut Environment,
    t: CATransform3D,
    tx: CGFloat,
    ty: CGFloat,
    tz: CGFloat,
) -> CATransform3D {
    make_translation(tx, ty, tz).multiply(t)
}

/// `t' = scale(sx, sy, sz) × t`.
fn CATransform3DScale(
    _env: &mut Environment,
    t: CATransform3D,
    sx: CGFloat,
    sy: CGFloat,
    sz: CGFloat,
) -> CATransform3D {
    make_scale(sx, sy, sz).multiply(t)
}

/// `t' = rotation(angle, x, y, z) × t`.
fn CATransform3DRotate(
    _env: &mut Environment,
    t: CATransform3D,
    angle: CGFloat,
    x: CGFloat,
    y: CGFloat,
    z: CGFloat,
) -> CATransform3D {
    make_rotation(angle, x, y, z).multiply(t)
}

/// `t = a × b` — transforming by the result is transforming by `a` and then by
/// `b`.
fn CATransform3DConcat(
    _env: &mut Environment,
    a: CATransform3D,
    b: CATransform3D,
) -> CATransform3D {
    a.multiply(b)
}

fn CATransform3DIsIdentity(_env: &mut Environment, t: CATransform3D) -> bool {
    t == CATransform3DIdentity
}

fn CATransform3DEqualToTransform(
    _env: &mut Environment,
    a: CATransform3D,
    b: CATransform3D,
) -> bool {
    a == b
}

fn CATransform3DIsAffine(_env: &mut Environment, t: CATransform3D) -> bool {
    is_affine(t)
}

fn CATransform3DMakeAffineTransform(_env: &mut Environment, m: CGAffineTransform) -> CATransform3D {
    from_affine(m)
}

/// The affine part of the transform.
///
/// Apple's documentation says the result is undefined when the transform is not
/// affine, and the real function returns the in-plane elements anyway rather
/// than failing. Doing the same, plus a log line, is better than asserting: an
/// app that has built a perspective transform and asks for its affine part is
/// usually about to fall back to 2-D drawing, and stopping it there loses that.
fn CATransform3DGetAffineTransform(_env: &mut Environment, t: CATransform3D) -> CGAffineTransform {
    if !is_affine(t) {
        log!(
            "CATransform3DGetAffineTransform() on a non-affine transform; \
             returning its in-plane elements, as the real function does"
        );
    }
    to_affine(t)
}

pub const CONSTANTS: ConstantExports = &[(
    "_CATransform3DIdentity",
    HostConstant::Custom(|env| {
        env.mem
            .alloc_and_write(CATransform3DIdentity)
            .cast()
            .cast_const()
    }),
)];

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CATransform3DMakeTranslation(_, _, _)),
    export_c_func!(CATransform3DMakeScale(_, _, _)),
    export_c_func!(CATransform3DMakeRotation(_, _, _, _)),
    export_c_func!(CATransform3DTranslate(_, _, _, _)),
    export_c_func!(CATransform3DScale(_, _, _, _)),
    export_c_func!(CATransform3DRotate(_, _, _, _, _)),
    export_c_func!(CATransform3DConcat(_, _)),
    export_c_func!(CATransform3DIsIdentity(_)),
    export_c_func!(CATransform3DEqualToTransform(_, _)),
    export_c_func!(CATransform3DIsAffine(_)),
    export_c_func!(CATransform3DMakeAffineTransform(_)),
    export_c_func!(CATransform3DGetAffineTransform(_)),
];

#[cfg(test)]
mod tests {
    use super::{
        from_affine, is_affine, make_rotation, make_scale, make_translation, to_affine,
        CATransform3D, CATransform3DIdentity,
    };
    use crate::frameworks::core_graphics::cg_affine_transform::CGAffineTransform;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn the_struct_is_sixteen_floats_in_subscript_order() {
        assert_eq!(std::mem::size_of::<CATransform3D>(), 64);
        // rows() and from_rows() must be exact inverses, since every operation
        // here round-trips through them.
        let t = make_rotation(0.7, 1.0, 2.0, 3.0);
        assert_eq!(CATransform3D::from_rows(t.rows()), t);
    }

    #[test]
    fn a_z_rotation_matches_the_two_dimensional_one() {
        // This is the check that catches a transposed rotation, which is
        // otherwise a plausible-looking rotation by the opposite angle.
        let angle = 0.4_f32;
        let three_d = make_rotation(angle, 0.0, 0.0, 1.0);
        let two_d = CGAffineTransform::make_rotation(angle);
        assert!(close(three_d.m11, two_d.a));
        assert!(close(three_d.m12, two_d.b));
        assert!(close(three_d.m21, two_d.c));
        assert!(close(three_d.m22, two_d.d));
    }

    #[test]
    fn a_rotation_about_a_zero_axis_is_the_identity() {
        assert_eq!(make_rotation(1.0, 0.0, 0.0, 0.0), CATransform3DIdentity);
    }

    #[test]
    fn a_rotation_axis_does_not_have_to_be_normalised() {
        let unit = make_rotation(0.9, 0.0, 1.0, 0.0);
        let scaled = make_rotation(0.9, 0.0, 5.0, 0.0);
        for (a, b) in unit
            .rows()
            .iter()
            .flatten()
            .zip(scaled.rows().iter().flatten())
        {
            assert!(close(*a, *b));
        }
    }

    #[test]
    fn translation_happens_before_the_transform_it_is_applied_to() {
        // t' = translate * t, so translating a doubling transform by 1 and then
        // transforming gives 2, not 1 + something.
        let doubled = make_scale(2.0, 2.0, 2.0);
        let then_translated = make_translation(1.0, 0.0, 0.0).multiply(doubled);
        assert_eq!({ then_translated.m41 }, 2.0);
        // The other order leaves the translation unscaled, which is what
        // getting the product backwards would produce.
        let other_order = doubled.multiply(make_translation(1.0, 0.0, 0.0));
        assert_eq!({ other_order.m41 }, 1.0);
    }

    #[test]
    fn multiplying_by_the_identity_changes_nothing() {
        let t = make_rotation(0.3, 1.0, 1.0, 0.0).multiply(make_scale(2.0, 3.0, 4.0));
        assert_eq!(t.multiply(CATransform3DIdentity), t);
        assert_eq!(CATransform3DIdentity.multiply(t), t);
    }

    #[test]
    fn affineness_recognises_the_in_plane_transforms() {
        assert!(is_affine(CATransform3DIdentity));
        assert!(is_affine(make_translation(3.0, 4.0, 0.0)));
        assert!(is_affine(make_scale(2.0, 3.0, 1.0)));
        assert!(is_affine(make_rotation(0.5, 0.0, 0.0, 1.0)));
        // A z translation, a z scale and a rotation about any other axis all
        // leave the plane.
        assert!(!is_affine(make_translation(0.0, 0.0, 1.0)));
        assert!(!is_affine(make_scale(1.0, 1.0, 2.0)));
        assert!(!is_affine(make_rotation(0.5, 1.0, 0.0, 0.0)));
    }

    #[test]
    fn an_affine_transform_survives_the_round_trip() {
        let affine = CGAffineTransform {
            a: 1.5,
            b: 0.25,
            c: -0.75,
            d: 2.0,
            tx: 10.0,
            ty: -20.0,
        };
        let promoted = from_affine(affine);
        assert!(is_affine(promoted));
        assert_eq!(to_affine(promoted), affine);
    }
}
