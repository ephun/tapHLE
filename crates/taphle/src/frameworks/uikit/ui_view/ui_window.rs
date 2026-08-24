/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIWindow`.
//!
//! Useful resources:
//! - [Technical Q&A QA1588: Automatic orientation support for iPhone and iPad apps](https://developer.apple.com/library/archive/qa/qa1588/_index.html)
//! - [Technical Q&A QA1688: Why won't my UIViewController rotate with the device?](https://developer.apple.com/library/archive/qa/qa1688/_index.html)

use super::UIViewHostObject;
use crate::dyld::{ConstantExports, HostConstant};
use crate::environment::Environment;
use crate::frameworks::core_graphics::cg_affine_transform::CGAffineTransform;
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect};
use crate::frameworks::foundation::ns_string;
use crate::frameworks::uikit::ui_application::{
    UIInterfaceOrientation, UIInterfaceOrientationLandscapeLeft,
    UIInterfaceOrientationLandscapeRight, UIInterfaceOrientationPortrait,
    UIInterfaceOrientationPortraitUpsideDown,
};
use crate::frameworks::uikit::ui_device::{
    UIDeviceOrientationLandscapeLeft, UIDeviceOrientationLandscapeRight,
    UIDeviceOrientationPortraitUpsideDown,
};
use crate::mem::ConstVoidPtr;
use crate::objc::{
    id, msg, msg_class, msg_super, nil, objc_classes, release, retain, ClassExports,
};

#[derive(Default)]
pub struct State {
    /// List of visible windows for internal purposes. Non-retaining!
    ///
    /// This is public because Core Animation also uses it.
    pub windows: Vec<id>,
    /// The most recent window which received `makeKeyAndVisible` message.
    /// Non-retaining!
    pub key_window: Option<id>,
    /// Root view controller owned by each window. The controller references
    /// are retained; window keys are non-retaining and removed on dealloc.
    root_view_controllers: Vec<(id, id)>,
}

/// Which interface orientation this window should present its root controller
/// in, rotating the device to reach it if necessary.
///
/// If something has already chosen a non-portrait orientation — a
/// `--landscape-*` option, `UIInterfaceOrientation` in `Info.plist`, or an
/// earlier `setStatusBarOrientation:` — that choice stands and the controller
/// is only asked to confirm it, which is what this code has always done.
///
/// The new part is the portrait case. Portrait is where an app that said
/// nothing ends up, so it cannot be read as a decision, and the controller has
/// to be asked. One that accepts portrait is left alone. One that refuses it is
/// asked about the others in turn and the device is rotated to the first it
/// accepts, which is how iOS gets a landscape-only app onto a landscape screen.
///
/// Returns `None` when the presentation should stay portrait, either because
/// the controller is happy there or because it refused everything and there is
/// nothing sensible left to do.
fn orientation_to_present_in(env: &mut Environment, vc: id) -> Option<UIInterfaceOrientation> {
    use crate::window::DeviceOrientation;

    // Headless runs have no window to read or rotate.
    let current = env.window.as_ref()?.current_rotation();

    if current != DeviceOrientation::Portrait {
        return Some(match current {
            DeviceOrientation::PortraitUpsideDown => UIDeviceOrientationPortraitUpsideDown,
            DeviceOrientation::LandscapeLeft => UIDeviceOrientationLandscapeLeft,
            DeviceOrientation::LandscapeRight => UIDeviceOrientationLandscapeRight,
            DeviceOrientation::Portrait => unreachable!(),
        });
    }

    let accepts_portrait: bool =
        msg![env; vc shouldAutorotateToInterfaceOrientation:UIInterfaceOrientationPortrait];
    if accepts_portrait {
        return None;
    }

    // Landscape first, and left before right, because a landscape-only app that
    // accepts both is overwhelmingly designed for the home button on the right,
    // which is this device orientation.
    for &(device, interface) in &[
        (
            DeviceOrientation::LandscapeLeft,
            UIInterfaceOrientationLandscapeRight,
        ),
        (
            DeviceOrientation::LandscapeRight,
            UIInterfaceOrientationLandscapeLeft,
        ),
        (
            DeviceOrientation::PortraitUpsideDown,
            UIInterfaceOrientationPortraitUpsideDown,
        ),
    ] {
        let accepted: bool = msg![env; vc shouldAutorotateToInterfaceOrientation:interface];
        if !accepted {
            continue;
        }
        log_dbg!(
            "Controller {:?} refuses portrait and accepts {:?}; rotating the device to match.",
            vc,
            interface
        );
        env.on_parent_stack_in_coroutine(|window, _| window.rotate_device(device));
        return Some(interface);
    }

    // Refused everything, including portrait. iOS would keep it where it is
    // rather than invent an orientation, and so does this.
    log!(
        "Warning: controller {:?} accepts no interface orientation; presenting portrait anyway.",
        vc
    );
    None
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIWindow: UIView

// TODO: more?

- (id)initWithFrame:(CGRect)frame {
    let this = msg_super![env; this initWithFrame:frame];
    // Undocumented: windows seem to be hidden by default on iOS, unlike views.
    // Super call to bypass the overriden setter on this class, which would post
    // a notification.
    () = msg_super![env; this setHidden:true];

    let list = &mut env.framework_state.uikit.ui_view.ui_window.windows;
    list.push(this);
    log_dbg!(
        "New window: {:?}. New list of all windows: {:?}",
        this,
        list,
    );

    this
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    let this = msg_super![env; this initWithCoder:coder];
    // Undocumented: windows seem to be hidden by default on iOS, unlike views.
    // Super call to bypass the overriden setter on this class, which would post
    // a notification.
    () = msg_super![env; this setHidden:true];

    let list = &mut env.framework_state.uikit.ui_view.ui_window.windows;
    list.push(this);
    log_dbg!(
        "New window: {:?}. New list of all windows: {:?}",
        this,
        list,
    );

    this
}

- (())dealloc {
    if let Some(key_window) = env.framework_state.uikit.ui_view.ui_window.key_window {
        if key_window == this {
            env.framework_state.uikit.ui_view.ui_window.key_window = None;
        }
    }
    let list = &mut env.framework_state.uikit.ui_view.ui_window.windows;
    let idx = list.iter().position(|&w| w == this).unwrap();
    list.remove(idx);
    log_dbg!(
        "Deallocating window {:?}. New list of all windows: {:?}",
        this,
        list,
    );

    let roots = &mut env.framework_state.uikit.ui_view.ui_window.root_view_controllers;
    if let Some(idx) = roots.iter().position(|&(window, _)| window == this) {
        let (_, root_view_controller) = roots.remove(idx);
        release(env, root_view_controller);
    }
    msg_super![env; this dealloc]
}

- (())setHidden:(bool)is_hidden {
    () = msg_super![env; this setHidden:is_hidden];

    // TODO: post UIWindowDidBecomeVisibleNotification,
    //            UIWindowDidBecomeHiddenNotification
    log_dbg!("[(UIWindow*){:?} setHidden:{:?}]", this, is_hidden);
}

- (())makeKeyWindow {
    // TODO: post UIWindowDidResignKeyNotification for previous key window
    env.framework_state.uikit.ui_view.ui_window.key_window = Some(this);

    let center: id = msg_class![env; NSNotificationCenter defaultCenter];
    let notif_name = ns_string::get_static_str(env, UIWindowDidBecomeKeyNotification);
    () = msg![env; center postNotificationName:notif_name object:this userInfo:nil];
}

- (bool)isKeyWindow {
    env.framework_state.uikit.ui_view.ui_window.key_window == Some(this)
}

- (())makeKeyAndVisible {
    // TODO: We don't currently have send any non-touch events to windows,
    // so there's no meaning in it yet.

    // FIXME: This should also bump the window to the top of the list.

    () = msg![env; this makeKeyWindow];

    // TODO: post UIWindowDidBecomeVisibleNotification
    () = msg![env; this setHidden:false];
}

- (id)rootViewController {
    env.framework_state
        .uikit
        .ui_view
        .ui_window
        .root_view_controllers
        .iter()
        .find_map(|&(window, controller)| (window == this).then_some(controller))
        .unwrap_or(nil)
}

- (())setRootViewController:(id)new_controller {
    let old_controller: id = msg![env; this rootViewController];
    if old_controller == new_controller {
        return;
    }

    retain(env, new_controller);

    if old_controller != nil {
        let old_view: id = msg![env; old_controller view];
        let old_superview: id = msg![env; old_view superview];
        if old_superview == this {
            () = msg![env; old_view removeFromSuperview];
        }
    }

    let roots = &mut env.framework_state.uikit.ui_view.ui_window.root_view_controllers;
    if let Some((_, controller)) = roots.iter_mut().find(|(window, _)| *window == this) {
        *controller = new_controller;
    } else {
        roots.push((this, new_controller));
    }

    if new_controller != nil {
        let new_view: id = msg![env; new_controller view];
        () = msg![env; this addSubview:new_view];
    }

    release(env, old_controller);
}

// We only model the single main screen
- (id)screen {
    msg_class![env; UIScreen mainScreen]
}
- (())setScreen:(id)screen {
    log_dbg!("[(UIWindow*){:?} setScreen:{:?}]", this, screen);
}

// UIResponder implementation
// From the Apple UIView docs regarding [UIResponder nextResponder]:
// "UIWindow returns the application object."
- (id)nextResponder {
    msg_class![env; UIApplication sharedApplication]
}

- (())addSubview:(id)view {
    log_dbg!("[(UIWindow*){:?} addSubview:{:?}] => ()", this, view);

    if view == nil || env.objc.borrow::<UIViewHostObject>(view).view_controller == nil {
        () = msg_super![env; this addSubview:view];
        return;
    }

    // Below we treat a special case of adding view controller's view
    // to a window, in order to generate display related notifications

    if env.objc.borrow::<UIViewHostObject>(this).subviews.contains(&view) {
        // For the case of existing view hidden by another view,
        // we need to delay a below sequence up until obstructions are removed
        log!("TODO: case of existing view hidden by another view for sending view[Will,Did]Appear");
    }

    let vc = env.objc.borrow::<UIViewHostObject>(view).view_controller;
    () = msg![env; vc viewWillAppear:false];
    () = msg_super![env; this addSubview:view];
    () = msg![env; vc viewDidAppear:false];

    // Support auto-rotation.
    //
    // An app can say which way up it goes in two places, and until now tapHLE
    // honoured only the first: `UIInterfaceOrientation` in `Info.plist` (or a
    // `--landscape-*` option), which sets the window's orientation before the
    // app runs. The second is `shouldAutorotateToInterfaceOrientation:`, and on
    // iOS that is the one that *causes* the rotation — an app launches portrait
    // and the controller's answer turns it. The check below used to read the
    // window's current rotation and return `None` for portrait, so a controller
    // was only ever asked once something else had already rotated the window,
    // and an app that declares landscape in code alone never got it: it drew a
    // 480-wide layout into a 320-wide screen, clipped.
    //
    // This is not a rare shape. In one collection of 1501 apps, 1167 declare no
    // `UIInterfaceOrientation` at all.
    //
    // Asking is safe for apps that do not care, because the inherited
    // `shouldAutorotateToInterfaceOrientation:` accepts portrait and nothing
    // else — the same default as iOS — so only an app that overrides it and
    // actively refuses portrait can change anything here.
    // TODO: rotation caused by the device being turned while running.
    // FIXME: It's unclear when and where this auto-rotation is supposed to
    //        happen. It must have something to do with mounting the view
    //        controller to a window, so we do it here. QA1688 (see top of file)
    //        mentions a breaking behaviour change in iOS 6 that makes
    //        auto-rotation rely on rootViewController (a property only found in
    //        iOS 6), so the current implementation is specific to iOS <= 5.
    // FIXME: Are we supposed to notify the view somehow of the rotation?
    // FIXME: What do we do if shouldAutorotateToInterfaceOrientation:
    //        returns false? The status bar has already been rotated…
    // FIXME: The device orientation stored on env.window can come from one of
    //        three places (user/default options, setStatusBarOrientation: etc,
    //        Info.plist UIInterfaceOrientation etc). It's not clear if these
    //        are really equivalent and should all trigger autorotation.
    if let Some(orientation) = orientation_to_present_in(env, vc) {
        // (UIInterfaceOrientation and UIDeviceOrientation are compatible enums,
        //  here we use whichever is clearer contextually.)
        let should = msg![env; vc shouldAutorotateToInterfaceOrientation:orientation];
        log_dbg!("[{:?} shouldAutorotateToInterfaceOrientation:{:?}] => {:?}", vc, orientation, should);
        if should {
            log_dbg!("App requested autorotation; applying orientation transform to view {:?}.", view);
            let transform = match orientation {
                UIInterfaceOrientationPortraitUpsideDown => CGAffineTransform::make_rotation(-std::f32::consts::PI),
                UIInterfaceOrientationLandscapeLeft => CGAffineTransform::make_rotation(-std::f32::consts::FRAC_PI_2),
                UIInterfaceOrientationLandscapeRight => CGAffineTransform::make_rotation(std::f32::consts::FRAC_PI_2),
                _ => unimplemented!(),
            };

            let window_frame: CGRect = msg![env; this frame];
            log_dbg!("Window frame: {window_frame:?}");
            let view_frame: CGRect = msg![env; view frame];
            log_dbg!("Old view frame: {view_frame:?}");

            () = msg![env; view setTransform:transform];

            // Re-apply the view's old frame to compensate for the rotation
            // effectively offseting its center position and changing the size.
            // FIXME: I have no idea if this is how this should be solved, but
            //        it works for DMC4 Refrain at least.

            let view_frame: CGRect = msg![env; view frame];
            log_dbg!("Old view frame after transform: {view_frame:?}");

            () = msg![env; view setFrame:window_frame];

            let view_frame: CGRect = msg![env; view frame];
            log_dbg!("New view frame after re-applying old view frame: {view_frame:?}");
        }
    }

    // The launch path performs an initial layout pass over the views that
    // already exist. A controller presented later must also receive a layout
    // pass after its root view is mounted and oriented. Custom OpenGL views in
    // particular commonly create their drawable storage in layoutSubviews.
    () = msg![env; view layoutSubviews];
}

- (CGPoint)convertPoint:(CGPoint)point
             fromWindow:(id)other { // UIWindow*
    let this_layer: id = msg![env; this layer];
    // Resolves to nil if other is nil.
    let other_layer: id = msg![env; other layer];
    msg![env; this_layer convertPoint:point fromLayer:other_layer]
}
- (CGPoint)convertPoint:(CGPoint)point
               toWindow:(id)other { // UIWindow*
    let this_layer: id = msg![env; this layer];
    // Resolves to nil if other is nil.
    let other_layer: id = msg![env; other layer];
    msg![env; this_layer convertPoint:point toLayer:other_layer]
}

@end

};

/// Window life-cycle notifications
/// TODO: more notifications
const UIWindowDidBecomeKeyNotification: &str = "UIWindowDidBecomeKeyNotification";
// The rest of the family. `makeKeyWindow` posts only the DidBecomeKey one; the
// others are declared so apps that observe them do not dereference a null name.
const UIWindowDidResignKeyNotification: &str = "UIWindowDidResignKeyNotification";
const UIWindowDidBecomeVisibleNotification: &str = "UIWindowDidBecomeVisibleNotification";
const UIWindowDidBecomeHiddenNotification: &str = "UIWindowDidBecomeHiddenNotification";
/// Keyboard notifications
/// TODO: more keyboard notifications
pub const UIKeyboardWillShowNotification: &str = "UIKeyboardWillShowNotification";
pub const UIKeyboardDidShowNotification: &str = "UIKeyboardDidShowNotification";
pub const UIKeyboardWillHideNotification: &str = "UIKeyboardWillHideNotification";
pub const UIKeyboardDidHideNotification: &str = "UIKeyboardDidHideNotification";
pub const UIKeyboardBoundsUserInfoKey: &str = "UIKeyboardBoundsUserInfoKey";
// The iOS 3.2+ replacements for the deprecated Bounds key. tapHLE posts the old
// one; these are declared because apps read whichever they were built against,
// and an unbound key is a null dereference rather than a missing dictionary
// entry.
pub const UIKeyboardFrameBeginUserInfoKey: &str = "UIKeyboardFrameBeginUserInfoKey";
pub const UIKeyboardFrameEndUserInfoKey: &str = "UIKeyboardFrameEndUserInfoKey";
pub const UIKeyboardAnimationDurationUserInfoKey: &str = "UIKeyboardAnimationDurationUserInfoKey";
pub const UIKeyboardAnimationCurveUserInfoKey: &str = "UIKeyboardAnimationCurveUserInfoKey";

/// `UIWindowLevel` values. These are `CGFloat`s rather than strings, so unlike
/// the notification names they have to be materialised into guest memory.
/// tapHLE does not order windows by level, but an app that sets `windowLevel`
/// reads the constant to do it, and an unbound one is a null pointer it will
/// dereference.
fn window_level(env: &mut Environment, value: CGFloat) -> ConstVoidPtr {
    env.mem.alloc_and_write(value).cast().cast_const()
}
fn UIWindowLevelNormal(env: &mut Environment) -> ConstVoidPtr {
    window_level(env, 0.0)
}
fn UIWindowLevelStatusBar(env: &mut Environment) -> ConstVoidPtr {
    window_level(env, 1000.0)
}
fn UIWindowLevelAlert(env: &mut Environment) -> ConstVoidPtr {
    window_level(env, 2000.0)
}

pub const CONSTANTS: ConstantExports = &[
    (
        "_UIWindowDidResignKeyNotification",
        HostConstant::NSString(UIWindowDidResignKeyNotification),
    ),
    (
        "_UIWindowDidBecomeVisibleNotification",
        HostConstant::NSString(UIWindowDidBecomeVisibleNotification),
    ),
    (
        "_UIWindowDidBecomeHiddenNotification",
        HostConstant::NSString(UIWindowDidBecomeHiddenNotification),
    ),
    (
        "_UIWindowLevelNormal",
        HostConstant::Custom(UIWindowLevelNormal),
    ),
    (
        "_UIWindowLevelStatusBar",
        HostConstant::Custom(UIWindowLevelStatusBar),
    ),
    (
        "_UIWindowLevelAlert",
        HostConstant::Custom(UIWindowLevelAlert),
    ),
    (
        "_UIWindowDidBecomeKeyNotification",
        HostConstant::NSString(UIWindowDidBecomeKeyNotification),
    ),
    (
        "_UIKeyboardWillShowNotification",
        HostConstant::NSString(UIKeyboardWillShowNotification),
    ),
    (
        "_UIKeyboardDidShowNotification",
        HostConstant::NSString(UIKeyboardDidShowNotification),
    ),
    (
        "_UIKeyboardWillHideNotification",
        HostConstant::NSString(UIKeyboardWillHideNotification),
    ),
    (
        "_UIKeyboardDidHideNotification",
        HostConstant::NSString(UIKeyboardDidHideNotification),
    ),
    (
        "_UIKeyboardBoundsUserInfoKey",
        HostConstant::NSString(UIKeyboardBoundsUserInfoKey),
    ),
    (
        "_UIKeyboardFrameBeginUserInfoKey",
        HostConstant::NSString(UIKeyboardFrameBeginUserInfoKey),
    ),
    (
        "_UIKeyboardFrameEndUserInfoKey",
        HostConstant::NSString(UIKeyboardFrameEndUserInfoKey),
    ),
    (
        "_UIKeyboardAnimationDurationUserInfoKey",
        HostConstant::NSString(UIKeyboardAnimationDurationUserInfoKey),
    ),
    (
        "_UIKeyboardAnimationCurveUserInfoKey",
        HostConstant::NSString(UIKeyboardAnimationCurveUserInfoKey),
    ),
];
