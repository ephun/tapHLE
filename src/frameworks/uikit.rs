/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The UIKit framework.
//!
//! For the time being the focus of this project is on running games, which are
//! likely to use UIKit in very simple and limited ways, so this implementation
//! will probably take a lot of shortcuts.

use crate::{msg, Environment};
use std::time::Instant;

pub mod ui_accelerometer;
pub mod ui_activity_indicator_view;
pub mod ui_application;
pub mod ui_color;
pub mod ui_device;
pub mod ui_event;
pub mod ui_font;
pub mod ui_geometry;
pub mod ui_gesture_recognizer;
pub mod ui_graphics;
pub mod ui_image;
pub mod ui_image_picker_controller;
pub mod ui_local_notification;
pub mod ui_localized_indexed_collation;
pub mod ui_nib;
pub mod ui_pasteboard;
pub mod ui_responder;
pub mod ui_screen;
pub mod ui_touch;
pub mod ui_view;
pub mod ui_view_controller;

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/UIKit.framework/UIKit",
    aliases: &[],
    class_exports: &[
        ui_accelerometer::CLASSES,
        ui_activity_indicator_view::CLASSES,
        ui_application::CLASSES,
        ui_color::CLASSES,
        ui_device::CLASSES,
        ui_event::CLASSES,
        ui_font::CLASSES,
        ui_gesture_recognizer::CLASSES,
        ui_image::CLASSES,
        ui_image_picker_controller::CLASSES,
        ui_local_notification::CLASSES,
        ui_nib::CLASSES,
        ui_pasteboard::CLASSES,
        ui_responder::CLASSES,
        ui_screen::CLASSES,
        ui_touch::CLASSES,
        ui_view::CLASSES,
        ui_view::ui_alert_view::CLASSES,
        ui_view::ui_control::CLASSES,
        ui_view::ui_control::ui_button::CLASSES,
        ui_view::ui_control::ui_segmented_control::CLASSES,
        ui_view::ui_control::ui_slider::CLASSES,
        ui_view::ui_control::ui_text_field::CLASSES,
        ui_view::ui_control::ui_switch::CLASSES,
        ui_view::ui_image_view::CLASSES,
        ui_view::ui_label::CLASSES,
        ui_view::ui_picker_view::CLASSES,
        ui_view::ui_scroll_view::CLASSES,
        ui_localized_indexed_collation::CLASSES,
        ui_view::ui_scroll_view::ui_table_view::CLASSES,
        ui_view::ui_scroll_view::ui_text_view::CLASSES,
        ui_view::ui_web_view::CLASSES,
        ui_view::ui_window::CLASSES,
        ui_view_controller::CLASSES,
        ui_view_controller::ui_navigation_controller::CLASSES,
        ui_view_controller::ui_tab_bar_controller::CLASSES,
    ],
    constant_exports: &[
        ui_application::CONSTANTS,
        ui_device::CONSTANTS,
        ui_geometry::CONSTANTS,
        ui_image_picker_controller::CONSTANTS,
        ui_view::ui_control::ui_text_field::CONSTANTS,
        ui_view::ui_window::CONSTANTS,
    ],
    function_exports: &[
        ui_application::FUNCTIONS,
        ui_geometry::FUNCTIONS,
        ui_graphics::FUNCTIONS,
    ],
};

#[cfg(test)]
mod tests {
    use super::{ui_pasteboard, ui_view, ui_view_controller::ui_navigation_controller, DYLIB};

    #[test]
    fn pasteboard_exports_named_lookup() {
        let pasteboard = &ui_pasteboard::CLASSES
            .iter()
            .find(|(name, _)| *name == "UIPasteboard")
            .expect("UIPasteboard is missing from its class module")
            .1;

        for selector in [
            "generalPasteboard",
            "pasteboardWithName:create:",
            "pasteboardWithUniqueName",
            "removePasteboardWithName:",
        ] {
            assert!(
                pasteboard
                    .class_methods
                    .iter()
                    .any(|(exported, _)| *exported == selector),
                "UIPasteboard is missing {selector}",
            );
        }
        for selector in [
            "name",
            "isPersistent",
            "setPersistent:",
            "string",
            "setString:",
            "changeCount",
            "pasteboardTypes",
            "containsPasteboardTypes:",
            "valueForPasteboardType:",
            "dataForPasteboardType:",
            "setValue:forPasteboardType:",
            "setData:forPasteboardType:",
        ] {
            assert!(
                pasteboard
                    .instance_methods
                    .iter()
                    .any(|(exported, _)| *exported == selector),
                "UIPasteboard is missing {selector}",
            );
        }
    }

    #[test]
    fn navigation_nib_classes_are_exported_with_coder_paths() {
        let exported_names: Vec<_> = DYLIB
            .class_exports
            .iter()
            .flat_map(|classes| classes.iter().map(|(name, _)| *name))
            .collect();

        assert!(exported_names.contains(&"UINavigationBar"));
        assert!(exported_names.contains(&"UINavigationItem"));

        let navigation_bar = &ui_navigation_controller::CLASSES
            .iter()
            .find(|(name, _)| *name == "UINavigationBar")
            .expect("UINavigationBar is missing from its class module")
            .1;
        assert_eq!(navigation_bar.superclass, Some("UIView"));

        let ui_view = &ui_view::CLASSES
            .iter()
            .find(|(name, _)| *name == "UIView")
            .expect("UIView is missing from its class module")
            .1;
        assert!(
            ui_view
                .instance_methods
                .iter()
                .any(|(selector, _)| *selector == "initWithCoder:"),
            "UINavigationBar's UIView superclass cannot restore keyed NIB state",
        );

        let navigation_item = &ui_navigation_controller::CLASSES
            .iter()
            .find(|(name, _)| *name == "UINavigationItem")
            .expect("UINavigationItem is missing from its class module")
            .1;
        assert_eq!(navigation_item.superclass, Some("NSObject"));
        assert!(
            navigation_item
                .instance_methods
                .iter()
                .any(|(selector, _)| *selector == "initWithCoder:"),
            "UINavigationItem cannot be restored from a keyed NIB",
        );

        let navigation_controller = &ui_navigation_controller::CLASSES
            .iter()
            .find(|(name, _)| *name == "UINavigationController")
            .expect("UINavigationController is missing from its class module")
            .1;
        for selector in [
            "initWithCoder:",
            "loadView",
            "navigationBar",
            "isNavigationBarHidden",
            "setNavigationBarHidden:animated:",
            "shouldAutorotateToInterfaceOrientation:",
        ] {
            assert!(
                navigation_controller
                    .instance_methods
                    .iter()
                    .any(|(exported, _)| *exported == selector),
                "UINavigationController is missing {selector}",
            );
        }

        let view_controller = &super::ui_view_controller::CLASSES
            .iter()
            .find(|(name, _)| *name == "UIViewController")
            .expect("UIViewController is missing from its class module")
            .1;
        for selector in [
            "parentViewController",
            "navigationController",
            "modalViewController",
            "presentModalViewController:animated:",
            "dismissModalViewControllerAnimated:",
        ] {
            assert!(
                view_controller
                    .instance_methods
                    .iter()
                    .any(|(exported, _)| *exported == selector),
                "UIViewController is missing {selector}",
            );
        }
    }

    #[test]
    fn image_view_exports_animation_repeat_count_state() {
        let image_view = &ui_view::ui_image_view::CLASSES
            .iter()
            .find(|(name, _)| *name == "UIImageView")
            .expect("UIImageView is missing from its class module")
            .1;

        for selector in [
            "animationImages",
            "animationDuration",
            "animationRepeatCount",
            "setAnimationRepeatCount:",
            "startAnimating",
            "stopAnimating",
            "isAnimating",
        ] {
            assert!(
                image_view
                    .instance_methods
                    .iter()
                    .any(|(exported, _)| *exported == selector),
                "UIImageView is missing {selector}",
            );
        }
    }

    #[test]
    fn button_content_exports_archived_images() {
        let button_content = &ui_view::ui_control::ui_button::CLASSES
            .iter()
            .find(|(name, _)| *name == "UIButtonContent")
            .expect("UIButtonContent is missing from its class module")
            .1;

        for selector in ["image", "backgroundImage"] {
            assert!(
                button_content
                    .instance_methods
                    .iter()
                    .any(|(exported, _)| *exported == selector),
                "UIButtonContent is missing {selector}",
            );
        }
    }
}

#[derive(Default)]
pub struct State {
    ui_localized_indexed_collation: ui_localized_indexed_collation::State,
    ui_accelerometer: ui_accelerometer::State,
    ui_application: ui_application::State,
    ui_color: ui_color::State,
    ui_device: ui_device::State,
    ui_font: ui_font::State,
    ui_graphics: ui_graphics::State,
    ui_image: ui_image::State,
    ui_pasteboard: ui_pasteboard::State,
    ui_screen: ui_screen::State,
    ui_touch: ui_touch::State,
    pub ui_view: ui_view::State,
    ui_responder: ui_responder::State,
}

/// For use by `NSRunLoop`: handles any events that have queued up.
///
/// Returns the next time this function must be called, if any, e.g. the next
/// time an accelerometer input is due.
pub fn handle_events(env: &mut Environment) -> Option<Instant> {
    use crate::window::Event;
    use crate::window::TextInputEvent;

    // NSRunLoop will never call this function in headless mode.
    while let Some(event) = env.window_mut().pop_event() {
        match event {
            Event::Quit => {
                echo!("User requested quit, exiting.");
                ui_application::exit(env);
            }
            Event::TouchesDown(..) | Event::TouchesMove(..) | Event::TouchesUp(..) => {
                ui_touch::handle_event(env, event)
            }
            Event::AppWillResignActive => {
                // Getting this event means tapHLE is becoming inactive, e.g.
                // due to switching apps. The obvious way to handle this would
                // be to just send `applicationWillResignActive:` to the
                // UIApplicationDelegate. However:
                // - tapHLE's event loop can't handle an inactive app well
                //   right now. For example, audio isn't paused.
                // - tapHLE's event loop can't handle the subsequent
                //   termination of an app right now: it doesn't manage to send
                //   the `applicationWillTerminate:` message in time. This can
                //   mean loss of data!
                // Therefore, for the moment we will simulate the early iOS
                // behavior where switching app usually resulted in termination.
                // We can usually handle this in time, so there won't be data
                // loss, nor problems with background resource usage or audio.
                // TODO: Handle this better.
                log!("Handling app-will-resign-active event: exiting.");
                ui_application::exit(env);
            }
            Event::AppWillTerminate => {
                log!("Handling app-will-terminate event.");
                ui_application::exit(env);
            }
            Event::EnterDebugger => {
                if env.is_debugging_enabled() {
                    log!("Handling EnterDebugger event: entering debugger.");
                    env.enter_debugger(/* reason: */ None);
                } else {
                    log!("Ignoring EnterDebugger event: no debugger connected.");
                }
            }
            Event::TextInput(text_event) => {
                let responder = env.framework_state.uikit.ui_responder.first_responder;
                let class = msg![env; responder class];
                let ui_text_field_class = env.objc.get_known_class("UITextField", &mut env.mem);
                if !responder.is_null() && env.objc.class_is_subclass_of(class, ui_text_field_class)
                {
                    match text_event {
                        TextInputEvent::Text(text) => {
                            ui_view::ui_control::ui_text_field::handle_text(env, responder, text)
                        }
                        TextInputEvent::Backspace => {
                            ui_view::ui_control::ui_text_field::handle_backspace(env, responder)
                        }
                        TextInputEvent::Return => {
                            ui_view::ui_control::ui_text_field::handle_return(env, responder)
                        }
                    }
                }
            }
        }
    }

    ui_accelerometer::handle_accelerometer(env)
}
