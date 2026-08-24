/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIViewController`.
//!
//! Resources:
//! - [View Controller Programming Guide for iOS (Legacy)](https://developer.apple.com/library/archive/documentation/WindowsViews/Conceptual/ViewControllerPGforiOSLegacy/BasicViewControllers/BasicViewControllers.html)
//! - [Presenting a View Controller Modally (Legacy)](https://developer.apple.com/library/archive/documentation/WindowsViews/Conceptual/ViewControllerPGforiOSLegacy/ModalViewControllers/ModalViewControllers.html)

use crate::frameworks::core_graphics::CGRect;
use crate::frameworks::foundation::ns_objc_runtime::NSStringFromClass;
use crate::frameworks::foundation::ns_string::{from_rust_string, get_static_str, to_rust_string};
use crate::frameworks::foundation::NSInteger;
use crate::frameworks::uikit::ui_application::{
    UIInterfaceOrientation, UIInterfaceOrientationLandscapeLeft,
    UIInterfaceOrientationLandscapeRight, UIInterfaceOrientationPortrait,
    UIInterfaceOrientationPortraitUpsideDown,
};
use crate::frameworks::uikit::ui_view::set_view_controller;
use crate::objc::{
    id, msg, msg_class, nil, objc_classes, release, retain, todo_objc_setter, Class, ClassExports,
    HostObject, NSZonePtr,
};
use crate::window::DeviceOrientation;
use crate::Environment;

pub mod ui_navigation_controller;
pub mod ui_tab_bar_controller;

#[derive(Default)]
struct UIViewControllerHostObject {
    /// The root view.
    /// `UIView*`
    view: id,
    /// Nib name to be used at the load
    /// of the root view, may be nil.
    /// `NSString*`
    nib_name: id,
    /// Bundle to be used for load
    /// of the nib by name, may be nil.
    /// `NSBundle*`
    bundle: id,
    /// The containing view controller, if any. Weak/non-retaining.
    /// `UIViewController*`
    parent_view_controller: id,
    /// The full-screen view controller presented by this controller. Retained.
    /// `UIViewController*`
    modal_view_controller: id,
    /// `UINavigationItem*`, retained. Created on first use, as UIKit's is: a
    /// controller that is never pushed onto a navigation stack should not pay
    /// for one.
    navigation_item: id,
    /// Whether `viewDidLoad` has already been sent for the current view.
    /// UIKit sends it exactly once each time the view is loaded, whichever
    /// route loaded it; see [send_view_did_load_if_needed].
    view_did_load_sent: bool,
}
impl HostObject for UIViewControllerHostObject {}

/// Send `viewDidLoad` to `controller` if its view is loaded and it has not
/// been sent already.
///
/// A view controller's view has two routes into existence, and `viewDidLoad`
/// belongs to both. The programmatic route is `-loadView`, driven lazily by
/// `-view`. The other route is unarchiving from a nib that already carries the
/// controller's view, where `-initWithCoder:` connects the view directly and
/// `-loadView` never runs. Sending `viewDidLoad` only from the first route
/// leaves nib-instantiated controllers without it, which silently skips
/// whatever setup the app put there — a real app used it to compute the
/// screen-to-engine coordinate mapping for touches, so every tap landed at the
/// origin.
///
/// The caller is responsible for choosing the moment: for the nib route this
/// must be after outlet connection and `awakeFromNib`, so the handler sees a
/// fully connected controller.
pub fn send_view_did_load_if_needed(env: &mut Environment, controller: id) {
    let host_object = env
        .objc
        .borrow_mut::<UIViewControllerHostObject>(controller);
    if host_object.view_did_load_sent || host_object.view == nil {
        return;
    }
    host_object.view_did_load_sent = true;
    () = msg![env; controller viewDidLoad];
}

type UIModalTransitionStyle = NSInteger;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIViewController: UIResponder

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIViewControllerHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

// TODO: this should be a designated initializer
- (id)initWithNibName:(id)nib_name // NSString *
               bundle:(id)bundle { // NSBundle *
    retain(env, nib_name);
    retain(env, bundle);

    log_dbg!("[(UIViewController*){:?} initWithNibName:{:?} bundle:{:?}]", this, nib_name, bundle);

    env.objc.borrow_mut::<UIViewControllerHostObject>(this).nib_name = nib_name;
    env.objc.borrow_mut::<UIViewControllerHostObject>(this).bundle = bundle;

    this
}

- (id)initWithCoder:(id)coder {
    let key_ns_string = get_static_str(env, "UIView");
    let view: id = msg![env; coder decodeObjectForKey:key_ns_string];

    () = msg![env; this setView:view];

    this
}

- (())dealloc {
    let &UIViewControllerHostObject {
        view,
        nib_name,
        bundle,
        parent_view_controller: _,
        modal_view_controller,
        navigation_item,
        view_did_load_sent: _,
    } = env.objc.borrow(this);
    release(env, navigation_item);

    if modal_view_controller != nil {
        let modal_view = env
            .objc
            .borrow::<UIViewControllerHostObject>(modal_view_controller)
            .view;
        if modal_view != nil {
            () = msg![env; modal_view removeFromSuperview];
        }
        let parent = env
            .objc
            .borrow::<UIViewControllerHostObject>(modal_view_controller)
            .parent_view_controller;
        if parent == this {
            set_parent_view_controller(env, modal_view_controller, nil);
        }
    }
    if view != nil {
        set_view_controller(env, view, nil);
    }
    release(env, modal_view_controller);
    release(env, view);
    release(env, nib_name);
    release(env, bundle);

    env.objc.dealloc_object(this, &mut env.mem);
}

- (())loadView {
    let bundle: id = env.objc.borrow::<UIViewControllerHostObject>(this).bundle;
    let bundle: id = if bundle == nil {
        msg_class![env; NSBundle mainBundle]
    } else {
        bundle
    };

    let nib_name: id = get_nib_name(env, this, bundle);
    if nib_name != nil {
        // If we do have nib name, try to load it!
        log_dbg!(
            "Load {:?} view controller's view by nib, using name {}", this, to_rust_string(env, nib_name)
        );

        let nib: id = msg_class![env; UINib nibWithNibName:nib_name bundle:bundle];
        release(env, nib_name);

        // The NIB's File's Owner will be substituted by `this`,
        // implicitly loading the view as well
        let _: id = msg![env; nib instantiateWithOwner:this options:nil];

        let view = env.objc.borrow::<UIViewControllerHostObject>(this).view;
        if view == nil {
            // The nib did not set the view outlet. That means tapHLE's nib
            // parsing missed it, the nib is an "integrated" one this does not
            // support yet, or the nib could not be loaded at all — and none of
            // those is a reason to end the app. Falling through to the plain
            // -loadView below gives the controller an empty view of the right
            // size, so its screen is blank instead of absent and everything
            // around it keeps working.
            log!(
                "Warning: the nib for {:?} did not set a view; using an empty one",
                this
            );
        } else {
            return;
        }
    };

    // As a last resort, use plain UIVIew for the root view
    let class: Class = msg![env; this class];
    log!("Unable to load {:?} {} view controller's view by nib, using plain UIView", this, env.objc.get_class_name(class).to_string());
    let view: id = msg_class![env; UIView alloc];
    // Docs are saying that "an empty UIView" is created,
    // but testing reveals that frame matches the screen one
    // (at least on the simulator)
    let screen: id = msg_class![env; UIScreen mainScreen];
    let app_frame: CGRect = msg![env; screen applicationFrame];
    let view: id = msg![env; view initWithFrame:app_frame];
    () = msg![env; this setView:view];
}

- (())setView:(id)new_view { // UIView*
    let host_obj = env.objc.borrow_mut::<UIViewControllerHostObject>(this);
    if new_view == nil {
        // The view was unloaded. UIKit sends viewDidLoad again the next time
        // it is loaded, so arm it again rather than suppressing it forever.
        host_obj.view_did_load_sent = false;
    }
    let old_view = std::mem::replace(&mut host_obj.view, new_view);
    if old_view != nil {
        set_view_controller(env, old_view, nil);
    }
    if new_view != nil {
        set_view_controller(env, new_view, this);
    }
    retain(env, new_view);
    release(env, old_view);
}
- (id)view {
    let view = env.objc.borrow_mut::<UIViewControllerHostObject>(this).view;
    if view == nil {
        // Loading the view is what viewDidLoad reports, so it is sent here and
        // only here. A controller whose view the app assigned with -setView:
        // never loaded one, and must not be told that it did: an app that
        // builds its OpenGL view by hand, hands it over, and implements
        // viewDidLoad as a teardown has its game view destroyed immediately
        // after it is created.
        () = msg![env; this loadView];
        send_view_did_load_if_needed(env, this);
    }
    env.objc.borrow_mut::<UIViewControllerHostObject>(this).view
}

- (id)navigationItem {
    let existing = env.objc.borrow::<UIViewControllerHostObject>(this).navigation_item;
    if existing != nil {
        return existing;
    }
    let item: id = msg_class![env; UINavigationItem alloc];
    let item: id = msg![env; item init];
    env.objc.borrow_mut::<UIViewControllerHostObject>(this).navigation_item = item;
    item
}

- (id)parentViewController {
    env.objc
        .borrow::<UIViewControllerHostObject>(this)
        .parent_view_controller
}

- (UIInterfaceOrientation)interfaceOrientation {
    // We model a single screen, so a controller's interface orientation is the
    // window's current orientation (the same value as
    // -[UIApplication statusBarOrientation]).
    match env.window().current_rotation() {
        DeviceOrientation::Portrait => UIInterfaceOrientationPortrait,
        DeviceOrientation::PortraitUpsideDown => UIInterfaceOrientationPortraitUpsideDown,
        DeviceOrientation::LandscapeLeft => UIInterfaceOrientationLandscapeLeft,
        DeviceOrientation::LandscapeRight => UIInterfaceOrientationLandscapeRight,
    }
}

- (id)navigationController {
    let navigation_controller_class: Class = msg_class![env; UINavigationController class];
    let mut parent = env
        .objc
        .borrow::<UIViewControllerHostObject>(this)
        .parent_view_controller;

    while parent != nil {
        let is_navigation_controller: bool =
            msg![env; parent isKindOfClass:navigation_controller_class];
        if is_navigation_controller {
            return parent;
        }
        parent = env
            .objc
            .borrow::<UIViewControllerHostObject>(parent)
            .parent_view_controller;
    }

    nil
}

// Usually overridden by the application
- (())viewDidLoad {
    log_dbg!("[(UIViewController*){:?} viewDidLoad]", this);
}
- (())viewWillAppear:(bool)animated {
    log_dbg!("[(UIViewController*){:?} viewWillAppear:{}]", this, animated);
}
- (())viewDidAppear:(bool)animated {
    log_dbg!("[(UIViewController*){:?} viewDidAppear:{}]", this, animated);
}
- (())viewWillDisappear:(bool)animated {
    log_dbg!("[(UIViewController*){:?} viewWillDisappear:{}]", this, animated);
}
- (())viewDidDisappear:(bool)animated {
    log_dbg!("[(UIViewController*){:?} viewDidDisappear:{}]", this, animated);
}

// `didReceiveMemoryWarning` is the most-sent selector no tapHLE class
// implemented: 863 of the 1192 distinct apps in the import-demand catalogue
// send it. Almost none of them are *delivering* a memory warning — they are
// overriding it and calling `[super didReceiveMemoryWarning]`, and it is that
// super-send that had nowhere to go. The same is true of `viewDidUnload` (606).
//
// Nothing in tapHLE currently originates a memory warning, so this is reached
// only through such an override. The unload behaviour is implemented anyway
// rather than left as a log line, because the app's override runs around it and
// expects it to have happened: an app that nils its outlets in `viewDidUnload`
// is relying on the base class to be what triggers that.
- (())didReceiveMemoryWarning {
    log_dbg!("[(UIViewController*){:?} didReceiveMemoryWarning]", this);
    let view = env.objc.borrow::<UIViewControllerHostObject>(this).view;
    if view == nil {
        return;
    }
    // UIKit only unloads a view that is not on screen, and that condition is
    // the whole safety of this: discarding the view a game is currently drawing
    // into would be far worse than using more memory. A view with no window is
    // in no view hierarchy that is being displayed.
    let window: id = msg![env; view window];
    if window != nil {
        log_dbg!("...view is on screen, keeping it");
        return;
    }
    log_dbg!("...unloading the off-screen view");
    // setView:nil also re-arms viewDidLoad, so the next -view reloads and
    // reports properly.
    () = msg![env; this setView:nil];
    () = msg![env; this viewDidUnload];
}

// Usually overridden by the application, to release what it retained in
// -viewDidLoad.
- (())viewDidUnload {
    log_dbg!("[(UIViewController*){:?} viewDidUnload]", this);
}

- (())setTitle:(id)title { // NSString *
    todo_objc_setter!(this, to_rust_string(env, title));
}
- (())setEditing:(bool)editing {
    todo_objc_setter!(this, editing);
}
- (())setWantsFullScreenLayout:(bool)wants {
    todo_objc_setter!(this, wants);
}
- (())setHidesBottomBarWhenPushed:(bool)hides {
    todo_objc_setter!(this, hides);
}
- (())setModalTransitionStyle:(UIModalTransitionStyle)style {
    todo_objc_setter!(this, style);
}

- (id)modalViewController {
    env.objc
        .borrow::<UIViewControllerHostObject>(this)
        .modal_view_controller
}

- (())presentModalViewController:(id)modal_view_controller
                         animated:(bool)animated {
    if modal_view_controller == nil || modal_view_controller == this {
        log!("Ignoring invalid modal presentation by {:?} of {:?}", this, modal_view_controller);
        return;
    }

    // Prior to iOS 5, a presented controller's parentViewController was its
    // modal presenter. If a controller that is already presenting receives a
    // new request, UIKit presents from the visible end of that legacy chain.
    let mut presenter = this;
    loop {
        let existing_modal = env
            .objc
            .borrow::<UIViewControllerHostObject>(presenter)
            .modal_view_controller;
        if existing_modal == nil {
            break;
        }
        if existing_modal == modal_view_controller {
            log!("Ignoring duplicate modal presentation of {:?}", modal_view_controller);
            return;
        }
        presenter = existing_modal;
    }

    // Reject a presentation that would make an existing ancestor a child of
    // its own modal chain. Besides leaking both controllers, that cycle would
    // make parent/navigation traversal non-terminating.
    let mut ancestor = presenter;
    while ancestor != nil {
        if ancestor == modal_view_controller {
            log!(
                "Ignoring modal presentation of ancestor {:?} by {:?}",
                modal_view_controller,
                presenter,
            );
            return;
        }
        ancestor = env
            .objc
            .borrow::<UIViewControllerHostObject>(ancestor)
            .parent_view_controller;
    }

    let target_parent = env
        .objc
        .borrow::<UIViewControllerHostObject>(modal_view_controller)
        .parent_view_controller;
    if target_parent != nil {
        log!("Ignoring modal presentation of {:?}: it already belongs to {:?}", modal_view_controller, target_parent);
        return;
    }

    let presenter_view: id = msg![env; presenter view];
    let window: id = msg![env; presenter_view window];
    if window == nil {
        log!("Ignoring modal presentation by {:?}: its view is not in a window", presenter);
        return;
    }

    retain(env, modal_view_controller);
    env.objc
        .borrow_mut::<UIViewControllerHostObject>(presenter)
        .modal_view_controller = modal_view_controller;
    set_parent_view_controller(env, modal_view_controller, presenter);

    () = msg![env; presenter viewWillDisappear:animated];
    let modal_view: id = msg![env; modal_view_controller view];
    // UIWindow's override supplies the modal appearance callbacks and applies
    // the presented controller's orientation transform.
    () = msg![env; window addSubview:modal_view];
    () = msg![env; presenter viewDidDisappear:animated];
}

- (())dismissModalViewControllerAnimated:(bool)animated {
    // UIKit permits dismissal from the presenter, the presented controller,
    // or a child of the presented controller. Walk containment until reaching
    // the controller that owns the retained modal relationship.
    let mut presenter = this;
    let first_modal_view_controller = loop {
        let host_object = env.objc.borrow::<UIViewControllerHostObject>(presenter);
        if host_object.modal_view_controller != nil {
            break host_object.modal_view_controller;
        }
        if host_object.parent_view_controller == nil {
            log_dbg!("No modal view controller to dismiss from {:?}", this);
            return;
        }
        presenter = host_object.parent_view_controller;
    };

    // Dismissing an earlier presenter dismisses the entire modal chain. Keep
    // each transferred retained reference alive until callbacks and view
    // removal finish, but only send disappearance callbacks to the visible
    // (deepest) controller.
    let mut modal_chain = vec![first_modal_view_controller];
    loop {
        let next = env
            .objc
            .borrow::<UIViewControllerHostObject>(*modal_chain.last().unwrap())
            .modal_view_controller;
        if next == nil {
            break;
        }
        modal_chain.push(next);
    }
    let visible_modal = *modal_chain.last().unwrap();

    let mut retained_modals = Vec::with_capacity(modal_chain.len());
    let mut edge_presenter = presenter;
    for &modal in &modal_chain {
        let retained_modal = std::mem::replace(
            &mut env
                .objc
                .borrow_mut::<UIViewControllerHostObject>(edge_presenter)
                .modal_view_controller,
            nil,
        );
        assert!(retained_modal == modal);
        retained_modals.push(retained_modal);

        let parent = env
            .objc
            .borrow::<UIViewControllerHostObject>(modal)
            .parent_view_controller;
        if parent == edge_presenter {
            set_parent_view_controller(env, modal, nil);
        }
        edge_presenter = modal;
    }

    () = msg![env; visible_modal viewWillDisappear:animated];
    () = msg![env; presenter viewWillAppear:animated];

    for &modal in modal_chain.iter().rev() {
        let modal_view = env
            .objc
            .borrow::<UIViewControllerHostObject>(modal)
            .view;
        if modal_view != nil {
            () = msg![env; modal_view removeFromSuperview];
        }
    }

    () = msg![env; visible_modal viewDidDisappear:animated];
    () = msg![env; presenter viewDidAppear:animated];

    for retained_modal in retained_modals.into_iter().rev() {
        release(env, retained_modal);
    }
}
- (())dismissMoviePlayerViewControllerAnimated {
    log!("TODO: [(UIViewController*){:?} dismissMoviePlayerViewControllerAnimated]", this); // TODO
}

- (bool)shouldAutorotateToInterfaceOrientation:(UIInterfaceOrientation)interface_orientation {
    interface_orientation == UIInterfaceOrientationPortrait
}

// UIResponder implementation
// From the Apple UIView docs regarding [UIResponder nextResponder]:
// "UIViewController similarly implements the method
// and returns its view’s superview."
// https://developer.apple.com/documentation/uikit/uiresponder/next?language=objc
- (id)nextResponder {
    let view = msg![env; this view];
    let next_responder = msg![env; view superview];
    log_dbg!("[(UIView*){:?} nextResponder] => {:?}", this, next_responder);
    next_responder
}

@end

};

/// Update a view controller's weak containment link without recursively
/// decoding archived `UIParentViewController` back-references.
fn set_parent_view_controller(env: &mut Environment, view_controller: id, parent: id) {
    env.objc
        .borrow_mut::<UIViewControllerHostObject>(view_controller)
        .parent_view_controller = parent;
}

/// A helper function to resolve suitable NIB name for a `view_controller`
/// in the `bundle`. Returns nil if fails.
///
/// Note: It's a responsibility of a caller to release the returned name
/// if not-nil!
fn get_nib_name(env: &mut Environment, view_controller: id, bundle: id) -> id {
    let provider_nib_name: id = env
        .objc
        .borrow::<UIViewControllerHostObject>(view_controller)
        .nib_name;
    if provider_nib_name != nil {
        // TODO: it's not clear how to handle situation when
        // provided nib name do not exist in the bundle.
        // It probably means that our bundle resource loading
        // is faulty, to check
        assert!(check_nib_exists(env, bundle, provider_nib_name));

        retain(env, provider_nib_name);
        return provider_nib_name;
    };

    let class: Class = msg![env; view_controller class];
    let class_name: id = NSStringFromClass(env, class);
    let class_name_str = to_rust_string(env, class_name);

    if let Some(name) = class_name_str.strip_suffix("Controller") {
        let ns_name: id = from_rust_string(env, name.to_string());
        if check_nib_exists(env, bundle, ns_name) {
            release(env, class_name);
            return ns_name;
        }
    }

    if check_nib_exists(env, bundle, class_name) {
        class_name
    } else {
        release(env, class_name);
        nil
    }
}

/// A helper function to check if `nib_name` NIB actually
/// existing in the `bundle`
fn check_nib_exists(env: &mut Environment, bundle: id, nib_name: id) -> bool {
    let type_: id = get_static_str(env, "nib");
    let res: id = msg![env; bundle pathForResource:nib_name ofType:type_];
    res != nil
}
