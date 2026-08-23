/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The window, composed for a touchscreen and a small one.
//!
//! Empty until tapHLE has a mobile entry point to draw it. It is declared now
//! because the shape of the interface is the reason [crate::ui] is split the
//! way it is, and a schema with one leaf does not explain itself.
//!
//! What belongs here when it arrives: the same [crate::state] and the same
//! [crate::ui::widgets], arranged for one hand. The details panel becomes a
//! sheet pulled up from the bottom; the log becomes a screen of its own
//! rather than a dock; primary actions sit within thumb reach.
//!
//! What does not arrive here at all is as important. A touchscreen device
//! needs no control-placement editor, because the guest's touch is the
//! person's touch — unless a controller is connected, which is the one case
//! that brings it back. Tilt settings exist to fake an accelerometer a
//! desktop has not got. Window and fullscreen settings describe a window
//! nobody can move. Roughly half of [crate::ui::desktop::settings_dialog] is
//! desktop-only by nature, so the mobile client is genuinely smaller rather
//! than the same one squeezed.
