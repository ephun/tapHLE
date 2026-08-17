/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UITabBarController`, `UITabBar` and `UITabBarItem`.
//!
//! `UITabBarController` was the largest missing class in a survey of 1501 apps,
//! blocking 18 of them before they reached any of their own code.
//!
//! This models the containment — which controllers there are and which one is
//! selected — and shows the selected controller's view. It does not draw a tab
//! bar, so there is nothing to tap: an app that relies on the bar to change
//! tabs will stay on the first one. That is a real limitation and it is
//! deliberate. The alternative is that none of these apps start at all, and the
//! tab bar is rarely where the interesting behaviour lives.

use super::UIViewControllerHostObject;
use crate::frameworks::foundation::{ns_string, NSInteger, NSUInteger};
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_class, msg_super, nil, objc_classes, release,
    retain, ClassExports, NSZonePtr,
};

#[derive(Default)]
struct UITabBarControllerHostObject {
    superclass: UIViewControllerHostObject,
    /// `NSArray*` of `UIViewController*`, retained.
    view_controllers: id,
    selected_index: NSUInteger,
    /// `UITabBar*`, retained.
    tab_bar: id,
    /// Something implementing `UITabBarControllerDelegate`. Weak, as delegates
    /// always are.
    delegate: id,
}
impl_HostObject_with_superclass!(UITabBarControllerHostObject);

#[derive(Default)]
struct UITabBarHostObject {
    superclass: crate::frameworks::uikit::ui_view::UIViewHostObject,
    /// `NSArray*` of `UITabBarItem*`, retained.
    items: id,
    /// `UITabBarItem*`, weak: it is one of `items`.
    selected_item: id,
    delegate: id,
}
impl_HostObject_with_superclass!(UITabBarHostObject);

#[derive(Default)]
struct UIProgressViewHostObject {
    superclass: crate::frameworks::uikit::ui_view::UIViewHostObject,
    progress: f32,
    /// `UIProgressViewStyle`.
    style: NSInteger,
}
impl_HostObject_with_superclass!(UIProgressViewHostObject);

/// A page control is a `UIControl`, not a plain view: the dots are how the
/// user changes page, so it has targets and actions like any other control.
/// The superclass here has to say so too, or UIControl's own methods cannot
/// borrow it.
///
/// TODO: this class would sit better under `ui_view/ui_control/` with the
/// other controls; it is here because it arrived alongside the tab bar.
#[derive(Default)]
struct UIPageControlHostObject {
    superclass: crate::frameworks::uikit::ui_view::ui_control::UIControlHostObject,
    number_of_pages: NSInteger,
    current_page: NSInteger,
}
impl_HostObject_with_superclass!(UIPageControlHostObject);

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UITabBarController: UIViewController

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UITabBarControllerHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];
    let key = ns_string::get_static_str(env, "UIViewControllers");
    let controllers: id = msg![env; coder decodeObjectForKey:key];
    if controllers != nil {
        () = msg![env; this setViewControllers:controllers];
    }
    this
}

- (id)viewControllers {
    env.objc.borrow::<UITabBarControllerHostObject>(this).view_controllers
}
- (())setViewControllers:(id)controllers { // NSArray*
    retain(env, controllers);
    let host_object = env.objc.borrow_mut::<UITabBarControllerHostObject>(this);
    let old = std::mem::replace(&mut host_object.view_controllers, controllers);
    release(env, old);
}
- (())setViewControllers:(id)controllers animated:(bool)_animated {
    () = msg![env; this setViewControllers:controllers];
}

- (NSUInteger)selectedIndex {
    env.objc.borrow::<UITabBarControllerHostObject>(this).selected_index
}
- (())setSelectedIndex:(NSUInteger)index {
    env.objc.borrow_mut::<UITabBarControllerHostObject>(this).selected_index = index;
}

- (id)selectedViewController {
    let &UITabBarControllerHostObject { view_controllers, selected_index, .. } =
        env.objc.borrow(this);
    if view_controllers == nil {
        return nil;
    }
    let count: NSUInteger = msg![env; view_controllers count];
    if selected_index >= count {
        return nil;
    }
    msg![env; view_controllers objectAtIndex:selected_index]
}
- (())setSelectedViewController:(id)controller {
    let view_controllers = env.objc
        .borrow::<UITabBarControllerHostObject>(this)
        .view_controllers;
    if view_controllers == nil || controller == nil {
        return;
    }
    let index: NSUInteger = msg![env; view_controllers indexOfObject:controller];
    // NSNotFound means the controller is not one of ours, which is a caller
    // error; leaving the selection alone is closer to UIKit than picking a
    // nonsensical index.
    let count: NSUInteger = msg![env; view_controllers count];
    if index < count {
        env.objc.borrow_mut::<UITabBarControllerHostObject>(this).selected_index = index;
    }
}

- (id)tabBar {
    let existing = env.objc.borrow::<UITabBarControllerHostObject>(this).tab_bar;
    if existing != nil {
        return existing;
    }
    let bar: id = msg_class![env; UITabBar new];
    env.objc.borrow_mut::<UITabBarControllerHostObject>(this).tab_bar = bar;
    bar
}

- (id)delegate {
    env.objc.borrow::<UITabBarControllerHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<UITabBarControllerHostObject>(this).delegate = delegate;
}

// Presenting the selected controller's view is the whole point of the
// container: without it the app shows an empty screen even though its content
// exists and is configured.
- (())loadView {
    let selected: id = msg![env; this selectedViewController];
    if selected != nil {
        let view: id = msg![env; selected view];
        () = msg![env; this setView:view];
        return;
    }
    () = msg_super![env; this loadView];
}

- (())dealloc {
    let &UITabBarControllerHostObject { view_controllers, tab_bar, .. } = env.objc.borrow(this);
    release(env, view_controllers);
    release(env, tab_bar);
    msg_super![env; this dealloc]
}

@end

@implementation UITabBar: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UITabBarHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)items {
    env.objc.borrow::<UITabBarHostObject>(this).items
}
- (())setItems:(id)items { // NSArray* of UITabBarItem*
    retain(env, items);
    let host_object = env.objc.borrow_mut::<UITabBarHostObject>(this);
    let old = std::mem::replace(&mut host_object.items, items);
    release(env, old);
}
- (())setItems:(id)items animated:(bool)_animated {
    () = msg![env; this setItems:items];
}

- (id)selectedItem {
    env.objc.borrow::<UITabBarHostObject>(this).selected_item
}
- (())setSelectedItem:(id)item {
    env.objc.borrow_mut::<UITabBarHostObject>(this).selected_item = item;
}

- (id)delegate {
    env.objc.borrow::<UITabBarHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<UITabBarHostObject>(this).delegate = delegate;
}

- (())dealloc {
    let items = env.objc.borrow::<UITabBarHostObject>(this).items;
    release(env, items);
    msg_super![env; this dealloc]
}

@end

// UITabBarItem is a UIBarItem, like UIBarButtonItem beside it. Its title and
// image live in the superclass, so nothing extra is needed to carry them.
@implementation UITabBarItem: UIBarItem

- (id)initWithCoder:(id)_coder {
    this
}

- (id)initWithTitle:(id)title image:(id)image tag:(NSInteger)_tag {
    () = msg![env; this setTitle:title];
    () = msg![env; this setImage:image];
    this
}

@end

// A determinate progress bar. Nothing draws it, but the value round-trips,
// because code driving a download or a load reads it back to decide what to do
// next.
@implementation UIProgressView: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIProgressViewHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (f32)progress {
    env.objc.borrow::<UIProgressViewHostObject>(this).progress
}
- (())setProgress:(f32)progress {
    env.objc.borrow_mut::<UIProgressViewHostObject>(this).progress = progress;
}
- (())setProgress:(f32)progress animated:(bool)_animated {
    () = msg![env; this setProgress:progress];
}

- (NSInteger)progressViewStyle {
    env.objc.borrow::<UIProgressViewHostObject>(this).style
}
- (())setProgressViewStyle:(NSInteger)style {
    env.objc.borrow_mut::<UIProgressViewHostObject>(this).style = style;
}

@end

// The dots under a paged scroll view. Same reasoning: the page count and
// current page are read back by the scroll view's delegate.
@implementation UIPageControl: UIControl

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIPageControlHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (NSInteger)numberOfPages {
    env.objc.borrow::<UIPageControlHostObject>(this).number_of_pages
}
- (())setNumberOfPages:(NSInteger)pages {
    env.objc.borrow_mut::<UIPageControlHostObject>(this).number_of_pages = pages;
}

- (NSInteger)currentPage {
    env.objc.borrow::<UIPageControlHostObject>(this).current_page
}
- (())setCurrentPage:(NSInteger)page {
    env.objc.borrow_mut::<UIPageControlHostObject>(this).current_page = page;
}

- (())setHidesForSinglePage:(bool)_hides {
}

@end

};
