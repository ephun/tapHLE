/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIView`.
//!
//! Useful resources:
//! - Apple's [View Programming Guide for iOS](https://developer.apple.com/library/archive/documentation/WindowsViews/Conceptual/ViewPG_iPhoneOS/Introduction/Introduction.html)

pub mod ui_alert_view;
pub mod ui_control;
pub mod ui_image_view;
pub mod ui_label;
pub mod ui_picker_view;
pub mod ui_scroll_view;
pub mod ui_web_view;
pub mod ui_window;

use core::panic;

use super::ui_graphics::{UIGraphicsPopContext, UIGraphicsPushContext};
use crate::abi::CallFromHost;
use crate::frameworks::core_animation::ca_animation::{
    kCAFillModeBackwards, CAMediaTimingFillMode,
};
use crate::frameworks::core_animation::ca_media_timing_function::{
    kCAMediaTimingFunctionEaseIn, kCAMediaTimingFunctionEaseInEaseOut,
    kCAMediaTimingFunctionEaseOut, kCAMediaTimingFunctionLinear,
};
use crate::frameworks::core_animation::ca_transaction;
use crate::frameworks::core_animation::CACurrentMediaTime;
use crate::frameworks::core_foundation::time::CFTimeInterval;
use crate::frameworks::core_graphics::cg_affine_transform::CGAffineTransform;
use crate::frameworks::core_graphics::cg_color::CGColorRef;
use crate::frameworks::core_graphics::cg_context::{CGContextClearRect, CGContextRef};
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::{from_rust_string, get_static_str, to_rust_string};
use crate::frameworks::foundation::{ns_array, NSInteger, NSTimeInterval, NSUInteger};
use crate::mem::{ConstVoidPtr, GuestUSize};
use crate::objc::{
    autorelease, block_invoke_function, id, msg, msg_class, msg_send, nil, objc_classes, release,
    retain, todo_objc_setter, Class, ClassExports, HostObject, NSZonePtr, ObjC, SEL,
};
use crate::Environment;

// Internal keys used to store UIView animation parameters in the wrapped
// CATransaction created by beginAnimations, set by the various setAnimation*
// methods, and later read by commitAnimations.
const tapHLE_kCATransactionAnimationId: &str = "_tapHLE_kCATransactionAnimationId";
const tapHLE_kCATransactionAnimationContext: &str = "_tapHLE_kCATransactionAnimationContext";
const tapHLE_kCATransactionAnimationDelay: &str = "_tapHLE_kCATransactionAnimationDelay";
const tapHLE_kCATransactionAnimationRepeatCount: &str =
    "_tapHLE_kCATransactionAnimationRepeatCount";
const tapHLE_kCATransactionAnimationRepeatAutoreverses: &str =
    "_tapHLE_kCATransactionAnimationRepeatAutoreverses";
const tapHLE_kCATransactionAnimationDelegate: &str = "_tapHLE_kCATransactionAnimationDelegate";
const tapHLE_kCATransactionAnimationWillStartSelector: &str =
    "_tapHLE_kCATransactionAnimationWillStartSelector";
const tapHLE_kCATransactionAnimationDidStopSelector: &str =
    "_tapHLE_kCATransactionAnimationDidStopSelector";

type UIViewAnimationCurve = NSInteger;
const UIViewAnimationCurveEaseInOut: UIViewAnimationCurve = 0;
const UIViewAnimationCurveEaseIn: UIViewAnimationCurve = 1;
const UIViewAnimationCurveEaseOut: UIViewAnimationCurve = 2;
const UIViewAnimationCurveLinear: UIViewAnimationCurve = 3;

#[derive(Default)]
pub struct State {
    /// List of views for internal purposes. Non-retaining!
    pub(super) views: Vec<id>,
    pub ui_window: ui_window::State,
    pub animation_block_count: usize,
}

/// Public so that a UIView subclass living in another framework (for example
/// iAd's ADBannerView) can embed it as its superclass host object.
pub struct UIViewHostObject {
    /// CALayer or subclass.
    layer: id,
    /// Subviews in back-to-front order. These are strong references.
    subviews: Vec<id>,
    /// The superview. This is a weak reference.
    superview: id,
    /// The view controller that controls this view. This is a weak reference
    view_controller: id,
    tag: NSInteger,
    clears_context_before_drawing: bool,
    user_interaction_enabled: bool,
    multiple_touch_enabled: bool,
    /// `UIViewAutoresizing`. Stored and reported back, but not acted on: tapHLE
    /// does not resize subviews when a superview's bounds change.
    /// Round-tripping it still matters, because layout code reads the mask back
    /// to decide what to do — and now that views actually receive a layout
    /// pass, that happens.
    autoresizing_mask: NSUInteger,
    /// Likewise stored and reported back rather than acted on.
    autoresizes_subviews: bool,
    /// Set by `-setNeedsLayout`, cleared when the layout actually happens.
    needs_layout: bool,
}
impl HostObject for UIViewHostObject {}
impl Default for UIViewHostObject {
    fn default() -> UIViewHostObject {
        // The Default trait is implemented so subclasses will get the same
        // defaults.
        UIViewHostObject {
            layer: nil,
            subviews: Vec::new(),
            superview: nil,
            view_controller: nil,
            tag: 0,
            clears_context_before_drawing: true,
            user_interaction_enabled: true,
            multiple_touch_enabled: false,
            // UIViewAutoresizingNone.
            autoresizing_mask: 0,
            // UIKit's default is YES.
            autoresizes_subviews: true,
            needs_layout: false,
        }
    }
}

#[derive(Default)]
struct UIViewAnimationDelegateHostObject {
    animation_id: id, // NSString*
    context: ConstVoidPtr,
    delegate: id,
    will_start_selector: Option<SEL>,
    did_stop_selector: Option<SEL>,
    total_animation_count: u32,
    started_animation_count: u32,
    finished_animation_count: u32,
}
impl HostObject for UIViewAnimationDelegateHostObject {}

#[derive(Default)]
struct UIViewBlockCompletionHostObject {
    /// The completion block, retained. `void (^)(BOOL finished)`.
    completion: id,
}
impl HostObject for UIViewBlockCompletionHostObject {}

pub fn set_view_controller(env: &mut Environment, view: id, controller: id) {
    let host_obj = env.objc.borrow_mut::<UIViewHostObject>(view);
    host_obj.view_controller = controller;
}

/// Shared parts of `initWithCoder:` and `initWithFrame:`. These can't call
/// `init`: the subclass may have overridden `init` and will not expect to be
/// called here.
///
/// Do not call this in subclasses of `UIView`.
/// Whether `view_class` supplies its own `-drawRect:` below `UIView`.
///
/// `class_has_method_named` cannot answer this: it walks the whole chain, and
/// every view inherits `UIView`'s do-nothing `-drawRect:`, so it says yes for
/// all of them. Only a class that overrides it actually draws anything, and
/// only those need backing store — hence `class_has_uninherited_method`, and
/// the walk stopping at `UIView` rather than at the root.
fn draws_its_own_content(env: &mut Environment, view_class: Class) -> bool {
    let Some(draw_rect) = env.objc.lookup_selector("drawRect:") else {
        return false;
    };
    let ui_view_class = env.objc.get_known_class("UIView", &mut env.mem);
    let mut class = view_class;
    while class != nil && class != ui_view_class {
        if env.objc.class_has_uninherited_method(class, draw_rect) {
            return true;
        }
        class = env.objc.class_get_superclass(class);
    }
    false
}

fn init_common(env: &mut Environment, this: id) -> id {
    let view_class: Class = msg![env; this class];
    let layer_class: Class = msg![env; view_class layerClass];
    let layer: id = msg![env; layer_class layer];

    // CALayer is not opaque by default, but UIView is
    () = msg![env; layer setDelegate:this];
    () = msg![env; layer setOpaque:true];

    env.objc.borrow_mut::<UIViewHostObject>(this).layer = layer;

    // A new view draws itself once without being asked. A bare CALayer does
    // not — you must call `-setNeedsDisplay` — and tapHLE was applying the
    // layer's rule to views, so a view whose entire appearance comes from
    // `-drawRect:` stayed blank forever unless the app happened to invalidate
    // it for some other reason. Nothing in UIKit's contract makes an app do
    // that, so most never did.
    //
    // The Jim and Frank Mysteries HD is the measured case: Chillingo's Crystal
    // presents a full-screen `CCSkinnedView` over the game, that view's buttons
    // are drawn in `-drawRect:`, and across a whole startup `-drawRect:` was
    // never called even once. The splash therefore had no button to press, and
    // pressing one is the only thing that dismisses it.
    //
    // Restricted to classes that override `-drawRect:`, because displaying a
    // layer allocates a bitmap the size of the view in guest memory. Views that
    // draw nothing would pay that for an empty image.
    if draws_its_own_content(env, view_class) {
        () = msg![env; layer setNeedsDisplay];
    }

    env.framework_state.uikit.ui_view.views.push(this);

    this
}

/// Flag a newly mounted view for layout.
///
/// UIKit lays out every view in a window on the next turn of the run loop, and
/// the emphasis is on *next turn*: the layout does not happen inside
/// `-addSubview:`. tapHLE only ever laid out at launch and for a window's root
/// view, so a view added later never received `-layoutSubviews` at all — which
/// matters because the standard `EAGLView` creates its renderbuffer there, and
/// without it presents every frame into no drawable.
///
/// Doing it **synchronously** here, which is what this did first, breaks apps
/// outright: JellyCar 2 faulted during startup because its layout ran while the
/// view hierarchy was still being built, a moment the app never expects. Merely
/// flagging it, and letting `handle_pending_layout` service it on the next run
/// loop turn, is both what UIKit does and what both apps survive.
///
/// A view not yet in a window is flagged anyway; `handle_pending_layout` skips
/// it until it is mounted, so it is laid out once it has a real size rather
/// than at the wrong one.
pub(super) fn mark_needs_layout_on_mount(env: &mut Environment, view: id) {
    env.objc.borrow_mut::<UIViewHostObject>(view).needs_layout = true;

    // During launch, leave it at the flag. The app is still assembling its view
    // hierarchy and running its layout code now is what killed JellyCar 2.
    if !env.framework_state.uikit.ui_application.finished_launching {
        return;
    }

    // Afterwards, lay out immediately if the view is already in a window.
    // Waiting for the next run loop turn is closer to UIKit, but it costs a
    // frame — and for an EAGLView that frame is presented into no drawable,
    // which is how Tap Tap Revenge 2 lost its background: the first frames of
    // its game screen were drawn before the layout that creates the surface.
    let window: id = msg![env; view window];
    if window == nil {
        return;
    }
    env.objc.borrow_mut::<UIViewHostObject>(view).needs_layout = false;
    () = msg![env; view layoutSubviews];
}

/// For use by `NSRunLoop`: perform the layout that `-setNeedsLayout` deferred.
///
/// UIKit coalesces every request made during a turn of the run loop into one
/// layout pass at the end of it, which is the whole reason `-setNeedsLayout` is
/// separate from `-layoutSubviews`. Doing it here rather than at the call site
/// is what makes it safe for a view to ask for layout from inside its own
/// layout, and it means N requests cost one pass rather than N.
///
/// A view not in a window is left flagged rather than laid out, so it gets its
/// pass when it is eventually mounted instead of being laid out at the wrong
/// size and never revisited.
pub fn handle_pending_layout(env: &mut Environment) {
    let views = env.framework_state.uikit.ui_view.views.clone();
    for view in views {
        if !env.objc.borrow::<UIViewHostObject>(view).needs_layout {
            continue;
        }
        let window: id = msg![env; view window];
        if window == nil {
            continue;
        }
        env.objc.borrow_mut::<UIViewHostObject>(view).needs_layout = false;
        () = msg![env; view layoutSubviews];
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIView: UIResponder

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIViewHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (Class)layerClass {
    env.objc.get_known_class("CALayer", &mut env.mem)
}

+ (())setAnimationDuration:(NSTimeInterval)duration {
    log_dbg!("[UIView setAnimationDuration:{:?}]", duration);
    () = msg_class![env; CATransaction setAnimationDuration:duration];
}

+ (())setAnimationDelay:(NSTimeInterval)delay {
    log_dbg!("[UIView setAnimationDelay:{:?}]", delay);
    let value: id = msg_class![env; NSNumber numberWithDouble:delay];
    () = msg_class![env; CATransaction setValue:value forKey:(get_static_str(env, tapHLE_kCATransactionAnimationDelay))];
}

+ (())setAnimationCurve:(UIViewAnimationCurve)curve {
    log_dbg!("[UIView setAnimationCurve:{:?}]", curve);
    let timing_function: id = match curve {
        UIViewAnimationCurveEaseInOut => {
            msg_class![env; CAMediaTimingFunction functionWithName:
                (get_static_str(env, kCAMediaTimingFunctionEaseInEaseOut))]
        },
        UIViewAnimationCurveEaseIn => {
            msg_class![env; CAMediaTimingFunction functionWithName:
                (get_static_str(env, kCAMediaTimingFunctionEaseIn))]
        },
        UIViewAnimationCurveEaseOut => {
            msg_class![env; CAMediaTimingFunction functionWithName:
                (get_static_str(env, kCAMediaTimingFunctionEaseOut))]
        },
        UIViewAnimationCurveLinear => {
            msg_class![env; CAMediaTimingFunction functionWithName:
                (get_static_str(env, kCAMediaTimingFunctionLinear))]
        },
        _ => panic!("Unknown UIViewAnimationCurve {:?}", curve),
    };
    () = msg_class![env; CATransaction setAnimationTimingFunction:timing_function];
}

+ (())setAnimationRepeatAutoreverses:(bool)repeat_autoreverses {
    log_dbg!("[UIView setAnimationRepeatAutoreverses:{:?}]", repeat_autoreverses);
    let value: id = msg_class![env; NSNumber numberWithBool:repeat_autoreverses];
    () = msg_class![env; CATransaction setValue:value forKey:(get_static_str(env, tapHLE_kCATransactionAnimationRepeatAutoreverses))];
}

+ (())setAnimationRepeatCount:(f32)repeat_count {
    log_dbg!("[UIView setAnimationRepeatCount:{:?}]", repeat_count);
    assert!(repeat_count >= 0.0);
    let value: id = msg_class![env; NSNumber numberWithFloat:repeat_count];
    () = msg_class![env; CATransaction setValue:value forKey:(get_static_str(env, tapHLE_kCATransactionAnimationRepeatCount))];
}

+ (())setAnimationBeginsFromCurrentState:(bool)from_current_state {
    // This chooses whether an animation that interrupts another starts from the
    // interrupted one's presented values or from the model values. tapHLE does
    // not model an in-flight animation's presented state, so both answers are
    // the same here and there is nothing to record. The view still reaches the
    // right final state either way; only the path it takes could differ.
    log_dbg!("TODO: ignoring [UIView setAnimationBeginsFromCurrentState:{}]", from_current_state);
}

+ (())setAnimationTransition:(NSInteger)transition
                     forView:(id)view
                       cache:(bool)cache {
    // Flip and curl transitions. Ignoring the transition leaves the view
    // hierarchy in its correct final state, just without the effect.
    log_dbg!(
        "TODO: ignoring [UIView setAnimationTransition:{} forView:{:?} cache:{}]",
        transition, view, cache
    );
}

+ (bool)areAnimationsEnabled {
    let disabled: bool = msg_class![env; CATransaction disableActions];
    !disabled
}

+ (())setAnimationsEnabled:(bool)enabled {
    log_dbg!("[UIView setAnimationsEnabled:{}]", enabled);
    () = msg_class![env; CATransaction setDisableActions:(!enabled)];
}

+ (())setAnimationDelegate:(id)delegate {
    log_dbg!("[UIView setAnimationDelegate:{:?}]", delegate);
    retain(env, delegate);
    () = msg_class![env; CATransaction setValue:delegate forKey:(get_static_str(env, tapHLE_kCATransactionAnimationDelegate))];
}

+ (())setAnimationWillStartSelector:(SEL)selector {
    let selector_str = selector.as_str(&env.mem);
    log_dbg!("[UIView setAnimationWillStartSelector:{:?} ({})]", selector, selector_str);
    let selector_nsstring = from_rust_string(env, selector_str.to_string());
    () = msg_class![env; CATransaction setValue:selector_nsstring forKey:(get_static_str(env, tapHLE_kCATransactionAnimationWillStartSelector))];
}

+ (())setAnimationDidStopSelector:(SEL)selector {
    let selector_str = selector.as_str(&env.mem);
    log_dbg!("[UIView setAnimationDidStopSelector:{:?} ({})]", selector, selector_str);
    let selector_nsstring = from_rust_string(env, selector_str.to_string());
    () = msg_class![env; CATransaction setValue:selector_nsstring forKey:(get_static_str(env, tapHLE_kCATransactionAnimationDidStopSelector))];
}

// iOS 4's block-based animation API. It is defined in terms of the older
// begin/commit API, which already owns the transaction, delegate and timing
// machinery; the only new part is running the two blocks at the right moments.
//
// The completion block must not run until the animation has actually stopped —
// apps use it to remove a view that has just faded out — so it is delivered
// through the same animation-delegate path as setAnimationDidStopSelector:.
// When the animations block schedules nothing there is no animation to wait
// for and no delegate callback will ever arrive, so completion is called
// directly, which is also what UIKit does.
+ (())animateWithDuration:(NSTimeInterval)duration
               animations:(id)animations // block
               completion:(id)completion { // block, may be nil
    () = msg_class![env; UIView beginAnimations:nil context:(ConstVoidPtr::null())];
    () = msg_class![env; UIView setAnimationDuration:duration];

    if animations != nil {
        let invoke = block_invoke_function(env, animations);
        () = invoke.call_from_host(env, (animations,));
    }

    let scheduled_any = !ca_transaction::ThreadLocalState::get_current_transaction(env)
        .unwrap()
        .get_animations()
        .is_empty();

    let block_delegate = if completion != nil && scheduled_any {
        let block_delegate: id = msg_class![env; _tapHLE_UIView_BlockCompletion new];
        () = msg![env; block_delegate setCompletionBlock:completion];
        () = msg_class![env; UIView setAnimationDelegate:block_delegate];
        let selector = env
            .objc
            .lookup_selector("tapHLE_animationDidStop:finished:context:")
            .unwrap();
        () = msg_class![env; UIView setAnimationDidStopSelector:selector];
        block_delegate
    } else {
        nil
    };

    () = msg_class![env; UIView commitAnimations];

    if block_delegate != nil {
        // setAnimationDelegate: took its own reference.
        release(env, block_delegate);
    } else if completion != nil {
        let invoke = block_invoke_function(env, completion);
        () = invoke.call_from_host(env, (completion, true));
    }
}

+ (())animateWithDuration:(NSTimeInterval)duration
               animations:(id)animations { // block
    () = msg_class![env; UIView animateWithDuration:duration animations:animations completion:nil];
}

+ (())animateWithDuration:(NSTimeInterval)duration
                    delay:(NSTimeInterval)delay
                  options:(NSUInteger)_options
               animations:(id)animations // block
               completion:(id)completion { // block, may be nil
    // TODO: UIViewAnimationOptions (curve, autoreverse, repeat, allow user
    // interaction). Delay is honoured because the older API already supports
    // it.
    () = msg_class![env; UIView beginAnimations:nil context:(ConstVoidPtr::null())];
    () = msg_class![env; UIView setAnimationDuration:duration];
    () = msg_class![env; UIView setAnimationDelay:delay];

    if animations != nil {
        let invoke = block_invoke_function(env, animations);
        () = invoke.call_from_host(env, (animations,));
    }

    let scheduled_any = !ca_transaction::ThreadLocalState::get_current_transaction(env)
        .unwrap()
        .get_animations()
        .is_empty();

    let block_delegate = if completion != nil && scheduled_any {
        let block_delegate: id = msg_class![env; _tapHLE_UIView_BlockCompletion new];
        () = msg![env; block_delegate setCompletionBlock:completion];
        () = msg_class![env; UIView setAnimationDelegate:block_delegate];
        let selector = env
            .objc
            .lookup_selector("tapHLE_animationDidStop:finished:context:")
            .unwrap();
        () = msg_class![env; UIView setAnimationDidStopSelector:selector];
        block_delegate
    } else {
        nil
    };

    () = msg_class![env; UIView commitAnimations];

    if block_delegate != nil {
        release(env, block_delegate);
    } else if completion != nil {
        let invoke = block_invoke_function(env, completion);
        () = invoke.call_from_host(env, (completion, true));
    }
}

+ (())beginAnimations:(id)animation_id // NSString*
              context:(ConstVoidPtr)context {
    log_dbg!("[UIView beginAnimations:{:?} context:{:?}]", animation_id, context);
    () = msg_class![env; CATransaction begin];
    () = msg_class![env; CATransaction setValue:animation_id forKey:(get_static_str(env, tapHLE_kCATransactionAnimationId))];
    if !context.is_null() {
        let context: id = msg_class![env; NSNumber numberWithUnsignedInt:(context.to_bits())];
        () = msg_class![env; CATransaction setValue:context forKey:(get_static_str(env, tapHLE_kCATransactionAnimationContext))];
    }
    // Default values
    () = msg_class![env; UIView setAnimationDuration:0.2];
    () = msg_class![env; UIView setAnimationCurve:UIViewAnimationCurveEaseInOut];

    env.framework_state.uikit.ui_view.animation_block_count += 1;
}

+ (())commitAnimations {
    log_dbg!("[UIView commitAnimations]");

    // TODO: What if there's interleaved UIView animations and CATransactions?
    let animations = ca_transaction::ThreadLocalState::get_current_transaction(env).unwrap().get_animations();

    let delegate: id = msg_class![env; CATransaction valueForKey:(get_static_str(env, tapHLE_kCATransactionAnimationDelegate))];
    if animations.is_empty() && delegate == nil {
        log_dbg!("[UIView commitAnimations] with no animations and no delegate, skipping");
    } else {
        // Even if the animation block is committed with no animations,
        // we still proceed so the delegate gets called
        let animation_delegate = if delegate == nil {
            nil
        } else {
            let animation_delegate = msg_class![env; _tapHLE_UIView_AnimationDelegate new];
            () = msg![env; animation_delegate setDelegate:delegate];
            let animation_id: id = msg_class![env; CATransaction valueForKey:(get_static_str(env, tapHLE_kCATransactionAnimationId))];
            () = msg![env; animation_delegate setAnimationId:animation_id];
            let context: id = msg_class![env; CATransaction valueForKey:(get_static_str(env, tapHLE_kCATransactionAnimationContext))];
            if context != nil {
                let context: u32 = msg![env; context unsignedIntValue];
                let context: ConstVoidPtr = ConstVoidPtr::from_bits(context as GuestUSize);
                () = msg![env; animation_delegate setContext:context];
            }
            let will_start_selector: id = msg_class![env; CATransaction valueForKey:(get_static_str(env, tapHLE_kCATransactionAnimationWillStartSelector))];
            if will_start_selector != nil {
                let will_start_selector = to_rust_string(env, will_start_selector);
                let will_start_selector = env.objc.lookup_selector(&will_start_selector).unwrap();
                () = msg![env; animation_delegate setWillStartSelector:will_start_selector];
            }
            let did_stop_selector: id = msg_class![env; CATransaction valueForKey:(get_static_str(env, tapHLE_kCATransactionAnimationDidStopSelector))];
            if did_stop_selector != nil {
                let did_stop_selector = to_rust_string(env, did_stop_selector);
                let did_stop_selector = env.objc.lookup_selector(&did_stop_selector).unwrap();
                () = msg![env; animation_delegate setDidStopSelector:did_stop_selector];
            }
            let total_animation_count = animations.len() as u32;
            () = msg![env; animation_delegate setTotalAnimationCount:total_animation_count];
            animation_delegate
        };
        let delay: id = msg_class![env; CATransaction valueForKey:(get_static_str(env, tapHLE_kCATransactionAnimationDelay))];
        let repeat_count: id = msg_class![env; CATransaction valueForKey:(get_static_str(env, tapHLE_kCATransactionAnimationRepeatCount))];
        let repeat_autoreverses: id = msg_class![env; CATransaction valueForKey:(get_static_str(env, tapHLE_kCATransactionAnimationRepeatAutoreverses))];
        for (layer, animation) in animations {
            log_dbg!("[UIView commitAnimations] adding animation {:?} to layer {:?}", animation, layer);
            () = msg![env; animation setDelegate:animation_delegate];
            if delay != nil {
                let delay: f32 = msg![env; delay floatValue];
                let begin_time: CFTimeInterval = CACurrentMediaTime(env) + delay as f64;
                () = msg![env; animation setBeginTime:begin_time];
                let fill_mode: CAMediaTimingFillMode = get_static_str(env, kCAFillModeBackwards);
                () = msg![env; animation setFillMode:fill_mode];
            }
            if repeat_count != nil {
                let repeat_count: f32 = msg![env; repeat_count floatValue];
                () = msg![env; animation setRepeatCount:repeat_count];
            }
            if repeat_autoreverses != nil {
                let repeat_autoreverses: bool = msg![env; repeat_autoreverses boolValue];
                () = msg![env; animation setAutoreverses:repeat_autoreverses];
            }
        }
    }

    () = msg_class![env; CATransaction commit];

    env.framework_state.uikit.ui_view.animation_block_count -= 1;
}

// TODO: accessors etc

// initWithCoder: and initWithFrame: are basically UIView's designated
// initializers. init is not, it's a shortcut for the latter.
// Subclasses need to override both.

- (id)init {
    msg![env; this initWithFrame:(<CGRect as Default>::default())]
}

- (id)initWithFrame:(CGRect)frame {
    let this = init_common(env, this);

    () = msg![env; this setFrame:frame];

    log_dbg!(
        "[(UIView*){:?} initWithFrame:{:?}] => bounds {:?}, center {:?}",
        this,
        frame,
        { let bounds: CGRect = msg![env; this bounds]; bounds },
        { let center: CGPoint = msg![env; this center]; center },
    );

    this
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    let this = init_common(env, this);

    // TODO: decode the various other UIView properties

    let key_ns_string = get_static_str(env, "UIBounds");
    let bounds: CGRect = msg![env; coder decodeCGRectForKey:key_ns_string];

    let key_ns_string = get_static_str(env, "UICenter");
    let center: CGPoint = msg![env; coder decodeCGPointForKey:key_ns_string];

    let key_ns_string = get_static_str(env, "UIHidden");
    let hidden: bool = msg![env; coder decodeBoolForKey:key_ns_string];

    let key_ns_string = get_static_str(env, "UIOpaque");
    let opaque: bool = msg![env; coder decodeBoolForKey:key_ns_string];

    let key_ns_string = get_static_str(env, "UIBackgroundColor");
    let bg_color: id = msg![env; coder decodeObjectForKey:key_ns_string];

    let key_ns_string = get_static_str(env, "UITag");
    let tag: NSInteger = msg![env; coder decodeIntegerForKey:key_ns_string];

    let key_ns_string = get_static_str(env, "UIMultipleTouchEnabled");
    let multi_touch_enabled: bool = msg![env; coder decodeBoolForKey:key_ns_string];

    // NIB archives store the inverse of UIView's public property. Do not
    // decode a missing key as false: subclasses such as UIImageView have a
    // different default for user interaction and must keep that default when
    // the archive does not override it.
    let user_interaction_disabled_key = get_static_str(env, "UIUserInteractionDisabled");
    let has_user_interaction_override: bool =
        msg![env; coder containsValueForKey:user_interaction_disabled_key];
    let user_interaction_disabled = has_user_interaction_override
        && msg![env; coder decodeBoolForKey:user_interaction_disabled_key];

    let key_ns_string = get_static_str(env, "UISubviews");
    let subviews: id = msg![env; coder decodeObjectForKey:key_ns_string];
    let subview_count: NSUInteger = msg![env; subviews count];

    log_dbg!(
        "[(UIView*){:?} initWithCoder:{:?}] => bounds {}, center {}, hidden {}, bg color {:?}, tag {}, opaque {}, multi touch enabled {}, {} subviews",
        this,
        coder,
        bounds,
        center,
        hidden,
        bg_color,
        tag,
        opaque,
        multi_touch_enabled,
        subview_count,
    );

    () = msg![env; this setBounds:bounds];
    () = msg![env; this setCenter:center];
    () = msg![env; this setHidden:hidden];
    () = msg![env; this setOpaque:opaque];
    () = msg![env; this setBackgroundColor:bg_color];
    () = msg![env; this setTag:tag];
    () = msg![env; this setMultipleTouchEnabled:multi_touch_enabled];
    if has_user_interaction_override {
        let user_interaction_enabled = !user_interaction_disabled;
        () = msg![env; this setUserInteractionEnabled:user_interaction_enabled];
    }

    for i in 0..subview_count {
        let subview: id = msg![env; subviews objectAtIndex:i];
        () = msg![env; this addSubview:subview];
    }

    this
}

- (NSInteger)tag {
    env.objc.borrow::<UIViewHostObject>(this).tag
}
- (())setTag:(NSInteger)tag {
    env.objc.borrow_mut::<UIViewHostObject>(this).tag = tag;
}

- (id)viewWithTag:(NSInteger)tag {
    let &UIViewHostObject {
        ref subviews,
        tag: view_tag,
        ..
    } = env.objc.borrow(this);
    if view_tag == tag {
        return this;
    }
    for view in subviews {
        if env.objc.borrow::<UIViewHostObject>(*view).tag == tag {
            return *view;
        }
    }
    nil
}

- (bool)isUserInteractionEnabled {
    env.objc.borrow::<UIViewHostObject>(this).user_interaction_enabled
}
- (())setUserInteractionEnabled:(bool)enabled {
    env.objc.borrow_mut::<UIViewHostObject>(this).user_interaction_enabled = enabled;
}

- (bool)isMultipleTouchEnabled {
    env.objc.borrow::<UIViewHostObject>(this).multiple_touch_enabled
}
- (())setMultipleTouchEnabled:(bool)enabled {
    env.objc.borrow_mut::<UIViewHostObject>(this).multiple_touch_enabled = enabled;
}

- (())setExclusiveTouch:(bool)exclusive {
    log!("TODO: ignoring setExclusiveTouch:{} for view {:?}", exclusive, this);
}

- (())layoutSubviews {
    // On iOS 5.1 and earlier, the default implementation of this method does
    // nothing.
}

- (id)superview {
    env.objc.borrow::<UIViewHostObject>(this).superview
}

- (id)window {
    // Looks up window in the superview hierarchy
    // TODO: cache the result somehow?
    let mut window: id = env.objc.borrow::<UIViewHostObject>(this).superview;
    let window_class = env.objc.get_known_class("UIWindow", &mut env.mem);
    while window != nil {
        let current_class: Class = msg![env; window class];
        log_dbg!("maybe window {:?} curr class {}", window, env.objc.get_class_name(current_class));
        if env.objc.class_is_subclass_of(current_class, window_class) {
            break;
        }
        window = env.objc.borrow::<UIViewHostObject>(window).superview;
    }
    log_dbg!("view {:?} has window {:?}", this, window);
    window
}

- (id)subviews {
    let views = env.objc.borrow::<UIViewHostObject>(this).subviews.clone();
    for view in &views {
        retain(env, *view);
    }
    let subs = ns_array::from_vec(env, views);
    autorelease(env, subs)
}

- (())addSubview:(id)view {
    if crate::log::debug_enabled_for(module_path!()) {
        fn describe(env: &Environment, o: id) -> String {
            if o == nil {
                return "nil".to_string();
            }
            let class = ObjC::read_isa(o, &env.mem);
            let name = env.objc.get_class_name(class);
            let layer = env.objc.borrow::<UIViewHostObject>(o).layer;
            if layer == nil {
                return format!("{name} {o:?} (no layer)");
            }
            let layer_class = ObjC::read_isa(layer, &env.mem);
            let layer_name = env.objc.get_class_name(layer_class);
            format!("{name} {o:?} (layer {layer_name} {layer:?})")
        }
        let this_desc = describe(env, this);
        let view_desc = describe(env, view);
        log_dbg!("MOUNT [{this_desc} addSubview:{view_desc}]");
    }
    log_dbg!("[(UIView*){:?} addSubview:{:?}] => ()", this, view);

    if view == nil {
        log_dbg!("Tolerating [(UIView*){:?} addSubview:nil]", this);
        return;
    }

    if env.objc.borrow::<UIViewHostObject>(view).superview == this {
        () = msg![env; this bringSubviewToFront:view];
    } else {
        retain(env, view);
        () = msg![env; view removeFromSuperview];
        let subview_obj = env.objc.borrow_mut::<UIViewHostObject>(view);
        subview_obj.superview = this;
        let subview_layer = subview_obj.layer;
        let this_obj = env.objc.borrow_mut::<UIViewHostObject>(this);
        this_obj.subviews.push(view);
        let this_layer = this_obj.layer;
        () = msg![env; this_layer addSublayer:subview_layer];
        mark_needs_layout_on_mount(env, view);
    }
}

- (())insertSubview:(id)view atIndex:(NSInteger)index {
    assert!(view != nil);
    retain(env, view);
    () = msg![env; view removeFromSuperview];

    let subview_obj = env.objc.borrow_mut::<UIViewHostObject>(view);
    subview_obj.superview = this;
    let subview_layer = subview_obj.layer;

    let &mut UIViewHostObject {
        ref mut subviews,
        layer: this_layer,
        ..
    } = env.objc.borrow_mut(this);

    subviews.insert(index as usize, view);

    assert!(index >= 0);
    () = msg![env; this_layer insertSublayer:subview_layer atIndex:(index as u32)];
    mark_needs_layout_on_mount(env, view);
}

- (())insertSubview:(id)view belowSubview:(id)sibling {
    retain(env, view);
    () = msg![env; view removeFromSuperview];

    let subview_obj = env.objc.borrow_mut::<UIViewHostObject>(view);
    subview_obj.superview = this;
    let subview_layer = subview_obj.layer;

    let sibling_layer = env.objc.borrow_mut::<UIViewHostObject>(sibling).layer;

    let &mut UIViewHostObject {
        ref mut subviews,
        layer: this_layer,
        ..
    } = env.objc.borrow_mut(this);

    let idx = subviews.iter().position(|&subview2| subview2 == sibling).unwrap();
    subviews.insert(idx, view);

    () = msg![env; this_layer insertSublayer:subview_layer below:sibling_layer];
    mark_needs_layout_on_mount(env, view);
}

- (())bringSubviewToFront:(id)subview {
    if subview == nil {
        // This happens in Touch & Go LITE. It's probably due to the ad classes
        // being replaced with fakes.
        log_dbg!("Tolerating [{:?} bringSubviewToFront:nil]", this);
        return;
    }

    let &mut UIViewHostObject {
        ref mut subviews,
        layer,
        ..
    } = env.objc.borrow_mut(this);

    let Some(idx) = subviews.iter().position(|&subview2| subview2 == subview) else {
        log_dbg!("Warning: Unable to find the subview {:?} in subviews of {:?}", subview, this);
        return;
    };
    let subview2 = subviews.remove(idx);
    assert!(subview2 == subview);
    subviews.push(subview);

    let subview_layer = env.objc.borrow::<UIViewHostObject>(subview).layer;
    () = msg![env; subview_layer removeFromSuperlayer];
    () = msg![env; layer addSublayer:subview_layer];
}

- (())sendSubviewToBack:(id)subview {
    if subview == nil {
        log_dbg!("Tolerating [{:?} sendSubviewToBack:nil]", this);
        return;
    }

    let &mut UIViewHostObject {
        ref mut subviews,
        layer,
        ..
    } = env.objc.borrow_mut(this);

    let Some(idx) = subviews.iter().position(|&subview2| subview2 == subview) else {
        log_dbg!("Warning: Unable to find the subview {:?} in subviews of {:?}", subview, this);
        return;
    };
    let subview2 = subviews.remove(idx);
    assert!(subview2 == subview);
    subviews.insert(0, subview);

    let subview_layer = env.objc.borrow::<UIViewHostObject>(subview).layer;
    () = msg![env; subview_layer removeFromSuperlayer];
    () = msg![env; layer insertSublayer:subview_layer atIndex:0u32];
}

- (())removeFromSuperview {
    let &mut UIViewHostObject {
        ref mut superview,
        layer: this_layer,
        ..
    } = env.objc.borrow_mut(this);
    let superview = std::mem::take(superview);
    if superview == nil {
        return;
    }
    () = msg![env; this_layer removeFromSuperlayer];

    let UIViewHostObject { ref mut subviews, .. } = env.objc.borrow_mut(superview);
    let idx = subviews.iter().position(|&subview| subview == this).unwrap();
    let subview = subviews.remove(idx);
    assert!(subview == this);
    release(env, this);
}

- (())dealloc {
    let UIViewHostObject {
        layer,
        superview,
        subviews,
        view_controller,
        tag: _,
        clears_context_before_drawing: _,
        user_interaction_enabled: _,
        multiple_touch_enabled: _,
        autoresizing_mask: _,
        autoresizes_subviews: _,
        needs_layout: _,
    } = std::mem::take(env.objc.borrow_mut(this));

    release(env, layer);
    assert!(view_controller == nil);
    assert!(superview == nil);
    for subview in subviews {
        env.objc.borrow_mut::<UIViewHostObject>(subview).superview = nil;
        release(env, subview);
    }

    let state = &mut env.framework_state.uikit.ui_view.views;
    state.swap_remove(
        state.iter().position(|&v| v == this).unwrap()
    );

    env.objc.dealloc_object(this, &mut env.mem);
}

- (id)layer {
    env.objc.borrow_mut::<UIViewHostObject>(this).layer
}

- (bool)isHidden {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer isHidden]
}
- (())setHidden:(bool)hidden {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer setHidden:hidden]
}

// UIKit's clipsToBounds is the view-level name for the layer's masksToBounds,
// so forward rather than storing a second copy. The compositor does not clip
// yet; see the TODO in the composition module.
- (bool)clipsToBounds {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer masksToBounds]
}
- (())setClipsToBounds:(bool)clips {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer setMasksToBounds:clips]
}

- (bool)isOpaque {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer isOpaque]
}
- (())setOpaque:(bool)opaque {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer setOpaque:opaque]
}

- (CGFloat)alpha {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer opacity]
}
- (())setAlpha:(CGFloat)alpha {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer setOpacity:alpha]
}

- (id)backgroundColor {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    let cg_color: CGColorRef = msg![env; layer backgroundColor];
    msg_class![env; UIColor colorWithCGColor:cg_color]
}
- (())setBackgroundColor:(id)color { // UIColor*
    let color: CGColorRef = msg![env; color CGColor];
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer setBackgroundColor:color]
}

// TODO: support setNeedsDisplayInRect:
- (())setNeedsDisplay {
    // UIView has a method called drawRect: that subclasses override if they
    // need custom drawing. tapHLE's UIView (a CALayerDelegate) provides
    // an implementation of drawLayer:inContext: that calls drawRect:.
    // This maintains a clean separation of UIView and CALayer.
    //
    // To avoid wasting space and time on unnecessary bitmaps and drawing,
    // let's optimize here by only marking the layer as needing display if
    // the UIView's subclass overrides drawRect: or drawLayer:inContext:.
    let this_class = ObjC::read_isa(this, &env.mem);

    let ui_view_class = env.objc.get_known_class("UIView", &mut env.mem);

    let draw_layer_sel = env.objc.lookup_selector("drawLayer:inContext:").unwrap();
    let draw_rect_sel = env.objc.lookup_selector("drawRect:").unwrap();

    if env
        .objc
        .class_overrides_method_of_superclass(this_class, draw_rect_sel, ui_view_class)
        || env
            .objc
            .class_overrides_method_of_superclass(this_class, draw_layer_sel, ui_view_class)
    {
        let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
        msg![env; layer setNeedsDisplay]
    }
}

- (CGRect)bounds {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer bounds]
}
- (())setBounds:(CGRect)bounds {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer setBounds:bounds]
}
- (CGPoint)center {
    // FIXME: what happens if [layer anchorPoint] isn't (0.5, 0.5)?
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer position]
}
- (())setCenter:(CGPoint)center {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer setPosition:center]
}
- (CGRect)frame {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer frame]
}
- (())setFrame:(CGRect)frame {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer setFrame:frame]
}
- (CGAffineTransform)transform {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer affineTransform]
}
- (())setTransform:(CGAffineTransform)transform {
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer setAffineTransform:transform]
}

- (())setContentMode:(NSInteger)content_mode { // should be UIViewContentMode
    todo_objc_setter!(this, content_mode);
}

- (bool)clearsContextBeforeDrawing {
    env.objc.borrow::<UIViewHostObject>(this).clears_context_before_drawing
}
- (())setClearsContextBeforeDrawing:(bool)v {
    env.objc.borrow_mut::<UIViewHostObject>(this).clears_context_before_drawing = v;
}

// Drawing stuff that views should override
- (())drawRect:(CGRect)_rect {
    // default implementation does nothing
}

// CALayerDelegate implementation
- (())drawLayer:(id)layer // CALayer*
      inContext:(CGContextRef)context {
    let mut bounds: CGRect = msg![env; layer bounds];
    bounds.origin = CGPoint { x: 0.0, y: 0.0 }; // FIXME: not tested
    if env.objc.borrow::<UIViewHostObject>(this).clears_context_before_drawing {
        CGContextClearRect(env, context, bounds);
    }
    UIGraphicsPushContext(env, context);
    () = msg![env; this drawRect:bounds];
    UIGraphicsPopContext(env);
}

// Event handling

- (bool)pointInside:(CGPoint)point
          withEvent:(id)_event { // UIEvent* (possibly nil)
    let layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    msg![env; layer containsPoint:point]
}

- (id)hitTest:(CGPoint)point
    withEvent:(id)event { // UIEvent* (possibly nil)
    if !msg![env; this pointInside:point withEvent:event] {
        return nil;
    }
    // TODO: avoid copy somehow?
    let subviews = env.objc.borrow::<UIViewHostObject>(this).subviews.clone();
    for subview in subviews.into_iter().rev() { // later views are on top
        let hidden: bool = msg![env; subview isHidden];
        let alpha: CGFloat = msg![env; subview alpha];
        let interactible: bool = msg![env; subview isUserInteractionEnabled];
        if hidden || alpha < 0.01 || !interactible {
           continue;
        }
        let point: CGPoint = msg![env; subview convertPoint:point fromView:this];
        let subview: id = msg![env; subview hitTest:point withEvent:event];
        if subview != nil {
            return subview;
        }
    }
    this
}

// Ending a view-editing session

- (bool)endEditing:(bool)force {
    assert!(force);
    let responder: id = env.framework_state.uikit.ui_responder.first_responder;
    let class = msg![env; responder class];
    let ui_text_field_class = env.objc.get_known_class("UITextField", &mut env.mem);
    if responder != nil && env.objc.class_is_subclass_of(class, ui_text_field_class) {
        // we need to check if text field is in the current view hierarchy
        let mut to_find = responder;
        while to_find != nil {
            if to_find == this {
                return msg![env; responder resignFirstResponder];
            }
            to_find = msg![env; to_find superview];
        }
    }
    false
}

// UIResponder implementation
// From the Apple UIView docs regarding [UIResponder nextResponder]:
// "UIView implements this method and returns the UIViewController object that
//  manages it (if it has one) or its superview (if it doesn’t)."
- (id)nextResponder {
    let host_object = env.objc.borrow::<UIViewHostObject>(this);
    if host_object.view_controller != nil {
        host_object.view_controller
    } else {
        host_object.superview
    }
}

// Co-ordinate space conversion
//
// A nil counterpart means the window's co-ordinate space. It does not require
// the receiver to be in a window: CALayer's conversion already resolves a nil
// layer to the top of the receiver's layer hierarchy, which is the window's
// layer whenever there is a window, and the highest ancestor otherwise. Passing
// the nil straight down therefore gives the same answer for a view in a window
// and a defined one for a view that is not in one yet, which is what UIKit does
// and what a nib-loaded view laying itself out before it is mounted needs.

- (CGPoint)convertPoint:(CGPoint)point
               fromView:(id)other { // UIView*
    let this_layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    let other_layer = view_layer_or_nil(env, other);
    msg![env; this_layer convertPoint:point fromLayer:other_layer]
}
- (CGPoint)convertPoint:(CGPoint)point
                 toView:(id)other { // UIView*
    let this_layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    let other_layer = view_layer_or_nil(env, other);
    msg![env; this_layer convertPoint:point toLayer:other_layer]
}
- (CGRect)convertRect:(CGRect)rect
             fromView:(id)other { // UIView*
    let this_layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    let other_layer = view_layer_or_nil(env, other);
    msg![env; this_layer convertRect:rect fromLayer:other_layer]
}
- (CGRect)convertRect:(CGRect)rect
               toView:(id)other { // UIView*
    let this_layer = env.objc.borrow::<UIViewHostObject>(this).layer;
    let other_layer = view_layer_or_nil(env, other);
    msg![env; this_layer convertRect:rect toLayer:other_layer]
}

// Stored and reported back, but not acted on: tapHLE does not resize subviews
// when a superview's bounds change. That is a real gap and a view relying on it
// will be laid out wrongly — but silently discarding the value was worse, since
// a view that reads its own mask back got a different answer than it set, and
// UIKit code branches on that.
- (())setAutoresizingMask:(NSUInteger)mask {
    log_dbg!("TODO: [(UIView*){:?} setAutoresizingMask:{}] is stored but not applied", this, mask);
    env.objc.borrow_mut::<UIViewHostObject>(this).autoresizing_mask = mask;
}
// Deferred, as UIKit defers it: the layout happens on the next turn of the run
// loop, not inside this call.
//
// That distinction is not pedantry here. Laying out synchronously would recurse
// without bound the moment any view calls -setNeedsLayout from inside its own
// -layoutSubviews, which is ordinary, correct UIKit code — a control that
// resizes itself in response to its content does exactly that.
- (())setNeedsLayout {
    env.objc.borrow_mut::<UIViewHostObject>(this).needs_layout = true;
}

// The escape hatch from the deferral: lay out now, whether or not anything
// asked for it. UIKit walks up to the top of the hierarchy first; tapHLE lays
// out this view and its subtree, which is what callers use it for (measure a
// view before reading its frame).
- (())layoutIfNeeded {
    env.objc.borrow_mut::<UIViewHostObject>(this).needs_layout = false;
    () = msg![env; this layoutSubviews];
}

- (NSUInteger)autoresizingMask {
    env.objc.borrow::<UIViewHostObject>(this).autoresizing_mask
}
- (())setAutoresizesSubviews:(bool)enabled {
    log_dbg!("TODO: [(UIView*){:?} setAutoresizesSubviews:{}] is stored but not applied", this, enabled);
    env.objc.borrow_mut::<UIViewHostObject>(this).autoresizes_subviews = enabled;
}
- (bool)autoresizesSubviews {
    env.objc.borrow::<UIViewHostObject>(this).autoresizes_subviews
}

- (CGSize)sizeThatFits:(CGSize)size {
    // default implementation, subclasses can override
    size
}
- (())sizeToFit {
    log!("TODO: [(UIView *){:?} sizeToFit]", this);
}

- (())setContentScaleFactor:(CGFloat)factor {
    todo_objc_setter!(this, factor);
}
- (CGFloat)contentScaleFactor {
    1.0 // TODO
}

@end

// Adapts the block-based animation API's completion block to the older
// delegate-and-selector callback, so both routes share one implementation of
// "when has the animation actually stopped?".
@implementation _tapHLE_UIView_BlockCompletion: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIViewBlockCompletionHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (())setCompletionBlock:(id)block {
    // The caller's completion block is normally a *stack* block literal, valid
    // only while +animateWithDuration:... is on the stack. This object outlives
    // that call — the completion runs when the animation stops, from the run
    // loop — so the block must be copied to the heap, not merely retained.
    // Retaining a stack block leaves its invoke pointer pointing into a dead
    // frame, which shows up as a branch to a null PC much later.
    let block: id = if block == nil { nil } else { msg![env; block copy] };
    let host_object = env.objc.borrow_mut::<UIViewBlockCompletionHostObject>(this);
    let old = std::mem::replace(&mut host_object.completion, block);
    release(env, old);
}

// The old delegate callback passes `finished` boxed in an NSNumber, which is
// UIKit's documented signature for it.
- (())tapHLE_animationDidStop:(id)_animation_id // NSString*
                     finished:(id)finished // NSNumber*
                      context:(ConstVoidPtr)_context {
    let finished: bool = if finished == nil { true } else { msg![env; finished boolValue] };
    let completion = env.objc.borrow::<UIViewBlockCompletionHostObject>(this).completion;
    if completion != nil {
        let invoke = block_invoke_function(env, completion);
        () = invoke.call_from_host(env, (completion, finished));
    }
}

- (())dealloc {
    let completion = env.objc.borrow::<UIViewBlockCompletionHostObject>(this).completion;
    release(env, completion);
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

@implementation _tapHLE_UIView_AnimationDelegate: NSObject

+ (id)alloc {
    let host_object = Box::<UIViewAnimationDelegateHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (())setAnimationId:(id)animation_id { // NSString*
    retain(env, animation_id);
    env.objc.borrow_mut::<UIViewAnimationDelegateHostObject>(this).animation_id = animation_id;
}

- (())setContext:(ConstVoidPtr)context {
    env.objc.borrow_mut::<UIViewAnimationDelegateHostObject>(this).context = context;
}

- (())setDelegate:(id)delegate {
    retain(env, delegate);
    env.objc.borrow_mut::<UIViewAnimationDelegateHostObject>(this).delegate = delegate;
}

- (())setWillStartSelector:(SEL)selector {
    env.objc.borrow_mut::<UIViewAnimationDelegateHostObject>(this).will_start_selector = Some(selector);
}

- (())setDidStopSelector:(SEL)selector {
    env.objc.borrow_mut::<UIViewAnimationDelegateHostObject>(this).did_stop_selector = Some(selector);
}

- (())setTotalAnimationCount:(NSUInteger)count {
    env.objc.borrow_mut::<UIViewAnimationDelegateHostObject>(this).total_animation_count = count;
}

- (())dealloc {
    let UIViewAnimationDelegateHostObject {
        animation_id,
        delegate,
        ..
    } = *env.objc.borrow::<UIViewAnimationDelegateHostObject>(this);
    release(env, animation_id);
    release(env, delegate);
    env.objc.dealloc_object(this, &mut env.mem)
}

// CAAnimationDelegate protocol implementation
- (())animationDidStart:(id)animation { // CAAnimation*
    let UIViewAnimationDelegateHostObject {
        started_animation_count,
        delegate,
        will_start_selector,
        context,
        animation_id,
        ..
    } = *env.objc.borrow::<UIViewAnimationDelegateHostObject>(this);
    let new_started_animation_count = started_animation_count + 1;
    log_dbg!("[(_tapHLE_UIView_AnimationDelegate*){:?} animationDidStart:{:?}] started_animation_count {} -> {}", this, animation, started_animation_count, new_started_animation_count);
    if started_animation_count == 0 && delegate != nil && will_start_selector.is_some() {
        let will_start_selector = will_start_selector.unwrap();
        log_dbg!("Notifying delegate {:?} {:?} {} with args {:?}, {:?}", delegate, will_start_selector, will_start_selector.as_str(&env.mem), animation_id, context);
        () = msg_send(env, (delegate, will_start_selector, animation_id, context));
    }
    env.objc.borrow_mut::<UIViewAnimationDelegateHostObject>(this).started_animation_count = new_started_animation_count;
}

- (())animationDidStop:(id)animation // CAAnimation*
              finished:(bool)finished {
    assert!(finished);
    let host_object = env.objc.borrow_mut::<UIViewAnimationDelegateHostObject>(this);
    let finished_animation_count = host_object.finished_animation_count;
    let new_finished_animation_count = finished_animation_count + finished as u32;
    log_dbg!("[(_tapHLE_UIView_AnimationDelegate*){:?} animationDidStop:{:?} finished:{}] finished_animation_count {} -> {}", this, animation, finished, finished_animation_count, new_finished_animation_count);
    env.objc.borrow_mut::<UIViewAnimationDelegateHostObject>(this).finished_animation_count = new_finished_animation_count;
    let UIViewAnimationDelegateHostObject {
        total_animation_count,
        finished_animation_count,
        delegate,
        did_stop_selector,
        context,
        animation_id,
        ..
    } = *env.objc.borrow::<UIViewAnimationDelegateHostObject>(this);
    if finished_animation_count == total_animation_count && delegate != nil && did_stop_selector.is_some() {
        let did_stop_selector = did_stop_selector.unwrap();
        let finished: id = msg_class![env; NSNumber numberWithBool:finished];
        log_dbg!("Notifying delegate {:?} {:?} {} with args {:?}, {:?}, {:?}", delegate, did_stop_selector, did_stop_selector.as_str(&env.mem), animation_id, finished, context);
        () = msg_send(env, (delegate, did_stop_selector, animation_id, finished, context));
    }
}

@end

};

/// The layer backing `view`, or `nil` for a nil view.
///
/// The co-ordinate conversion methods accept a nil counterpart to mean the
/// window's co-ordinate space, and `CALayer`'s conversion spells that same case
/// as a nil layer, so the nil passes straight through.
fn view_layer_or_nil(env: &Environment, view: id) -> id {
    if view == nil {
        nil
    } else {
        env.objc.borrow::<UIViewHostObject>(view).layer
    }
}
