/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UINavigationController`.

use crate::frameworks::foundation::ns_string::get_static_str;
use crate::frameworks::foundation::{ns_array, NSInteger, NSUInteger};
use crate::frameworks::uikit::ui_application::UIInterfaceOrientation;
use crate::objc::{
    autorelease, id, impl_HostObject_with_superclass, msg, msg_super, nil, objc_classes, release,
    retain, ClassExports, NSZonePtr, SEL,
};

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

@end

// UIToolbar was the third most common missing class in a 1501-app survey, in 28
// of them — and in most it is merely constructed and configured during setup,
// long before anything would look at it.
//
// So it is a real UIView that holds its items rather than a drawn toolbar. The
// items round-trip, which is what layout and configuration code reads back;
// nothing paints them yet, exactly as UINavigationBar above does not paint a
// navigation bar. A toolbar that is present but blank is a far smaller problem
// than an app that cannot start.
@implementation UIToolbar: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIToolbarHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
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

// Bar items come out of nibs already configured, and nothing here draws them,
// so decoding is accepting the object. The title, target and action are read
// back from the archive by whatever asks for them, via the accessors below.
- (id)initWithCoder:(id)_coder {
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
