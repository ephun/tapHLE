/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIApplication` and `UIApplicationMain`.

use super::ui_device::*;
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::{from_rust_string, get_static_str};
use crate::frameworks::foundation::{ns_array, ns_string, NSInteger, NSUInteger};
use crate::mem::{ConstVoidPtr, MutPtr};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, todo_objc_setter, Class,
    ClassExports, HostObject, NSZonePtr,
};
use crate::window::DeviceOrientation;
use crate::Environment;

#[derive(Default)]
pub struct State {
    /// [UIApplication sharedApplication]
    shared_application: Option<id>,
    pub(super) status_bar_hidden: bool,
    /// Set once the launch-time layout pass has run. Before that the view
    /// hierarchy is still being built and running guest layout code is unsafe;
    /// see `ui_view::mark_needs_layout_on_mount`.
    pub(super) finished_launching: bool,
    pub(super) network_activity_indicator_visible: bool,
}

struct UIApplicationHostObject {
    delegate: id,
    delegate_is_retained: bool,
    /// Retained. The main nib's top-level objects, kept alive for the lifetime
    /// of the application.
    ///
    /// A nib's top-level objects are returned to the loader at +1 and the
    /// loader owns them; nothing else does. For the main nib that loader is
    /// `UIApplicationMain`, and dropping them means the window an app got from
    /// its nib is deallocated at the first autorelease pool drain — see the
    /// comment where this is set.
    main_nib_top_level_objects: id,
}
impl HostObject for UIApplicationHostObject {}

pub type UIInterfaceOrientation = UIDeviceOrientation;
#[allow(unused)]
pub const UIInterfaceOrientationPortrait: UIInterfaceOrientation = UIDeviceOrientationPortrait;
#[allow(unused)]
pub const UIInterfaceOrientationPortraitUpsideDown: UIInterfaceOrientation =
    UIDeviceOrientationPortraitUpsideDown;
// These are intentionally swapped and documented as such (the UI on the device
// rotates in the opposite direction to how the device is rotated).
pub const UIInterfaceOrientationLandscapeLeft: UIInterfaceOrientation =
    UIDeviceOrientationLandscapeRight;
pub const UIInterfaceOrientationLandscapeRight: UIInterfaceOrientation =
    UIDeviceOrientationLandscapeLeft;

const STATUS_BAR_HEIGHT: f32 = 20.0;

/// Make the window that came from the main nib the key, visible window, as
/// launching does on a device.
///
/// A window built in code is made key by the app itself, but a window that
/// arrives in the main nib is already key by the time the delegate hears that
/// launching finished, and apps written that way never call
/// `-makeKeyAndVisible`. They only *ask*: the usual shape is
/// `[[[UIApplication sharedApplication] keyWindow] addSubview:someView]`.
///
/// With no key window that is a message to nil, which is silent — the view is
/// simply dropped and the app runs on with nothing on screen. An app loses its
/// entire OpenGL view that way: it asks for the key window, asks its view
/// controller for the `EAGLView` holding the game's `CAEAGLLayer`, adds one to
/// the other, and both calls succeed while nothing
/// is mounted.
///
/// Only the first window is taken, and only when no window is key already, so
/// an app that does make its own window key keeps it.
fn make_main_nib_window_key(env: &mut Environment, top_level_objects: id) {
    if env
        .framework_state
        .uikit
        .ui_view
        .ui_window
        .key_window
        .is_some()
    {
        return;
    }
    let window_class: Class = msg_class![env; UIWindow class];
    let count: NSUInteger = msg![env; top_level_objects count];
    for i in 0..count {
        let object: id = msg![env; top_level_objects objectAtIndex:i];
        let is_window: bool = msg![env; object isKindOfClass:window_class];
        if is_window {
            log_dbg!("Making the main nib's window {:?} key and visible", object);
            () = msg![env; object makeKeyAndVisible];
            return;
        }
    }
}

fn status_bar_frame(
    portrait_width: f32,
    portrait_height: f32,
    hidden: bool,
    orientation: DeviceOrientation,
) -> CGRect {
    if hidden {
        return CGRect::default();
    }

    match orientation {
        DeviceOrientation::Portrait => CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: portrait_width,
                height: STATUS_BAR_HEIGHT,
            },
        },
        DeviceOrientation::PortraitUpsideDown => CGRect {
            origin: CGPoint {
                x: 0.0,
                y: portrait_height - STATUS_BAR_HEIGHT,
            },
            size: CGSize {
                width: portrait_width,
                height: STATUS_BAR_HEIGHT,
            },
        },
        DeviceOrientation::LandscapeLeft => CGRect {
            origin: CGPoint {
                x: portrait_width - STATUS_BAR_HEIGHT,
                y: 0.0,
            },
            size: CGSize {
                width: STATUS_BAR_HEIGHT,
                height: portrait_height,
            },
        },
        DeviceOrientation::LandscapeRight => CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: STATUS_BAR_HEIGHT,
                height: portrait_height,
            },
        },
    }
}

type UIRemoteNotificationType = NSUInteger;
type UIStatusBarAnimation = NSInteger;
type UIStatusBarStyle = NSInteger;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIApplication: UIResponder

// This should only be called by UIApplicationMain
+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIApplicationHostObject {
        delegate: nil,
        delegate_is_retained: false,
        main_nib_top_level_objects: nil,
    });
    env.objc.alloc_static_object(this, host_object, &mut env.mem)
}

+ (id)sharedApplication {
    env.framework_state.uikit.ui_application.shared_application.unwrap_or(nil)
}

// This should only be called by UIApplicationMain
- (id)init {
    assert!(env.framework_state.uikit.ui_application.shared_application.is_none());
    env.framework_state.uikit.ui_application.shared_application = Some(this);
    this
}

// This is a singleton, it shouldn't be deallocated.
- (id)retain { this }
- (id)autorelease { this }
- (())release {}

- (id)delegate {
    env.objc.borrow::<UIApplicationHostObject>(this).delegate
}
- (())setDelegate:(id)delegate { // something implementing UIApplicationDelegate
    let host_object = env.objc.borrow_mut::<UIApplicationHostObject>(this);
    // This property is quasi-non-retaining: https://stackoverflow.com/a/14271150/736162
    let old_delegate = std::mem::replace(&mut host_object.delegate, delegate);
    if host_object.delegate_is_retained {
        host_object.delegate_is_retained = false;
        if delegate != old_delegate {
            release(env, old_delegate);
        }
    }
}

- (bool)isStatusBarHidden {
    env.framework_state.uikit.ui_application.status_bar_hidden
}
- (())setStatusBarHidden:(bool)hidden {
    env.framework_state.uikit.ui_application.status_bar_hidden = hidden;
}
- (())setStatusBarHidden:(bool)hidden
                animated:(bool)_animated {
    // TODO: animation
    msg![env; this setStatusBarHidden:hidden]
}
- (())setStatusBarHidden:(bool)hidden
           withAnimation:(UIStatusBarAnimation)_animation {
    // TODO: animation
    msg![env; this setStatusBarHidden:hidden]
}

- (())setStatusBarStyle:(UIStatusBarStyle)style {
    todo_objc_setter!(this, style);
}

// The network activity spinner lives in the status bar, which tapHLE does not
// draw, so there is nothing to show. The value is still stored, because an app
// that switches it on and off around requests may read it back to decide
// whether a request is already in flight.
- (bool)isNetworkActivityIndicatorVisible {
    env.framework_state.uikit.ui_application.network_activity_indicator_visible
}
- (())setNetworkActivityIndicatorVisible:(bool)visible {
    env.framework_state.uikit.ui_application.network_activity_indicator_visible = visible;
}
- (())setStatusBarStyle:(UIStatusBarStyle)style
               animated:(bool)_animated {
    // TODO: animation
    msg![env; this setStatusBarStyle:style]
}

- (UIInterfaceOrientation)statusBarOrientation {
    match env.window().current_rotation() {
        DeviceOrientation::Portrait => UIDeviceOrientationPortrait,
        DeviceOrientation::PortraitUpsideDown => UIDeviceOrientationPortraitUpsideDown,
        DeviceOrientation::LandscapeLeft => UIDeviceOrientationLandscapeLeft,
        DeviceOrientation::LandscapeRight => UIDeviceOrientationLandscapeRight
    }
}

- (CGRect)statusBarFrame {
    // Landscape-native mode reports a landscape-shaped screen.
    let (width, height) = env.window().size_unrotated_unscaled();
    status_bar_frame(
        width as f32,
        height as f32,
        env.framework_state.uikit.ui_application.status_bar_hidden,
        env.window().current_rotation(),
    )
}

- (())setStatusBarOrientation:(UIInterfaceOrientation)orientation {
    env.on_parent_stack_in_coroutine(|window, _| {window.rotate_device(match orientation {
        UIDeviceOrientationPortrait => DeviceOrientation::Portrait,
        UIDeviceOrientationPortraitUpsideDown => DeviceOrientation::PortraitUpsideDown,
        UIDeviceOrientationLandscapeLeft => DeviceOrientation::LandscapeLeft,
        UIDeviceOrientationLandscapeRight => DeviceOrientation::LandscapeRight,
        _ => unimplemented!("Orientation {} not handled yet", orientation),
    })});
}
- (())setStatusBarOrientation:(UIInterfaceOrientation)orientation
                     animated:(bool)_animated {
    // TODO: animation
    msg![env; this setStatusBarOrientation:orientation]
}

- (bool)isIdleTimerDisabled {
    !env.window().is_screen_saver_enabled()
}
- (())setIdleTimerDisabled:(bool)disabled {
    env.on_parent_stack_in_coroutine(|window, _| window.set_screen_saver_enabled(!disabled))
}

- (bool)openURL:(id)url { // NSURL
    let ns_string = msg![env; url absoluteString];
    let url_string = ns_string::to_rust_string(env, ns_string);
    if let Err(e) = crate::window::open_url(env, &url_string) {
        echo!("App opened URL {:?} unsuccessfully ({}), exiting.", url_string, e);
    } else {
        echo!("App opened URL {:?}, exiting.", url_string);
    }

    // iPhone OS doesn't really do multitasking, so the app expects to close
    // when a URL is opened, e.g. Super Monkey Ball keeps opening the URL every
    // frame! Super Monkey Ball also doesn't check whether opening failed, so
    // it's probably best to always exit.
    exit(env);
    true
}

// TODO: ignore touches
-(())beginIgnoringInteractionEvents {
    log!("TODO: ignoring beginIgnoringInteractionEvents");
}
- (bool)isIgnoringInteractionEvents {
    false
}
-(())endIgnoringInteractionEvents {
    log!("TODO: ignoring endIgnoringInteractionEvents");
}

- (id)keyWindow {
    let Some(key_window) = env
        .framework_state
        .uikit
        .ui_view
        .ui_window
        .key_window else {
        return nil;
    };
    assert!(env
        .framework_state
        .uikit
        .ui_view
        .ui_window
        .windows
        .contains(&key_window));
    key_window
}

- (id)windows {
    let windows: Vec<id> = (*env
        .framework_state
        .uikit
        .ui_view
        .ui_window
        .windows).to_vec();
    for window in &windows {
        retain(env, *window);
    }
    let windows = ns_array::from_vec(env, windows);
    autorelease(env, windows)
}

- (())registerForRemoteNotificationTypes:(UIRemoteNotificationType)types {
    log!("TODO: ignoring registerForRemoteNotificationTypes:{}", types);
}

- (NSInteger)applicationIconBadgeNumber {
    0 // default value
}
- (())setApplicationIconBadgeNumber:(NSInteger)bn {
    log!("TODO: ignoring setApplicationIconBadgeNumber:{}", bn);
}

- (bool)applicationSupportsShakeToEdit {
    true // default value
}
- (())setApplicationSupportsShakeToEdit:(bool)enable {
    log!("TODO: ignoring setApplicationSupportsShakeToEdit:{}", enable);
}

// UIResponder implementation
// From the Apple UIView docs regarding [UIResponder nextResponder]:
// "The shared UIApplication object normally returns nil, but it returns its
//  app delegate if that object is a subclass of UIResponder and hasn’t
//  already been called to handle the event."
- (id)nextResponder {
    let delegate = msg![env; this delegate];
    let app_delegate_class = msg![env; delegate class];
    let ui_responder_class = env.objc.get_known_class("UIResponder", &mut env.mem);
    if env.objc.class_is_subclass_of(app_delegate_class, ui_responder_class) {
        // TODO: Send nil if it's already been called to handle the event
        delegate
    } else {
        nil
    }
}

- (())cancelAllLocalNotifications {
    log!("TODO: [(UIApplication*){:?} cancelAllLocalNotifications", this);
}
- (())scheduleLocalNotification:(id)local_notif { // UILocalNotification *
    log!("TODO: [(UIApplication*){:?} scheduleLocalNotification:{:?}", this, local_notif);
}

@end

};

/// `UIApplicationMain`, the entry point of the application.
///
/// This function should never return.
pub(super) fn UIApplicationMain(
    env: &mut Environment,
    _argc: i32,
    _argv: MutPtr<MutPtr<u8>>,
    principal_class_name: id, // NSString*
    delegate_class_name: id,  // NSString*
) {
    // UIKit creates and drains autorelease pools when handling events.
    // It's not clear what granularity this should happen with, but this
    // granularity has already caught several bugs. :)

    let ui_application = {
        let pool: id = msg_class![env; NSAutoreleasePool new];

        let principal_class = if principal_class_name != nil {
            let name = ns_string::to_rust_string(env, principal_class_name);
            env.objc.get_known_class(&name, &mut env.mem)
        } else {
            env.objc.get_known_class("UIApplication", &mut env.mem)
        };
        let ui_application: id = msg![env; principal_class new];

        let device_family = env.options.device_family;
        if let Some(main_nib_filename) = env.bundle.main_nib_filename(device_family) {
            let ns_main_nib_filename = from_rust_string(env, main_nib_filename.to_string());
            // We need to check first if main nib file exists,
            // as `UINib nibWithNibName:bundle:` will crash on nonexistent
            // nib otherwise
            let type_: id = get_static_str(env, "nib");
            let bundle: id = msg_class![env; NSBundle mainBundle];
            let res: id = msg![env; bundle pathForResource:ns_main_nib_filename ofType:type_];
            if res != nil {
                let nib: id = msg_class![env; UINib nibWithNibName:ns_main_nib_filename bundle:nil];
                release(env, ns_main_nib_filename);
                let top_level_objects: id = msg![env; nib instantiateWithOwner:ui_application
                                                                       options:nil];
                // The loader owns a nib's top-level objects. Discarding the
                // array left them owned by nothing but the autorelease pool, so
                // an app whose window comes from its main nib — rather than
                // building one in code — lost that window at the first drain,
                // taking its registration in `ui_view::ui_window::windows` with
                // it. Composition then found no windows and presented nothing
                // for the rest of the run, and every touch was discarded for
                // want of a window to hit-test. This is not a rare shape: it
                // is what every Xcode template produced for years.
                retain(env, top_level_objects);
                env.objc
                    .borrow_mut::<UIApplicationHostObject>(ui_application)
                    .main_nib_top_level_objects = top_level_objects;

                make_main_nib_window_key(env, top_level_objects);
            } else {
                log!(
                    "Warning: couldn't load main nib file {:?}",
                    env.bundle.main_nib_filename(device_family)
                );
            }
        }

        if env.bundle.status_bar_hidden() {
            let _: () = msg![env; ui_application setStatusBarHidden:true];
        }

        let delegate: id = msg![env; ui_application delegate];
        if delegate != nil {
            // The delegate was created while loading the nib file.
            // Retain it so it doesn't get deallocated when the autorelease pool
            // is drained. (See discussion in `setDelegate:`.)
            env.objc
                .borrow_mut::<UIApplicationHostObject>(ui_application)
                .delegate_is_retained = true;
            retain(env, delegate);
        } else if delegate_class_name == nil {
            // UIApplicationMain accepts a nil delegate class, and a nib that
            // sets no delegate is not an error either: an app that builds its
            // window in main() and never implements the delegate protocol is
            // unusual but legal, and UIKit simply leaves the delegate nil.
            // Messages to it are then messages to nil, which is exactly what
            // such an app expects.
            log!("No application delegate class was named and the nib set none; running without a delegate");
        } else if msg![env; delegate_class_name isEqual:principal_class_name] {
            // If same non-nil class name is used for both principal and
            // delegate, it means that app is using itself as a delegate
            let _: () = msg![env; ui_application setDelegate:ui_application];
        } else {
            // We have to construct the delegate.
            let name = ns_string::to_rust_string(env, delegate_class_name);
            let class = env.objc.get_known_class(&name, &mut env.mem);
            let delegate: id = msg![env; class new];
            let _: () = msg![env; ui_application setDelegate:delegate];
            assert!(delegate != nil);
        };
        // We can't hang on to the delegate, the guest app may change it at any
        // time.

        let _: () = msg![env; pool drain];

        ui_application
    };

    {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let delegate: id = msg![env; ui_application delegate];
        // iOS 3+ apps usually use application:didFinishLaunchingWithOptions:,
        // and it seems to be prioritized over applicationDidFinishLaunching:.
        if env.objc.object_has_method_named(
            &env.mem,
            delegate,
            "application:didFinishLaunchingWithOptions:",
        ) {
            let empty_dict: id = msg_class![env; NSDictionary dictionary];
            () = msg![env; delegate application:ui_application didFinishLaunchingWithOptions:empty_dict];
        } else if env.objc.object_has_method_named(
            &env.mem,
            delegate,
            "applicationDidFinishLaunching:",
        ) {
            () = msg![env; delegate applicationDidFinishLaunching:ui_application];
        }

        let center: id = msg_class![env; NSNotificationCenter defaultCenter];
        let notif_name = get_static_str(env, UIApplicationDidFinishLaunchingNotification);
        // TODO: launch options in `userInfo` if it'll ever become a concern
        () = msg![env; center postNotificationName:notif_name object:ui_application userInfo:nil];

        let _: () = msg![env; pool drain];
    }

    // Call layoutSubviews on all views in the view hierarchy.
    // See https://medium.com/geekculture/uiview-lifecycle-part-5-faa2d44511c9
    let views = env.framework_state.uikit.ui_view.views.clone();
    for view in views {
        () = msg![env; view layoutSubviews];
    }
    // From here on, a view added to a window can be laid out as soon as it is
    // mounted: the hierarchy the app builds during launch is complete.
    env.framework_state.uikit.ui_application.finished_launching = true;

    // Send applicationDidBecomeActive now that the application is ready to
    // become active.
    {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let delegate: id = msg![env; ui_application delegate];
        if env
            .objc
            .object_has_method_named(&env.mem, delegate, "applicationDidBecomeActive:")
        {
            () = msg![env; delegate applicationDidBecomeActive:ui_application];
        }

        let center: id = msg_class![env; NSNotificationCenter defaultCenter];
        let notif_name = get_static_str(env, UIApplicationDidBecomeActiveNotification);
        () = msg![env; center postNotificationName:notif_name object:ui_application userInfo:nil];

        let _: () = msg![env; pool drain];
    }

    // FIXME: There are more messages we should send.

    let run_loop: id = msg_class![env; NSRunLoop mainRunLoop];
    let _: () = msg![env; run_loop run];
}

/// Tell the app it's about to quit and then exit.
pub(super) fn exit(env: &mut Environment) {
    // A delegate may terminate the pthread carrying this callback. Preserve
    // the close request so the executor can still finish shutting down.
    env.request_shutdown();

    let ui_application: id = msg_class![env; UIApplication sharedApplication];

    let center: id = msg_class![env; NSNotificationCenter defaultCenter];

    {
        let pool: id = msg_class![env; NSAutoreleasePool new];

        // Apple's docs (used to) vaguely mention that `synchronize` is
        // invoked on periodic intervals.
        // Second best - and implemented here - is to save before app exits.
        // TODO: call `synchronize` periodically
        let user_defaults: id = msg_class![env; NSUserDefaults standardUserDefaults];
        let _: bool = msg![env; user_defaults synchronize];

        let delegate: id = msg![env; ui_application delegate];
        if env
            .objc
            .object_has_method_named(&env.mem, delegate, "applicationWillResignActive:")
        {
            () = msg![env; delegate applicationWillResignActive:ui_application];
        }

        let notif_name = get_static_str(env, UIApplicationWillResignActiveNotification);
        () = msg![env; center postNotificationName:notif_name object:ui_application userInfo:nil];

        let _: () = msg![env; pool drain];
    };

    {
        let pool: id = msg_class![env; NSAutoreleasePool new];
        let delegate: id = msg![env; ui_application delegate];
        if env
            .objc
            .object_has_method_named(&env.mem, delegate, "applicationWillTerminate:")
        {
            () = msg![env; delegate applicationWillTerminate:ui_application];
        }

        let notif_name = get_static_str(env, UIApplicationWillTerminateNotification);
        () = msg![env; center postNotificationName:notif_name object:ui_application userInfo:nil];

        let _: () = msg![env; pool drain];
    };

    // Ends the run rather than the process: the frontend is in this process
    // too, and a game closing must not close the window that launched it.
    env.exit_run(0);
}

/// App life-cycle notifications
const UIApplicationDidFinishLaunchingNotification: &str =
    "UIApplicationDidFinishLaunchingNotification";
const UIApplicationDidBecomeActiveNotification: &str = "UIApplicationDidBecomeActiveNotification";
const UIApplicationDidEnterBackgroundNotification: &str =
    "UIApplicationDidEnterBackgroundNotification";
const UIApplicationWillEnterForegroundNotification: &str =
    "UIApplicationWillEnterForegroundNotification";
const UIApplicationWillResignActiveNotification: &str = "UIApplicationWillResignActiveNotification";
const UIApplicationWillTerminateNotification: &str = "UIApplicationWillTerminateNotification";
/// Status bar notifications, and the `userInfo` keys they carry
const UIApplicationWillChangeStatusBarOrientationNotification: &str =
    "UIApplicationWillChangeStatusBarOrientationNotification";
const UIApplicationDidChangeStatusBarOrientationNotification: &str =
    "UIApplicationDidChangeStatusBarOrientationNotification";
const UIApplicationWillChangeStatusBarFrameNotification: &str =
    "UIApplicationWillChangeStatusBarFrameNotification";
const UIApplicationDidChangeStatusBarFrameNotification: &str =
    "UIApplicationDidChangeStatusBarFrameNotification";
const UIApplicationStatusBarOrientationUserInfoKey: &str =
    "UIApplicationStatusBarOrientationUserInfoKey";
const UIApplicationStatusBarFrameUserInfoKey: &str = "UIApplicationStatusBarFrameUserInfoKey";
/// Launch option keys. tapHLE always passes an empty launch options dictionary,
/// so looking any of these up yields nil, which is the same answer the real
/// thing gives for an ordinary launch from the home screen. They still have to
/// exist: an app that reads its launch options dereferences the key without
/// checking it, so leaving one unbound is a null pointer waiting to be read.
const UIApplicationLaunchOptionsURLKey: &str = "UIApplicationLaunchOptionsURLKey";
const UIApplicationLaunchOptionsSourceApplicationKey: &str =
    "UIApplicationLaunchOptionsSourceApplicationKey";
const UIApplicationLaunchOptionsRemoteNotificationKey: &str =
    "UIApplicationLaunchOptionsRemoteNotificationKey";
const UIApplicationLaunchOptionsLocalNotificationKey: &str =
    "UIApplicationLaunchOptionsLocalNotificationKey";
const UIApplicationLaunchOptionsAnnotationKey: &str = "UIApplicationLaunchOptionsAnnotationKey";
/// Other app notifications
const UIApplicationDidReceiveMemoryWarningNotification: &str =
    "UIApplicationDidReceiveMemoryWarningNotification";

/// `UIBackgroundTaskIdentifier`, the value meaning "no task". tapHLE has no
/// background execution, so `beginBackgroundTask...` has nothing to hand back,
/// but apps compare their stored identifier against this constant.
fn UIBackgroundTaskInvalid(env: &mut Environment) -> ConstVoidPtr {
    let invalid: NSUInteger = 0;
    env.mem.alloc_and_write(invalid).cast().cast_const()
}

/// `UIApplicationLaunchOptionsKey` and `NSNotificationName` values.
/// (Both types are strings)
pub const CONSTANTS: ConstantExports = &[
    (
        "_UIBackgroundTaskInvalid",
        HostConstant::Custom(UIBackgroundTaskInvalid),
    ),
    (
        "_UIApplicationDidFinishLaunchingNotification",
        HostConstant::NSString(UIApplicationDidFinishLaunchingNotification),
    ),
    (
        "_UIApplicationDidBecomeActiveNotification",
        HostConstant::NSString(UIApplicationDidBecomeActiveNotification),
    ),
    (
        "_UIApplicationDidEnterBackgroundNotification",
        HostConstant::NSString(UIApplicationDidEnterBackgroundNotification),
    ),
    (
        "_UIApplicationWillEnterForegroundNotification",
        HostConstant::NSString(UIApplicationWillEnterForegroundNotification),
    ),
    (
        "_UIApplicationWillResignActiveNotification",
        HostConstant::NSString(UIApplicationWillResignActiveNotification),
    ),
    (
        "_UIApplicationWillTerminateNotification",
        HostConstant::NSString(UIApplicationWillTerminateNotification),
    ),
    (
        "_UIApplicationDidReceiveMemoryWarningNotification",
        HostConstant::NSString(UIApplicationDidReceiveMemoryWarningNotification),
    ),
    (
        "_UIApplicationWillChangeStatusBarOrientationNotification",
        HostConstant::NSString(UIApplicationWillChangeStatusBarOrientationNotification),
    ),
    (
        "_UIApplicationDidChangeStatusBarOrientationNotification",
        HostConstant::NSString(UIApplicationDidChangeStatusBarOrientationNotification),
    ),
    (
        "_UIApplicationWillChangeStatusBarFrameNotification",
        HostConstant::NSString(UIApplicationWillChangeStatusBarFrameNotification),
    ),
    (
        "_UIApplicationDidChangeStatusBarFrameNotification",
        HostConstant::NSString(UIApplicationDidChangeStatusBarFrameNotification),
    ),
    (
        "_UIApplicationStatusBarOrientationUserInfoKey",
        HostConstant::NSString(UIApplicationStatusBarOrientationUserInfoKey),
    ),
    (
        "_UIApplicationStatusBarFrameUserInfoKey",
        HostConstant::NSString(UIApplicationStatusBarFrameUserInfoKey),
    ),
    (
        "_UIApplicationLaunchOptionsURLKey",
        HostConstant::NSString(UIApplicationLaunchOptionsURLKey),
    ),
    (
        "_UIApplicationLaunchOptionsSourceApplicationKey",
        HostConstant::NSString(UIApplicationLaunchOptionsSourceApplicationKey),
    ),
    (
        "_UIApplicationLaunchOptionsRemoteNotificationKey",
        HostConstant::NSString(UIApplicationLaunchOptionsRemoteNotificationKey),
    ),
    (
        "_UIApplicationLaunchOptionsLocalNotificationKey",
        HostConstant::NSString(UIApplicationLaunchOptionsLocalNotificationKey),
    ),
    (
        "_UIApplicationLaunchOptionsAnnotationKey",
        HostConstant::NSString(UIApplicationLaunchOptionsAnnotationKey),
    ),
];

pub const FUNCTIONS: FunctionExports = &[export_c_func!(UIApplicationMain(_, _, _, _))];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_bar_frame_uses_portrait_screen_coordinates() {
        assert_eq!(
            status_bar_frame(320.0, 480.0, true, DeviceOrientation::Portrait),
            CGRect::default()
        );
        assert_eq!(
            status_bar_frame(320.0, 480.0, false, DeviceOrientation::Portrait),
            CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize {
                    width: 320.0,
                    height: 20.0,
                },
            }
        );
        assert_eq!(
            status_bar_frame(320.0, 480.0, false, DeviceOrientation::LandscapeLeft),
            CGRect {
                origin: CGPoint { x: 300.0, y: 0.0 },
                size: CGSize {
                    width: 20.0,
                    height: 480.0,
                },
            }
        );
    }
}
