/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! OpenGL ES and EAGL.
//!
//! This module is specific to OpenGL ES's role as a part of the iPhone OS API
//! surface. See [crate::gles] for other uses and a discussion of the broader
//! topic.

mod eagl;
mod gles_guest;

use std::path::PathBuf;

use tapHLE_gl_bindings::gles11::types::GLenum;

use crate::mem::ConstPtr;

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/OpenGLES.framework/OpenGLES",
    aliases: &[],
    class_exports: &[eagl::CLASSES],
    constant_exports: &[eagl::CONSTANTS],
    function_exports: &[gles_guest::FUNCTIONS, eagl::FUNCTIONS],
};

const FRAME_CAPTURE_REQUEST_ENV: &str = "TAPHLE_FRAME_CAPTURE_REQUEST";
const FRAME_CAPTURE_OUTPUT_ENV: &str = "TAPHLE_FRAME_CAPTURE_OUTPUT";

pub(crate) struct FrameCaptureRequest {
    pub(crate) marker_path: PathBuf,
    pub(crate) output_path: PathBuf,
}

pub struct State {
    /// Current EAGLContext for each thread
    current_ctxs: std::collections::HashMap<crate::ThreadId, Option<crate::objc::id>>,
    strings_cache: std::collections::HashMap<GLenum, ConstPtr<u8>>,
    /// Optional one-shot frame capture configured by the host environment.
    frame_capture_request: Option<FrameCaptureRequest>,
}

impl Default for State {
    fn default() -> Self {
        let request_path = std::env::var_os(FRAME_CAPTURE_REQUEST_ENV);
        let output_path = std::env::var_os(FRAME_CAPTURE_OUTPUT_ENV);
        let frame_capture_request = match (request_path, output_path) {
            (Some(marker_path), Some(output_path))
                if !marker_path.is_empty() && !output_path.is_empty() =>
            {
                Some(FrameCaptureRequest {
                    marker_path: marker_path.into(),
                    output_path: output_path.into(),
                })
            }
            (None, None) => None,
            _ => {
                log!(
                    "Warning: frame capture is disabled because {} and {} must both be set to non-empty paths.",
                    FRAME_CAPTURE_REQUEST_ENV,
                    FRAME_CAPTURE_OUTPUT_ENV,
                );
                None
            }
        };

        Self {
            current_ctxs: Default::default(),
            strings_cache: Default::default(),
            frame_capture_request,
        }
    }
}

impl State {
    fn current_ctx_for_thread(&mut self, thread: crate::ThreadId) -> &mut Option<crate::objc::id> {
        self.current_ctxs.entry(thread).or_insert(None);
        self.current_ctxs.get_mut(&thread).unwrap()
    }

    /// Return and disarm a configured capture once its marker appears.
    pub(crate) fn take_triggered_frame_capture(&mut self) -> Option<FrameCaptureRequest> {
        let request = self.frame_capture_request.as_ref()?;
        match std::fs::metadata(&request.marker_path) {
            Ok(metadata) if metadata.is_file() => self.frame_capture_request.take(),
            Ok(_) => {
                let request = self.frame_capture_request.take().unwrap();
                log!(
                    "Warning: frame capture marker {} is not a file; capture is disabled.",
                    request.marker_path.display(),
                );
                None
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => {
                let request = self.frame_capture_request.take().unwrap();
                log!(
                    "Warning: could not inspect frame capture marker {}: {}; capture is disabled.",
                    request.marker_path.display(),
                    err,
                );
                None
            }
        }
    }

    /// Re-arm a successfully completed capture. The harness must move the
    /// output and create a new marker before another capture can occur.
    pub(crate) fn rearm_frame_capture(&mut self, request: FrameCaptureRequest) {
        assert!(self.frame_capture_request.is_none());
        self.frame_capture_request = Some(request);
    }
}

fn sync_context<'objc, 'win: 'objc>(
    state: &mut State,
    objc: &'objc mut crate::objc::ObjC,
    window: &'win mut crate::window::Window,
    current_thread: crate::ThreadId,
) -> Box<dyn crate::gles::GLES + 'objc> {
    let gles_ctx = get_thread_context(state, objc, current_thread);
    gles_ctx.make_current(window)
}

fn get_thread_context<'objc>(
    state: &mut State,
    objc: &'objc mut crate::objc::ObjC,
    current_thread: crate::ThreadId,
) -> &'objc mut dyn crate::gles::GLESContext {
    let current_ctx = state.current_ctx_for_thread(current_thread);
    let host_obj = objc.borrow_mut::<eagl::EAGLContextHostObject>(current_ctx.unwrap());
    let gles_ctx = host_obj.gles_ctx.as_deref_mut().unwrap();
    gles_ctx
}
