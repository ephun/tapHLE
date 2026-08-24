/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UINavigationController`.

use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::frameworks::foundation::{ns_array, NSInteger, NSUInteger};
use crate::frameworks::uikit::ui_application::UIInterfaceOrientation;
use crate::objc::{
    autorelease, id, impl_HostObject_with_superclass, msg, msg_send, msg_super, nil, objc_classes,
    release, retain, ClassExports, NSZonePtr, SEL,
};
use crate::Environment;

// TODO: navigation bar and toolbar
// TODO: animations

#[derive(Default)]
struct UINavigationItemHostObject {
    /// `NSString*`, retained.
    title: id,
    /// `UIBarButtonItem*`, retained.
    left_bar_button_item: id,
    /// `UIBarButtonItem*`, retained.
    right_bar_button_item: id,
    /// `UIBarButtonItem*`, retained.
    back_bar_button_item: id,
    /// `UIView*`, retained.
    title_view: id,
}
impl crate::objc::HostObject for UINavigationItemHostObject {}

/// What a navigation bar keeps. Nothing draws it; the stack exists so that an
/// app which pushes an item can read back what it pushed.
#[derive(Default)]
struct UINavigationBarHostObject {
    superclass: crate::frameworks::uikit::ui_view::UIViewHostObject,
    /// `UINavigationItem*`, retained, oldest first.
    items: Vec<id>,
}
impl_HostObject_with_superclass!(UINavigationBarHostObject);

#[derive(Default)]
struct UIToolbarHostObject {
    superclass: crate::frameworks::uikit::ui_view::UIViewHostObject,
    /// `NSArray*` of `UIBarButtonItem*`, retained.
    items: id,
    /// `UIBarStyle`.
    bar_style: NSInteger,
    /// `UIColor*`, retained.
    tint_color: id,
}
impl_HostObject_with_superclass!(UIToolbarHostObject);

struct UIBarButtonItemHostObject {
    /// `NSString*`, retained.
    title: id,
    /// `UIImage*`, retained.
    image: id,
    /// `UIView*`, retained.
    custom_view: id,
    /// Weak, as a control's target is: the target usually owns the item.
    target: id,
    action: Option<SEL>,
    enabled: bool,
    system_item: NSInteger,
}
impl crate::objc::HostObject for UIBarButtonItemHostObject {}
impl Default for UIBarButtonItemHostObject {
    fn default() -> Self {
        UIBarButtonItemHostObject {
            title: nil,
            image: nil,
            custom_view: nil,
            target: nil,
            action: None,
            // UIKit's default.
            enabled: true,
            system_item: 0,
        }
    }
}

#[derive(Default)]
struct UINavigationControllerHostObject {
    superclass: super::UIViewControllerHostObject,
    /// something implementing UINavigationControllerDelegate
    delegate: id,
    /// Navigation stack of view controllers, non-retaining
    /// (we explicitly retain/release on push/pop messages)
    navigation_stack: Vec<id>,
    /// Navigation bar restored from a NIB, retained.
    navigation_bar: id,
    navigation_bar_hidden: bool,
}
impl_HostObject_with_superclass!(UINavigationControllerHostObject);

/// Where a toolbar's items sit, in the toolbar's own coordinates.
///
/// tapHLE does not draw the items, so this exists only to decide which one a
/// touch landed on. It follows UIKit's arrangement: items are laid out left to
/// right, a flexible space takes an equal share of whatever is left over, and a
/// fixed space takes its own width. Widths for ordinary items are estimated
/// from their titles, which is the part that cannot be exact — an item's real
/// width depends on the font the bar draws it in.
fn toolbar_item_frames(env: &mut Environment, toolbar: id) -> Vec<(id, CGRect)> {
    let items: id = env.objc.borrow::<UIToolbarHostObject>(toolbar).items;
    if items == nil {
        return Vec::new();
    }
    let count: NSUInteger = msg![env; items count];
    let bounds: CGRect = msg![env; toolbar bounds];

    /// `UIBarButtonSystemItemFlexibleSpace` and `...FixedSpace`.
    const FLEXIBLE_SPACE: NSInteger = 5;
    const FIXED_SPACE: NSInteger = 6;
    /// A toolbar button is at least this wide on a device, and the gap either
    /// side of the row is about this much again.
    const MINIMUM_WIDTH: CGFloat = 44.0;
    const EDGE_INSET: CGFloat = 8.0;

    let mut widths: Vec<(id, CGFloat, bool)> = Vec::new();
    for index in 0..count {
        let item: id = msg![env; items objectAtIndex:index];
        let &UIBarButtonItemHostObject {
            title,
            custom_view,
            system_item,
            ..
        } = env.objc.borrow(item);

        if system_item == FLEXIBLE_SPACE {
            widths.push((item, 0.0, true));
            continue;
        }
        if system_item == FIXED_SPACE {
            widths.push((item, MINIMUM_WIDTH / 2.0, false));
            continue;
        }
        if custom_view != nil {
            let frame: CGRect = msg![env; custom_view frame];
            widths.push((item, frame.size.width.max(MINIMUM_WIDTH), false));
            continue;
        }
        // About ten points per character plus the button's own padding. This is
        // a guess, and it is why an item's frame here is only good enough to
        // decide which button a finger is nearest.
        let characters = if title == nil {
            0.0
        } else {
            let length: NSUInteger = msg![env; title length];
            length as CGFloat
        };
        widths.push((item, (characters * 10.0 + 24.0).max(MINIMUM_WIDTH), false));
    }

    let fixed_total: CGFloat = widths
        .iter()
        .filter(|(_, _, flexible)| !flexible)
        .map(|(_, width, _)| *width)
        .sum();
    let flexible_count = widths.iter().filter(|(_, _, flexible)| *flexible).count();
    let spare = (bounds.size.width - EDGE_INSET * 2.0 - fixed_total).max(0.0);
    let per_flexible = if flexible_count > 0 {
        spare / flexible_count as CGFloat
    } else {
        0.0
    };

    let mut frames = Vec::new();
    let mut x = bounds.origin.x + EDGE_INSET;
    for (item, width, flexible) in widths {
        let width = if flexible { per_flexible } else { width };
        if !flexible {
            frames.push((
                item,
                CGRect {
                    origin: CGPoint {
                        x,
                        y: bounds.origin.y,
                    },
                    size: CGSize {
                        width,
                        height: bounds.size.height,
                    },
                },
            ));
        }
        x += width;
    }
    frames
}

/// The item a point falls on, if any.
fn toolbar_item_at_point(env: &mut Environment, toolbar: id, point: CGPoint) -> Option<id> {
    for (item, frame) in toolbar_item_frames(env, toolbar) {
        if point.x >= frame.origin.x
            && point.x < frame.origin.x + frame.size.width
            && point.y >= frame.origin.y
            && point.y < frame.origin.y + frame.size.height
        {
            return Some(item);
        }
    }
    None
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UINavigationController: UIViewController

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UINavigationControllerHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];

    // Early UIKit archives may contain both keys with equivalent controller
    // arrays. Decode one authoritative representation so child -> parent
    // back-references do not recursively instantiate this controller.
    let view_controllers_key = get_static_str(env, "UIViewControllers");
    let child_view_controllers_key = get_static_str(env, "UIChildViewControllers");
    let controllers: id = if msg![env; coder containsValueForKey:view_controllers_key] {
        msg![env; coder decodeObjectForKey:view_controllers_key]
    } else {
        msg![env; coder decodeObjectForKey:child_view_controllers_key]
    };

    let mut navigation_stack = Vec::new();
    if controllers != nil {
        let count: NSUInteger = msg![env; controllers count];
        navigation_stack.reserve(count as usize);
        for i in 0..count {
            let controller: id = msg![env; controllers objectAtIndex:i];
            retain(env, controller);
            super::set_parent_view_controller(env, controller, this);
            navigation_stack.push(controller);
        }
    }

    let navigation_bar_key = get_static_str(env, "UINavigationBar");
    let navigation_bar: id = msg![env; coder decodeObjectForKey:navigation_bar_key];
    retain(env, navigation_bar);

    let navigation_bar_hidden_key = get_static_str(env, "UINavigationBarHidden");
    let navigation_bar_hidden: bool =
        msg![env; coder decodeBoolForKey:navigation_bar_hidden_key];
    if navigation_bar != nil {
        () = msg![env; navigation_bar setHidden:navigation_bar_hidden];
    }

    let host_object = env.objc.borrow_mut::<UINavigationControllerHostObject>(this);
    assert!(host_object.navigation_stack.is_empty());
    assert!(host_object.navigation_bar == nil);
    host_object.navigation_stack = navigation_stack;
    host_object.navigation_bar = navigation_bar;
    host_object.navigation_bar_hidden = navigation_bar_hidden;

    this
}

- (())dealloc {
    let (navigation_stack, navigation_bar) = {
        let host_object = env.objc.borrow_mut::<UINavigationControllerHostObject>(this);
        (
            std::mem::take(&mut host_object.navigation_stack),
            std::mem::replace(&mut host_object.navigation_bar, nil),
        )
    };

    for controller in navigation_stack {
        let parent = env
            .objc
            .borrow::<super::UIViewControllerHostObject>(controller)
            .parent_view_controller;
        if parent == this {
            super::set_parent_view_controller(env, controller, nil);
        }
        release(env, controller);
    }
    release(env, navigation_bar);

    msg_super![env; this dealloc]
}

- (id)initWithRootViewController:(id)root_vc { // UIViewController *
    () = msg![env; this pushViewController:root_vc animated:false];
    this
}

- (())loadView {
    // Restore only the model during initWithCoder:. Loading the top child here
    // keeps view creation and appearance callbacks out of recursive NIB
    // decoding while making the archived hierarchy visible on first use.
    () = msg_super![env; this loadView];

    let (self_view, top_view_controller) = {
        let host_object = env.objc.borrow::<UINavigationControllerHostObject>(this);
        (
            host_object.superclass.view,
            host_object.navigation_stack.last().copied(),
        )
    };
    if let Some(top_view_controller) = top_view_controller {
        let view: id = msg![env; top_view_controller view];
        () = msg![env; top_view_controller viewWillAppear:false];
        () = msg![env; self_view addSubview:view];
        () = msg![env; top_view_controller viewDidAppear:false];
    }
}

// weak/non-retaining
- (())setDelegate:(id)delegate { // something implementing UINavigationControllerDelegate
    log_dbg!("[(UINavigationController*){:?} setDelegate:{:?}]", this, delegate);
    let host_object = env.objc.borrow_mut::<UINavigationControllerHostObject>(this);
    host_object.delegate = delegate;
}
- (id)delegate {
    env.objc.borrow::<UINavigationControllerHostObject>(this).delegate
}

- (())pushViewController:(id)view_controller // UIViewController *
                animated:(bool)_animated {
    // Load the container before changing the stack. For an ordinary
    // initWithRootViewController: this prevents loadView from mistaking the
    // newly pushed controller for an archived controller that still needs to
    // be attached.
    let self_view: id = msg![env; this view];

    let stack = &mut env.objc.borrow_mut::<UINavigationControllerHostObject>(this).navigation_stack;
    assert!(!stack.contains(&view_controller));
    stack.push(view_controller);
    retain(env, view_controller);
    super::set_parent_view_controller(env, view_controller, this);

    let delegate = env.objc.borrow::<UINavigationControllerHostObject>(this).delegate;
    let sel: SEL = env
        .objc
        .register_host_selector(
            "navigationController:willShowViewController:animated:".to_string(),
            &mut env.mem
        );
    let responds: bool = msg![env; delegate respondsToSelector:sel];
    if responds {
        () = msg![env; delegate navigationController:this willShowViewController:view_controller animated:false];
    }
    let vc_view: id = msg![env; view_controller view];
    // TODO: animations
    () = msg![env; view_controller viewWillAppear:false];
    () = msg![env; self_view addSubview:vc_view];
    () = msg![env; view_controller viewDidAppear:false];
    let sel: SEL = env
        .objc
        .register_host_selector(
            "navigationController:didShowViewController:animated:".to_string(),
            &mut env.mem
        );
    let responds: bool  = msg![env; delegate respondsToSelector:sel];
    if responds {
        () = msg![env; delegate navigationController:this didShowViewController:view_controller animated:false];
    }
}

// Pop repeatedly rather than unwinding the stack in one step, so every
// controller on the way out gets its -viewWillDisappear: and its release. A
// bulk removal would skip both, and an app that frees resources in
// -viewWillDisappear: would leak them.
//
// Returns the controllers that were popped, outermost first, as UIKit does.
- (id)popToRootViewControllerAnimated:(bool)animated {
    let mut popped: Vec<id> = Vec::new();
    loop {
        let depth = env
            .objc
            .borrow::<UINavigationControllerHostObject>(this)
            .navigation_stack
            .len();
        if depth <= 1 {
            break;
        }
        let controller: id = msg![env; this popViewControllerAnimated:animated];
        if controller == nil {
            break;
        }
        popped.push(controller);
    }
    let array = ns_array::from_vec(env, popped);
    autorelease(env, array)
}

- (id)popToViewController:(id)target // UIViewController*
                 animated:(bool)animated {
    let mut popped: Vec<id> = Vec::new();
    loop {
        let stack = env
            .objc
            .borrow::<UINavigationControllerHostObject>(this)
            .navigation_stack
            .clone();
        // Stop if the target is already on top, or is not on the stack at all —
        // popping to a controller that was never pushed would empty the stack.
        if stack.len() <= 1 || *stack.last().unwrap() == target || !stack.contains(&target) {
            break;
        }
        let controller: id = msg![env; this popViewControllerAnimated:animated];
        if controller == nil {
            break;
        }
        popped.push(controller);
    }
    let array = ns_array::from_vec(env, popped);
    autorelease(env, array)
}

- (id)popViewControllerAnimated:(bool)_animated {
    let (popped_view_controller, next_view_controller) = {
        let host_object = env.objc.borrow_mut::<UINavigationControllerHostObject>(this);
        if host_object.navigation_stack.len() <= 1 {
            return nil;
        }
        let popped_view_controller = host_object.navigation_stack.pop().unwrap();
        let next_view_controller = *host_object.navigation_stack.last().unwrap();
        (popped_view_controller, next_view_controller)
    };

    // The stack owns the popped controller. Keep a conventional autoreleased
    // return value alive while releasing that ownership after disappearance.
    retain(env, popped_view_controller);

    let self_view: id = msg![env; this view];
    () = msg![env; popped_view_controller viewWillDisappear:false];
    () = msg![env; next_view_controller viewWillAppear:false];

    let popped_view: id = msg![env; popped_view_controller view];
    () = msg![env; popped_view removeFromSuperview];

    // A normal push leaves the previous view underneath the new one, but
    // restore it defensively for archives/controllers that removed it.
    let next_view: id = msg![env; next_view_controller view];
    let next_superview: id = msg![env; next_view superview];
    if next_superview != self_view {
        () = msg![env; self_view addSubview:next_view];
    }

    () = msg![env; popped_view_controller viewDidDisappear:false];
    () = msg![env; next_view_controller viewDidAppear:false];

    super::set_parent_view_controller(env, popped_view_controller, nil);
    release(env, popped_view_controller); // navigation stack ownership
    autorelease(env, popped_view_controller);
    popped_view_controller
}

- (id)topViewController {
    if let Some(top_vc) = env.objc.borrow::<UINavigationControllerHostObject>(this).navigation_stack.last() {
        *top_vc
    } else {
        nil
    }
}

// What the user can actually see, which is not always the top of the stack: a
// modal presented over the navigation controller covers it, and UIKit reports
// the modal here while `-topViewController` keeps reporting the stack. An app
// asking this question is usually deciding whether the screen it cares about is
// in front, so answering with the stack top would tell it the opposite of the
// truth whenever a modal is up.
- (id)visibleViewController {
    let modal: id = msg![env; this modalViewController];
    if modal != nil {
        return modal;
    }
    msg![env; this topViewController]
}

- (id)viewControllers {
    let vcs = env.objc.borrow::<UINavigationControllerHostObject>(this).navigation_stack.to_vec();
    for vc in &vcs {
        retain(env, *vc);
    }
    let res = ns_array::from_vec(env, vcs);
    autorelease(env, res)
}
- (())setViewControllers:(id)controllers { // NSArray *
    msg![env; this setViewControllers:controllers animated:false]
}

- (())setViewControllers:(id)controllers // NSArray *
                animated:(bool)animated {
    // Clean existing view controllers
    let self_view: id = msg![env; this view];
    let mut stack = std::mem::take(&mut env.objc.borrow_mut::<UINavigationControllerHostObject>(this).navigation_stack);
    for controller in stack.drain(..) {
        let vc_view = env.objc.borrow::<super::UIViewControllerHostObject>(controller).view;
        if vc_view != nil {
            let vc_view_superview: id = msg![env; vc_view superview];
            if self_view == vc_view_superview {
                // TODO: view{Will,Did}Disappear: messages for vc?
                () = msg![env; vc_view removeFromSuperview];
            }
        }

        let parent = env
            .objc
            .borrow::<super::UIViewControllerHostObject>(controller)
            .parent_view_controller;
        if parent == this {
            super::set_parent_view_controller(env, controller, nil);
        }

        release(env, controller);
    }

    let mut tmp_stack: Vec<id> = Vec::new();
    let count: NSUInteger = msg![env; controllers count];
    if count == 0 {
        return;
    }
    for i in 0..(count - 1) {
        let next: id = msg![env; controllers objectAtIndex:i];
        tmp_stack.push(next);
        retain(env, next);
        super::set_parent_view_controller(env, next, this);
    }
    env.objc.borrow_mut::<UINavigationControllerHostObject>(this).navigation_stack = tmp_stack;

    // The n-1 element in the controllers array is special and need to be pushed
    // TODO: double check this behavior
    let last_vc: id = msg![env; controllers objectAtIndex:(count - 1)];
    () = msg![env; this pushViewController:last_vc animated:animated];
}

- (id)navigationBar {
    env.objc
        .borrow::<UINavigationControllerHostObject>(this)
        .navigation_bar
}

- (bool)shouldAutorotateToInterfaceOrientation:(UIInterfaceOrientation)interface_orientation {
    let top_view_controller = env
        .objc
        .borrow::<UINavigationControllerHostObject>(this)
        .navigation_stack
        .last()
        .copied();
    if let Some(top_view_controller) = top_view_controller {
        msg![env; top_view_controller shouldAutorotateToInterfaceOrientation:interface_orientation]
    } else {
        msg_super![env; this shouldAutorotateToInterfaceOrientation:interface_orientation]
    }
}
- (bool)isNavigationBarHidden {
    env.objc
        .borrow::<UINavigationControllerHostObject>(this)
        .navigation_bar_hidden
}
- (())setNavigationBarHidden:(bool)hidden {
    let navigation_bar = {
        let host_object = env.objc.borrow_mut::<UINavigationControllerHostObject>(this);
        host_object.navigation_bar_hidden = hidden;
        host_object.navigation_bar
    };
    if navigation_bar != nil {
        () = msg![env; navigation_bar setHidden:hidden];
    }
}
- (())setNavigationBarHidden:(bool)hidden animated:(bool)_animated {
    () = msg![env; this setNavigationBarHidden:hidden];
}

@end

// Early Interface Builder archives may instantiate these objects even when
// the app keeps its navigation bar hidden. UINavigationBar inherits UIView's
// allocation and keyed-unarchiving behavior. UINavigationItem needs its own
// placeholder initializer because NSObject does not implement initWithCoder:.

@implementation UINavigationBar: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UINavigationBarHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

// Nothing draws a navigation bar, so no delegate callback is ever sent. The
// property still has to exist: nibs set it while wiring the interface up, long
// before anything would push an item onto the bar.
- (())setDelegate:(id)_delegate {
}
- (id)delegate {
    nil
}
- (())setTintColor:(id)_color {
}
- (())setBarStyle:(NSInteger)_style {
}
- (())setTranslucent:(bool)_translucent {
}

// The stack of navigation items. Nothing here draws them, but an app that
// pushes one is usually doing it as part of changing screens, and a bar that
// cannot be pushed onto ends the app in the middle of that — which is how a
// game's menu button turns into a crash once the button starts working at all.
//
// The items are kept rather than dropped so that `-topItem` answers what was
// last pushed, which is what an app reads back to decide what to show.
- (())pushNavigationItem:(id)item // UINavigationItem*
                animated:(bool)_animated {
    retain(env, item);
    env.objc
        .borrow_mut::<UINavigationBarHostObject>(this)
        .items
        .push(item);
}
- (())pushNavigationItem:(id)item { // UINavigationItem*
    () = msg![env; this pushNavigationItem:item animated:false];
}
- (id)popNavigationItemAnimated:(bool)_animated {
    let popped = env
        .objc
        .borrow_mut::<UINavigationBarHostObject>(this)
        .items
        .pop();
    match popped {
        Some(item) => {
            autorelease(env, item)
        }
        None => nil,
    }
}
- (id)topItem {
    env.objc
        .borrow::<UINavigationBarHostObject>(this)
        .items
        .last()
        .copied()
        .unwrap_or(nil)
}
- (id)backItem {
    let items = &env.objc.borrow::<UINavigationBarHostObject>(this).items;
    if items.len() < 2 {
        return nil;
    }
    items[items.len() - 2]
}
- (())setItems:(id)_items animated:(bool)_animated {
    // TODO: replace the whole stack. Nothing reads it back yet beyond the two
    // accessors above.
}

- (())dealloc {
    let items = std::mem::take(&mut env.objc.borrow_mut::<UINavigationBarHostObject>(this).items);
    for item in items {
        release(env, item);
    }
    msg_super![env; this dealloc]
}

@end

// UIToolbar was the third most common missing class in a 1501-app survey, in 28
// of them — and in most it is merely constructed and configured during setup,
// long before anything would look at it.
//
// So it is a real UIView that holds its items rather than a drawn toolbar. The
// items round-trip, which is what layout and configuration code reads back;
// nothing paints them, exactly as UINavigationBar above does not paint a
// navigation bar.
//
// It does, however, *answer* a touch. A toolbar that draws nothing is still a
// solid rectangle in the view hierarchy, so before this it swallowed every
// touch that landed on it and the app's buttons were dead — a game whose entire
// menu is a toolbar could not be started at all, and the player could see the
// buttons, because the artwork under the bar is the app's own. Working out
// which item was pressed needs the layout UIKit would have used, which is what
// [toolbar_item_frames] reconstructs.
@implementation UIToolbar: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIToolbarHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

// A toolbar built in Interface Builder carries its buttons in the archive, and
// without reading them the bar comes up empty: the app's own artwork shows
// buttons, the bar covers them, and nothing can be pressed. That is what a game
// whose entire menu is a toolbar looks like — a menu that ignores you.
- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];

    let items_key = get_static_str(env, "UIItems");
    let items: id = msg![env; coder decodeObjectForKey:items_key];
    if items != nil {
        () = msg![env; this setItems:items];
    }

    let bar_style_key = get_static_str(env, "UIBarStyle");
    if msg![env; coder containsValueForKey:bar_style_key] {
        let bar_style: NSInteger = msg![env; coder decodeIntegerForKey:bar_style_key];
        () = msg![env; this setBarStyle:bar_style];
    }

    this
}

- (id)items {
    env.objc.borrow::<UIToolbarHostObject>(this).items
}
- (())setItems:(id)items { // NSArray* of UIBarButtonItem*
    retain(env, items);
    let host_object = env.objc.borrow_mut::<UIToolbarHostObject>(this);
    let old = std::mem::replace(&mut host_object.items, items);
    release(env, old);
}
- (())setItems:(id)items animated:(bool)_animated {
    () = msg![env; this setItems:items];
}

- (NSInteger)barStyle {
    env.objc.borrow::<UIToolbarHostObject>(this).bar_style
}
- (())setBarStyle:(NSInteger)style {
    env.objc.borrow_mut::<UIToolbarHostObject>(this).bar_style = style;
}

- (id)tintColor {
    env.objc.borrow::<UIToolbarHostObject>(this).tint_color
}
- (())setTintColor:(id)color { // UIColor*
    retain(env, color);
    let host_object = env.objc.borrow_mut::<UIToolbarHostObject>(this);
    let old = std::mem::replace(&mut host_object.tint_color, color);
    release(env, old);
}

- (())setTranslucent:(bool)_translucent {
}
- (bool)isTranslucent {
    false
}

// Pressing a toolbar button. UIKit sends the item's action when the touch ends
// inside the item it started in; this is the same rule, simplified to where the
// touch ended, because a toolbar item has no highlighted state here to track.
- (())touchesEnded:(id)touches // NSSet* of UITouch*
         withEvent:(id)event {
    let touch: id = msg![env; touches anyObject];
    if touch == nil {
        return;
    }
    let point: CGPoint = msg![env; touch locationInView:this];

    let Some(item) = toolbar_item_at_point(env, this, point) else {
        // Nothing there: a toolbar has gaps, and a touch in one is not a
        // button press. It is also not something to pass up the responder
        // chain, because on a device the toolbar would have eaten it too.
        return;
    };

    let &UIBarButtonItemHostObject { target, action, enabled, .. } = env.objc.borrow(item);
    if !enabled {
        return;
    }
    let (Some(action), true) = (action, target != nil) else {
        // An item with no action, or one that expects the responder chain to
        // find a handler, which is not modelled here.
        log_dbg!("Toolbar item {:?} pressed, but it has no target and action", item);
        return;
    };
    log_dbg!("Toolbar item {:?} pressed, sending {:?}", item, action.as_str(&env.mem));
    () = msg_send(env, (target, action, item));

    let _ = event;
}

- (())dealloc {
    let &UIToolbarHostObject { items, tint_color, .. } = env.objc.borrow(this);
    release(env, items);
    release(env, tint_color);
    msg_super![env; this dealloc]
}

@end

@implementation UINavigationItem: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UINavigationItemHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithCoder:(id)_coder {
    this
}

- (id)initWithTitle:(id)title { // NSString*
    let title: id = msg![env; title copy];
    env.objc.borrow_mut::<UINavigationItemHostObject>(this).title = title;
    this
}

- (id)title {
    env.objc.borrow::<UINavigationItemHostObject>(this).title
}
- (())setTitle:(id)title { // NSString*
    let title: id = msg![env; title copy];
    let host_object = env.objc.borrow_mut::<UINavigationItemHostObject>(this);
    let old = std::mem::replace(&mut host_object.title, title);
    release(env, old);
}

// The bar items are stored and handed back, but nothing draws a navigation
// bar, so they are never shown or tapped. An app whose only route onward is a
// bar button is therefore stuck — and the missing piece is the bar, not these
// classes. Worth knowing before blaming input handling.
- (id)leftBarButtonItem {
    env.objc.borrow::<UINavigationItemHostObject>(this).left_bar_button_item
}
- (())setLeftBarButtonItem:(id)item { // UIBarButtonItem*
    retain(env, item);
    let host_object = env.objc.borrow_mut::<UINavigationItemHostObject>(this);
    let old = std::mem::replace(&mut host_object.left_bar_button_item, item);
    release(env, old);
}
- (())setLeftBarButtonItem:(id)item animated:(bool)_animated {
    () = msg![env; this setLeftBarButtonItem:item];
}

- (id)rightBarButtonItem {
    env.objc.borrow::<UINavigationItemHostObject>(this).right_bar_button_item
}
- (())setRightBarButtonItem:(id)item { // UIBarButtonItem*
    retain(env, item);
    let host_object = env.objc.borrow_mut::<UINavigationItemHostObject>(this);
    let old = std::mem::replace(&mut host_object.right_bar_button_item, item);
    release(env, old);
}
- (())setRightBarButtonItem:(id)item animated:(bool)_animated {
    () = msg![env; this setRightBarButtonItem:item];
}

- (id)backBarButtonItem {
    env.objc.borrow::<UINavigationItemHostObject>(this).back_bar_button_item
}
- (())setBackBarButtonItem:(id)item { // UIBarButtonItem*
    retain(env, item);
    let host_object = env.objc.borrow_mut::<UINavigationItemHostObject>(this);
    let old = std::mem::replace(&mut host_object.back_bar_button_item, item);
    release(env, old);
}

- (id)titleView {
    env.objc.borrow::<UINavigationItemHostObject>(this).title_view
}
- (())setTitleView:(id)view { // UIView*
    retain(env, view);
    let host_object = env.objc.borrow_mut::<UINavigationItemHostObject>(this);
    let old = std::mem::replace(&mut host_object.title_view, view);
    release(env, old);
}

- (())setHidesBackButton:(bool)_hides {
}
- (())setHidesBackButton:(bool)_hides animated:(bool)_animated {
}

- (())dealloc {
    let &UINavigationItemHostObject {
        title,
        left_bar_button_item,
        right_bar_button_item,
        back_bar_button_item,
        title_view,
    } = env.objc.borrow(this);
    release(env, title);
    release(env, left_bar_button_item);
    release(env, right_bar_button_item);
    release(env, back_bar_button_item);
    release(env, title_view);
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

// The abstract base UIBarButtonItem inherits from. It is here so that an app
// subclassing it, or reaching -setTitle:/-setEnabled: through it, resolves.
@implementation UIBarItem: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIBarButtonItemHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)title {
    env.objc.borrow::<UIBarButtonItemHostObject>(this).title
}
- (())setTitle:(id)title { // NSString*
    let title: id = msg![env; title copy];
    let host_object = env.objc.borrow_mut::<UIBarButtonItemHostObject>(this);
    let old = std::mem::replace(&mut host_object.title, title);
    release(env, old);
}

- (id)image {
    env.objc.borrow::<UIBarButtonItemHostObject>(this).image
}
- (())setImage:(id)image { // UIImage*
    retain(env, image);
    let host_object = env.objc.borrow_mut::<UIBarButtonItemHostObject>(this);
    let old = std::mem::replace(&mut host_object.image, image);
    release(env, old);
}

- (bool)isEnabled {
    env.objc.borrow::<UIBarButtonItemHostObject>(this).enabled
}
- (())setEnabled:(bool)enabled {
    env.objc.borrow_mut::<UIBarButtonItemHostObject>(this).enabled = enabled;
}

- (())dealloc {
    let &UIBarButtonItemHostObject { title, image, .. } = env.objc.borrow(this);
    release(env, title);
    release(env, image);
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

// A button in a navigation bar or toolbar. Neither bar is drawn, so an item is
// stored, answers its accessors, and is never displayed or tapped. Its target
// and action are kept so an app that reads them back or fires them itself
// behaves; tapHLE will never fire them on its own.
@implementation UIBarButtonItem: UIBarItem

// What the archive says about the item. Nothing here draws it, but the title
// and the system-item kind decide where the item sits when a touch has to be
// matched to one — a flexible space is not a button and must not be treated as
// one — and the enabled flag decides whether pressing it does anything. The
// target and action arrive separately, as a runtime event connection, which is
// why this can be complete without decoding them.
- (id)initWithCoder:(id)coder {
    let title_key = get_static_str(env, "UITitle");
    let title: id = msg![env; coder decodeObjectForKey:title_key];
    let title: id = if title == nil { nil } else { msg![env; title copy] };

    let is_system_key = get_static_str(env, "UIIsSystemItem");
    let is_system_item: bool = msg![env; coder decodeBoolForKey:is_system_key];
    let system_item: NSInteger = if is_system_item {
        let system_item_key = get_static_str(env, "UISystemItem");
        msg![env; coder decodeIntegerForKey:system_item_key]
    } else {
        0
    };

    let enabled_key = get_static_str(env, "UIEnabled");
    let enabled: bool = if msg![env; coder containsValueForKey:enabled_key] {
        msg![env; coder decodeBoolForKey:enabled_key]
    } else {
        true
    };

    let host_object = env.objc.borrow_mut::<UIBarButtonItemHostObject>(this);
    host_object.title = title;
    host_object.system_item = system_item;
    host_object.enabled = enabled;

    this
}

// A bar button item is not a UIControl, but apps configure it as though it
// were — this was the single most common missing selector once bar items began
// decoding from nibs, in 40 apps. A bar item has exactly one thing it can do,
// so the event mask has nothing to select between and the target/action pair
// lands where -setTarget:/-setAction: would have put it.
- (())addTarget:(id)target action:(SEL)action forControlEvents:(NSUInteger)_events {
    let host_object = env.objc.borrow_mut::<UIBarButtonItemHostObject>(this);
    host_object.target = target;
    host_object.action = Some(action);
}
- (())removeTarget:(id)_target action:(SEL)_action forControlEvents:(NSUInteger)_events {
    let host_object = env.objc.borrow_mut::<UIBarButtonItemHostObject>(this);
    host_object.target = nil;
    host_object.action = None;
}

- (id)initWithTitle:(id)title // NSString*
              style:(NSInteger)_style
             target:(id)target
             action:(SEL)action {
    let title: id = msg![env; title copy];
    let host_object = env.objc.borrow_mut::<UIBarButtonItemHostObject>(this);
    host_object.title = title;
    host_object.target = target;
    host_object.action = Some(action);
    this
}

- (id)initWithImage:(id)image // UIImage*
              style:(NSInteger)_style
             target:(id)target
             action:(SEL)action {
    retain(env, image);
    let host_object = env.objc.borrow_mut::<UIBarButtonItemHostObject>(this);
    host_object.image = image;
    host_object.target = target;
    host_object.action = Some(action);
    this
}

// The system item picks a standard title or glyph (Done, Cancel, Add, ...).
// None is drawn, so it is only recorded.
- (id)initWithBarButtonSystemItem:(NSInteger)system_item
                           target:(id)target
                           action:(SEL)action {
    let host_object = env.objc.borrow_mut::<UIBarButtonItemHostObject>(this);
    host_object.system_item = system_item;
    host_object.target = target;
    host_object.action = Some(action);
    this
}

- (id)initWithCustomView:(id)view { // UIView*
    retain(env, view);
    env.objc.borrow_mut::<UIBarButtonItemHostObject>(this).custom_view = view;
    this
}

- (id)customView {
    env.objc.borrow::<UIBarButtonItemHostObject>(this).custom_view
}
- (())setCustomView:(id)view { // UIView*
    retain(env, view);
    let host_object = env.objc.borrow_mut::<UIBarButtonItemHostObject>(this);
    let old = std::mem::replace(&mut host_object.custom_view, view);
    release(env, old);
}

- (id)target {
    env.objc.borrow::<UIBarButtonItemHostObject>(this).target
}
// Weak, as a control's target is: the target usually owns the item.
- (())setTarget:(id)target {
    env.objc.borrow_mut::<UIBarButtonItemHostObject>(this).target = target;
}
- (())setAction:(SEL)action {
    env.objc.borrow_mut::<UIBarButtonItemHostObject>(this).action = Some(action);
}

- (())setStyle:(NSInteger)_style {
}
- (())setWidth:(f32)_width {
}
- (())setTintColor:(id)_color { // UIColor*
}

- (())dealloc {
    let &UIBarButtonItemHostObject { custom_view, .. } = env.objc.borrow(this);
    release(env, custom_view);
    msg_super![env; this dealloc]
}

@end

};
