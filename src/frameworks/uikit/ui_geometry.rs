//! `UIGeometry.h`
//!
//! See also [crate::frameworks::core_graphics::cg_geometry].

use crate::abi::{impl_GuestRet_for_large_struct, GuestArg};
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string;
use crate::mem::{ConstVoidPtr, SafeRead};
use crate::objc::{autorelease, id, nil};
use crate::Environment;

/// `UIEdgeInsets`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C, packed)]
pub struct UIEdgeInsets {
    pub top: CGFloat,
    pub left: CGFloat,
    pub bottom: CGFloat,
    pub right: CGFloat,
}
unsafe impl SafeRead for UIEdgeInsets {}
impl_GuestRet_for_large_struct!(UIEdgeInsets);
impl GuestArg for UIEdgeInsets {
    const REG_COUNT: usize = 4;

    fn from_regs(regs: &[u32]) -> Self {
        UIEdgeInsets {
            top: GuestArg::from_regs(&regs[0..1]),
            left: GuestArg::from_regs(&regs[1..2]),
            bottom: GuestArg::from_regs(&regs[2..3]),
            right: GuestArg::from_regs(&regs[3..4]),
        }
    }
    fn to_regs(self, regs: &mut [u32]) {
        self.top.to_regs(&mut regs[0..1]);
        self.left.to_regs(&mut regs[1..2]);
        self.bottom.to_regs(&mut regs[2..3]);
        self.right.to_regs(&mut regs[3..4]);
    }
}

/// `UIEdgeInsetsZero`. Apps that lay out with insets read this constant, and
/// reading it means dereferencing it, so leaving the symbol unbound turns an
/// ordinary layout into a null-pointer crash.
fn UIEdgeInsetsZero(env: &mut Environment) -> ConstVoidPtr {
    env.mem
        .alloc_and_write(UIEdgeInsets::default())
        .cast()
        .cast_const()
}

pub const CONSTANTS: ConstantExports =
    &[("_UIEdgeInsetsZero", HostConstant::Custom(UIEdgeInsetsZero))];

// Apple's documentation says all of these return zeroes if the input is not
// well-formed. A nil string is not well-formed, so treat it as zeroes rather
// than dereferencing it as an NSString.
pub fn CGPointFromString(env: &mut Environment, string: id) -> CGPoint {
    if string == nil {
        return Default::default();
    }
    // TODO: avoid copy
    ns_string::to_rust_string(env, string)
        .parse()
        .unwrap_or_default()
}
pub fn CGSizeFromString(env: &mut Environment, string: id) -> CGSize {
    if string == nil {
        return Default::default();
    }
    // TODO: avoid copy
    ns_string::to_rust_string(env, string)
        .parse()
        .unwrap_or_default()
}
pub fn CGRectFromString(env: &mut Environment, string: id) -> CGRect {
    if string == nil {
        return Default::default();
    }
    // TODO: avoid copy
    ns_string::to_rust_string(env, string)
        .parse()
        .unwrap_or_default()
}

pub fn NSStringFromCGPoint(env: &mut Environment, point: CGPoint) -> id {
    let s = ns_string::from_rust_string(env, point.to_string());
    autorelease(env, s)
}
pub fn NSStringFromCGSize(env: &mut Environment, size: CGSize) -> id {
    let s = ns_string::from_rust_string(env, size.to_string());
    autorelease(env, s)
}
pub fn NSStringFromCGRect(env: &mut Environment, rect: CGRect) -> id {
    let s = ns_string::from_rust_string(env, rect.to_string());
    autorelease(env, s)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGPointFromString(_)),
    export_c_func!(CGSizeFromString(_)),
    export_c_func!(CGRectFromString(_)),
    export_c_func!(NSStringFromCGPoint(_)),
    export_c_func!(NSStringFromCGSize(_)),
    export_c_func!(NSStringFromCGRect(_)),
];
