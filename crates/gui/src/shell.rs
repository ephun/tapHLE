/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The window the interface is drawn in, and the loop that drives it.
//!
//! This is what `eframe` used to be. egui is the interface library and knows
//! nothing about windows; something has to open one, hand egui the input,
//! paint the triangles it returns, and act on what it asks for afterwards.
//! `eframe` does that with winit, which is a good desktop answer and not a
//! mobile one.
//!
//! SDL is the answer here instead, because the emulator already opens an SDL
//! window with a GL context on every platform tapHLE targets — iOS and
//! Android included — and `android/` is already an `SDLActivity`. Using the
//! same window system for the frontend means one input, lifecycle and
//! graphics layer for the whole product rather than one for the desktop and a
//! different one for each phone. It also means that when the frontend and the
//! emulator become one process, there is one event loop to merge rather than
//! two.
//!
//! Nothing above this module knows SDL is here. [Application] is the whole
//! contract, and it is the shape `eframe::App` had.

pub mod input;

use std::sync::Arc;

use egui::{Pos2, Rect, Vec2, ViewportCommand, ViewportId, ViewportInfo};

/// What the shell draws.
///
/// Deliberately the shape of `eframe::App`: an interface that is handed a
/// context once a frame and reports what it wants through the context. The
/// shell may call [Application::update] many times between paints, because
/// egui can ask for another pass, so it must stay a function of state.
pub trait Application {
    fn update(&mut self, ctx: &egui::Context);

    /// Whether the application wants the window's input to itself for a
    /// while, blocking this one.
    ///
    /// SDL allows exactly one `EventPump` in a process, so a second thing
    /// that reads input — the emulator, running an app in here rather than in
    /// a process of its own — cannot start one while the shell holds its own.
    /// Asking first is how the shell knows to put its pump down.
    fn wants_to_hand_over(&mut self) -> bool {
        false
    }

    /// Do the blocking thing [Application::wants_to_hand_over] asked for.
    ///
    /// Called with no event pump in existence, so whatever runs here may make
    /// its own. The shell takes its pump back afterwards.
    fn hand_over(&mut self, _lease: Option<tapHLE::AppWindowLease>) {}

    /// Called once, after the loop ends and before the window closes.
    fn on_exit(&mut self) {}
}

/// How the window should first appear.
pub struct WindowSettings {
    pub title: String,
    pub size: [f32; 2],
    pub minimum_size: [f32; 2],
    pub position: Option<[f32; 2]>,
    pub maximized: bool,
    /// RGBA, and its dimensions.
    pub icon: Option<(Vec<u8>, u32, u32)>,
}

fn create_renderer(
    video: &sdl2::VideoSubsystem,
    settings: &WindowSettings,
) -> Result<
    (
        sdl2::video::Window,
        sdl2::video::GLContext,
        egui_glow::Painter,
        Borders,
    ),
    String,
> {
    request_gl(video);
    let mut builder = video.window(
        &settings.title,
        settings.size[0] as u32,
        settings.size[1] as u32,
    );
    builder.opengl().resizable().allow_highdpi();
    match settings.position {
        Some([x, y]) if x > -20_000.0 && y > -20_000.0 => {
            builder.position(x as i32, y as i32);
        }
        _ => {
            builder.position_centered();
        }
    }
    if settings.maximized {
        builder.maximized();
    }
    let mut window = builder.build().map_err(|e| e.to_string())?;
    window
        .set_minimum_size(
            settings.minimum_size[0] as u32,
            settings.minimum_size[1] as u32,
        )
        .map_err(|e| e.to_string())?;
    if let Some((rgba, width, height)) = &settings.icon {
        set_icon(&mut window, rgba, *width, *height);
    }
    let borders = Borders::of(&window);
    if let Some([x, y]) = settings.position {
        window.set_position(
            sdl2::video::WindowPos::Positioned((x + borders.left) as i32),
            sdl2::video::WindowPos::Positioned((y + borders.top) as i32),
        );
    }
    let gl_context = window.gl_create_context()?;
    window.gl_make_current(&gl_context)?;
    let _ = video.gl_set_swap_interval(sdl2::video::SwapInterval::VSync);
    let gl = Arc::new(unsafe {
        glow::Context::from_loader_function(|name| video.gl_get_proc_address(name) as *const _)
    });
    let painter = egui_glow::Painter::new(gl, "", None, false).map_err(|e| e.to_string())?;
    Ok((window, gl_context, painter, borders))
}

/// Open a window, draw `build`'s application in it until it is closed.
///
/// `build` is handed the egui context rather than being given one afterwards,
/// because an interface wants to set its fonts and its style before the first
/// frame rather than have one frame look different from the rest.
pub fn run<A: Application>(
    settings: WindowSettings,
    build: impl FnOnce(&egui::Context) -> A,
) -> Result<(), String> {
    // Windows scales a DPI-unaware program by blowing up its bitmap, which
    // for an interface made of text means a soft, slightly wrong-sized
    // window. Asking for per-monitor awareness before SDL initialises is what
    // makes `drawable_size` larger than `size` further down, and that ratio
    // is the whole of tapHLE's HiDPI handling.
    sdl2::hint::set("SDL_WINDOWS_DPI_AWARENESS", "permonitorv2");
    sdl2::hint::set("SDL_WINDOWS_DPI_SCALING", "1");

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let (mut window, _gl_context, mut painter, borders) = create_renderer(&video, &settings)?;

    // A repaint asked for from a background thread — a finished download, an
    // app that has just exited — has to wake the loop. Without this the
    // window sleeps in SDL until the next input, and work that finished a
    // second ago appears whenever the pointer next moves.
    let events = sdl.event()?;
    events.register_custom_event::<Wake>()?;
    let waker = events.event_sender();
    let egui_ctx = egui::Context::default();
    egui_ctx.set_request_repaint_callback(move |_| {
        let _ = waker.push_custom_event(Wake);
    });
    let mut app = build(&egui_ctx);

    // SDL delivers no typed text at all until text input is started, and
    // starting it is also what raises the on-screen keyboard on a touch
    // device. So it follows the focus: on while egui has a text field to type
    // into, off otherwise. It starts off, because a window that opens with
    // the keyboard already up is wrong on a phone.
    video.text_input().stop();
    let mut typing = false;

    let mut event_pump = sdl.event_pump()?;
    let mut viewport = ViewportInfo {
        native_pixels_per_point: Some(pixels_per_point(&window)),
        focused: Some(true),
        ..Default::default()
    };
    read_geometry(&window, borders, &mut viewport);
    let mut cursor = CursorState::new();
    let mut pending: Vec<sdl2::event::Event> = Vec::new();
    let mut closing = false;
    let started = std::time::Instant::now();

    'frames: loop {
        let ppp = pixels_per_point(&window);
        let mut consumed = input::Consumed::default();
        let mut raw = egui::RawInput {
            time: Some(started.elapsed().as_secs_f64()),
            max_texture_side: Some(painter.max_texture_side()),
            // Read live rather than remembered from the last key event, so a
            // Ctrl-click is one whenever the Ctrl is down.
            modifiers: input::modifiers_now(&sdl.keyboard()),
            ..Default::default()
        };
        for event in pending.drain(..).chain(event_pump.poll_iter()) {
            input::absorb(&event, &mut raw, &mut consumed);
        }
        if let Some(focused) = consumed.focus {
            viewport.focused = Some(focused);
        }
        raw.focused = viewport.focused.unwrap_or(true);
        if consumed.geometry_changed {
            read_geometry(&window, borders, &mut viewport);
        }
        if consumed.wants_paste {
            if let Ok(text) = video.clipboard().clipboard_text() {
                raw.events.push(egui::Event::Paste(text));
            }
        }
        if consumed.close_requested {
            viewport.events.push(egui::ViewportEvent::Close);
        }
        viewport.native_pixels_per_point = Some(ppp);
        let (width, height) = window.drawable_size();
        let screen_rect =
            Rect::from_min_size(Pos2::ZERO, Vec2::new(width as f32, height as f32) / ppp);
        raw.screen_rect = Some(safe_area_rect(screen_rect));
        raw.viewports = std::iter::once((ViewportId::ROOT, viewport.take())).collect();

        let output = egui_ctx.run(raw, |ctx| app.update(ctx));

        for command in &output.platform_output.commands {
            match command {
                egui::OutputCommand::CopyText(text) => {
                    let _ = video.clipboard().set_clipboard_text(text);
                }
                egui::OutputCommand::OpenUrl(url) => {
                    let _ = crate::platform::process::open_in_desktop(&url.url);
                }
                egui::OutputCommand::CopyImage(_) => {}
            }
        }
        cursor.apply(&sdl, output.platform_output.cursor_icon);
        // `wants_keyboard_input` is the question actually being asked: does
        // egui have somewhere to put a character. Gating this on the IME
        // output instead looked equivalent and was not — it left SDL sending
        // no text at all, so the search box could be clicked and not typed
        // into.
        let wants_typing = egui_ctx.wants_keyboard_input();
        if wants_typing != typing {
            typing = wants_typing;
            if typing {
                video.text_input().start();
            } else {
                video.text_input().stop();
            }
        }

        let mut wait = std::time::Duration::MAX;
        for (id, out) in &output.viewport_output {
            if *id != ViewportId::ROOT {
                continue;
            }
            wait = wait.min(out.repaint_delay);
            for command in &out.commands {
                match command {
                    ViewportCommand::Close => closing = true,
                    ViewportCommand::CancelClose => closing = false,
                    ViewportCommand::Title(title) => {
                        let _ = window.set_title(title);
                    }
                    _ => {}
                }
            }
        }
        if consumed.close_requested && !closing {
            // egui saw the close and said nothing about it, which means it
            // does not object. Only a viewport command can keep the window.
            closing = !output.viewport_output.values().any(|out| {
                out.commands
                    .iter()
                    .any(|c| matches!(c, ViewportCommand::CancelClose))
            });
        }

        let primitives = egui_ctx.tessellate(output.shapes, ppp);
        painter.clear(
            [width, height],
            egui::Rgba::from(egui::Color32::BLACK).to_array(),
        );
        painter.paint_and_update_textures(
            [width, height],
            ppp,
            &primitives,
            &output.textures_delta,
        );
        window.gl_swap_window();

        if closing {
            break 'frames;
        }

        // Handing over: the pump has to be dropped, not merely unused, before
        // anything else can make one. Taken back straight afterwards, and the
        // frontend has been frozen the whole time — which is the honest cost
        // of running an app in here rather than in a process of its own.
        if app.wants_to_hand_over() {
            drop(event_pump);
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                let lease = tapHLE::AppWindowLease::new(sdl.clone(), video.clone(), window.clone());
                app.hand_over(Some(lease));
                request_gl(&video);
                window.gl_make_current(&_gl_context)?;
                let _ = video.gl_set_swap_interval(sdl2::video::SwapInterval::VSync);
                event_pump = sdl.event_pump()?;
                video.text_input().stop();
                typing = false;
                viewport.native_pixels_per_point = Some(pixels_per_point(&window));
                read_geometry(&window, borders, &mut viewport);
            }
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                app.hand_over(None);
                event_pump = sdl.event_pump()?;
            }
            // Whatever happened while the pump was gone is not this window's
            // to replay, and the frontend has a frame's worth of catching up.
            pending.clear();
            egui_ctx.request_repaint();
        }
        // Nothing to redraw and nothing waiting: sleep in SDL until something
        // happens, rather than spinning a core to draw the same window again.
        if !wait.is_zero() {
            let millis = wait.as_millis().min(u32::from(u16::MAX).into()) as u32;
            if millis > 0 {
                // Kept rather than dropped: the event that ended the wait is
                // usually the click the person just made.
                pending.extend(event_pump.wait_event_timeout(millis));
            }
        }
    }

    app.on_exit();
    painter.destroy();
    Ok(())
}

#[cfg(target_os = "ios")]
extern "C" {
    fn tapHLE_iOS_safe_area(top: *mut f32, right: *mut f32, bottom: *mut f32, left: *mut f32);
}

fn safe_area_rect(rect: Rect) -> Rect {
    #[cfg(target_os = "ios")]
    {
        let (mut top, mut right, mut bottom, mut left) = (0.0, 0.0, 0.0, 0.0);
        unsafe { tapHLE_iOS_safe_area(&mut top, &mut right, &mut bottom, &mut left) };
        if [top, right, bottom, left]
            .into_iter()
            .all(|inset| inset.is_finite() && inset >= 0.0)
        {
            let safe = Rect::from_min_max(
                rect.min + Vec2::new(left, top),
                rect.max - Vec2::new(right, bottom),
            );
            if safe.is_positive() {
                return safe;
            }
        }
    }
    rect
}

/// Nothing but a marker: pushing any custom event into SDL ends a wait, and
/// what the event says does not matter.
struct Wake;

/// How much bigger a window's frame is than the area drawn in, in points.
///
/// Points, because everything else about a window's geometry is in points and
/// SDL is the odd one out here: it reports a border in physical pixels while
/// reporting the position it sits at in scaled ones. Mixing the two put the
/// window 22 points below where it was left, every run, on a display at 175%.
#[derive(Copy, Clone, Default)]
struct Borders {
    top: f32,
    left: f32,
    bottom: f32,
    right: f32,
}

impl Borders {
    fn of(window: &sdl2::video::Window) -> Borders {
        let scale = pixels_per_point(window);
        match window.border_size() {
            Ok((top, left, bottom, right)) => Borders {
                top: f32::from(top) / scale,
                left: f32::from(left) / scale,
                bottom: f32::from(bottom) / scale,
                right: f32::from(right) / scale,
            },
            // Not every window system will say. A window a few points from
            // where it was left is a much smaller problem than not opening.
            Err(_) => Borders::default(),
        }
    }
}

/// Ask for the OpenGL the interface is painted with.
///
/// Desktop drivers all offer a 3.3 core profile and that is what egui's
/// painter is written against. A phone has GLES instead, and asking for a
/// desktop profile there fails outright rather than falling back.
fn request_gl(video: &sdl2::VideoSubsystem) {
    let attr = video.gl_attr();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        attr.set_context_profile(sdl2::video::GLProfile::GLES);
        attr.set_context_version(3, 0);
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        attr.set_context_profile(sdl2::video::GLProfile::Core);
        attr.set_context_version(3, 3);
    }
    attr.set_double_buffer(true);
}

/// How many physical pixels there are to one of egui's points.
///
/// Read from the window rather than from the display, because they disagree
/// the moment a window is dragged between two monitors set to different
/// scalings and only the window knows which one it is on.
fn pixels_per_point(window: &sdl2::video::Window) -> f32 {
    let (points, _) = window.size();
    let (pixels, _) = window.drawable_size();
    if points == 0 {
        return 1.0;
    }
    let scale = pixels as f32 / points as f32;
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

/// Tell egui where the window is and what state it is in.
///
/// The interface reads this to remember its geometry between runs, so the
/// rectangles are in points and match what was asked for at startup.
fn read_geometry(window: &sdl2::video::Window, borders: Borders, viewport: &mut ViewportInfo) {
    let (width, height) = window.size();
    let (x, y) = window.position();
    let inner = Rect::from_min_size(
        Pos2::new(x as f32, y as f32),
        Vec2::new(width as f32, height as f32),
    );
    let flags = window.window_flags();
    viewport.inner_rect = Some(inner);
    viewport.outer_rect = Some(Rect::from_min_max(
        inner.min - Vec2::new(borders.left, borders.top),
        inner.max + Vec2::new(borders.right, borders.bottom),
    ));
    viewport.maximized = Some(flags & sdl2::sys::SDL_WindowFlags::SDL_WINDOW_MAXIMIZED as u32 != 0);
    viewport.minimized = Some(flags & sdl2::sys::SDL_WindowFlags::SDL_WINDOW_MINIMIZED as u32 != 0);
    viewport.fullscreen =
        Some(flags & sdl2::sys::SDL_WindowFlags::SDL_WINDOW_FULLSCREEN as u32 != 0);
}

fn set_icon(window: &mut sdl2::video::Window, rgba: &[u8], width: u32, height: u32) {
    let mut pixels = rgba.to_vec();
    let Ok(surface) = sdl2::surface::Surface::from_data(
        &mut pixels,
        width,
        height,
        width * 4,
        sdl2::pixels::PixelFormatEnum::ABGR8888,
    ) else {
        return;
    };
    window.set_icon(surface);
}

/// The pointer, changed only when egui asks for a different one.
///
/// SDL frees a cursor when it is dropped, and the current one is not allowed
/// to be the freed one, so the live cursor is held here for as long as it is
/// set. Setting it every frame would also allocate a system cursor per frame.
struct CursorState {
    current: egui::CursorIcon,
    held: Option<sdl2::mouse::Cursor>,
}

impl CursorState {
    fn new() -> CursorState {
        CursorState {
            current: egui::CursorIcon::Default,
            held: None,
        }
    }

    fn apply(&mut self, sdl: &sdl2::Sdl, icon: egui::CursorIcon) {
        if icon == self.current {
            return;
        }
        self.current = icon;
        let mouse = sdl.mouse();
        if icon == egui::CursorIcon::None {
            mouse.show_cursor(false);
            return;
        }
        mouse.show_cursor(true);
        if let Ok(cursor) = sdl2::mouse::Cursor::from_system(system_cursor(icon)) {
            cursor.set();
            self.held = Some(cursor);
        }
    }
}

/// The nearest thing SDL has to each of egui's cursors.
///
/// SDL offers ten shapes against egui's thirty-odd, so most of them land on
/// the arrow. That is the honest answer: a wrong-but-plausible shape tells
/// somebody they can do something they cannot.
fn system_cursor(icon: egui::CursorIcon) -> sdl2::mouse::SystemCursor {
    use egui::CursorIcon as C;
    use sdl2::mouse::SystemCursor as S;
    match icon {
        C::Text | C::VerticalText => S::IBeam,
        C::Wait | C::Progress => S::WaitArrow,
        C::Crosshair | C::Cell => S::Crosshair,
        C::PointingHand => S::Hand,
        C::NotAllowed | C::NoDrop => S::No,
        C::ResizeHorizontal | C::ResizeEast | C::ResizeWest | C::ResizeColumn => S::SizeWE,
        C::ResizeVertical | C::ResizeNorth | C::ResizeSouth | C::ResizeRow => S::SizeNS,
        C::ResizeNwSe | C::ResizeNorthWest | C::ResizeSouthEast => S::SizeNWSE,
        C::ResizeNeSw | C::ResizeNorthEast | C::ResizeSouthWest => S::SizeNESW,
        C::Move | C::Grab | C::Grabbing | C::AllScroll => S::SizeAll,
        _ => S::Arrow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cursor egui asks for that SDL has no shape for becomes the arrow
    /// rather than something that looks nearly right. Every resize cursor
    /// does have a real shape, because those are the ones that say a panel
    /// edge can be dragged.
    #[test]
    fn every_resize_cursor_has_a_shape_of_its_own() {
        use egui::CursorIcon as C;
        use sdl2::mouse::SystemCursor as S;
        for icon in [C::ResizeHorizontal, C::ResizeEast, C::ResizeColumn] {
            assert_eq!(system_cursor(icon), S::SizeWE, "{icon:?}");
        }
        for icon in [C::ResizeVertical, C::ResizeNorth, C::ResizeRow] {
            assert_eq!(system_cursor(icon), S::SizeNS, "{icon:?}");
        }
        assert_eq!(system_cursor(C::Text), S::IBeam);
        assert_eq!(system_cursor(C::Default), S::Arrow);
        assert_eq!(system_cursor(C::ContextMenu), S::Arrow);
    }
}
