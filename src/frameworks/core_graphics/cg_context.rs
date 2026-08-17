/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CGContext.h`

use super::cg_affine_transform::{CGAffineTransform, CGAffineTransformIdentity};
use super::cg_bitmap_context::{
    CGBitmapContextDrawer, CGBitmapContextGetHeight, CGBitmapContextGetWidth,
};
use super::cg_color::CGColorRef;
use super::cg_color_space::{
    components_in_model, kCGColorSpaceModelMonochrome, kCGColorSpaceModelRGB, CGColorSpaceGetModel,
    CGColorSpaceModel, CGColorSpaceRef,
};
use super::cg_font::{CGFontHostObject, CGFontRef, CGFontRelease, CGFontRetain, CGGlyph};
use super::cg_geometry::CGPointZero;
use super::cg_gradient::{CGGradientDrawingOptions, CGGradientRef};
use super::cg_image::CGImageRef;
use super::cg_path::{borrow_path, CGPathRef, Path};
use super::{cg_bitmap_context, cg_color, cg_gradient, CGFloat, CGPoint, CGRect, CGSize};
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_foundation::{CFRelease, CFRetain, CFTypeRef};
use crate::frameworks::uikit;
use crate::mem::{ConstPtr, GuestUSize};
use crate::objc::{objc_classes, ClassExports, HostObject};
use crate::Environment;

type CGInterpolationQuality = i32;

type CGPathDrawingMode = i32;
const kCGPathFill: CGPathDrawingMode = 0;
const kCGPathEOFill: CGPathDrawingMode = 1;
const kCGPathStroke: CGPathDrawingMode = 2;
const kCGPathFillStroke: CGPathDrawingMode = 3;
const kCGPathEOFillStroke: CGPathDrawingMode = 4;

type CGTextDrawingMode = i32;
const kCGTextFill: CGTextDrawingMode = 0;
const kCGTextFillStroke: CGTextDrawingMode = 2;

pub type CGBlendMode = i32;
pub const kCGBlendModeNormal: CGBlendMode = 0;
pub const kCGBlendModeMultiply: CGBlendMode = 1;
pub const kCGBlendModeScreen: CGBlendMode = 2;
#[allow(unused)]
pub const kCGBlendModeOverlay: CGBlendMode = 3;
pub const kCGBlendModeDarken: CGBlendMode = 4;
pub const kCGBlendModeLighten: CGBlendMode = 5;
pub const kCGBlendModeCopy: CGBlendMode = 17;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// CGContext seems to be a CFType-based type, but in our implementation those
// are just Objective-C types, so we need a class for it, but its name is not
// visible anywhere.
@implementation _tapHLE_CGContext: NSObject

- (())dealloc {
    let host_obj = env.objc.borrow::<CGContextHostObject>(this);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    if bitmap_data.data_is_owned {
        env.mem.free(bitmap_data.data);
    }
    CGFontRelease(env, host_obj.font);

    env.objc.dealloc_object(this, &mut env.mem)
}

@end

};

/// The part of a context's graphics state that `CGContextSaveGState` keeps.
///
/// **The line width and the current path are still not saved**, and that is a
/// gap rather than a design: a caller that saves, changes the line width, and
/// restores will find the new width still in force. Adding them means deciding
/// what a saved path should do on restore, which is why they are named here
/// instead of being quietly absent.
///
/// This was a positional tuple, which is exactly the shape a field gets added
/// to wrongly — every `state.3` after the insertion point silently means
/// something else.
pub(super) struct ContextState {
    rgb_fill_color: (CGFloat, CGFloat, CGFloat, CGFloat),
    rgb_stroke_color: (CGFloat, CGFloat, CGFloat, CGFloat),
    fill_color_space: CGColorSpaceModel,
    stroke_color_space: CGColorSpaceModel,
    transform: CGAffineTransform,
    font: CGFontRef,
    font_size: CGFloat,
    blend_mode: CGBlendMode,
}

pub(super) struct CGContextHostObject {
    pub(super) subclass: CGContextSubclass,
    /// The colour space a component array handed to [CGContextSetFillColor] is
    /// read in. Not the same thing as the bitmap's colour space: an RGB bitmap
    /// can perfectly well be filled with a grey colour.
    pub(super) fill_color_space: CGColorSpaceModel,
    pub(super) stroke_color_space: CGColorSpaceModel,
    pub(super) rgb_fill_color: (CGFloat, CGFloat, CGFloat, CGFloat),
    pub(super) font: CGFontRef,
    pub(super) font_size: CGFloat,
    /// Current transform.
    pub(super) transform: CGAffineTransform,
    pub(super) blend_mode: CGBlendMode,
    /// Text transform.
    pub(super) text_transform: Option<CGAffineTransform>,
    pub(super) state_stack: Vec<ContextState>,
    /// The path being built, in user space. It is transformed by the CTM at
    /// draw time rather than as points are added, because the CTM in force when
    /// the path is *drawn* is the one CoreGraphics uses.
    pub(super) path: Path,
    pub(super) rgb_stroke_color: (CGFloat, CGFloat, CGFloat, CGFloat),
    pub(super) line_width: CGFloat,
    /// The text pen position. See `CGContextGetTextPosition` for the caveat.
    pub(super) text_position: CGPoint,
}
impl HostObject for CGContextHostObject {}

pub(super) enum CGContextSubclass {
    CGBitmapContext(cg_bitmap_context::CGBitmapContextData),
}

pub type CGContextRef = CFTypeRef;

pub fn CGContextRelease(env: &mut Environment, c: CGContextRef) {
    if !c.is_null() {
        CFRelease(env, c);
    }
}
pub fn CGContextRetain(env: &mut Environment, c: CGContextRef) -> CGContextRef {
    if !c.is_null() {
        CFRetain(env, c)
    } else {
        c
    }
}

// Public so UIKit's rectangle-filling helpers can set the mode for one fill.
pub fn CGContextSetBlendMode(
    env: &mut Environment,
    context: CGContextRef,
    blend_mode: CGBlendMode,
) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .blend_mode = blend_mode;
}

/// Read a colour from a component array in `model`'s colour space, as RGBA.
///
/// The array carries one entry per component of the space and then one for
/// alpha, so the component count has to come from the space rather than being
/// assumed: reading four entries out of a two-entry grey colour picks up
/// whatever follows it.
fn read_color_components(
    env: &mut Environment,
    model: CGColorSpaceModel,
    components: ConstPtr<CGFloat>,
) -> (CGFloat, CGFloat, CGFloat, CGFloat) {
    let count = components_in_model(model);
    let alpha = env.mem.read(components + count);
    match model {
        kCGColorSpaceModelMonochrome => {
            let gray = env.mem.read(components);
            (gray, gray, gray, alpha)
        }
        kCGColorSpaceModelRGB => (
            env.mem.read(components),
            env.mem.read(components + 1),
            env.mem.read(components + 2),
            alpha,
        ),
        _ => unimplemented!("colour space model {}", model),
    }
}

fn CGContextSetFillColorSpace(
    env: &mut Environment,
    context: CGContextRef,
    space: CGColorSpaceRef,
) {
    let color_model = CGColorSpaceGetModel(env, space);
    assert!(color_model == kCGColorSpaceModelMonochrome || color_model == kCGColorSpaceModelRGB);
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .fill_color_space = color_model;
}

fn CGContextSetStrokeColorSpace(
    env: &mut Environment,
    context: CGContextRef,
    space: CGColorSpaceRef,
) {
    let color_model = CGColorSpaceGetModel(env, space);
    assert!(color_model == kCGColorSpaceModelMonochrome || color_model == kCGColorSpaceModelRGB);
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .stroke_color_space = color_model;
}

/// `CGContextSetFillColor` — the colour space-relative fill setter.
///
/// The array is interpreted in whatever colour space
/// [CGContextSetFillColorSpace] last set, which is why that function had to
/// stop discarding its argument for this one to be possible at all. A caller is
/// required to set the space first; the documented default if it does not is
/// device grey, and that is what a fresh context reports here.
fn CGContextSetFillColor(
    env: &mut Environment,
    context: CGContextRef,
    components: ConstPtr<CGFloat>,
) {
    let model = env
        .objc
        .borrow::<CGContextHostObject>(context)
        .fill_color_space;
    let (r, g, b, a) = read_color_components(env, model, components);
    CGContextSetRGBFillColor(env, context, r, g, b, a)
}

/// `CGContextSetStrokeColor`, the stroke half of [CGContextSetFillColor].
fn CGContextSetStrokeColor(
    env: &mut Environment,
    context: CGContextRef,
    components: ConstPtr<CGFloat>,
) {
    let model = env
        .objc
        .borrow::<CGContextHostObject>(context)
        .stroke_color_space;
    let (r, g, b, a) = read_color_components(env, model, components);
    CGContextSetRGBStrokeColor(env, context, r, g, b, a)
}

fn CGContextSetFillColorWithColor(env: &mut Environment, context: CGContextRef, color: CGColorRef) {
    let (r, g, b, a) = cg_color::to_rgba(&env.objc, color);
    CGContextSetRGBFillColor(env, context, r, g, b, a)
}

fn CGContextSetStrokeColorWithColor(
    env: &mut Environment,
    context: CGContextRef,
    color: CGColorRef,
) {
    let (r, g, b, a) = cg_color::to_rgba(&env.objc, color);
    CGContextSetRGBStrokeColor(env, context, r, g, b, a)
}

pub fn CGContextSetRGBFillColor(
    env: &mut Environment,
    context: CGContextRef,
    red: CGFloat,
    green: CGFloat,
    blue: CGFloat,
    alpha: CGFloat,
) {
    let color = (red, green, blue, alpha);
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .rgb_fill_color = color;
}

fn CGContextSetGrayFillColor(
    env: &mut Environment,
    context: CGContextRef,
    gray: CGFloat,
    alpha: CGFloat,
) {
    let color = (gray, gray, gray, alpha);
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .rgb_fill_color = color;
}

fn CGContextSetGrayStrokeColor(
    env: &mut Environment,
    context: CGContextRef,
    gray: CGFloat,
    alpha: CGFloat,
) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .rgb_stroke_color = (gray, gray, gray, alpha);
}
fn CGContextSetRGBStrokeColor(
    env: &mut Environment,
    context: CGContextRef,
    r: CGFloat,
    g: CGFloat,
    b: CGFloat,
    a: CGFloat,
) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .rgb_stroke_color = (r, g, b, a);
}

fn CGContextSetShadowWithColor(
    _env: &mut Environment,
    context: CGContextRef,
    offset: CGSize,
    blur: CGFloat,
    color: CGColorRef,
) {
    log!(
        "TODO: CGContextSetShadowWithColor({:?}, {}, {}, {:?})",
        context,
        offset,
        blur,
        color
    );
}

pub fn CGContextFillRect(env: &mut Environment, context: CGContextRef, rect: CGRect) {
    cg_bitmap_context::fill_rect(env, context, rect, /* clear: */ false);
}

pub fn CGContextClearRect(env: &mut Environment, context: CGContextRef, rect: CGRect) {
    cg_bitmap_context::fill_rect(env, context, rect, /* clear: */ true);
}

fn CGContextClipToRect(env: &mut Environment, context: CGContextRef, rect: CGRect) {
    if rect.origin == CGPointZero
        && rect.size.height == CGBitmapContextGetHeight(env, context) as f32
        && rect.size.width == CGBitmapContextGetWidth(env, context) as f32
    {
        assert!(env
            .objc
            .borrow_mut::<CGContextHostObject>(context)
            .transform
            .is_identity());
        // All good, clipping is not needed!
        return;
    }
    // tapHLE has no clip state: no drawing primitive here consults one, so
    // honouring this would mean adding a clip mask to CGBitmapContextDrawer and
    // threading it through every one of them. Until that exists, ignoring the
    // clip draws too much rather than too little — content that should have
    // been cut off spills outside the intended rectangle.
    //
    // That is worse output but a live app. Aborting here, which is what this
    // did, loses everything the app would otherwise have drawn correctly.
    log_once!("TODO: CGContextClipToRect() is ignored; drawing will not be clipped");
}

/// Clip to the current path. Ignored, for the reasons in
/// [CGContextClipToRect] — and additionally because a path clip needs a
/// coverage mask, not just a rectangle.
///
/// The path is consumed either way, as CoreGraphics does, so a caller that
/// clips and then draws does not accidentally fill the clip path as well.
fn CGContextClip(env: &mut Environment, context: CGContextRef) {
    log_once!("TODO: CGContextClip() is ignored; drawing will not be clipped");
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .path
        .clear();
}

pub fn CGContextConcatCTM(
    env: &mut Environment,
    context: CGContextRef,
    transform: CGAffineTransform,
) {
    log_dbg!("CGContextConcatCTM({:?})", transform);
    let host_obj = env.objc.borrow_mut::<CGContextHostObject>(context);
    host_obj.transform = transform.concat(host_obj.transform);
}
pub fn CGContextGetCTM(env: &mut Environment, context: CGContextRef) -> CGAffineTransform {
    let res = env.objc.borrow::<CGContextHostObject>(context).transform;
    log_dbg!("CGContextGetCTM() => {:?}", res);
    res
}
pub fn CGContextRotateCTM(env: &mut Environment, context: CGContextRef, angle: CGFloat) {
    log_dbg!("CGContextRotateCTM({:?})", angle);
    let host_obj = env.objc.borrow_mut::<CGContextHostObject>(context);
    host_obj.transform = host_obj.transform.rotate(angle);
}
pub fn CGContextScaleCTM(env: &mut Environment, context: CGContextRef, x: CGFloat, y: CGFloat) {
    log_dbg!("CGContextScaleCTM({:?})", (x, y));
    let host_obj = env.objc.borrow_mut::<CGContextHostObject>(context);
    host_obj.transform = host_obj.transform.scale(x, y);
}
pub fn CGContextTranslateCTM(
    env: &mut Environment,
    context: CGContextRef,
    tx: CGFloat,
    ty: CGFloat,
) {
    log_dbg!("CGContextTranslateCTM({:?})", (tx, ty));
    let host_obj = env.objc.borrow_mut::<CGContextHostObject>(context);
    host_obj.transform = host_obj.transform.translate(tx, ty);
}

pub fn CGContextDrawImage(
    env: &mut Environment,
    context: CGContextRef,
    rect: CGRect,
    image: CGImageRef,
) {
    cg_bitmap_context::draw_image(env, context, rect, image);
}

fn CGContextDrawLinearGradient(
    env: &mut Environment,
    context: CGContextRef,
    gradient: CGGradientRef,
    start_point: CGPoint,
    end_point: CGPoint,
    options: CGGradientDrawingOptions,
) {
    if gradient.is_null() {
        return;
    }
    cg_gradient::draw_linear_gradient(env, context, gradient, start_point, end_point, options);
}

fn CGContextDrawRadialGradient(
    env: &mut Environment,
    context: CGContextRef,
    gradient: CGGradientRef,
    start_center: CGPoint,
    start_radius: CGFloat,
    end_center: CGPoint,
    end_radius: CGFloat,
    options: CGGradientDrawingOptions,
) {
    if gradient.is_null() {
        return;
    }
    cg_gradient::draw_radial_gradient(
        env,
        context,
        gradient,
        start_center,
        start_radius,
        end_center,
        end_radius,
        options,
    );
}

pub fn CGContextSaveGState(env: &mut Environment, context: CGContextRef) {
    let host_obj = env.objc.borrow_mut::<CGContextHostObject>(context);
    host_obj.state_stack.push(ContextState {
        rgb_fill_color: host_obj.rgb_fill_color,
        rgb_stroke_color: host_obj.rgb_stroke_color,
        fill_color_space: host_obj.fill_color_space,
        stroke_color_space: host_obj.stroke_color_space,
        transform: host_obj.transform,
        font: host_obj.font,
        font_size: host_obj.font_size,
        blend_mode: host_obj.blend_mode,
    });
    CGFontRetain(env, env.objc.borrow::<CGContextHostObject>(context).font);
}

pub fn CGContextRestoreGState(env: &mut Environment, context: CGContextRef) {
    // We need to release _old_ font, there are 2 cases:
    // - font hasn't been set between save/restore -> this release corresponds
    // the font retain from save
    // - font has been set between save/restore -> we need to release old font
    // retained on the set
    CGFontRelease(env, env.objc.borrow::<CGContextHostObject>(context).font);
    let host_obj = env.objc.borrow_mut::<CGContextHostObject>(context);
    let state = host_obj.state_stack.pop().unwrap();
    host_obj.rgb_fill_color = state.rgb_fill_color;
    host_obj.rgb_stroke_color = state.rgb_stroke_color;
    host_obj.fill_color_space = state.fill_color_space;
    host_obj.stroke_color_space = state.stroke_color_space;
    host_obj.transform = state.transform;
    host_obj.font = state.font;
    host_obj.font_size = state.font_size;
    host_obj.blend_mode = state.blend_mode;
}

fn CGContextSetInterpolationQuality(
    _env: &mut Environment,
    context: CGContextRef,
    quality: CGInterpolationQuality,
) {
    log!(
        "TODO: CGContextSetInterpolationQuality({:?}, {:?})",
        context,
        quality
    );
}
fn CGContextSetAllowsAntialiasing(_env: &mut Environment, context: CGContextRef, allow: bool) {
    log!(
        "TODO: CGContextSetAllowsAntialiasing({:?}, {})",
        context,
        allow
    );
}

// The per-context switch, as against the allows-antialiasing one above which
// says whether it may happen at all. tapHLE's rasterisers do not offer the
// choice, so both are accepted and recorded rather than acted on; refusing
// either ends an app over the quality of its edges.
fn CGContextSetShouldAntialias(_env: &mut Environment, context: CGContextRef, should: bool) {
    log!(
        "TODO: CGContextSetShouldAntialias({:?}, {})",
        context,
        should
    );
}

fn CGContextSetShouldSmoothFonts(_env: &mut Environment, context: CGContextRef, should: bool) {
    log!(
        "TODO: CGContextSetShouldSmoothFonts({:?}, {})",
        context,
        should
    );
}

fn CGContextSetFont(env: &mut Environment, context: CGContextRef, font: CGFontRef) {
    CGFontRetain(env, font);
    let old_font = env.objc.borrow_mut::<CGContextHostObject>(context).font;
    CGFontRelease(env, old_font);
    env.objc.borrow_mut::<CGContextHostObject>(context).font = font;
}

fn CGContextSetFontSize(env: &mut Environment, context: CGContextRef, size: CGFloat) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .font_size = size;
}

fn CGContextSetTextDrawingMode(
    _env: &mut Environment,
    _context: CGContextRef,
    mode: CGTextDrawingMode,
) {
    assert!(mode == kCGTextFill || mode == kCGTextFillStroke); // TODO: support other modes
}

fn CGContextSetTextMatrix(
    env: &mut Environment,
    context: CGContextRef,
    transform: CGAffineTransform,
) {
    log_dbg!("CGContextSetTextMatrix({:?})", transform);
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .text_transform = Some(transform);
}

fn CGContextShowGlyphsAtPoint(
    env: &mut Environment,
    context: CGContextRef,
    x: CGFloat,
    y: CGFloat,
    glyphs: ConstPtr<CGGlyph>,
    count: GuestUSize,
) {
    let mut glyph_ids = Vec::new();
    for i in 0..count {
        let glyph_id = env.mem.read(glyphs + i);
        glyph_ids.push(rusttype::GlyphId(glyph_id));
    }

    let font = env.objc.borrow::<CGContextHostObject>(context).font;
    let font_size = env.objc.borrow::<CGContextHostObject>(context).font_size;
    let text_transform = env
        .objc
        .borrow::<CGContextHostObject>(context)
        .text_transform
        .unwrap_or(CGAffineTransformIdentity);

    let font = &env.objc.borrow::<CGFontHostObject>(font).font;

    let mut drawer = CGBitmapContextDrawer::new(&env.objc, &mut env.mem, context);
    let fill_color = drawer.rgb_fill_color();

    font.draw_glyphs(
        font_size,
        glyph_ids,
        (x, y),
        text_transform,
        |raster_glyph| {
            uikit::ui_font::draw_font_glyph(
                &mut drawer,
                raster_glyph,
                fill_color,
                /* clip_x: */ None,
                /* clip_y: */ None,
            )
        },
    );

    // Record where the run started. See CGContextGetTextPosition: this is not
    // advanced past the glyphs drawn, because the rasteriser does not report
    // how wide they were.
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .text_position = CGPoint { x, y };
}

fn CGContextShowGlyphsAtPositions(
    env: &mut Environment,
    context: CGContextRef,
    glyphs: ConstPtr<CGGlyph>,
    positions: ConstPtr<CGPoint>,
    count: GuestUSize,
) {
    let text_transform = env
        .objc
        .borrow::<CGContextHostObject>(context)
        .text_transform
        .unwrap_or(CGAffineTransformIdentity);
    assert!(text_transform.tx == 0.0 && text_transform.ty == 0.0); // TODO

    for i in 0..count {
        let glyph_ptr = glyphs + i;
        let point = env.mem.read(positions + i);
        let transformed_point = text_transform.apply_to_point(point);
        CGContextShowGlyphsAtPoint(
            env,
            context,
            transformed_point.x,
            transformed_point.y,
            glyph_ptr,
            1,
        );
    }
}

/// What [rasterise_path] should do with the path.
#[derive(Copy, Clone, PartialEq, Eq)]
enum PathRaster {
    /// Fill by the non-zero winding rule, CoreGraphics' default and what
    /// `CGContextFillPath` selects.
    FillNonZero,
    /// Fill by the even-odd rule, which `CGContextEOFillPath` selects. The two
    /// differ only where a path overlaps itself: a shape drawn inside another
    /// in the same direction is solid under the non-zero rule and a hole under
    /// even-odd.
    FillEvenOdd,
    Stroke,
}

/// Fill or stroke the current path into a bitmap context.
///
/// Each scanline is sampled at its centre and the crossings of every edge
/// collected. That is exact for a polygon, and since curves are flattened to
/// polygons when they are added, exact for everything a path here can hold.
///
/// There is no anti-aliasing, matching the rest of tapHLE's CoreGraphics
/// rasterisation, so a diagonal or curved edge comes out visibly stepped. That
/// is the main quality limitation and it is worth knowing before blaming a
/// game's own artwork.
fn rasterise_path(env: &mut Environment, context: CGContextRef, mode: PathRaster) {
    let host_obj = env.objc.borrow::<CGContextHostObject>(context);
    let transform = host_obj.transform;
    let line_width = host_obj.line_width;
    let stroke_color = host_obj.rgb_stroke_color;
    let path = host_obj.path.clone();
    if path.is_empty() {
        return;
    }

    // Into device space once, up front: every consumer below wants pixels.
    let subpaths: Vec<Vec<CGPoint>> = path
        .subpaths
        .iter()
        .filter(|s| s.points.len() >= 2)
        .map(|s| {
            s.points
                .iter()
                .map(|&p| transform.apply_to_point(p))
                .collect()
        })
        .collect();
    if subpaths.is_empty() {
        return;
    }

    let mut drawer = CGBitmapContextDrawer::new(&env.objc, &mut env.mem, context);
    // The drawer only knows the fill colour, so a stroke supplies its own. It
    // still needs the same gamma and premultiply treatment the fill colour
    // gets, which is what prepare_color does.
    let color = if mode == PathRaster::Stroke {
        drawer.prepare_color(stroke_color)
    } else {
        drawer.rgb_fill_color()
    };

    if mode == PathRaster::Stroke {
        let half = ((line_width.max(1.0) - 1.0) / 2.0).round() as i32;
        for points in &subpaths {
            for pair in points.windows(2) {
                draw_line(&mut drawer, pair[0], pair[1], color, half);
            }
        }
        return;
    }

    let (mut min_y, mut max_y) = (f32::INFINITY, f32::NEG_INFINITY);
    for points in &subpaths {
        for p in points {
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
    }
    let y_start = min_y.floor().max(0.0) as i32;
    let y_end = max_y.ceil().min(drawer.height() as f32) as i32;
    let width = drawer.width() as i32;

    let mut crossings: Vec<(f32, i32)> = Vec::new();
    for y in y_start..y_end {
        let sample_y = y as f32 + 0.5;
        crossings.clear();
        for points in &subpaths {
            let n = points.len();
            for i in 0..n {
                let a = points[i];
                // A fill treats every subpath as closed, as CoreGraphics does,
                // so the last edge wraps whether or not `closed` was set.
                let b = points[(i + 1) % n];
                if (a.y <= sample_y) == (b.y <= sample_y) {
                    continue;
                }
                let t = (sample_y - a.y) / (b.y - a.y);
                crossings.push((a.x + t * (b.x - a.x), if b.y > a.y { 1 } else { -1 }));
            }
        }
        if crossings.len() < 2 {
            continue;
        }
        crossings.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut winding = 0;
        for pair in 0..crossings.len() - 1 {
            winding += crossings[pair].1;
            if !span_is_inside(mode, pair, winding) {
                continue;
            }
            let x_from = crossings[pair].0.ceil().max(0.0) as i32;
            let x_to = crossings[pair + 1].0.floor().min(width as f32 - 1.0) as i32;
            for x in x_from..=x_to {
                if x >= 0 && x < width {
                    drawer.put_pixel((x, y), color, /* blend: */ true);
                }
            }
        }
    }
}

/// Whether the span between crossing `index` and the next one is inside the
/// path, under the fill rule in force.
///
/// `winding` is the running signed sum up to and including crossing `index`.
/// The even-odd rule ignores direction and counts instead: `index + 1` edges
/// have been crossed to reach this span, so it is inside when that count is
/// odd, which is when `index` is even.
fn span_is_inside(mode: PathRaster, index: usize, winding: i32) -> bool {
    match mode {
        PathRaster::FillEvenOdd => index.is_multiple_of(2),
        _ => winding != 0,
    }
}

/// A straight line, thickened by stamping a square of side `2 * half + 1` at
/// each step. Crude next to a real stroker — joins and caps are whatever the
/// squares happen to produce — but right for the thin lines apps of this era
/// draw, and it does not pretend otherwise.
fn draw_line(
    drawer: &mut CGBitmapContextDrawer,
    a: CGPoint,
    b: CGPoint,
    color: (CGFloat, CGFloat, CGFloat, CGFloat),
    half: i32,
) {
    let steps = ((b.x - a.x).abs().max((b.y - a.y).abs()).ceil() as i32).max(1);
    let (width, height) = (drawer.width() as i32, drawer.height() as i32);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (a.x + (b.x - a.x) * t).round() as i32;
        let y = (a.y + (b.y - a.y) * t).round() as i32;
        for dy in -half..=half {
            for dx in -half..=half {
                let (px, py) = (x + dx, y + dy);
                if px >= 0 && px < width && py >= 0 && py < height {
                    drawer.put_pixel((px, py), color, /* blend: */ true);
                }
            }
        }
    }
}

fn CGContextBeginPath(env: &mut Environment, context: CGContextRef) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .path
        .clear();
}

fn CGContextMoveToPoint(env: &mut Environment, context: CGContextRef, x: CGFloat, y: CGFloat) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .path
        .move_to(CGPoint { x, y });
}

fn CGContextAddLineToPoint(env: &mut Environment, context: CGContextRef, x: CGFloat, y: CGFloat) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .path
        .line_to(CGPoint { x, y });
}

fn CGContextAddRect(env: &mut Environment, context: CGContextRef, rect: CGRect) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .path
        .add_rect(rect);
}

fn CGContextAddArc(
    env: &mut Environment,
    context: CGContextRef,
    x: CGFloat,
    y: CGFloat,
    radius: CGFloat,
    start_angle: CGFloat,
    end_angle: CGFloat,
    clockwise: i32,
) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .path
        .add_arc(
            CGPoint { x, y },
            radius,
            start_angle,
            end_angle,
            clockwise != 0,
        );
}

fn CGContextAddArcToPoint(
    env: &mut Environment,
    context: CGContextRef,
    x1: CGFloat,
    y1: CGFloat,
    x2: CGFloat,
    y2: CGFloat,
    radius: CGFloat,
) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .path
        .add_arc_to_point(CGPoint { x: x1, y: y1 }, CGPoint { x: x2, y: y2 }, radius);
}

fn CGContextClosePath(env: &mut Environment, context: CGContextRef) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .path
        .close();
}

fn CGContextAddPath(env: &mut Environment, context: CGContextRef, path: CGPathRef) {
    if path.is_null() {
        return;
    }
    let other = borrow_path(env, path);
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .path
        .append(&other, CGAffineTransformIdentity);
}

fn CGContextFillPath(env: &mut Environment, context: CGContextRef) {
    draw_current_path(
        env,
        context,
        PathRaster::FillNonZero,
        /* stroke: */ false,
    );
}

/// `CGContextEOFillPath` - the same fill under the even-odd rule.
fn CGContextEOFillPath(env: &mut Environment, context: CGContextRef) {
    draw_current_path(
        env,
        context,
        PathRaster::FillEvenOdd,
        /* stroke: */ false,
    );
}

fn CGContextStrokePath(env: &mut Environment, context: CGContextRef) {
    draw_current_path(env, context, PathRaster::Stroke, /* stroke: */ false);
}

/// Paint the current path and then consume it, which every path-painting
/// function in CoreGraphics does.
///
/// `fill` says how, or whether, to fill; `stroke` adds a stroke on top, in that
/// order, which is the order `CGContextDrawPath`'s combined modes specify.
fn draw_current_path(env: &mut Environment, context: CGContextRef, fill: PathRaster, stroke: bool) {
    if fill != PathRaster::Stroke {
        rasterise_path(env, context, fill);
    }
    if stroke || fill == PathRaster::Stroke {
        rasterise_path(env, context, PathRaster::Stroke);
    }
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .path
        .clear();
}

/// `CGContextDrawPath` - paint the current path in whichever of the five
/// combinations of fill rule and stroke the caller names.
fn CGContextDrawPath(env: &mut Environment, context: CGContextRef, mode: CGPathDrawingMode) {
    let (fill, stroke) = match mode {
        kCGPathFill => (PathRaster::FillNonZero, false),
        kCGPathEOFill => (PathRaster::FillEvenOdd, false),
        kCGPathStroke => (PathRaster::Stroke, false),
        kCGPathFillStroke => (PathRaster::FillNonZero, true),
        kCGPathEOFillStroke => (PathRaster::FillEvenOdd, true),
        _ => {
            // An unrecognised mode is the app's mistake, not a reason to end
            // the app: consume the path and draw nothing.
            log!(
                "CGContextDrawPath() with unknown mode {}; drawing nothing",
                mode
            );
            env.objc
                .borrow_mut::<CGContextHostObject>(context)
                .path
                .clear();
            return;
        }
    };
    draw_current_path(env, context, fill, stroke);
}

/// `CGContextStrokeLineSegments` - stroke `count / 2` disconnected segments.
///
/// `count` is the number of *points*, and consecutive pairs are independent
/// segments rather than a polyline: this is the function for drawing a grid or
/// a set of tick marks, and treating it as [CGContextAddLines] would join them
/// all together. An odd count leaves a trailing point with no partner, which is
/// dropped.
fn CGContextStrokeLineSegments(
    env: &mut Environment,
    context: CGContextRef,
    points: ConstPtr<CGPoint>,
    count: GuestUSize,
) {
    let read: Vec<CGPoint> = (0..count).map(|i| env.mem.read(points + i)).collect();
    let path = &mut env.objc.borrow_mut::<CGContextHostObject>(context).path;
    path.clear();
    for pair in read.chunks_exact(2) {
        path.move_to(pair[0]);
        path.line_to(pair[1]);
    }
    draw_current_path(env, context, PathRaster::Stroke, /* stroke: */ false);
}

/// `CGContextAddLines` - a polyline appended to the current path.
fn CGContextAddLines(
    env: &mut Environment,
    context: CGContextRef,
    points: ConstPtr<CGPoint>,
    count: GuestUSize,
) {
    let read: Vec<CGPoint> = (0..count).map(|i| env.mem.read(points + i)).collect();
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .path
        .add_lines(&read);
}

fn CGContextAddEllipseInRect(env: &mut Environment, context: CGContextRef, rect: CGRect) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .path
        .add_ellipse_in_rect(rect);
}

/// Replace the current path with one shape and paint it.
///
/// The convenience painting functions begin a new path rather than adding to
/// whatever was there, so a half-built path is discarded rather than painted
/// along with the shape.
fn draw_shape(
    env: &mut Environment,
    context: CGContextRef,
    fill: PathRaster,
    build: impl FnOnce(&mut Path),
) {
    let path = &mut env.objc.borrow_mut::<CGContextHostObject>(context).path;
    path.clear();
    build(path);
    draw_current_path(env, context, fill, /* stroke: */ false);
}

fn CGContextFillEllipseInRect(env: &mut Environment, context: CGContextRef, rect: CGRect) {
    draw_shape(env, context, PathRaster::FillNonZero, |path| {
        path.add_ellipse_in_rect(rect)
    });
}

fn CGContextStrokeEllipseInRect(env: &mut Environment, context: CGContextRef, rect: CGRect) {
    draw_shape(env, context, PathRaster::Stroke, |path| {
        path.add_ellipse_in_rect(rect)
    });
}

fn CGContextStrokeRect(env: &mut Environment, context: CGContextRef, rect: CGRect) {
    draw_shape(env, context, PathRaster::Stroke, |path| path.add_rect(rect));
}

/// `CGContextStrokeRectWithWidth`.
///
/// The width change is not scoped to this call: the documentation is explicit
/// that it sets the context's line width, so a later stroke uses it too.
fn CGContextStrokeRectWithWidth(
    env: &mut Environment,
    context: CGContextRef,
    rect: CGRect,
    width: CGFloat,
) {
    CGContextSetLineWidth(env, context, width);
    CGContextStrokeRect(env, context, rect);
}

/// `CGContextFillRects` - fill an array of rectangles.
fn CGContextFillRects(
    env: &mut Environment,
    context: CGContextRef,
    rects: ConstPtr<CGRect>,
    count: GuestUSize,
) {
    for i in 0..count {
        let rect = env.mem.read(rects + i);
        CGContextFillRect(env, context, rect);
    }
}

/// The text pen position.
///
/// **It does not advance past glyphs that were drawn.**
/// `CGContextShowGlyphsAtPoint` sets it to where the text started, and tapHLE's
/// glyph rasteriser does not report how far it got, so the width of what was
/// drawn is not known here.
///
/// The consequence is specific and worth stating: a caller that draws a run,
/// reads the position, and draws the next run from it will overdraw the first —
/// which is exactly what this function exists for. Fixing it means having
/// `Font::draw_glyphs` return its final pen position; it already tracks that
/// internally.
fn CGContextGetTextPosition(env: &mut Environment, context: CGContextRef) -> CGPoint {
    env.objc
        .borrow::<CGContextHostObject>(context)
        .text_position
}

fn CGContextSetTextPosition(env: &mut Environment, context: CGContextRef, x: CGFloat, y: CGFloat) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .text_position = CGPoint { x, y };
}

fn CGContextSetLineWidth(env: &mut Environment, context: CGContextRef, width: CGFloat) {
    env.objc
        .borrow_mut::<CGContextHostObject>(context)
        .line_width = width;
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGContextSetStrokeColorWithColor(_, _)),
    export_c_func!(CGContextClip(_)),
    export_c_func!(CGContextBeginPath(_)),
    export_c_func!(CGContextMoveToPoint(_, _, _)),
    export_c_func!(CGContextAddLineToPoint(_, _, _)),
    export_c_func!(CGContextAddRect(_, _)),
    export_c_func!(CGContextAddArc(_, _, _, _, _, _, _)),
    export_c_func!(CGContextAddArcToPoint(_, _, _, _, _, _)),
    export_c_func!(CGContextClosePath(_)),
    export_c_func!(CGContextAddPath(_, _)),
    export_c_func!(CGContextFillPath(_)),
    export_c_func!(CGContextEOFillPath(_)),
    export_c_func!(CGContextStrokePath(_)),
    export_c_func!(CGContextDrawPath(_, _)),
    export_c_func!(CGContextStrokeLineSegments(_, _, _)),
    export_c_func!(CGContextAddLines(_, _, _)),
    export_c_func!(CGContextAddEllipseInRect(_, _)),
    export_c_func!(CGContextFillEllipseInRect(_, _)),
    export_c_func!(CGContextStrokeEllipseInRect(_, _)),
    export_c_func!(CGContextStrokeRect(_, _)),
    export_c_func!(CGContextStrokeRectWithWidth(_, _, _)),
    export_c_func!(CGContextFillRects(_, _, _)),
    export_c_func!(CGContextSetLineWidth(_, _)),
    export_c_func!(CGContextGetTextPosition(_)),
    export_c_func!(CGContextSetTextPosition(_, _, _)),
    export_c_func!(CGContextRetain(_)),
    export_c_func!(CGContextRelease(_)),
    export_c_func!(CGContextSetBlendMode(_, _)),
    export_c_func!(CGContextSetFillColorSpace(_, _)),
    export_c_func!(CGContextSetStrokeColorSpace(_, _)),
    export_c_func!(CGContextSetFillColor(_, _)),
    export_c_func!(CGContextSetStrokeColor(_, _)),
    export_c_func!(CGContextSetFillColorWithColor(_, _)),
    export_c_func!(CGContextSetRGBFillColor(_, _, _, _, _)),
    export_c_func!(CGContextSetGrayFillColor(_, _, _)),
    export_c_func!(CGContextSetGrayStrokeColor(_, _, _)),
    export_c_func!(CGContextSetRGBStrokeColor(_, _, _, _, _)),
    export_c_func!(CGContextSetShadowWithColor(_, _, _, _)),
    export_c_func!(CGContextFillRect(_, _)),
    export_c_func!(CGContextClearRect(_, _)),
    export_c_func!(CGContextClipToRect(_, _)),
    export_c_func!(CGContextConcatCTM(_, _)),
    export_c_func!(CGContextGetCTM(_)),
    export_c_func!(CGContextRotateCTM(_, _)),
    export_c_func!(CGContextScaleCTM(_, _, _)),
    export_c_func!(CGContextTranslateCTM(_, _, _)),
    export_c_func!(CGContextDrawImage(_, _, _)),
    export_c_func!(CGContextDrawLinearGradient(_, _, _, _, _)),
    export_c_func!(CGContextDrawRadialGradient(_, _, _, _, _, _, _)),
    export_c_func!(CGContextSaveGState(_)),
    export_c_func!(CGContextRestoreGState(_)),
    export_c_func!(CGContextSetInterpolationQuality(_, _)),
    export_c_func!(CGContextSetAllowsAntialiasing(_, _)),
    export_c_func!(CGContextSetShouldAntialias(_, _)),
    export_c_func!(CGContextSetShouldSmoothFonts(_, _)),
    export_c_func!(CGContextSetFont(_, _)),
    export_c_func!(CGContextSetFontSize(_, _)),
    export_c_func!(CGContextSetTextDrawingMode(_, _)),
    export_c_func!(CGContextSetTextMatrix(_, _)),
    export_c_func!(CGContextShowGlyphsAtPoint(_, _, _, _, _)),
    export_c_func!(CGContextShowGlyphsAtPositions(_, _, _, _)),
];

#[cfg(test)]
mod tests {
    use super::{span_is_inside, PathRaster};

    #[test]
    fn the_even_odd_rule_alternates_regardless_of_direction() {
        // Four crossings from two nested shapes wound the same way: even-odd
        // makes the inner one a hole, and the winding number does not.
        for (index, winding) in [(0, 1), (1, 2), (2, 1)] {
            let even_odd = span_is_inside(PathRaster::FillEvenOdd, index, winding);
            assert_eq!(even_odd, index.is_multiple_of(2));
            assert!(span_is_inside(PathRaster::FillNonZero, index, winding));
        }
    }

    #[test]
    fn the_non_zero_rule_is_outside_only_where_the_winding_cancels() {
        // Two shapes wound opposite ways: the overlap sums to zero and drops
        // out under the winding rule.
        assert!(!span_is_inside(PathRaster::FillNonZero, 1, 0));
        assert!(span_is_inside(PathRaster::FillNonZero, 1, -1));
    }
}
