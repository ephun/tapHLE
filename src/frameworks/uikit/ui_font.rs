/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIFont`.

use super::ui_graphics::UIGraphicsGetCurrentContext;
use crate::font::{Font, TextAlignment, WrapMode};
use crate::frameworks::core_graphics::cg_bitmap_context::CGBitmapContextDrawer;
use crate::frameworks::core_graphics::cg_context::{
    CGContextGetCTM, CGContextRef, CGContextRestoreGState, CGContextSaveGState, CGContextScaleCTM,
    CGContextTranslateCTM,
};
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::{from_rust_string, get_static_str, to_rust_string};
use crate::frameworks::foundation::NSInteger;
use crate::objc::{
    autorelease, id, msg, msg_class, objc_classes, release, retain, Class, ClassExports, HostObject,
};
use crate::Environment;
use std::collections::HashMap;
use std::ops::Range;

#[derive(Default)]
pub(super) struct State {
    fonts: HashMap<FontKind, Font>,
    sans_regular_ja: Option<Font>,
    sans_bold_ja: Option<Font>,
}
impl State {
    fn get_font_by_kind(&mut self, font_kind: FontKind) -> &Font {
        self.fonts
            .entry(font_kind)
            .or_insert_with(|| match font_kind {
                FontKind::MonoRegular => Font::mono_regular(),
                FontKind::MonoBold => Font::mono_bold(),
                FontKind::MonoBoldItalic => Font::mono_bold_italic(),
                FontKind::MonoItalic => Font::mono_italic(),
                FontKind::SansRegular => Font::sans_regular(),
                FontKind::SansBold => Font::sans_bold(),
                FontKind::SansBoldItalic => Font::sans_bold_italic(),
                FontKind::SansItalic => Font::sans_italic(),
                FontKind::SerifRegular => Font::serif_regular(),
                FontKind::SerifBold => Font::serif_bold(),
                FontKind::SerifBoldItalic => Font::serif_bold_italic(),
                FontKind::SerifItalic => Font::serif_italic(),
            })
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
enum FontKind {
    MonoRegular,
    MonoBold,
    MonoBoldItalic,
    MonoItalic,
    SansRegular,
    SansBold,
    SansBoldItalic,
    SansItalic,
    SerifRegular,
    SerifBold,
    SerifBoldItalic,
    SerifItalic,
}

struct UIFontHostObject {
    size: CGFloat,
    kind: FontKind,
    /// PostScript name reported by `-fontName`. For fonts created by name this
    /// is the requested name (even when we substitute a bundled font); for the
    /// system-font factories it is the corresponding Helvetica variant, as on
    /// early iPhone OS.
    name: String,
}
impl HostObject for UIFontHostObject {}

/// Line break mode.
///
/// This is put here for convenience since it's font-related.
/// Apple puts it in its own header, also in UIKit.
pub type UILineBreakMode = NSInteger;
pub const UILineBreakModeWordWrap: UILineBreakMode = 0;
pub const UILineBreakModeCharacterWrap: UILineBreakMode = 1;
#[allow(dead_code)]
pub const UILineBreakModeClip: UILineBreakMode = 2;
#[allow(dead_code)]
pub const UILineBreakModeHeadTruncation: UILineBreakMode = 3;
pub const UILineBreakModeTailTruncation: UILineBreakMode = 4;
#[allow(dead_code)]
pub const UILineBreakModeMiddleTruncation: UILineBreakMode = 5;

/// Text alignment.
///
/// This is put here for convenience since it's font-related.
/// Apple puts it in its own header, also in UIKit.
pub type UITextAlignment = NSInteger;
pub const UITextAlignmentLeft: UITextAlignment = 0;
pub const UITextAlignmentCenter: UITextAlignment = 1;
pub const UITextAlignmentRight: UITextAlignment = 2;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIFont: NSObject

// Values are checked against iPhone 3GS, iOS 4.0.1
+ (CGFloat)labelFontSize {
    17.0
}
+ (CGFloat)buttonFontSize {
    18.0
}
+ (CGFloat)smallSystemFontSize {
    12.0
}
+ (CGFloat)systemFontSize {
    14.0
}

+ (id)systemFontOfSize:(CGFloat)size {
    let host_object = UIFontHostObject {
        size,
        kind: FontKind::SansRegular,
        name: "Helvetica".to_string(),
    };
    let new = env.objc.alloc_object(this, Box::new(host_object), &mut env.mem);
    autorelease(env, new)
}
+ (id)boldSystemFontOfSize:(CGFloat)size {
    let host_object = UIFontHostObject {
        size,
        kind: FontKind::SansBold,
        name: "Helvetica-Bold".to_string(),
    };
    let new = env.objc.alloc_object(this, Box::new(host_object), &mut env.mem);
    autorelease(env, new)
}
+ (id)italicSystemFontOfSize:(CGFloat)size {
    let host_object = UIFontHostObject {
        size,
        kind: FontKind::SansItalic,
        name: "Helvetica-Oblique".to_string(),
    };
    let new = env.objc.alloc_object(this, Box::new(host_object), &mut env.mem);
    autorelease(env, new)
}
+ (id)fontWithName:(id)fontName // NSString*
            size:(CGFloat)fontSize {
    let font_name = to_rust_string(env, fontName).to_string();
    let host_object = UIFontHostObject {
        kind: get_equivalent_font(&font_name).unwrap_or_else(|| {
            log!("No replacement found for font {}. Using system font instead.", font_name);
            FontKind::SansRegular
        }),
        size: fontSize,
        name: font_name,
    };
    let new = env.objc.alloc_object(this, Box::new(host_object), &mut env.mem);
    autorelease(env, new)
}

// NSCoding implementation used by Interface Builder archives.
- (id)initWithCoder:(id)coder {
    let name_key = get_static_str(env, "UIFontName");
    let size_key = get_static_str(env, "UIFontPointSize");
    let font_name: id = msg![env; coder decodeObjectForKey:name_key];
    let font_size: CGFloat = msg![env; coder decodeFloatForKey:size_key];

    // UIFont is a class cluster here: replace the placeholder allocated by
    // NSObject with the concrete font produced by the normal factory path.
    release(env, this);
    let font: id = msg_class![env; UIFont fontWithName:font_name size:font_size];
    retain(env, font)
}

// The same typeface at a different size. Resolving through the name keeps the
// font-substitution table as the single place that maps a name to a FontKind.
- (id)fontWithSize:(CGFloat)fontSize {
    let host_object = env.objc.borrow::<UIFontHostObject>(this);
    if host_object.size == fontSize {
        return this;
    }
    let host_object = UIFontHostObject {
        kind: host_object.kind,
        size: fontSize,
        name: host_object.name.clone(),
    };
    let class: Class = msg![env; this class];
    let new = env.objc.alloc_object(class, Box::new(host_object), &mut env.mem);
    autorelease(env, new)
}

- (CGFloat)pointSize {
    env.objc.borrow::<UIFontHostObject>(this).size
}
- (id)fontName {
    let name = env.objc.borrow::<UIFontHostObject>(this).name.clone();
    let string = from_rust_string(env, name);
    autorelease(env, string)
}
- (id)familyName {
    // The family name drops the PostScript style suffix (e.g. "Helvetica-Bold"
    // -> "Helvetica"). This is the common shape apps read; it is not a full
    // PostScript-name parser.
    let name = env.objc.borrow::<UIFontHostObject>(this).name.clone();
    let family = name.split('-').next().unwrap_or(&name).to_string();
    let string = from_rust_string(env, family);
    autorelease(env, string)
}

- (CGFloat)ascender {
    let host_object = env.objc.borrow::<UIFontHostObject>(this);
    let font = env.framework_state.uikit.ui_font.get_font_by_kind(host_object.kind);
    font.ascent(host_object.size)
}
- (CGFloat)descender {
    let host_object = env.objc.borrow::<UIFontHostObject>(this);
    let font = env.framework_state.uikit.ui_font.get_font_by_kind(host_object.kind);
    font.descent(host_object.size)
}
- (CGFloat)leading {
    let host_object = env.objc.borrow::<UIFontHostObject>(this);
    let font = env.framework_state.uikit.ui_font.get_font_by_kind(host_object.kind);
    font.line_gap(host_object.size)
}

- (CGFloat)lineHeight {
    // This is calculated based on the documentation:
    // https://developer.apple.com/library/archive/documentation/TextFonts/Conceptual/CocoaTextArchitecture/FontHandling/FontHandling.html
    let ascender: CGFloat = msg![env; this ascender];
    let descender: CGFloat = msg![env; this descender];
    let leading: CGFloat = msg![env; this leading];
    assert!(descender <= 0.0);
    ascender + leading - descender
}

@end

};

fn convert_line_break_mode(ui_mode: UILineBreakMode) -> WrapMode {
    match ui_mode {
        UILineBreakModeWordWrap => WrapMode::Word,
        UILineBreakModeCharacterWrap => WrapMode::Char,
        // Clipping and the three truncation modes all mean "do not wrap": the
        // text stays on one line and the part that does not fit is cut or
        // replaced with an ellipsis. tapHLE draws neither, and it has only the
        // two wrapping modes to choose between, so they are approximated by
        // word wrapping — the text that would have been cut appears on a
        // following line instead of vanishing. That is wrong, but it is wrong
        // in a way that shows the app's text; refusing the mode ended the app,
        // which is worse, and one of these was already faked for exactly that
        // reason because it is UILabel's default.
        UILineBreakModeClip
        | UILineBreakModeHeadTruncation
        | UILineBreakModeTailTruncation
        | UILineBreakModeMiddleTruncation => WrapMode::Word,
        _ => {
            log!(
                "Warning: unknown line break mode {}, wrapping on words",
                ui_mode
            );
            WrapMode::Word
        }
    }
}

#[rustfmt::skip]
fn get_font<'a>(state: &'a mut State, kind: FontKind, text: &str) -> &'a Font {
    // The default fonts (see font.rs) are the Liberation family, which are a
    // good substitute for Helvetica, the iPhone OS system font. Unfortunately,
    // there is no CJK support in these fonts. To support Super Monkey Ball in
    // Japanese, let's fall back to Noto Sans JP when necessary.
    // FIXME: This heuristic is incomplete and a proper font fallback system
    // should be used instead.
    for c in text.chars() {
        let c = c as u32;
        if (0x3000..=0x30FF).contains(&c) || // JA punctuation, kana
           (0xFF00..=0xFFEF).contains(&c) || // full-width/half-width chars
           (0x4e00..=0x9FA0).contains(&c) || // various kanji
           (0x3400..=0x4DBF).contains(&c) { // more kanji
            match kind {
                // CJK has no italic equivalent
                FontKind::MonoRegular | FontKind::MonoItalic | FontKind::SansRegular | FontKind::SansItalic | FontKind::SerifRegular | FontKind::SerifItalic => {
                    if state.sans_regular_ja.is_none() {
                        state.sans_regular_ja = Some(Font::sans_regular_ja());
                    }
                    return state.sans_regular_ja.as_ref().unwrap();
                },
                FontKind::MonoBold | FontKind::MonoBoldItalic | FontKind::SansBold | FontKind::SansBoldItalic | FontKind::SerifBold | FontKind::SerifBoldItalic => {
                    if state.sans_bold_ja.is_none() {
                        state.sans_bold_ja = Some(Font::sans_bold_ja());
                    }
                    return state.sans_bold_ja.as_ref().unwrap();
                },
            }
        }
    }

    state.get_font_by_kind(kind)
}

/// Called by the `sizeWithFont:` method family on `NSString`.
pub fn size_with_font(
    env: &mut Environment,
    font: id,
    text: &str,
    constrained: Option<(CGSize, UILineBreakMode)>,
) -> CGSize {
    let host_object = env.objc.borrow::<UIFontHostObject>(font);

    let font = get_font(
        &mut env.framework_state.uikit.ui_font,
        host_object.kind,
        text,
    );

    let wrap = constrained.map(|(size, ui_mode)| (size.width, convert_line_break_mode(ui_mode)));

    let (width, height) = font.calculate_text_size(host_object.size, text, wrap);

    CGSize { width, height }
}

/// Determine how the text lines will be rendered given a constraint
pub fn break_lines_with_font<'a>(
    env: &mut Environment,
    font: id,
    text: &'a str,
    constrained: Option<(CGSize, UILineBreakMode)>,
) -> Vec<(f32, &'a str)> {
    let host_object = env.objc.borrow::<UIFontHostObject>(font);

    let font = get_font(
        &mut env.framework_state.uikit.ui_font,
        host_object.kind,
        text,
    );

    let wrap = constrained.map(|(size, ui_mode)| (size.width, convert_line_break_mode(ui_mode)));

    font.break_lines(host_object.size, text, wrap)
}

#[inline(always)]
pub fn draw_font_glyph(
    drawer: &mut CGBitmapContextDrawer,
    raster_glyph: crate::font::RasterGlyph,
    fill_color: (f32, f32, f32, f32),
    clip_x: Option<Range<f32>>,
    clip_y: Option<Range<f32>>,
) {
    let mut glyph_rect = {
        let (x, y) = raster_glyph.origin();
        let (width, height) = raster_glyph.dimensions();
        CGRect {
            origin: CGPoint { x, y },
            size: CGSize {
                width: width as f32,
                height: height as f32,
            },
        }
    };
    // The code in font.rs won't and can't clip glyphs hanging over the right
    // and bottom sides of the rect, so it has to be done here. Bear in mind
    // that this must not incorrectly affect the texture co-ordinates, otherwise
    // the glyphs become squashed instead.
    // Note that there isn't clipping for the other sides currently because it
    // doesn't seem to be needed.
    if let Some(clip_x) = clip_x {
        if glyph_rect.origin.x >= clip_x.end {
            return;
        }
        if glyph_rect.origin.x + glyph_rect.size.width > clip_x.end {
            glyph_rect.size.width = clip_x.end - glyph_rect.origin.x;
        }
    }
    if let Some(clip_y) = clip_y {
        if glyph_rect.origin.y >= clip_y.end {
            return;
        }
        if glyph_rect.origin.y + glyph_rect.size.height > clip_y.end {
            glyph_rect.size.height = clip_y.end - glyph_rect.origin.y;
        }
    }

    for ((x, y), (tex_x, tex_y)) in drawer.iter_transformed_pixels(glyph_rect) {
        // TODO: bilinear sampling
        let coverage = raster_glyph.pixel_at((
            (tex_x * glyph_rect.size.width - 0.5).round() as i32,
            (tex_y * glyph_rect.size.height - 0.5).round() as i32,
        ));
        let (r, g, b, a) = fill_color;
        let (r, g, b, a) = (r * coverage, g * coverage, b * coverage, a * coverage);
        drawer.put_pixel((x, y), (r, g, b, a), /* blend: */ true);
    }
}

/// Flip the context about the horizontal band the text is about to occupy,
/// when that is what makes the text land upright. Returns whether anything was
/// changed, so the caller knows whether to restore.
///
/// Text layout here runs y-downward: lines are placed one below the next and a
/// glyph's bitmap rows run top to bottom. Two separate things can turn that
/// over on its way to the screen, and whether the text ends up upright depends
/// on **both**:
///
/// - **The transform.** UIKit lays out downward and Core Graphics upward, so
///   `UIView` installs a y-axis flip around every `-drawRect:` call.
/// - **The destination.** The compositor draws a layer's backing bitmap with
///   its vertical texture coordinate inverted, so everything in it is turned
///   over once more on its way to a texture. A bitmap an app made for itself
///   is not touched.
///
/// Two flips cancel. So the text needs flipping back exactly when those two
/// **agree** — both on, or both off — and must be left alone when they differ.
///
/// Deciding this from the transform alone is what produced the two remaining
/// faults. An app that draws a string into its own bitmap with no flip in
/// force got no correction and came out mirrored; an app that flips its own
/// bitmap context first — the correct way to draw UIKit text into one — got a
/// correction it did not need. Both are the same mistake: the transform is
/// only half of the question.
///
/// Doing it about the band, rather than per glyph, is what keeps line order
/// right: mirroring each glyph on its own fixes the letters and leaves the
/// lines of a paragraph stacked upwards.
fn counter_flip_for_text(
    env: &mut Environment,
    context: CGContextRef,
    band_origin_y: CGFloat,
    band_height: CGFloat,
) -> bool {
    let transform_flipped = CGContextGetCTM(env, context).d < 0.0;
    let destination_flipped = {
        let drawer = CGBitmapContextDrawer::new(&env.objc, &mut env.mem, context);
        drawer.flipped_on_presentation()
    };
    if transform_flipped != destination_flipped {
        return false;
    }
    CGContextSaveGState(env, context);
    CGContextTranslateCTM(env, context, 0.0, band_origin_y * 2.0 + band_height);
    CGContextScaleCTM(env, context, 1.0, -1.0);
    true
}

/// Called by the `drawAtPoint:` method family on `NSString`.
pub fn draw_at_point(
    env: &mut Environment,
    font: id,
    text: &str,
    point: CGPoint,
    width_and_line_break_mode: Option<(CGFloat, UILineBreakMode)>,
) -> CGSize {
    let context = UIGraphicsGetCurrentContext(env);

    let font_id = font;
    let &UIFontHostObject {
        size: font_size,
        kind,
        ..
    } = env.objc.borrow::<UIFontHostObject>(font_id);

    let width_and_line_break_mode =
        width_and_line_break_mode.map(|(width, ui_mode)| (width, convert_line_break_mode(ui_mode)));
    let clip_x = width_and_line_break_mode.map(|(width, _)| point.x..(point.x + width));
    let (width, height) = {
        let font = get_font(&mut env.framework_state.uikit.ui_font, kind, text);
        font.calculate_text_size(font_size, text, width_and_line_break_mode)
    };

    // The context is flipped back for the duration of the drawing; see
    // [counter_flip_for_text].
    let flipped = counter_flip_for_text(env, context, point.y, height);

    let font = get_font(&mut env.framework_state.uikit.ui_font, kind, text);
    let mut drawer = CGBitmapContextDrawer::new(&env.objc, &mut env.mem, context);
    let fill_color = drawer.rgb_fill_color();

    font.draw(
        font_size,
        text,
        (point.x, point.y),
        width_and_line_break_mode,
        TextAlignment::Left,
        |raster_glyph| {
            draw_font_glyph(
                &mut drawer,
                raster_glyph,
                fill_color,
                clip_x.clone(),
                /* clip_y: */ None,
            )
        },
    );

    if flipped {
        CGContextRestoreGState(env, context);
    }

    CGSize { width, height }
}

/// Called by the `drawInRect:` method family on `NSString`.
pub fn draw_in_rect(
    env: &mut Environment,
    font: id,
    text: &str,
    rect: CGRect,
    line_break_mode: UILineBreakMode,
    alignment: UITextAlignment,
) -> CGSize {
    let context = UIGraphicsGetCurrentContext(env);

    let text_size = size_with_font(env, font, text, Some((rect.size, line_break_mode)));

    let &UIFontHostObject {
        size: font_size,
        kind,
        ..
    } = env.objc.borrow::<UIFontHostObject>(font);

    // The context is flipped back for the duration of the drawing; see
    // [counter_flip_for_text]. The band is the rect the caller asked for, so
    // clipping below still refers to the same place.
    let flipped = counter_flip_for_text(env, context, rect.origin.y, rect.size.height);

    let font = get_font(&mut env.framework_state.uikit.ui_font, kind, text);

    let mut drawer = CGBitmapContextDrawer::new(&env.objc, &mut env.mem, context);
    let fill_color = drawer.rgb_fill_color();

    let (origin_x_offset, alignment) = match alignment {
        UITextAlignmentLeft => (0.0, TextAlignment::Left),
        UITextAlignmentCenter => (rect.size.width / 2.0, TextAlignment::Center),
        UITextAlignmentRight => (rect.size.width, TextAlignment::Right),
        _ => unimplemented!(),
    };

    font.draw(
        font_size,
        text,
        (rect.origin.x + origin_x_offset, rect.origin.y),
        Some((rect.size.width, convert_line_break_mode(line_break_mode))),
        alignment,
        |raster_glyph| {
            draw_font_glyph(
                &mut drawer,
                raster_glyph,
                fill_color,
                /* clip_x: */ Some(rect.origin.x..(rect.origin.x + rect.size.width)),
                /* clip_y: */ Some(rect.origin.y..(rect.origin.y + rect.size.height)),
            )
        },
    );

    if flipped {
        CGContextRestoreGState(env, context);
    }

    text_size
}

fn get_equivalent_font(system_font: &str) -> Option<FontKind> {
    // Maps every font found in every font family in an iOS 2 Simulator
    match system_font {
        // Font Family: Courier
        "Courier" => None,
        "Courier-BoldOblique" => None,
        "Courier-Oblique" => None,
        "Courier-Bold" => None,
        // Font Family: AppleGothic
        "AppleGothic" => None,
        // Font Family: Arial
        "ArialMT" => Some(FontKind::SansRegular),
        "Arial-BoldMT" => Some(FontKind::SansBold),
        "Arial-BoldItalicMT" => Some(FontKind::SansBoldItalic),
        "Arial-ItalicMT" => Some(FontKind::SansItalic),
        // Font Family: STHeiti TC
        "STHeitiTC-Light" => None,
        "STHeitiTC-Medium" => None,
        // Font Family: Hiragino Kaku Gothic ProN
        "HiraKakuProN-W6" => None,
        "HiraKakuProN-W3" => None,
        // Font Family: Courier New
        "CourierNewPS-BoldMT" => Some(FontKind::MonoRegular),
        "CourierNewPS-ItalicMT" => Some(FontKind::MonoBold),
        "CourierNewPS-BoldItalicMT" => Some(FontKind::MonoBoldItalic),
        "CourierNewPSMT" => Some(FontKind::MonoItalic),
        // Font Family: Zapfino
        "Zapfino" => None,
        // Font Family: Arial Unicode MS
        "ArialUnicodeMS" => None,
        // Font Family: STHeiti SC
        "STHeitiSC-Medium" => None,
        "STHeitiSC-Light" => None,
        // Font Family: American Typewriter
        "AmericanTypewriter" => Some(FontKind::MonoRegular),
        "AmericanTypewriter-Bold" => Some(FontKind::MonoBold),
        // Font Family: Helvetica
        "Helvetica-Oblique" => None,
        "Helvetica-BoldOblique" => None,
        "Helvetica" => None,
        "Helvetica-Bold" => None,
        // Font Family: Marker Felt
        "MarkerFelt-Thin" => None,
        // Font Family: Helvetica Neue
        "HelveticaNeue" => Some(FontKind::SansRegular),
        "HelveticaNeue-Bold" => Some(FontKind::SansBold),
        // Font Family: DB LCD Temp
        "DBLCDTempBlack" => None,
        // Font Family: Verdana
        "Verdana-Bold" => None,
        "Verdana-BoldItalic" => None,
        "Verdana" => None,
        "Verdana-Italic" => None,
        // Font Family: Times New Roman
        "TimesNewRomanPSMT" => Some(FontKind::SerifRegular),
        "TimesNewRomanPS-BoldMT" => Some(FontKind::SerifBold),
        "TimesNewRomanPS-BoldItalicMT" => Some(FontKind::SerifBoldItalic),
        "TimesNewRomanPS-ItalicMT" => Some(FontKind::SerifItalic),
        // Font Family: Georgia
        "Georgia-Bold" => None,
        "Georgia" => None,
        "Georgia-BoldItalic" => None,
        "Georgia-Italic" => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helvetica_neue_uses_matching_sans_weight() {
        assert!(matches!(
            get_equivalent_font("HelveticaNeue"),
            Some(FontKind::SansRegular)
        ));
        assert!(matches!(
            get_equivalent_font("HelveticaNeue-Bold"),
            Some(FontKind::SansBold)
        ));
    }
}
