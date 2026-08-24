/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIActionSheet`.
//!
//! The same problem as `UIAlertView`, and answered the same way: tapHLE cannot
//! draw a sheet, an app that shows one is usually waiting to be told which
//! button was pressed, so showing it logs its buttons and immediately reports
//! it as dismissed by **cancel**. Cancel never confirms a purchase, a deletion
//! or a network action on the player's behalf. Read `ui_alert_view` for the
//! full reasoning; this module differs only in where the buttons sit.
//!
//! **The ordering is not the alert view's.** UIKit lays a sheet out as the
//! destructive button first, then the other buttons in the order they were
//! given, then the cancel button *last* — so the cancel index depends on how
//! many other buttons there are, and a sheet built with three others has its
//! cancel button at index 3. Getting that wrong presses a real button, which is
//! precisely the mistake `ui_alert_view` records having made once already, so
//! the variadic list of other titles is walked properly here rather than
//! ignored.

use crate::frameworks::foundation::{ns_string, NSInteger};
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_send, msg_super, nil, objc_classes, release,
    retain, ClassExports, NSZonePtr,
};
use crate::Environment;

#[derive(Default)]
struct UIActionSheetHostObject {
    superclass: super::UIViewHostObject,
    /// Weak, as delegates always are.
    delegate: id,
    /// Button titles in the order UIKit lays them out: destructive, then the
    /// others, then cancel. Index arithmetic is done against this.
    buttons: Vec<String>,
    /// Index of the cancel button, or -1 when the sheet has none. Stored rather
    /// than derived because `-addButtonWithTitle:` appends after the cancel
    /// button, which does not move it.
    cancel_button_index: NSInteger,
    /// Index of the destructive button, or -1. Kept only so the getter can
    /// answer; nothing here treats it specially.
    destructive_button_index: NSInteger,
    /// Retained; settable after construction, as UIKit's is.
    title: id,
}
impl_HostObject_with_superclass!(UIActionSheetHostObject);

/// The index `-show...` reports.
///
/// A cancel button is chosen whenever there is one, wherever it sits. Failing
/// that, a sheet with exactly one button is an announcement rather than a
/// choice, so its lone button is reported as pressed — the case that otherwise
/// leaves an app waiting forever. Two or more buttons and no cancel button
/// reports -1, because picking one of those is picking for the player.
fn dismissal_button_index(cancel_button_index: NSInteger, button_count: usize) -> NSInteger {
    if cancel_button_index >= 0 {
        cancel_button_index
    } else if button_count == 1 {
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

/// Send an optional `(actionSheet, buttonIndex)` delegate method, if the
/// delegate implements it.
fn send_index_callback(
    env: &mut Environment,
    delegate: id,
    selector: &str,
    sheet: id,
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
    () = msg_send(env, (delegate, sel, sheet, index));
}

/// Report the sheet as dismissed by `index`, in the order UIKit does.
fn dismiss(env: &mut Environment, sheet: id, index: NSInteger, clicked: bool) {
    let delegate = env.objc.borrow::<UIActionSheetHostObject>(sheet).delegate;
    // Keep the sheet alive across the callbacks: a delegate commonly releases
    // it in the first one it receives.
    retain(env, sheet);
    if clicked {
        send_index_callback(
            env,
            delegate,
            "actionSheet:clickedButtonAtIndex:",
            sheet,
            index,
        );
    }
    send_index_callback(
        env,
        delegate,
        "actionSheet:willDismissWithButtonIndex:",
        sheet,
        index,
    );
    send_index_callback(
        env,
        delegate,
        "actionSheet:didDismissWithButtonIndex:",
        sheet,
        index,
    );
    release(env, sheet);
}

/// Body of every `-show...` method: they differ only in where UIKit would have
/// put the sheet, and it is not drawn either way.
fn show(env: &mut Environment, sheet: id) {
    let title = env.objc.borrow::<UIActionSheetHostObject>(sheet).title;
    let title_str = describe(env, title);
    let host_object = env.objc.borrow::<UIActionSheetHostObject>(sheet);
    let buttons = host_object.buttons.join(", ");
    let index = dismissal_button_index(host_object.cancel_button_index, host_object.buttons.len());
    let reported = match usize::try_from(index)
        .ok()
        .and_then(|i| host_object.buttons.get(i))
    {
        Some(button) if index == host_object.cancel_button_index => {
            format!("its cancel button ({button:?})")
        }
        Some(button) => format!("its only button ({button:?})"),
        None => "no button, as it has several and none of them is cancel".to_string(),
    };
    log!(
        "UIActionSheet: title: {:?}, buttons: [{}]",
        title_str,
        buttons
    );
    log!("UIActionSheet: cannot be displayed; reporting it as dismissed by {reported}");

    dismiss(env, sheet, index, /* clicked: */ true);
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIActionSheet: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIActionSheetHostObject {
        cancel_button_index: -1,
        destructive_button_index: -1,
        ..Default::default()
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithTitle:(id)title
           delegate:(id)delegate
  cancelButtonTitle:(id)cancelButtonTitle
destructiveButtonTitle:(id)destructiveButtonTitle
  otherButtonTitles:(id)firstOtherButtonTitle, ...args {
    let new: id = msg_super![env; this init];

    // UIKit's order: destructive, others, cancel. Building the list in that
    // order is what makes every index below simply a position in it.
    let mut buttons: Vec<String> = Vec::new();
    let mut destructive_button_index: NSInteger = -1;
    if destructiveButtonTitle != nil {
        destructive_button_index = 0;
        buttons.push(ns_string::to_rust_string(env, destructiveButtonTitle).to_string());
    }
    if firstOtherButtonTitle != nil {
        buttons.push(ns_string::to_rust_string(env, firstOtherButtonTitle).to_string());
        let mut varargs = args.start();
        loop {
            let next: id = varargs.next(env);
            if next == nil {
                break;
            }
            buttons.push(ns_string::to_rust_string(env, next).to_string());
        }
    }
    let mut cancel_button_index: NSInteger = -1;
    if cancelButtonTitle != nil {
        cancel_button_index = buttons.len() as NSInteger;
        buttons.push(ns_string::to_rust_string(env, cancelButtonTitle).to_string());
    }

    retain(env, title);
    let host_object = env.objc.borrow_mut::<UIActionSheetHostObject>(new);
    host_object.delegate = delegate;
    host_object.buttons = buttons;
    host_object.cancel_button_index = cancel_button_index;
    host_object.destructive_button_index = destructive_button_index;
    host_object.title = title;
    new
}

- (id)title {
    env.objc.borrow::<UIActionSheetHostObject>(this).title
}
- (())setTitle:(id)title {
    retain(env, title);
    let host_object = env.objc.borrow_mut::<UIActionSheetHostObject>(this);
    let old = std::mem::replace(&mut host_object.title, title);
    release(env, old);
}

- (id)delegate {
    env.objc.borrow::<UIActionSheetHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<UIActionSheetHostObject>(this).delegate = delegate;
}

- (NSInteger)numberOfButtons {
    env.objc.borrow::<UIActionSheetHostObject>(this).buttons.len() as NSInteger
}

- (NSInteger)cancelButtonIndex {
    env.objc.borrow::<UIActionSheetHostObject>(this).cancel_button_index
}
- (())setCancelButtonIndex:(NSInteger)index {
    env.objc.borrow_mut::<UIActionSheetHostObject>(this).cancel_button_index = index;
}

- (NSInteger)destructiveButtonIndex {
    env.objc.borrow::<UIActionSheetHostObject>(this).destructive_button_index
}
- (())setDestructiveButtonIndex:(NSInteger)index {
    env.objc.borrow_mut::<UIActionSheetHostObject>(this).destructive_button_index = index;
}

- (id)buttonTitleAtIndex:(NSInteger)index {
    let title = usize::try_from(index)
        .ok()
        .and_then(|i| env.objc.borrow::<UIActionSheetHostObject>(this).buttons.get(i))
        .cloned();
    match title {
        Some(title) => ns_string::from_rust_string(env, title),
        None => nil,
    }
}

- (bool)isVisible {
    // Never actually displayed, so never visible.
    false
}

// A button added afterwards goes on the end, which is *after* the cancel
// button if there is one. UIKit does the same thing, and it is why the cancel
// index is stored rather than recomputed from the count.
- (NSInteger)addButtonWithTitle:(id)title {
    let title_str = ns_string::to_rust_string(env, title).to_string();
    log!("UIActionSheet: button: {:?}", title_str);
    let host_object = env.objc.borrow_mut::<UIActionSheetHostObject>(this);
    host_object.buttons.push(title_str);
    (host_object.buttons.len() - 1) as NSInteger
}

// The four ways to present one. None of them can draw, so they share a body.
- (())showInView:(id)_view {
    show(env, this)
}
- (())showFromToolbar:(id)_toolbar {
    show(env, this)
}
- (())showFromTabBar:(id)_tab_bar {
    show(env, this)
}
- (())showFromRect:(crate::frameworks::core_graphics::CGRect)_rect
            inView:(id)_view
          animated:(bool)_animated {
    show(env, this)
}

- (())dismissWithClickedButtonIndex:(NSInteger)index animated:(bool)_animated {
    dismiss(env, this, index, /* clicked: */ false);
}

- (())dealloc {
    let title = env.objc.borrow::<UIActionSheetHostObject>(this).title;
    release(env, title);
    msg_super![env; this dealloc]
}

@end

};

#[cfg(test)]
mod tests {
    use super::dismissal_button_index;

    /// The ordering that separates a sheet from an alert: cancel is last, so
    /// its index is not 0 and must not be assumed to be.
    #[test]
    fn a_cancel_button_is_reported_wherever_it_sits() {
        assert_eq!(dismissal_button_index(3, 4), 3);
        assert_eq!(dismissal_button_index(0, 1), 0);
    }

    /// A lone button is an announcement, and an app waiting to be told it was
    /// pressed waits forever otherwise.
    #[test]
    fn a_lone_button_is_reported_as_pressed() {
        assert_eq!(dismissal_button_index(-1, 1), 0);
    }

    /// The case that must press nothing: a real choice with no cancel option.
    #[test]
    fn several_buttons_and_no_cancel_press_nothing() {
        assert_eq!(dismissal_button_index(-1, 2), -1);
        assert_eq!(dismissal_button_index(-1, 3), -1);
    }

    /// A sheet with no buttons cannot report one.
    #[test]
    fn no_buttons_reports_no_button() {
        assert_eq!(dismissal_button_index(-1, 0), -1);
    }
}
