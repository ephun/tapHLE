/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Generic OpenGL ES 1.1 interface.
//!
//! Unfortunately this does not provide the types and constants, so the correct
//! usage is to import `GLES` and `types` from this module, but get the
//! constants from [super::gles11_raw].

use crate::window::{GLContext, Window};

use super::gles11_raw::types::*;

/// `GLchar` from the ES 2.0 type set. Not defined by the ES 1.1 registry, so
/// we provide our own alias here for use in the [GLES] trait's ES 2.0 entry
/// points.
pub type GLchar = std::os::raw::c_char;

/// Trait representing an OpenGL ES implementation and context.
///
/// The GL context is not necessarily active, so GL functions can't be called
/// from this trait. It can be made active from [GLESContext::make_current].
#[allow(clippy::upper_case_acronyms)]
pub trait GLESContext {
    /// Get a human-friendly description of this implementation.
    fn description() -> &'static str
    where
        Self: Sized;

    /// Construct a new context. This might fail if the host OS doesn't have a
    /// compatible driver, for example.
    #[allow(clippy::new_ret_no_self)]
    fn new(window: &mut crate::window::Window) -> Result<Self, String>
    where
        Self: Sized;

    /// Make this context (and any underlying context) the active OpenGL
    /// context.
    ///
    /// The lifetime ensures safety - the GLES object can't be destroyed while
    /// the instance is active, so the OpenGL state remains valid, and the
    /// window reference prevents the thread from yielding while the GLES
    /// object is being used, and prevents multiple contexts from existing at
    /// the same time (which can cause a UAF).
    fn make_current<'gl_ctx, 'win: 'gl_ctx>(
        &'gl_ctx mut self,
        window: &'win mut Window,
    ) -> Box<dyn GLES + 'gl_ctx>;

    /// Make this context (and any underlying context) the active OpenGL
    /// context, without checking if it is the only context. You shouldn't use
    /// this outside of [crate::window::Window], as this is function exists to
    /// work around lifetime splitting issues inside of it.
    ///
    /// SAFETY: Callers must ensure that this is the only active context,
    /// that the GLES instance does not outlive the self or window
    /// parameter, that make_current_fn makes the passed context current,
    /// and that loader_fn properly loads the requested function.
    unsafe fn make_current_unchecked_for_window<'gl_ctx>(
        &'gl_ctx mut self,
        make_current_fn: &mut dyn FnMut(&GLContext),
        loader_fn: &mut dyn FnMut(&'static str) -> *const std::ffi::c_void,
    ) -> Box<dyn GLES + 'gl_ctx>;
}

/// An active GLES context that can be used.
///
/// These are effectively direct wrappers around the raw OpenGL functions,
/// but they make sure that the context is active while it is using it.
/// # Safety
/// These functions (should) act as documented by the OpenGL ES spec. Callers
/// should ensure that all uses of raw pointers are verfied to be valid and
/// of the correct size as documented in the OpenGL ES spec.
#[allow(clippy::upper_case_acronyms)]
#[allow(clippy::too_many_arguments)] // not our fault :(
#[allow(unused_variables)]
// This is a binding surface, not application code: it names every entry point
// a backend may implement, and which of them a guest happens to call is the
// app's business. A method nothing calls yet is the normal state here, so
// dead-code analysis has nothing useful to say about this trait.
#[allow(dead_code)]
pub trait GLES {
    /// Get some string describing the underlying driver. For OpenGL this is
    /// `GL_VENDOR`, `GL_RENDERER` and `GL_VERSION`.
    unsafe fn driver_description(&self) -> String {
        unimplemented!("driver_description not implemented by this backend")
    }
    /// Returns `true` if this backend is a real OpenGL ES 2.0 / 3.0 driver and
    /// therefore does NOT support fixed-function pipeline calls (`MatrixMode`,
    /// `EnableClientState`, `Color4f`, …). Used by `present_renderbuffer` so it
    /// can take a shader-based code path on such backends.
    fn is_es2(&self) -> bool {
        false
    }
    // Generic state manipulation
    unsafe fn GetError(&mut self) -> GLenum {
        unimplemented!("GetError not implemented by this backend")
    }
    unsafe fn Enable(&mut self, cap: GLenum) {
        unimplemented!("Enable not implemented by this backend")
    }
    unsafe fn IsEnabled(&mut self, cap: GLenum) -> GLboolean {
        unimplemented!("IsEnabled not implemented by this backend")
    }
    unsafe fn Disable(&mut self, cap: GLenum) {
        unimplemented!("Disable not implemented by this backend")
    }
    unsafe fn ClientActiveTexture(&mut self, texture: GLenum) {
        unimplemented!("ClientActiveTexture not implemented by this backend")
    }
    unsafe fn EnableClientState(&mut self, array: GLenum) {
        unimplemented!("EnableClientState not implemented by this backend")
    }
    unsafe fn DisableClientState(&mut self, array: GLenum) {
        unimplemented!("DisableClientState not implemented by this backend")
    }
    unsafe fn GetBooleanv(&mut self, pname: GLenum, params: *mut GLboolean) {
        unimplemented!("GetBooleanv not implemented by this backend")
    }
    unsafe fn GetFloatv(&mut self, pname: GLenum, params: *mut GLfloat) {
        unimplemented!("GetFloatv not implemented by this backend")
    }
    unsafe fn GetIntegerv(&mut self, pname: GLenum, params: *mut GLint) {
        unimplemented!("GetIntegerv not implemented by this backend")
    }
    unsafe fn GetTexEnviv(&mut self, target: GLenum, pname: GLenum, params: *mut GLint) {
        unimplemented!("GetTexEnviv not implemented by this backend")
    }
    unsafe fn GetTexEnvfv(&mut self, target: GLenum, pname: GLenum, params: *mut GLfloat) {
        unimplemented!("GetTexEnvfv not implemented by this backend")
    }
    unsafe fn GetPointerv(&mut self, pname: GLenum, params: *mut *const GLvoid) {
        unimplemented!("GetPointerv not implemented by this backend")
    }
    unsafe fn Hint(&mut self, target: GLenum, mode: GLenum) {
        unimplemented!("Hint not implemented by this backend")
    }
    unsafe fn Finish(&mut self) {
        unimplemented!("Finish not implemented by this backend")
    }
    unsafe fn Flush(&mut self) {
        unimplemented!("Flush not implemented by this backend")
    }
    #[allow(dead_code)]
    unsafe fn GetString(&mut self, name: GLenum) -> *const GLubyte {
        unimplemented!("GetString not implemented by this backend")
    }

    // Other state manipulation
    unsafe fn AlphaFunc(&mut self, func: GLenum, ref_: GLclampf) {
        unimplemented!("AlphaFunc not implemented by this backend")
    }
    unsafe fn AlphaFuncx(&mut self, func: GLenum, ref_: GLclampx) {
        unimplemented!("AlphaFuncx not implemented by this backend")
    }
    unsafe fn BlendFunc(&mut self, sfactor: GLenum, dfactor: GLenum) {
        unimplemented!("BlendFunc not implemented by this backend")
    }
    unsafe fn BlendEquationOES(&mut self, mode: GLenum) {
        unimplemented!("BlendEquationOES not implemented by this backend")
    }
    unsafe fn ColorMask(
        &mut self,
        red: GLboolean,
        green: GLboolean,
        blue: GLboolean,
        alpha: GLboolean,
    ) {
        unimplemented!("ColorMask not implemented by this backend")
    }
    unsafe fn ClipPlanef(&mut self, plane: GLenum, equation: *const GLfloat) {
        unimplemented!("ClipPlanef not implemented by this backend")
    }
    unsafe fn ClipPlanex(&mut self, plane: GLenum, equation: *const GLfixed) {
        unimplemented!("ClipPlanex not implemented by this backend")
    }
    unsafe fn CullFace(&mut self, mode: GLenum) {
        unimplemented!("CullFace not implemented by this backend")
    }
    unsafe fn DepthFunc(&mut self, func: GLenum) {
        unimplemented!("DepthFunc not implemented by this backend")
    }
    unsafe fn DepthMask(&mut self, flag: GLboolean) {
        unimplemented!("DepthMask not implemented by this backend")
    }
    unsafe fn DepthRangef(&mut self, near: GLclampf, far: GLclampf) {
        unimplemented!("DepthRangef not implemented by this backend")
    }
    unsafe fn DepthRangex(&mut self, near: GLclampx, far: GLclampx) {
        unimplemented!("DepthRangex not implemented by this backend")
    }
    unsafe fn FrontFace(&mut self, mode: GLenum) {
        unimplemented!("FrontFace not implemented by this backend")
    }
    unsafe fn PolygonOffset(&mut self, factor: GLfloat, units: GLfloat) {
        unimplemented!("PolygonOffset not implemented by this backend")
    }
    unsafe fn PolygonOffsetx(&mut self, factor: GLfixed, units: GLfixed) {
        unimplemented!("PolygonOffsetx not implemented by this backend")
    }
    unsafe fn SampleCoverage(&mut self, value: GLclampf, invert: GLboolean) {
        unimplemented!("SampleCoverage not implemented by this backend")
    }
    unsafe fn SampleCoveragex(&mut self, value: GLclampx, invert: GLboolean) {
        unimplemented!("SampleCoveragex not implemented by this backend")
    }
    unsafe fn ShadeModel(&mut self, mode: GLenum) {
        unimplemented!("ShadeModel not implemented by this backend")
    }
    unsafe fn Scissor(&mut self, x: GLint, y: GLint, width: GLsizei, height: GLsizei) {
        unimplemented!("Scissor not implemented by this backend")
    }
    unsafe fn Viewport(&mut self, x: GLint, y: GLint, width: GLsizei, height: GLsizei) {
        unimplemented!("Viewport not implemented by this backend")
    }
    unsafe fn LineWidth(&mut self, val: GLfloat) {
        unimplemented!("LineWidth not implemented by this backend")
    }
    unsafe fn LineWidthx(&mut self, val: GLfixed) {
        unimplemented!("LineWidthx not implemented by this backend")
    }
    unsafe fn StencilFunc(&mut self, func: GLenum, ref_: GLint, mask: GLuint) {
        unimplemented!("StencilFunc not implemented by this backend")
    }
    unsafe fn StencilOp(&mut self, sfail: GLenum, dpfail: GLenum, dppass: GLenum) {
        unimplemented!("StencilOp not implemented by this backend")
    }
    unsafe fn StencilMask(&mut self, mask: GLuint) {
        unimplemented!("StencilMask not implemented by this backend")
    }
    unsafe fn LogicOp(&mut self, opcode: GLenum) {
        unimplemented!("LogicOp not implemented by this backend")
    }

    // Points
    unsafe fn PointSize(&mut self, size: GLfloat) {
        unimplemented!("PointSize not implemented by this backend")
    }
    unsafe fn PointSizex(&mut self, size: GLfixed) {
        unimplemented!("PointSizex not implemented by this backend")
    }
    unsafe fn PointParameterf(&mut self, pname: GLenum, param: GLfloat) {
        unimplemented!("PointParameterf not implemented by this backend")
    }
    unsafe fn PointParameterx(&mut self, pname: GLenum, param: GLfixed) {
        unimplemented!("PointParameterx not implemented by this backend")
    }
    unsafe fn PointParameterfv(&mut self, pname: GLenum, params: *const GLfloat) {
        unimplemented!("PointParameterfv not implemented by this backend")
    }
    unsafe fn PointParameterxv(&mut self, pname: GLenum, params: *const GLfixed) {
        unimplemented!("PointParameterxv not implemented by this backend")
    }

    // Lighting and materials
    unsafe fn Fogf(&mut self, pname: GLenum, param: GLfloat) {
        unimplemented!("Fogf not implemented by this backend")
    }
    unsafe fn Fogx(&mut self, pname: GLenum, param: GLfixed) {
        unimplemented!("Fogx not implemented by this backend")
    }
    unsafe fn Fogfv(&mut self, pname: GLenum, params: *const GLfloat) {
        unimplemented!("Fogfv not implemented by this backend")
    }
    unsafe fn Fogxv(&mut self, pname: GLenum, params: *const GLfixed) {
        unimplemented!("Fogxv not implemented by this backend")
    }
    unsafe fn Lightf(&mut self, light: GLenum, pname: GLenum, param: GLfloat) {
        unimplemented!("Lightf not implemented by this backend")
    }
    unsafe fn Lightx(&mut self, light: GLenum, pname: GLenum, param: GLfixed) {
        unimplemented!("Lightx not implemented by this backend")
    }
    unsafe fn Lightfv(&mut self, light: GLenum, pname: GLenum, params: *const GLfloat) {
        unimplemented!("Lightfv not implemented by this backend")
    }
    unsafe fn Lightxv(&mut self, light: GLenum, pname: GLenum, params: *const GLfixed) {
        unimplemented!("Lightxv not implemented by this backend")
    }
    unsafe fn LightModelf(&mut self, pname: GLenum, param: GLfloat) {
        unimplemented!("LightModelf not implemented by this backend")
    }
    unsafe fn LightModelx(&mut self, pname: GLenum, param: GLfixed) {
        unimplemented!("LightModelx not implemented by this backend")
    }
    unsafe fn LightModelfv(&mut self, pname: GLenum, params: *const GLfloat) {
        unimplemented!("LightModelfv not implemented by this backend")
    }
    unsafe fn LightModelxv(&mut self, pname: GLenum, params: *const GLfixed) {
        unimplemented!("LightModelxv not implemented by this backend")
    }
    unsafe fn Materialf(&mut self, face: GLenum, pname: GLenum, param: GLfloat) {
        unimplemented!("Materialf not implemented by this backend")
    }
    unsafe fn Materialx(&mut self, face: GLenum, pname: GLenum, param: GLfixed) {
        unimplemented!("Materialx not implemented by this backend")
    }
    unsafe fn Materialfv(&mut self, face: GLenum, pname: GLenum, params: *const GLfloat) {
        unimplemented!("Materialfv not implemented by this backend")
    }
    unsafe fn Materialxv(&mut self, face: GLenum, pname: GLenum, params: *const GLfixed) {
        unimplemented!("Materialxv not implemented by this backend")
    }

    // Buffers
    unsafe fn IsBuffer(&mut self, buffer: GLuint) -> GLboolean {
        unimplemented!("IsBuffer not implemented by this backend")
    }
    unsafe fn GenBuffers(&mut self, n: GLsizei, buffers: *mut GLuint) {
        unimplemented!("GenBuffers not implemented by this backend")
    }
    unsafe fn DeleteBuffers(&mut self, n: GLsizei, buffers: *const GLuint) {
        unimplemented!("DeleteBuffers not implemented by this backend")
    }
    unsafe fn BindBuffer(&mut self, target: GLenum, buffer: GLuint) {
        unimplemented!("BindBuffer not implemented by this backend")
    }
    unsafe fn BufferData(
        &mut self,
        target: GLenum,
        size: GLsizeiptr,
        data: *const GLvoid,
        usage: GLenum,
    ) {
        unimplemented!("BufferData not implemented by this backend")
    }
    unsafe fn BufferSubData(
        &mut self,
        target: GLenum,
        offset: GLintptr,
        size: GLsizeiptr,
        data: *const GLvoid,
    ) {
        unimplemented!("BufferSubData not implemented by this backend")
    }

    // Non-pointers
    unsafe fn Color4f(&mut self, red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) {
        unimplemented!("Color4f not implemented by this backend")
    }
    unsafe fn Color4x(&mut self, red: GLfixed, green: GLfixed, blue: GLfixed, alpha: GLfixed) {
        unimplemented!("Color4x not implemented by this backend")
    }
    unsafe fn Color4ub(&mut self, red: GLubyte, green: GLubyte, blue: GLubyte, alpha: GLubyte) {
        unimplemented!("Color4ub not implemented by this backend")
    }
    unsafe fn Normal3f(&mut self, nx: GLfloat, ny: GLfloat, nz: GLfloat) {
        unimplemented!("Normal3f not implemented by this backend")
    }
    unsafe fn Normal3x(&mut self, nx: GLfixed, ny: GLfixed, nz: GLfixed) {
        unimplemented!("Normal3x not implemented by this backend")
    }

    // Pointers
    unsafe fn ColorPointer(
        &mut self,
        size: GLint,
        type_: GLenum,
        stride: GLsizei,
        pointer: *const GLvoid,
    ) {
        unimplemented!("ColorPointer not implemented by this backend")
    }
    unsafe fn NormalPointer(&mut self, type_: GLenum, stride: GLsizei, pointer: *const GLvoid) {
        unimplemented!("NormalPointer not implemented by this backend")
    }
    unsafe fn TexCoordPointer(
        &mut self,
        size: GLint,
        type_: GLenum,
        stride: GLsizei,
        pointer: *const GLvoid,
    ) {
        unimplemented!("TexCoordPointer not implemented by this backend")
    }
    unsafe fn VertexPointer(
        &mut self,
        size: GLint,
        type_: GLenum,
        stride: GLsizei,
        pointer: *const GLvoid,
    ) {
        unimplemented!("VertexPointer not implemented by this backend")
    }

    // Drawing
    unsafe fn DrawArrays(&mut self, mode: GLenum, first: GLint, count: GLsizei) {
        unimplemented!("DrawArrays not implemented by this backend")
    }
    unsafe fn DrawElements(
        &mut self,
        mode: GLenum,
        count: GLsizei,
        type_: GLenum,
        indices: *const GLvoid,
    ) {
        unimplemented!("DrawElements not implemented by this backend")
    }

    // Clearing
    unsafe fn Clear(&mut self, mask: GLbitfield) {
        unimplemented!("Clear not implemented by this backend")
    }
    unsafe fn ClearColor(
        &mut self,
        red: GLclampf,
        green: GLclampf,
        blue: GLclampf,
        alpha: GLclampf,
    ) {
        unimplemented!("ClearColor not implemented by this backend")
    }
    unsafe fn ClearColorx(
        &mut self,
        red: GLclampx,
        green: GLclampx,
        blue: GLclampx,
        alpha: GLclampx,
    ) {
        unimplemented!("ClearColorx not implemented by this backend")
    }
    unsafe fn ClearDepthf(&mut self, depth: GLclampf) {
        unimplemented!("ClearDepthf not implemented by this backend")
    }
    unsafe fn ClearDepthx(&mut self, depth: GLclampx) {
        unimplemented!("ClearDepthx not implemented by this backend")
    }
    unsafe fn ClearStencil(&mut self, s: GLint) {
        unimplemented!("ClearStencil not implemented by this backend")
    }

    // Textures
    unsafe fn PixelStorei(&mut self, pname: GLenum, param: GLint) {
        unimplemented!("PixelStorei not implemented by this backend")
    }
    unsafe fn ReadPixels(
        &mut self,
        x: GLint,
        y: GLint,
        width: GLsizei,
        height: GLsizei,
        format: GLenum,
        type_: GLenum,
        pixels: *mut GLvoid,
    ) {
        unimplemented!("ReadPixels not implemented by this backend")
    }
    unsafe fn GenTextures(&mut self, n: GLsizei, textures: *mut GLuint) {
        unimplemented!("GenTextures not implemented by this backend")
    }
    unsafe fn DeleteTextures(&mut self, n: GLsizei, textures: *const GLuint) {
        unimplemented!("DeleteTextures not implemented by this backend")
    }
    unsafe fn ActiveTexture(&mut self, texture: GLenum) {
        unimplemented!("ActiveTexture not implemented by this backend")
    }
    unsafe fn IsTexture(&mut self, texture: GLuint) -> GLboolean {
        unimplemented!("IsTexture not implemented by this backend")
    }
    unsafe fn BindTexture(&mut self, target: GLenum, texture: GLuint) {
        unimplemented!("BindTexture not implemented by this backend")
    }
    unsafe fn TexParameteri(&mut self, target: GLenum, pname: GLenum, param: GLint) {
        unimplemented!("TexParameteri not implemented by this backend")
    }
    unsafe fn TexParameterf(&mut self, target: GLenum, pname: GLenum, param: GLfloat) {
        unimplemented!("TexParameterf not implemented by this backend")
    }
    unsafe fn TexParameterx(&mut self, target: GLenum, pname: GLenum, param: GLfixed) {
        unimplemented!("TexParameterx not implemented by this backend")
    }
    unsafe fn TexParameteriv(&mut self, target: GLenum, pname: GLenum, params: *const GLint) {
        unimplemented!("TexParameteriv not implemented by this backend")
    }
    unsafe fn GetTexParameteriv(&mut self, target: GLenum, pname: GLenum, params: *mut GLint) {
        unimplemented!("GetTexParameteriv not implemented by this backend")
    }
    unsafe fn TexParameterfv(&mut self, target: GLenum, pname: GLenum, params: *const GLfloat) {
        unimplemented!("TexParameterfv not implemented by this backend")
    }
    unsafe fn TexParameterxv(&mut self, target: GLenum, pname: GLenum, params: *const GLfixed) {
        unimplemented!("TexParameterxv not implemented by this backend")
    }
    unsafe fn TexImage2D(
        &mut self,
        target: GLenum,
        level: GLint,
        internalformat: GLint,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
        format: GLenum,
        type_: GLenum,
        pixels: *const GLvoid,
    ) {
        unimplemented!("TexImage2D not implemented by this backend")
    }
    unsafe fn TexSubImage2D(
        &mut self,
        target: GLenum,
        level: GLint,
        xoffset: GLint,
        yoffset: GLint,
        width: GLsizei,
        height: GLsizei,
        format: GLenum,
        type_: GLenum,
        pixels: *const GLvoid,
    ) {
        unimplemented!("TexSubImage2D not implemented by this backend")
    }
    unsafe fn CompressedTexImage2D(
        &mut self,
        target: GLenum,
        level: GLint,
        internalformat: GLenum,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
        image_size: GLsizei,
        data: *const GLvoid,
    ) {
        unimplemented!("CompressedTexImage2D not implemented by this backend")
    }
    unsafe fn CopyTexImage2D(
        &mut self,
        target: GLenum,
        level: GLint,
        internalformat: GLenum,
        x: GLint,
        y: GLint,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
    ) {
        unimplemented!("CopyTexImage2D not implemented by this backend")
    }
    unsafe fn CopyTexSubImage2D(
        &mut self,
        target: GLenum,
        level: GLint,
        xoffset: GLint,
        yoffset: GLint,
        x: GLint,
        y: GLint,
        width: GLsizei,
        height: GLsizei,
    ) {
        unimplemented!("CopyTexSubImage2D not implemented by this backend")
    }
    unsafe fn TexEnvf(&mut self, target: GLenum, pname: GLenum, param: GLfloat) {
        unimplemented!("TexEnvf not implemented by this backend")
    }
    unsafe fn TexEnvx(&mut self, target: GLenum, pname: GLenum, param: GLfixed) {
        unimplemented!("TexEnvx not implemented by this backend")
    }
    unsafe fn TexEnvi(&mut self, target: GLenum, pname: GLenum, param: GLint) {
        unimplemented!("TexEnvi not implemented by this backend")
    }
    unsafe fn TexEnvfv(&mut self, target: GLenum, pname: GLenum, params: *const GLfloat) {
        unimplemented!("TexEnvfv not implemented by this backend")
    }
    unsafe fn TexEnvxv(&mut self, target: GLenum, pname: GLenum, params: *const GLfixed) {
        unimplemented!("TexEnvxv not implemented by this backend")
    }
    unsafe fn TexEnviv(&mut self, target: GLenum, pname: GLenum, params: *const GLint) {
        unimplemented!("TexEnviv not implemented by this backend")
    }

    unsafe fn MultiTexCoord4f(
        &mut self,
        target: GLenum,
        s: GLfloat,
        t: GLfloat,
        r: GLfloat,
        q: GLfloat,
    ) {
        unimplemented!("MultiTexCoord4f not implemented by this backend")
    }
    unsafe fn MultiTexCoord4x(
        &mut self,
        target: GLenum,
        s: GLfixed,
        t: GLfixed,
        r: GLfixed,
        q: GLfixed,
    ) {
        unimplemented!("MultiTexCoord4x not implemented by this backend")
    }

    // Matrix stack operations
    unsafe fn MatrixMode(&mut self, mode: GLenum) {
        unimplemented!("MatrixMode not implemented by this backend")
    }
    unsafe fn LoadIdentity(&mut self) {
        unimplemented!("LoadIdentity not implemented by this backend")
    }
    unsafe fn LoadMatrixf(&mut self, m: *const GLfloat) {
        unimplemented!("LoadMatrixf not implemented by this backend")
    }
    unsafe fn LoadMatrixx(&mut self, m: *const GLfixed) {
        unimplemented!("LoadMatrixx not implemented by this backend")
    }
    unsafe fn MultMatrixf(&mut self, m: *const GLfloat) {
        unimplemented!("MultMatrixf not implemented by this backend")
    }
    unsafe fn MultMatrixx(&mut self, m: *const GLfixed) {
        unimplemented!("MultMatrixx not implemented by this backend")
    }
    unsafe fn PushMatrix(&mut self) {
        unimplemented!("PushMatrix not implemented by this backend")
    }
    unsafe fn PopMatrix(&mut self) {
        unimplemented!("PopMatrix not implemented by this backend")
    }
    unsafe fn Orthof(
        &mut self,
        left: GLfloat,
        right: GLfloat,
        bottom: GLfloat,
        top: GLfloat,
        near: GLfloat,
        far: GLfloat,
    ) {
        unimplemented!("Orthof not implemented by this backend")
    }
    unsafe fn Orthox(
        &mut self,
        left: GLfixed,
        right: GLfixed,
        bottom: GLfixed,
        top: GLfixed,
        near: GLfixed,
        far: GLfixed,
    ) {
        unimplemented!("Orthox not implemented by this backend")
    }
    unsafe fn Frustumf(
        &mut self,
        left: GLfloat,
        right: GLfloat,
        bottom: GLfloat,
        top: GLfloat,
        near: GLfloat,
        far: GLfloat,
    ) {
        unimplemented!("Frustumf not implemented by this backend")
    }
    unsafe fn Frustumx(
        &mut self,
        left: GLfixed,
        right: GLfixed,
        bottom: GLfixed,
        top: GLfixed,
        near: GLfixed,
        far: GLfixed,
    ) {
        unimplemented!("Frustumx not implemented by this backend")
    }
    unsafe fn Rotatef(&mut self, angle: GLfloat, x: GLfloat, y: GLfloat, z: GLfloat) {
        unimplemented!("Rotatef not implemented by this backend")
    }
    unsafe fn Rotatex(&mut self, angle: GLfixed, x: GLfixed, y: GLfixed, z: GLfixed) {
        unimplemented!("Rotatex not implemented by this backend")
    }
    unsafe fn Scalef(&mut self, x: GLfloat, y: GLfloat, z: GLfloat) {
        unimplemented!("Scalef not implemented by this backend")
    }
    unsafe fn Scalex(&mut self, x: GLfixed, y: GLfixed, z: GLfixed) {
        unimplemented!("Scalex not implemented by this backend")
    }
    unsafe fn Translatef(&mut self, x: GLfloat, y: GLfloat, z: GLfloat) {
        unimplemented!("Translatef not implemented by this backend")
    }
    unsafe fn Translatex(&mut self, x: GLfixed, y: GLfixed, z: GLfixed) {
        unimplemented!("Translatex not implemented by this backend")
    }

    // OES_framebuffer_object (incomplete)
    unsafe fn GenFramebuffersOES(&mut self, n: GLsizei, framebuffers: *mut GLuint) {
        unimplemented!("GenFramebuffersOES not implemented by this backend")
    }
    unsafe fn GenRenderbuffersOES(&mut self, n: GLsizei, renderbuffers: *mut GLuint) {
        unimplemented!("GenRenderbuffersOES not implemented by this backend")
    }
    unsafe fn IsFramebufferOES(&mut self, framebuffer: GLuint) -> GLboolean {
        unimplemented!("IsFramebufferOES not implemented by this backend")
    }
    unsafe fn IsRenderbufferOES(&mut self, renderbuffer: GLuint) -> GLboolean {
        unimplemented!("IsRenderbufferOES not implemented by this backend")
    }
    unsafe fn BindFramebufferOES(&mut self, target: GLenum, framebuffer: GLuint) {
        unimplemented!("BindFramebufferOES not implemented by this backend")
    }
    unsafe fn BindRenderbufferOES(&mut self, target: GLenum, renderbuffer: GLuint) {
        unimplemented!("BindRenderbufferOES not implemented by this backend")
    }
    unsafe fn RenderbufferStorageOES(
        &mut self,
        target: GLenum,
        internalformat: GLenum,
        width: GLsizei,
        height: GLsizei,
    ) {
        unimplemented!("RenderbufferStorageOES not implemented by this backend")
    }
    unsafe fn FramebufferRenderbufferOES(
        &mut self,
        target: GLenum,
        attachment: GLenum,
        renderbuffertarget: GLenum,
        renderbuffer: GLuint,
    ) {
        unimplemented!("FramebufferRenderbufferOES not implemented by this backend")
    }
    unsafe fn FramebufferTexture2DOES(
        &mut self,
        target: GLenum,
        attachment: GLenum,
        textarget: GLenum,
        texture: GLuint,
        level: i32,
    ) {
        unimplemented!("FramebufferTexture2DOES not implemented by this backend")
    }
    unsafe fn GetFramebufferAttachmentParameterivOES(
        &mut self,
        target: GLenum,
        attachment: GLenum,
        pname: GLenum,
        params: *mut GLint,
    ) {
        unimplemented!("GetFramebufferAttachmentParameterivOES not implemented by this backend")
    }
    unsafe fn GetRenderbufferParameterivOES(
        &mut self,
        target: GLenum,
        pname: GLenum,
        params: *mut GLint,
    ) {
        unimplemented!("GetRenderbufferParameterivOES not implemented by this backend")
    }
    unsafe fn CheckFramebufferStatusOES(&mut self, target: GLenum) -> GLenum {
        unimplemented!("CheckFramebufferStatusOES not implemented by this backend")
    }
    unsafe fn DeleteFramebuffersOES(&mut self, n: GLsizei, framebuffers: *const GLuint) {
        unimplemented!("DeleteFramebuffersOES not implemented by this backend")
    }
    unsafe fn DeleteRenderbuffersOES(&mut self, n: GLsizei, renderbuffers: *const GLuint) {
        unimplemented!("DeleteRenderbuffersOES not implemented by this backend")
    }
    unsafe fn GenerateMipmapOES(&mut self, target: GLenum) {
        unimplemented!("GenerateMipmapOES not implemented by this backend")
    }

    // OES_vertex_array_object.
    //
    // These defaults keep a guest that calls them alive, and nothing more.
    // Binding does not switch attribute state and generating hands out the
    // same names every call, so an app that stores its arrays in a vertex
    // array object and later binds it gets whatever the previous draw left
    // behind. That is invisible at the call site and shows up as another
    // object's geometry, so a backend that inherits these must not let
    // `GL_OES_vertex_array_object` be advertised — see `EXTENSIONS` in
    // `crate::frameworks::opengles::gles_guest`, which is where the extension
    // string is decided and where the measured case is written up.
    unsafe fn GenVertexArraysOES(&mut self, n: GLsizei, arrays: *mut GLuint) {
        for i in 0..n {
            arrays.add(i as usize).write((i + 1) as GLuint);
        }
    }
    unsafe fn BindVertexArrayOES(&mut self, array: GLuint) {}
    unsafe fn DeleteVertexArraysOES(&mut self, n: GLsizei, arrays: *const GLuint) {}
    unsafe fn IsVertexArrayOES(&mut self, array: GLuint) -> GLboolean {
        (array != 0) as GLboolean
    }

    // Non-OES aliases for OES_framebuffer_object functions.
    // Some GLES1 apps call the suffix-free ES2-style names directly.
    unsafe fn GenFramebuffers(&mut self, n: GLsizei, framebuffers: *mut GLuint) {
        unimplemented!("GenFramebuffers not implemented by this backend")
    }
    unsafe fn GenRenderbuffers(&mut self, n: GLsizei, renderbuffers: *mut GLuint) {
        unimplemented!("GenRenderbuffers not implemented by this backend")
    }
    unsafe fn IsFramebuffer(&mut self, framebuffer: GLuint) -> GLboolean {
        unimplemented!("IsFramebuffer not implemented by this backend")
    }
    unsafe fn IsRenderbuffer(&mut self, renderbuffer: GLuint) -> GLboolean {
        unimplemented!("IsRenderbuffer not implemented by this backend")
    }
    unsafe fn BindFramebuffer(&mut self, target: GLenum, framebuffer: GLuint) {
        unimplemented!("BindFramebuffer not implemented by this backend")
    }
    unsafe fn BindRenderbuffer(&mut self, target: GLenum, renderbuffer: GLuint) {
        unimplemented!("BindRenderbuffer not implemented by this backend")
    }
    unsafe fn RenderbufferStorage(
        &mut self,
        target: GLenum,
        internalformat: GLenum,
        width: GLsizei,
        height: GLsizei,
    ) {
        unimplemented!("RenderbufferStorage not implemented by this backend")
    }
    unsafe fn FramebufferRenderbuffer(
        &mut self,
        target: GLenum,
        attachment: GLenum,
        renderbuffertarget: GLenum,
        renderbuffer: GLuint,
    ) {
        unimplemented!("FramebufferRenderbuffer not implemented by this backend")
    }
    unsafe fn FramebufferTexture2D(
        &mut self,
        target: GLenum,
        attachment: GLenum,
        textarget: GLenum,
        texture: GLuint,
        level: i32,
    ) {
        unimplemented!("FramebufferTexture2D not implemented by this backend")
    }
    unsafe fn CheckFramebufferStatus(&mut self, target: GLenum) -> GLenum {
        unimplemented!("CheckFramebufferStatus not implemented by this backend")
    }
    unsafe fn DeleteFramebuffers(&mut self, n: GLsizei, framebuffers: *const GLuint) {
        unimplemented!("DeleteFramebuffers not implemented by this backend")
    }
    unsafe fn DeleteRenderbuffers(&mut self, n: GLsizei, renderbuffers: *const GLuint) {
        unimplemented!("DeleteRenderbuffers not implemented by this backend")
    }
    unsafe fn GenerateMipmap(&mut self, target: GLenum) {
        unimplemented!("GenerateMipmap not implemented by this backend")
    }
    unsafe fn GetFramebufferAttachmentParameteriv(
        &mut self,
        target: GLenum,
        attachment: GLenum,
        pname: GLenum,
        params: *mut GLint,
    ) {
        unimplemented!("GetFramebufferAttachmentParameteriv not implemented by this backend")
    }
    unsafe fn GetRenderbufferParameteriv(
        &mut self,
        target: GLenum,
        pname: GLenum,
        params: *mut GLint,
    ) {
        unimplemented!("GetRenderbufferParameteriv not implemented by this backend")
    }

    unsafe fn GetBufferParameteriv(&mut self, target: GLenum, pname: GLenum, params: *mut GLint) {
        unimplemented!("GetBufferParameteriv not implemented by this backend")
    }
    unsafe fn MapBufferOES(&mut self, target: GLenum, access: GLenum) -> *mut GLvoid {
        unimplemented!("MapBufferOES not implemented by this backend")
    }
    unsafe fn UnmapBufferOES(&mut self, target: GLenum) -> GLboolean {
        unimplemented!("UnmapBufferOES not implemented by this backend")
    }

    // OpenGL ES 2.0 entry points. Backends that actually support shaders
    // (currently [super::gles1_on_gl2] and [super::gles2_native]) implement
    // these. EAGL routes ES 2.0 contexts to such a backend.
    //
    // The default implementations log a warning once and return safe defaults
    // rather than panicking. This lets apps that probe for shader support on
    // an ES 1.1 context (or that mistakenly call ES 2.0 entry points on an
    // ES 1.1 context) survive instead of hard-crashing the emulator. Apps
    // that have a fixed-function fallback path will use it; apps that rely
    // on shaders working will render incorrectly but still run.
    unsafe fn CreateShader(&mut self, _type_: GLenum) -> GLuint {
        log_once!("CreateShader (OpenGL ES 2.0) not supported by this backend [stubbed]");
        0
    }
    unsafe fn DeleteShader(&mut self, _shader: GLuint) {
        log_once!("DeleteShader (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn ShaderSource(
        &mut self,
        _shader: GLuint,
        _count: GLsizei,
        _string: *const *const GLchar,
        _length: *const GLint,
    ) {
        log_once!("ShaderSource (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn CompileShader(&mut self, _shader: GLuint) {
        log_once!("CompileShader (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn GetShaderiv(&mut self, _shader: GLuint, _pname: GLenum, _params: *mut GLint) {
        log_once!("GetShaderiv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn GetShaderInfoLog(
        &mut self,
        _shader: GLuint,
        _maxLength: GLsizei,
        _length: *mut GLsizei,
        _infoLog: *mut GLchar,
    ) {
        log_once!("GetShaderInfoLog (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn IsShader(&mut self, _shader: GLuint) -> GLboolean {
        log_once!("IsShader (OpenGL ES 2.0) not supported by this backend [stubbed]");
        0
    }
    unsafe fn CreateProgram(&mut self) -> GLuint {
        log_once!("CreateProgram (OpenGL ES 2.0) not supported by this backend [stubbed]");
        0
    }
    unsafe fn DeleteProgram(&mut self, _program: GLuint) {
        log_once!("DeleteProgram (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn AttachShader(&mut self, _program: GLuint, _shader: GLuint) {
        log_once!("AttachShader (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn DetachShader(&mut self, _program: GLuint, _shader: GLuint) {
        log_once!("DetachShader (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn LinkProgram(&mut self, _program: GLuint) {
        log_once!("LinkProgram (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn UseProgram(&mut self, _program: GLuint) {
        log_once!("UseProgram (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn GetProgramiv(&mut self, _program: GLuint, _pname: GLenum, _params: *mut GLint) {
        log_once!("GetProgramiv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn GetProgramInfoLog(
        &mut self,
        _program: GLuint,
        _maxLength: GLsizei,
        _length: *mut GLsizei,
        _infoLog: *mut GLchar,
    ) {
        log_once!("GetProgramInfoLog (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn IsProgram(&mut self, _program: GLuint) -> GLboolean {
        log_once!("IsProgram (OpenGL ES 2.0) not supported by this backend [stubbed]");
        0
    }
    unsafe fn ValidateProgram(&mut self, _program: GLuint) {
        log_once!("ValidateProgram (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn BindAttribLocation(
        &mut self,
        _program: GLuint,
        _index: GLuint,
        _name: *const GLchar,
    ) {
        log_once!("BindAttribLocation (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn GetAttribLocation(&mut self, _program: GLuint, _name: *const GLchar) -> GLint {
        log_once!("GetAttribLocation (OpenGL ES 2.0) not supported by this backend [stubbed]");
        -1
    }
    unsafe fn GetUniformLocation(&mut self, _program: GLuint, _name: *const GLchar) -> GLint {
        log_once!("GetUniformLocation (OpenGL ES 2.0) not supported by this backend [stubbed]");
        -1
    }
    unsafe fn GetActiveAttrib(
        &mut self,
        _program: GLuint,
        _index: GLuint,
        _bufSize: GLsizei,
        _length: *mut GLsizei,
        _size: *mut GLint,
        _type_: *mut GLenum,
        _name: *mut GLchar,
    ) {
        log_once!("GetActiveAttrib (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn GetActiveUniform(
        &mut self,
        _program: GLuint,
        _index: GLuint,
        _bufSize: GLsizei,
        _length: *mut GLsizei,
        _size: *mut GLint,
        _type_: *mut GLenum,
        _name: *mut GLchar,
    ) {
        log_once!("GetActiveUniform (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn EnableVertexAttribArray(&mut self, _index: GLuint) {
        log_once!(
            "EnableVertexAttribArray (OpenGL ES 2.0) not supported by this backend [stubbed]"
        );
    }
    unsafe fn DisableVertexAttribArray(&mut self, _index: GLuint) {
        log_once!(
            "DisableVertexAttribArray (OpenGL ES 2.0) not supported by this backend [stubbed]"
        );
    }
    unsafe fn VertexAttribPointer(
        &mut self,
        _index: GLuint,
        _size: GLint,
        _type_: GLenum,
        _normalized: GLboolean,
        _stride: GLsizei,
        _pointer: *const GLvoid,
    ) {
        log_once!("VertexAttribPointer (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn VertexAttrib1f(&mut self, _index: GLuint, _x: GLfloat) {
        log_once!("VertexAttrib1f (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn VertexAttrib2f(&mut self, _index: GLuint, _x: GLfloat, _y: GLfloat) {
        log_once!("VertexAttrib2f (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn VertexAttrib3f(&mut self, _index: GLuint, _x: GLfloat, _y: GLfloat, _z: GLfloat) {
        log_once!("VertexAttrib3f (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn VertexAttrib4f(
        &mut self,
        _index: GLuint,
        _x: GLfloat,
        _y: GLfloat,
        _z: GLfloat,
        _w: GLfloat,
    ) {
        log_once!("VertexAttrib4f (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn VertexAttrib1fv(&mut self, _index: GLuint, _v: *const GLfloat) {
        log_once!("VertexAttrib1fv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn VertexAttrib2fv(&mut self, _index: GLuint, _v: *const GLfloat) {
        log_once!("VertexAttrib2fv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn VertexAttrib3fv(&mut self, _index: GLuint, _v: *const GLfloat) {
        log_once!("VertexAttrib3fv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn VertexAttrib4fv(&mut self, _index: GLuint, _v: *const GLfloat) {
        log_once!("VertexAttrib4fv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform1f(&mut self, _location: GLint, _v0: GLfloat) {
        log_once!("Uniform1f (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform2f(&mut self, _location: GLint, _v0: GLfloat, _v1: GLfloat) {
        log_once!("Uniform2f (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform3f(&mut self, _location: GLint, _v0: GLfloat, _v1: GLfloat, _v2: GLfloat) {
        log_once!("Uniform3f (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform4f(
        &mut self,
        _location: GLint,
        _v0: GLfloat,
        _v1: GLfloat,
        _v2: GLfloat,
        _v3: GLfloat,
    ) {
        log_once!("Uniform4f (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform1i(&mut self, _location: GLint, _v0: GLint) {
        log_once!("Uniform1i (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform2i(&mut self, _location: GLint, _v0: GLint, _v1: GLint) {
        log_once!("Uniform2i (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform3i(&mut self, _location: GLint, _v0: GLint, _v1: GLint, _v2: GLint) {
        log_once!("Uniform3i (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform4i(
        &mut self,
        _location: GLint,
        _v0: GLint,
        _v1: GLint,
        _v2: GLint,
        _v3: GLint,
    ) {
        log_once!("Uniform4i (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform1fv(&mut self, _location: GLint, _count: GLsizei, _value: *const GLfloat) {
        log_once!("Uniform1fv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform2fv(&mut self, _location: GLint, _count: GLsizei, _value: *const GLfloat) {
        log_once!("Uniform2fv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform3fv(&mut self, _location: GLint, _count: GLsizei, _value: *const GLfloat) {
        log_once!("Uniform3fv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform4fv(&mut self, _location: GLint, _count: GLsizei, _value: *const GLfloat) {
        log_once!("Uniform4fv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform1iv(&mut self, _location: GLint, _count: GLsizei, _value: *const GLint) {
        log_once!("Uniform1iv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform2iv(&mut self, _location: GLint, _count: GLsizei, _value: *const GLint) {
        log_once!("Uniform2iv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform3iv(&mut self, _location: GLint, _count: GLsizei, _value: *const GLint) {
        log_once!("Uniform3iv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn Uniform4iv(&mut self, _location: GLint, _count: GLsizei, _value: *const GLint) {
        log_once!("Uniform4iv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn UniformMatrix2fv(
        &mut self,
        _location: GLint,
        _count: GLsizei,
        _transpose: GLboolean,
        _value: *const GLfloat,
    ) {
        log_once!("UniformMatrix2fv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn UniformMatrix3fv(
        &mut self,
        _location: GLint,
        _count: GLsizei,
        _transpose: GLboolean,
        _value: *const GLfloat,
    ) {
        log_once!("UniformMatrix3fv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn UniformMatrix4fv(
        &mut self,
        _location: GLint,
        _count: GLsizei,
        _transpose: GLboolean,
        _value: *const GLfloat,
    ) {
        log_once!("UniformMatrix4fv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn BlendColor(&mut self, _r: GLclampf, _g: GLclampf, _b: GLclampf, _a: GLclampf) {
        log_once!("BlendColor (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn BlendEquation(&mut self, _mode: GLenum) {
        log_once!("BlendEquation (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn BlendEquationSeparate(&mut self, _modeRGB: GLenum, _modeAlpha: GLenum) {
        log_once!("BlendEquationSeparate (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn BlendFuncSeparate(
        &mut self,
        _srcRGB: GLenum,
        _dstRGB: GLenum,
        _srcAlpha: GLenum,
        _dstAlpha: GLenum,
    ) {
        log_once!("BlendFuncSeparate (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn StencilFuncSeparate(
        &mut self,
        _face: GLenum,
        _func: GLenum,
        _ref_: GLint,
        _mask: GLuint,
    ) {
        log_once!("StencilFuncSeparate (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn StencilOpSeparate(
        &mut self,
        _face: GLenum,
        _sfail: GLenum,
        _dpfail: GLenum,
        _dppass: GLenum,
    ) {
        log_once!("StencilOpSeparate (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn StencilMaskSeparate(&mut self, _face: GLenum, _mask: GLuint) {
        log_once!("StencilMaskSeparate (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn GetVertexAttribiv(&mut self, _index: GLuint, _pname: GLenum, _params: *mut GLint) {
        log_once!("GetVertexAttribiv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn GetVertexAttribfv(&mut self, _index: GLuint, _pname: GLenum, _params: *mut GLfloat) {
        log_once!("GetVertexAttribfv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn GetVertexAttribPointerv(
        &mut self,
        _index: GLuint,
        _pname: GLenum,
        _pointer: *mut *mut GLvoid,
    ) {
        log_once!(
            "GetVertexAttribPointerv (OpenGL ES 2.0) not supported by this backend [stubbed]"
        );
    }
    unsafe fn GetUniformiv(&mut self, _program: GLuint, _location: GLint, _params: *mut GLint) {
        log_once!("GetUniformiv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn GetUniformfv(&mut self, _program: GLuint, _location: GLint, _params: *mut GLfloat) {
        log_once!("GetUniformfv (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn GetAttachedShaders(
        &mut self,
        _program: GLuint,
        _maxCount: GLsizei,
        _count: *mut GLsizei,
        _shaders: *mut GLuint,
    ) {
        log_once!("GetAttachedShaders (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn GetShaderSource(
        &mut self,
        _shader: GLuint,
        _bufSize: GLsizei,
        _length: *mut GLsizei,
        _source: *mut GLchar,
    ) {
        log_once!("GetShaderSource (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
    unsafe fn ReleaseShaderCompiler(&mut self) {
        // No-op: we always have a shader compiler.
    }
    unsafe fn GetShaderPrecisionFormat(
        &mut self,
        _shadertype: GLenum,
        _precisiontype: GLenum,
        _range: *mut GLint,
        _precision: *mut GLint,
    ) {
        log_once!(
            "GetShaderPrecisionFormat (OpenGL ES 2.0) not supported by this backend [stubbed]"
        );
    }
    unsafe fn ShaderBinary(
        &mut self,
        _count: GLsizei,
        _shaders: *const GLuint,
        _binaryformat: GLenum,
        _binary: *const GLvoid,
        _length: GLsizei,
    ) {
        log_once!("ShaderBinary (OpenGL ES 2.0) not supported by this backend [stubbed]");
    }
}
