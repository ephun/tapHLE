/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIAlertView`.
//!
//! tapHLE cannot draw an alert, so `-show` logs the alert's text and then
//! immediately reports it as dismissed via the **cancel** button. Doing nothing
//! is not a neutral choice: an app that shows an alert is usually waiting for
//! `alertView:clickedButtonAtIndex:` before it continues, so a silent alert
//! stalls it. Cancel is the conservative answer — it is what a user declining
//! an unexpected prompt would choose, and it never confirms a purchase, a
//! deletion or a network action on their behalf.
//!
//! That reasoning only holds if the reported index really is the cancel
//! button's. An alert with several buttons and no cancel button is dismissed
//! with -1, which is what UIKit reports for one, rather than with some other
//! button standing in for it — see [cancel_button_index].
//!
//! The exception is an alert with exactly one button, which is a statement
//! rather than a question: see [dismissal_button_index].

use crate::frameworks::foundation::{ns_string, NSInteger};
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_send, msg_super, nil, objc_classes, release,
    retain, ClassExports, NSZonePtr,
};
use crate::Environment;

#[derive(Default)]
struct UIAlertViewHostObject {
    superclass: super::UIViewHostObject,
    /// Weak, as delegates always are.
    delegate: id,
    /// Button titles in index order, so a dismissal can say which button it
    /// reported rather than only its number.
    buttons: Vec<String>,
    /// Whether a cancel button title was supplied to the initializer. Only then
    /// does the alert have a cancel button, and only then is it index 0.
    ///
    /// This is not the same question as "are there any buttons". An app that
    /// passes a nil `cancelButtonTitle:` and then calls `addButtonWithTitle:`
    /// has ordinary buttons and no cancel button, and UIKit reports
    /// `cancelButtonIndex` as -1 for it.
    has_cancel_button: bool,
    /// Retained. `UIAlertView` also exposes these as settable properties, so
    /// they cannot simply be logged at construction time and discarded: apps
    /// commonly build the alert with a bare `init` and fill them in afterwards.
    title: id,
    message: id,
}
impl_HostObject_with_superclass!(UIAlertViewHostObject);

/// Replace one of the retained string properties, in the order a setter must:
/// retain the new value before releasing the old, in case they are the same.
fn set_string_property(env: &mut Environment, alert: id, new: id, is_title: bool) {
    retain(env, new);
    let host_object = env.objc.borrow_mut::<UIAlertViewHostObject>(alert);
    let old = if is_title {
        std::mem::replace(&mut host_object.title, new)
    } else {
        std::mem::replace(&mut host_object.message, new)
    };
    release(env, old);
}

/// The index UIKit reports for an alert's cancel button.
///
/// An alert has a cancel button only if one was named when it was created;
/// there is no rule that some button must be the cancel button. Treating "has
/// buttons" as "has a cancel button" is not a harmless approximation, because
/// index 0 is then whichever button the app happened to add first. The Jim and
/// Frank Mysteries builds its rate prompt with a nil `cancelButtonTitle:` and
/// three `addButtonWithTitle:` calls — "Rate Now", "Remind me later", "No,
/// Thanks" — so reporting 0 pressed *Rate Now*, the app opened its App Store
/// URL, and tapHLE exited before the game ever started.
fn cancel_button_index(has_cancel_button: bool) -> NSInteger {
    if has_cancel_button {
        0
    } else {
        -1
    }
}

/// The index `-show` reports, which is not always [cancel_button_index].
///
/// The caution above is about *choosing between* buttons. An alert with exactly
/// one button offers no choice: it is informational, the button is the only
/// thing on it, and a user who wants to keep playing has one option. Reporting
/// -1 for that alert says it was dismissed without any button, which UIKit says
/// only when the app dismissed it itself, so an app waiting to be told its
/// single button was pressed waits forever. A game stalls on its own mission
/// briefing that way — one "OK", added with `addButtonWithTitle:` after a nil
/// `cancelButtonTitle:`.
///
/// Two or more buttons with no cancel button stay -1. That is the case where
/// picking one is picking *for* the user, and it is the case the rate prompt
/// described above falls into.
fn dismissal_button_index(has_cancel_button: bool, button_count: usize) -> NSInteger {
    // Index 0 is the cancel button when there is one, and the lone button when
    // there is not; the two cases coincide because a cancel button is always
    // first.
    if has_cancel_button || button_count == 1 {
        0
    } else {
        -1
    }
}

/// Render a string property for logging, tolerating nil.
fn describe(env: &mut Environment, string: id) -> String {
    if string == nil {
        "(nil)".to_string()
    } else {
        ns_string::to_rust_string(env, string).to_string()
    }
}

/// Send an optional `(alertView, buttonIndex)` delegate method, if implemented.
fn send_index_callback(
    env: &mut Environment,
    delegate: id,
    selector: &str,
    alert: id,
    index: NSInteger,
) {
    if delegate == nil {
        return;
    }
    let Some(sel) = env.objc.lookup_selector(selector) else {
        return;
    };
    let responds: bool = msg![env; delegate respondsToSelector:sel];
    if !responds {
        return;
    }
    () = msg_send(env, (delegate, sel, alert, index));
}

/// Report the alert as dismissed by `index`, in the order UIKit does.
fn dismiss(env: &mut Environment, alert: id, index: NSInteger, clicked: bool) {
    let delegate = env.objc.borrow::<UIAlertViewHostObject>(alert).delegate;
    // Keep the alert alive across the callbacks: a delegate commonly releases
    // it in the first one it receives.
    retain(env, alert);
    if clicked {
        send_index_callback(
            env,
            delegate,
            "alertView:clickedButtonAtIndex:",
            alert,
            index,
        );
    }
    send_index_callback(
        env,
        delegate,
        "alertView:willDismissWithButtonIndex:",
        alert,
        index,
    );
    send_index_callback(
        env,
        delegate,
        "alertView:didDismissWithButtonIndex:",
        alert,
        index,
    );
    release(env, alert);
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIAlertView: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIAlertViewHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithTitle:(id)title
                      message:(id)message
                     delegate:(id)delegate
            cancelButtonTitle:(id)cancelButtonTitle
            otherButtonTitles:(id)otherButtonTitles {
    // TODO: otherButtonTitles is a nil-terminated variadic list. Only the
    // cancel button's presence is tracked, which is what dismissal needs.
    let _ = otherButtonTitles;

    let new: id = msg_super![env; this init];
    let cancel_title = if cancelButtonTitle == nil {
        None
    } else {
        Some(ns_string::to_rust_string(env, cancelButtonTitle).to_string())
    };
    let host_object = env.objc.borrow_mut::<UIAlertViewHostObject>(new);
    host_object.delegate = delegate;
    host_object.has_cancel_button = cancel_title.is_some();
    host_object.buttons.extend(cancel_title);
    set_string_property(env, new, title, /* is_title: */ true);
    set_string_property(env, new, message, /* is_title: */ false);
    new
}

- (id)title {
    env.objc.borrow::<UIAlertViewHostObject>(this).title
}
- (())setTitle:(id)title {
    set_string_property(env, this, title, /* is_title: */ true);
}

- (id)message {
    env.objc.borrow::<UIAlertViewHostObject>(this).message
}
- (())setMessage:(id)message {
    set_string_property(env, this, message, /* is_title: */ false);
}

- (())dealloc {
    let &UIAlertViewHostObject { title, message, .. } = env.objc.borrow(this);
    release(env, title);
    release(env, message);
    msg_super![env; this dealloc]
}

- (id)delegate {
    env.objc.borrow::<UIAlertViewHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<UIAlertViewHostObject>(this).delegate = delegate;
}

- (NSInteger)numberOfButtons {
    env.objc.borrow::<UIAlertViewHostObject>(this).buttons.len() as NSInteger
}

- (NSInteger)cancelButtonIndex {
    cancel_button_index(env.objc.borrow::<UIAlertViewHostObject>(this).has_cancel_button)
}

- (bool)isVisible {
    // Never actually displayed, so never visible.
    false
}

- (())addButtonWithTitle:(id)title {
    let title_str = ns_string::to_rust_string(env, title).to_string();
    log!("UIAlertView: button: {:?}", title_str);
    env.objc.borrow_mut::<UIAlertViewHostObject>(this).buttons.push(title_str);
}

- (())show {
    // Logged here rather than at construction: the title and message are
    // settable, so this is the first point at which they are certainly final.
    let &UIAlertViewHostObject { title, message, .. } = env.objc.borrow(this);
    let title_str = describe(env, title);
    let message_str = describe(env, message);
    log!("UIAlertView: title: {:?}, message: {:?}", title_str, message_str);

    let host_object = env.objc.borrow::<UIAlertViewHostObject>(this);
    let index = dismissal_button_index(host_object.has_cancel_button, host_object.buttons.len());
    let reported = match usize::try_from(index).ok().and_then(|i| host_object.buttons.get(i)) {
        Some(button) if host_object.has_cancel_button => format!("its cancel button ({button:?})"),
        Some(button) => format!("its only button ({button:?})"),
        None => "no button, as it has several and none of them is cancel".to_string(),
    };
    log!("UIAlertView: cannot be displayed; reporting it as dismissed by {reported}");

    dismiss(env, this, index, /* clicked: */ true);
}

- (())dismissWithClickedButtonIndex:(NSInteger)index animated:(bool)_animated {
    dismiss(env, this, index, /* clicked: */ false);
}

@end

};

#[cfg(test)]
mod tests {
    use super::{cancel_button_index, dismissal_button_index};

    /// The one-button case this exists for: informational alerts an app blocks
    /// on. There is nothing else the user could have pressed.
    #[test]
    fn a_lone_button_is_reported_as_pressed() {
        assert_eq!(dismissal_button_index(false, 1), 0);
    }

    /// The case it must not touch. A rate prompt is three buttons and no
    /// cancel; reporting 0 there presses "Rate Now" for the user.
    #[test]
    fn several_buttons_and_no_cancel_still_press_nothing() {
        assert_eq!(dismissal_button_index(false, 3), -1);
        assert_eq!(dismissal_button_index(false, 2), -1);
    }

    /// A cancel button is still preferred whenever there is one, whatever else
    /// the alert offers.
    #[test]
    fn a_cancel_button_still_wins() {
        assert_eq!(dismissal_button_index(true, 1), 0);
        assert_eq!(dismissal_button_index(true, 3), 0);
    }

    /// An alert with no buttons at all cannot report one.
    #[test]
    fn no_buttons_reports_no_button() {
        assert_eq!(dismissal_button_index(false, 0), -1);
    }

    /// The index only means "cancel" when the alert was given a cancel button.
    /// Getting this wrong presses a real button on the user's behalf, which is
    /// exactly what the module is written to avoid.
    #[test]
    fn an_alert_without_a_cancel_button_reports_minus_one() {
        assert_eq!(cancel_button_index(false), -1);
    }

    /// When there is one, it is always first: UIKit puts the initializer's
    /// `cancelButtonTitle:` at index 0, ahead of any button added later.
    #[test]
    fn a_cancel_button_is_always_index_zero() {
        assert_eq!(cancel_button_index(true), 0);
    }
}
