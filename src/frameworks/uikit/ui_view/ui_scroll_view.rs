/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIScrollView`.

pub mod ui_table_view;
pub mod ui_text_view;
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::NSInteger;
use crate::frameworks::uikit::ui_geometry::UIEdgeInsets;
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_super, nil, objc_classes, todo_objc_setter,
    ClassExports, NSZonePtr, SEL,
};

type UIScrollViewIndicatorStyle = NSInteger;

pub struct UIScrollViewHostObject {
    superclass: super::UIViewHostObject,
    /// UIScrollViewDelegate, weak reference
    delegate: id,
    scroll_enabled: bool,
    content_offset: CGPoint,
    content_size: CGSize,
    /// Zoom is stored and reported back but not applied: nothing here
    /// scales the content. Apps configure the limits during setup and read
    /// them back, which is what round-tripping serves.
    minimum_zoom_scale: CGFloat,
    maximum_zoom_scale: CGFloat,
    zoom_scale: CGFloat,
    /// Scroll indicators are not drawn, so these are stored to be read back
    /// rather than acted on. They still have to exist: an app that turns an
    /// indicator off is describing a scroll view it does not want to look
    /// scrollable, and it may check later that the setting took.
    shows_horizontal_scroll_indicator: bool,
    shows_vertical_scroll_indicator: bool,
    scroll_indicator_insets: UIEdgeInsets,
    /// Whether a drag is confined to one axis once it has picked one, and
    /// whether a tap on the status bar scrolls this view to the top. Stored
    /// and reported back for the same reason as the indicators above: one
    /// constrains dragging and the other answers a gesture on a status bar
    /// tapHLE does not draw, but an app configures both during setup and may
    /// read them back.
    directional_lock_enabled: bool,
    scrolls_to_top: bool,
    /// The rest of the scroll view's drag behaviour, stored on the same terms.
    /// Nothing here drags, decelerates or pages the content, so what these
    /// buy is that setting one is not fatal and reading it back gives the
    /// answer the app set.
    paging_enabled: bool,
    bounces: bool,
    always_bounce_horizontal: bool,
    always_bounce_vertical: bool,
    delays_content_touches: bool,
    can_cancel_content_touches: bool,
    /// Whether a touch is currently down on the scroll view, and whether that
    /// touch has moved the content yet. Apps read these to tell "the user is
    /// working the view right now" from "the view is idle", and defer work
    /// accordingly, so answering has to follow the actual touches.
    tracking: bool,
    dragging: bool,
}
impl_HostObject_with_superclass!(UIScrollViewHostObject);
impl Default for UIScrollViewHostObject {
    fn default() -> Self {
        UIScrollViewHostObject {
            superclass: Default::default(),
            delegate: nil,
            scroll_enabled: true,
            content_offset: CGPoint { x: 0.0, y: 0.0 },
            content_size: CGSize {
                width: 0.0,
                height: 0.0,
            },
            minimum_zoom_scale: 1.0,
            maximum_zoom_scale: 1.0,
            zoom_scale: 1.0,
            shows_horizontal_scroll_indicator: true,
            shows_vertical_scroll_indicator: true,
            scroll_indicator_insets: Default::default(),
            directional_lock_enabled: false,
            // UIKit's default, and an app turning it off is the common case:
            // it has more than one scroll view and only one of them may claim
            // the gesture.
            scrolls_to_top: true,
            paging_enabled: false,
            bounces: true,
            always_bounce_horizontal: false,
            always_bounce_vertical: false,
            delays_content_touches: true,
            can_cancel_content_touches: true,
            tracking: false,
            dragging: false,
        }
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIScrollView: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIScrollViewHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (CGFloat)minimumZoomScale {
    env.objc.borrow::<UIScrollViewHostObject>(this).minimum_zoom_scale
}
- (())setMinimumZoomScale:(CGFloat)scale {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).minimum_zoom_scale = scale;
}
- (CGFloat)maximumZoomScale {
    env.objc.borrow::<UIScrollViewHostObject>(this).maximum_zoom_scale
}
- (())setMaximumZoomScale:(CGFloat)scale {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).maximum_zoom_scale = scale;
}
- (CGFloat)zoomScale {
    env.objc.borrow::<UIScrollViewHostObject>(this).zoom_scale
}
- (())setZoomScale:(CGFloat)scale {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).zoom_scale = scale;
}
- (())setZoomScale:(CGFloat)scale animated:(bool)_animated {
    () = msg![env; this setZoomScale:scale];
}
- (())setBouncesZoom:(bool)_bounces {
}

- (id)delegate {
    env.objc.borrow::<UIScrollViewHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).delegate = delegate;
}

- (bool)delaysContentTouches {
    env.objc.borrow::<UIScrollViewHostObject>(this).delays_content_touches
}
- (())setDelaysContentTouches:(bool)delays {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).delays_content_touches = delays;
}

- (bool)canCancelContentTouches {
    env.objc.borrow::<UIScrollViewHostObject>(this).can_cancel_content_touches
}
- (())setCanCancelContentTouches:(bool)can_cancel {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).can_cancel_content_touches = can_cancel;
}

- (bool)bounces {
    env.objc.borrow::<UIScrollViewHostObject>(this).bounces
}
- (())setBounces:(bool)bounces {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).bounces = bounces;
}

- (bool)alwaysBounceHorizontal {
    env.objc.borrow::<UIScrollViewHostObject>(this).always_bounce_horizontal
}
- (())setAlwaysBounceHorizontal:(bool)always {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).always_bounce_horizontal = always;
}

- (bool)alwaysBounceVertical {
    env.objc.borrow::<UIScrollViewHostObject>(this).always_bounce_vertical
}
- (())setAlwaysBounceVertical:(bool)always {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).always_bounce_vertical = always;
}

- (bool)isPagingEnabled {
    env.objc.borrow::<UIScrollViewHostObject>(this).paging_enabled
}
- (())setPagingEnabled:(bool)paging_enabled {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).paging_enabled = paging_enabled;
}

- (bool)isDirectionalLockEnabled {
    env.objc.borrow::<UIScrollViewHostObject>(this).directional_lock_enabled
}
- (())setDirectionalLockEnabled:(bool)enabled {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).directional_lock_enabled = enabled;
}

- (bool)scrollsToTop {
    env.objc.borrow::<UIScrollViewHostObject>(this).scrolls_to_top
}
- (())setScrollsToTop:(bool)scrolls_to_top {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).scrolls_to_top = scrolls_to_top;
}

- (bool)scrollEnabled {
    env.objc.borrow::<UIScrollViewHostObject>(this).scroll_enabled
}
- (())setScrollEnabled:(bool)scroll_enabled {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).scroll_enabled = scroll_enabled;
}

// Move the content so that `rect` is inside the visible area, which is what an
// app calls when it has just made something the current item — a selected row,
// a focused field — and wants it on screen. Both Carnivores games call it while
// building their menus, and the missing selector ended both.
//
// The scroll is not animated even when asked, as with `setContentOffset:
// animated:` above: the destination is what the caller wants and the travel was
// decoration.
- (())scrollRectToVisible:(CGRect)rect animated:(bool)_animated {
    let bounds: CGRect = msg![env; this bounds];
    let content_size: CGSize = msg![env; this contentSize];
    let mut offset: CGPoint = msg![env; this contentOffset];

    // Scroll the least distance that brings each edge inside. A rect already
    // visible moves nothing; a rect larger than the visible area is aligned to
    // its top-left corner, which is what a caller means by "show me this".
    if rect.origin.x < offset.x {
        offset.x = rect.origin.x;
    } else if rect.origin.x + rect.size.width > offset.x + bounds.size.width {
        offset.x = rect.origin.x + rect.size.width - bounds.size.width;
    }
    if rect.origin.y < offset.y {
        offset.y = rect.origin.y;
    } else if rect.origin.y + rect.size.height > offset.y + bounds.size.height {
        offset.y = rect.origin.y + rect.size.height - bounds.size.height;
    }

    // A scroll view does not scroll past its content, and an app that reads the
    // offset back afterwards is entitled to a value it could have reached by
    // dragging.
    offset.x = offset.x.clamp(0.0, (content_size.width - bounds.size.width).max(0.0));
    offset.y = offset.y.clamp(0.0, (content_size.height - bounds.size.height).max(0.0));

    () = msg![env; this setContentOffset:offset];
}

- (CGPoint)contentOffset {
    env.objc.borrow::<UIScrollViewHostObject>(this).content_offset
}
- (())setContentOffset:(CGPoint)offset
              animated:(bool)_animated {
    // Scrolling here is not animated, so this arrives where the animated
    // version would have ended up, immediately. The destination is what the
    // caller asked for; the travel was decoration.
    () = msg![env; this setContentOffset:offset];
}
- (())setContentOffset:(CGPoint)offset {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).content_offset = offset;
    // Bounds origin should be equals to the content offset
    let mut bounds: CGRect = msg![env; this bounds];
    bounds.origin = offset;
    () = msg![env; this setBounds:bounds];
    () = msg![env; this setNeedsDisplay];
}

- (CGSize)contentSize {
    env.objc.borrow::<UIScrollViewHostObject>(this).content_size
}
- (())setContentSize:(CGSize)size {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).content_size = size;
}

- (())setIndicatorStyle:(UIScrollViewIndicatorStyle)style {
    todo_objc_setter!(this, style);
}

- (bool)showsHorizontalScrollIndicator {
    env.objc.borrow::<UIScrollViewHostObject>(this).shows_horizontal_scroll_indicator
}
- (())setShowsHorizontalScrollIndicator:(bool)shows {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).shows_horizontal_scroll_indicator = shows;
}
- (bool)showsVerticalScrollIndicator {
    env.objc.borrow::<UIScrollViewHostObject>(this).shows_vertical_scroll_indicator
}
- (())setShowsVerticalScrollIndicator:(bool)shows {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).shows_vertical_scroll_indicator = shows;
}

- (UIEdgeInsets)scrollIndicatorInsets {
    env.objc.borrow::<UIScrollViewHostObject>(this).scroll_indicator_insets
}
- (())setScrollIndicatorInsets:(UIEdgeInsets)insets {
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).scroll_indicator_insets = insets;
}

- (())flashScrollIndicators {
    // Nothing is drawn to flash. The call is a hint to the user that the view
    // scrolls, never a change to what it contains, so ignoring it costs the
    // hint and nothing else.
}

- (bool)isTracking {
    env.objc.borrow::<UIScrollViewHostObject>(this).tracking
}
- (bool)isDragging {
    env.objc.borrow::<UIScrollViewHostObject>(this).dragging
}
- (bool)isDecelerating {
    // Scrolling here stops with the finger: there is no inertia to run down,
    // so the view is never in the middle of coasting to a halt.
    false
}

- (())touchesBegan:(id)touches // NSSet* of UITouch*
         withEvent:(id)event { // UIEvent*
    env.objc.borrow_mut::<UIScrollViewHostObject>(this).tracking = true;
    // Forwarded so a scroll view stays an ordinary responder: something
    // further up the chain may be the thing that actually handles the touch.
    msg_super![env; this touchesBegan:touches withEvent:event]
}

- (())touchesEnded:(id)touches // NSSet* of UITouch*
         withEvent:(id)event { // UIEvent*
    let host_obj = env.objc.borrow_mut::<UIScrollViewHostObject>(this);
    host_obj.tracking = false;
    host_obj.dragging = false;
    msg_super![env; this touchesEnded:touches withEvent:event]
}

- (())touchesCancelled:(id)touches // NSSet* of UITouch*
             withEvent:(id)event { // UIEvent*
    let host_obj = env.objc.borrow_mut::<UIScrollViewHostObject>(this);
    host_obj.tracking = false;
    host_obj.dragging = false;
    msg_super![env; this touchesCancelled:touches withEvent:event]
}

- (())touchesMoved:(id)touches // NSSet* of UITouch*
         withEvent:(id)_event { // UIEvent*
    let scroll_enabled: bool = msg![env; this scrollEnabled];
    if !scroll_enabled {
        return;
    }

    let touch_arr: id = msg![env; touches allObjects];
    // Assume single finger touches for now
    let touch: id = msg![env; touch_arr objectAtIndex:0u32];
    let bounds: CGRect = msg![env; this bounds];

    let prev_location: CGPoint = msg![env; touch previousLocationInView:this];
    let prev_x = prev_location.x;
    let prev_y = prev_location.y;

    let new_location: CGPoint = msg![env; touch locationInView:this];
    let y = new_location.y;
    let x = new_location.x;

    let delta_y = y - prev_y;
    let delta_x = x - prev_x;

    let offset: CGPoint = msg![env; this contentOffset];
    let content_size: CGSize = msg![env; this contentSize];

    // Very rudimentary scrolling.
    // We emulate sliding up to scroll down like on the real iPhone.
    let mut new_content_offset: CGPoint = CGPoint { x: offset.x - delta_x, y: offset.y - delta_y };

    // Update content offset within bounds
    new_content_offset.y = new_content_offset.y.min(content_size.height - bounds.size.height).max(0.0);
    new_content_offset.x = new_content_offset.x.min(content_size.width - bounds.size.width).max(0.0);

    // Trigger rerender only if required.
    log_dbg!("content offset: old {:?}, new {:?}", offset, new_content_offset);
    if new_content_offset != offset {
        env.objc.borrow_mut::<UIScrollViewHostObject>(this).dragging = true;
        () = msg![env; this setContentOffset:new_content_offset];

        let delegate: id = msg![env; this delegate];
        let sel: SEL = env
            .objc
            .register_host_selector("scrollViewDidScroll:".to_string(), &mut env.mem);
        let responds: bool = msg![env; delegate respondsToSelector:sel];
        if responds {
            () = msg![env; delegate scrollViewDidScroll:this];
        }
    }
}

@end

};
