/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIGraphics.h`

use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_graphics::cg_bitmap_context::{
    CGBitmapContextCreate, CGBitmapContextCreateImage,
};
use crate::frameworks::core_graphics::cg_color_space::{
    CGColorSpaceCreateDeviceRGB, CGColorSpaceRelease,
};
use crate::frameworks::core_graphics::cg_context::{
    kCGBlendModeCopy, CGBlendMode, CGContextFillRect, CGContextRef, CGContextRelease,
    CGContextRestoreGState, CGContextRetain, CGContextSaveGState, CGContextSetBlendMode,
};
use crate::frameworks::core_graphics::cg_image::{kCGImageAlphaPremultipliedLast, CGImageRelease};
use crate::frameworks::core_graphics::{CGFloat, CGRect, CGSize};
use crate::mem::{GuestUSize, Ptr};
use crate::objc::{id, msg_class, nil};
use crate::Environment;

#[derive(Default)]
pub(super) struct State {
    pub(super) context_stack: Vec<CGContextRef>,
}

pub fn UIGraphicsPushContext(env: &mut Environment, context: CGContextRef) {
    CGContextRetain(env, context);
    env.framework_state
        .uikit
        .ui_graphics
        .context_stack
        .push(context);
}
pub fn UIGraphicsPopContext(env: &mut Environment) {
    let context = env.framework_state.uikit.ui_graphics.context_stack.pop();
    CGContextRelease(env, context.unwrap());
}
pub fn UIGraphicsGetCurrentContext(env: &mut Environment) -> CGContextRef {
    env.framework_state
        .uikit
        .ui_graphics
        .context_stack
        .last()
        .copied()
        .unwrap_or(nil)
}

/// Begin drawing into an offscreen bitmap that can be turned into a UIImage.
///
/// This is the same context stack the rest of UIGraphics uses, so ordinary
/// drawing code lands in the bitmap without knowing anything has changed —
/// which is the whole point of the API.
fn UIGraphicsBeginImageContext(env: &mut Environment, size: CGSize) {
    UIGraphicsBeginImageContextWithOptions(env, size, false, 1.0)
}

/// `opaque` is a hint that the caller will fill every pixel; the bitmap here is
/// always RGBA either way, so honouring it would only change the alpha channel
/// of pixels the caller promised to overwrite. `scale` of 0 means "use the
/// device scale", which is 1 on the devices tapHLE models.
fn UIGraphicsBeginImageContextWithOptions(
    env: &mut Environment,
    size: CGSize,
    _opaque: bool,
    scale: CGFloat,
) {
    let scale = if scale == 0.0 { 1.0 } else { scale };
    let width = (size.width * scale).round().max(1.0) as GuestUSize;
    let height = (size.height * scale).round().max(1.0) as GuestUSize;

    let color_space = CGColorSpaceCreateDeviceRGB(env);
    let context = CGBitmapContextCreate(
        env,
        // Null asks Core Graphics to allocate and own the backing store, so
        // there is no buffer for this module to track or free.
        Ptr::null(),
        width,
        height,
        8,
        // Zero means "work it out from the width and colour space".
        0,
        color_space,
        kCGImageAlphaPremultipliedLast,
    );
    CGColorSpaceRelease(env, color_space);

    UIGraphicsPushContext(env, context);
    // The stack holds its own reference now.
    CGContextRelease(env, context);
}

/// Snapshot the current image context. Does not end it: UIKit allows several
/// snapshots of the same context, and callers rely on that.
fn UIGraphicsGetImageFromCurrentImageContext(env: &mut Environment) -> id {
    let context = UIGraphicsGetCurrentContext(env);
    if context == nil {
        log!("Warning: UIGraphicsGetImageFromCurrentImageContext() with no current context, returning nil");
        return nil;
    }
    let cg_image = CGBitmapContextCreateImage(env, context);
    if cg_image == nil {
        return nil;
    }
    let image: id = msg_class![env; UIImage imageWithCGImage:cg_image];
    // imageWithCGImage: takes its own reference.
    CGImageRelease(env, cg_image);
    image
}

fn UIGraphicsEndImageContext(env: &mut Environment) {
    if env
        .framework_state
        .uikit
        .ui_graphics
        .context_stack
        .is_empty()
    {
        log!("Warning: UIGraphicsEndImageContext() with no current context, ignoring");
        return;
    }
    UIGraphicsPopContext(env);
}

/// `UIRectFillUsingBlendMode` — fill a rectangle in the current context with
/// the current fill colour and a given blend mode.
///
/// The blend mode is set for the fill and put back afterwards, which is what
/// UIKit documents: this is a drawing convenience, not a state change the
/// caller has to undo.
fn UIRectFillUsingBlendMode(env: &mut Environment, rect: CGRect, blend_mode: CGBlendMode) {
    let context = UIGraphicsGetCurrentContext(env);
    if context == nil {
        // Drawing outside a -drawRect: or an image context has nowhere to go.
        // UIKit logs and does nothing, and so does this.
        log!("Warning: UIRectFill outside a drawing context, ignoring");
        return;
    }
    CGContextSaveGState(env, context);
    CGContextSetBlendMode(env, context, blend_mode);
    CGContextFillRect(env, context, rect);
    CGContextRestoreGState(env, context);
}

/// `UIRectFill` — the same thing with the blend mode UIKit uses by default.
/// Copy rather than normal, so filling with a transparent colour clears the
/// rectangle instead of leaving what was underneath.
fn UIRectFill(env: &mut Environment, rect: CGRect) {
    UIRectFillUsingBlendMode(env, rect, kCGBlendModeCopy)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(UIRectFill(_)),
    export_c_func!(UIRectFillUsingBlendMode(_, _)),
    export_c_func!(UIGraphicsPushContext(_)),
    export_c_func!(UIGraphicsPopContext()),
    export_c_func!(UIGraphicsGetCurrentContext()),
    export_c_func!(UIGraphicsBeginImageContext(_)),
    export_c_func!(UIGraphicsBeginImageContextWithOptions(_, _, _)),
    export_c_func!(UIGraphicsGetImageFromCurrentImageContext()),
    export_c_func!(UIGraphicsEndImageContext()),
];
