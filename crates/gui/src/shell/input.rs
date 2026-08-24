/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! SDL's events, said in egui's vocabulary.
//!
//! egui does not read a keyboard or a mouse. It is handed an
//! [egui::RawInput] describing what happened since the last frame and returns
//! what to draw, which is the whole reason the same interface code can run
//! under a window system it knows nothing about. This module is the one place
//! that knows both sides.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Vec2};
use sdl2::event::{Event as SdlEvent, WindowEvent};
use sdl2::keyboard::{Keycode, Mod};
use sdl2::mouse::MouseButton;

/// How far one notch of a wheel scrolls, in points.
///
/// SDL reports a wheel in notches and egui wants a distance. The number is
/// the one winit uses for a line, so scrolling feels the way it does in every
/// other program on the desktop.
const LINE_HEIGHT: f32 = 50.0;

/// The modifier keys held right now.
///
/// Read once a frame by the shell rather than remembered from the last key
/// event. Remembered, a Ctrl-click only counted as one when the Ctrl went
/// down in the same frame as the click.
pub fn modifiers_now(keyboard: &sdl2::keyboard::KeyboardUtil) -> Modifiers {
    modifiers(keyboard.mod_state())
}

/// What the shell has to act on rather than egui.
#[derive(Default)]
pub struct Consumed {
    /// The window manager's close button, or Alt+F4.
    pub close_requested: bool,
    /// A paste was asked for, and the clipboard has to be read to answer it.
    pub wants_paste: bool,
    /// The window's own geometry changed, so egui's copy of it is stale.
    pub geometry_changed: bool,
    /// The window gained or lost the focus, if it changed at all. The shell
    /// keeps it, because egui is told the focus every frame and not only on
    /// the frame it changed.
    pub focus: Option<bool>,
}

fn modifiers(state: Mod) -> Modifiers {
    Modifiers {
        alt: state.intersects(Mod::LALTMOD | Mod::RALTMOD),
        ctrl: state.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD),
        shift: state.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD),
        // The command key is Ctrl here. egui asks for both so that a Mac
        // build can say Cmd without every shortcut being rewritten.
        mac_cmd: false,
        command: state.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD),
    }
}

/// Fold one SDL event into the input being built for the next frame.
///
/// SDL reports a pointer in the same units as the window's own size, which is
/// what egui calls points, so nothing is scaled here. Scaling it looked
/// obviously right on a display at 175% and was obviously wrong the moment
/// anything was clicked: every click landed a row above the icon under the
/// pointer, and the search box could not be focused at all. The pixels are on
/// the other side, in the drawable size the painter is given.
pub fn absorb(event: &SdlEvent, input: &mut RawInput, consumed: &mut Consumed) {
    let point = |x: i32, y: i32| Pos2::new(x as f32, y as f32);

    match event {
        SdlEvent::Quit { .. } => consumed.close_requested = true,
        SdlEvent::Window { win_event, .. } => match win_event {
            WindowEvent::Close => consumed.close_requested = true,
            WindowEvent::FocusGained => {
                consumed.focus = Some(true);
                input.events.push(Event::WindowFocused(true));
            }
            WindowEvent::FocusLost => {
                consumed.focus = Some(false);
                input.events.push(Event::WindowFocused(false));
            }
            WindowEvent::Leave => input.events.push(Event::PointerGone),
            WindowEvent::Resized(..)
            | WindowEvent::SizeChanged(..)
            | WindowEvent::Moved(..)
            | WindowEvent::Maximized
            | WindowEvent::Restored
            | WindowEvent::Minimized => consumed.geometry_changed = true,
            _ => {}
        },
        SdlEvent::MouseMotion { x, y, .. } => {
            input.events.push(Event::PointerMoved(point(*x, *y)));
        }
        SdlEvent::MouseButtonDown {
            x, y, mouse_btn, ..
        }
        | SdlEvent::MouseButtonUp {
            x, y, mouse_btn, ..
        } => {
            let Some(button) = pointer_button(*mouse_btn) else {
                return;
            };
            input.events.push(Event::PointerButton {
                pos: point(*x, *y),
                button,
                pressed: matches!(event, SdlEvent::MouseButtonDown { .. }),
                modifiers: input.modifiers,
            });
        }
        SdlEvent::MouseWheel {
            precise_x,
            precise_y,
            direction,
            ..
        } => {
            // SDL reports natural scrolling by flipping the direction rather
            // than the sign, so it has to be undone here or the page moves
            // the wrong way for anybody who has turned it on.
            let sign = match direction {
                sdl2::mouse::MouseWheelDirection::Flipped => -1.0,
                _ => 1.0,
            };
            input.events.push(Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(precise_x * sign, precise_y * sign) * LINE_HEIGHT,
                modifiers: input.modifiers,
            });
        }
        SdlEvent::TextInput { text, .. } => {
            input.events.push(Event::Text(text.clone()));
        }
        SdlEvent::KeyDown {
            keycode: Some(keycode),
            keymod,
            repeat,
            ..
        }
        | SdlEvent::KeyUp {
            keycode: Some(keycode),
            keymod,
            repeat,
            ..
        } => {
            let pressed = matches!(event, SdlEvent::KeyDown { .. });
            // This event's own modifiers rather than the shell's live
            // reading, because they are the ones held when the key went
            // down, and the two differ for the modifier key itself.
            let state = modifiers(*keymod);
            // Copy, cut and paste are egui events of their own rather than
            // key presses, because only the shell can reach the clipboard.
            if pressed && state.command {
                match *keycode {
                    Keycode::C => input.events.push(Event::Copy),
                    Keycode::X => input.events.push(Event::Cut),
                    Keycode::V => {
                        consumed.wants_paste = true;
                        return;
                    }
                    _ => {}
                }
            }
            if let Some(key) = translate_key(*keycode) {
                input.events.push(Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: *repeat,
                    modifiers: state,
                });
            }
        }
        SdlEvent::DropFile { filename, .. } => {
            input.dropped_files.push(egui::DroppedFile {
                path: Some(std::path::PathBuf::from(filename)),
                ..Default::default()
            });
        }
        _ => {}
    }
}

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::X1 => Some(PointerButton::Extra1),
        MouseButton::X2 => Some(PointerButton::Extra2),
        MouseButton::Unknown => None,
    }
}

/// The egui key an SDL keycode stands for, where there is one.
///
/// Anything egui has no name for is dropped rather than approximated: a key
/// that does nothing is better than a key that does something else. Typing is
/// unaffected, because printable characters arrive as text rather than here.
fn translate_key(keycode: Keycode) -> Option<Key> {
    use Keycode as K;
    Some(match keycode {
        K::Left => Key::ArrowLeft,
        K::Right => Key::ArrowRight,
        K::Up => Key::ArrowUp,
        K::Down => Key::ArrowDown,
        K::Escape => Key::Escape,
        K::Tab => Key::Tab,
        K::Backspace => Key::Backspace,
        K::Return | K::KpEnter => Key::Enter,
        K::Space => Key::Space,
        K::Insert => Key::Insert,
        K::Delete => Key::Delete,
        K::Home => Key::Home,
        K::End => Key::End,
        K::PageUp => Key::PageUp,
        K::PageDown => Key::PageDown,
        K::Colon => Key::Colon,
        K::Comma => Key::Comma,
        K::Backslash => Key::Backslash,
        K::Slash | K::KpDivide => Key::Slash,
        K::LeftBracket => Key::OpenBracket,
        K::RightBracket => Key::CloseBracket,
        K::Backquote => Key::Backtick,
        K::Minus | K::KpMinus => Key::Minus,
        K::Period | K::KpPeriod => Key::Period,
        K::Plus | K::KpPlus => Key::Plus,
        K::Equals => Key::Equals,
        K::Semicolon => Key::Semicolon,
        K::Quote => Key::Quote,
        K::Num0 | K::Kp0 => Key::Num0,
        K::Num1 | K::Kp1 => Key::Num1,
        K::Num2 | K::Kp2 => Key::Num2,
        K::Num3 | K::Kp3 => Key::Num3,
        K::Num4 | K::Kp4 => Key::Num4,
        K::Num5 | K::Kp5 => Key::Num5,
        K::Num6 | K::Kp6 => Key::Num6,
        K::Num7 | K::Kp7 => Key::Num7,
        K::Num8 | K::Kp8 => Key::Num8,
        K::Num9 | K::Kp9 => Key::Num9,
        K::A => Key::A,
        K::B => Key::B,
        K::C => Key::C,
        K::D => Key::D,
        K::E => Key::E,
        K::F => Key::F,
        K::G => Key::G,
        K::H => Key::H,
        K::I => Key::I,
        K::J => Key::J,
        K::K => Key::K,
        K::L => Key::L,
        K::M => Key::M,
        K::N => Key::N,
        K::O => Key::O,
        K::P => Key::P,
        K::Q => Key::Q,
        K::R => Key::R,
        K::S => Key::S,
        K::T => Key::T,
        K::U => Key::U,
        K::V => Key::V,
        K::W => Key::W,
        K::X => Key::X,
        K::Y => Key::Y,
        K::Z => Key::Z,
        K::F1 => Key::F1,
        K::F2 => Key::F2,
        K::F3 => Key::F3,
        K::F4 => Key::F4,
        K::F5 => Key::F5,
        K::F6 => Key::F6,
        K::F7 => Key::F7,
        K::F8 => Key::F8,
        K::F9 => Key::F9,
        K::F10 => Key::F10,
        K::F11 => Key::F11,
        K::F12 => Key::F12,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The number row and the numeric keypad are the same digit to whoever is
    /// typing one, and egui has a single key for both.
    #[test]
    fn the_keypad_types_the_same_digits_as_the_number_row() {
        assert_eq!(translate_key(Keycode::Num7), Some(Key::Num7));
        assert_eq!(translate_key(Keycode::Kp7), Some(Key::Num7));
        assert_eq!(translate_key(Keycode::KpEnter), Some(Key::Enter));
    }

    /// A key egui has no name for is dropped. Approximating it would bind a
    /// shortcut to the wrong key, which is worse than the key doing nothing.
    #[test]
    fn a_key_egui_cannot_name_is_dropped() {
        assert_eq!(translate_key(Keycode::PrintScreen), None);
        assert_eq!(translate_key(Keycode::NumLockClear), None);
    }

    /// A pointer arrives in points and is passed on untouched. Scaling it by
    /// the display's factor put every click a row above the icon it was made
    /// on, because SDL had already done that conversion.
    #[test]
    fn a_pointer_position_is_passed_through_unscaled() {
        let mut input = RawInput::default();
        let mut consumed = Consumed::default();
        absorb(
            &SdlEvent::MouseMotion {
                timestamp: 0,
                window_id: 0,
                which: 0,
                mousestate: sdl2::mouse::MouseState::from_sdl_state(0),
                x: 350,
                y: 175,
                xrel: 0,
                yrel: 0,
            },
            &mut input,
            &mut consumed,
        );
        assert_eq!(
            input.events,
            vec![Event::PointerMoved(Pos2::new(350.0, 175.0))]
        );
    }

    /// A pointer button carries whatever modifiers the frame started with,
    /// so a Ctrl-click is one even when the Ctrl went down frames earlier.
    #[test]
    fn a_click_carries_the_modifiers_the_frame_started_with() {
        let mut input = RawInput::default();
        input.modifiers = Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let mut consumed = Consumed::default();
        absorb(
            &SdlEvent::MouseButtonDown {
                timestamp: 0,
                window_id: 0,
                which: 0,
                mouse_btn: MouseButton::Left,
                clicks: 1,
                x: 10,
                y: 20,
            },
            &mut input,
            &mut consumed,
        );
        let Event::PointerButton { modifiers, .. } = input.events[0] else {
            panic!("expected a pointer button, got {:?}", input.events[0]);
        };
        assert!(modifiers.ctrl);
    }

    /// Closing is the shell's business. egui is told the window wants to go
    /// away and can refuse, but it never sees the event itself.
    #[test]
    fn the_close_button_is_the_shells_to_act_on() {
        let mut input = RawInput::default();
        let mut consumed = Consumed::default();
        absorb(&SdlEvent::Quit { timestamp: 0 }, &mut input, &mut consumed);
        assert!(consumed.close_requested);
        assert!(input.events.is_empty());
    }
}
