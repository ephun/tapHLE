/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The CFNetwork framework, specifically its CFHTTPMessage / CFReadStream HTTP
//! client.
//!
//! Early games bundle analytics and reporting SDKs that POST telemetry over
//! CFNetwork. tapHLE has no network stack,
//! so the goal here is not to make the request succeed: it is to let the SDK
//! *build* a request and *attempt* to send it without crashing, then observe
//! the attempt fail, exactly as it would on a device with no connectivity, and
//! fall back to its offline path. Accordingly the HTTP message objects are
//! opaque placeholders and the read stream never opens.

use crate::abi::GuestFunction;
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant, HostDylib};
use crate::frameworks::core_foundation::cf_allocator::CFAllocatorRef;
use crate::frameworks::core_foundation::cf_data::CFDataRef;
use crate::frameworks::core_foundation::cf_run_loop::CFRunLoopRef;
use crate::frameworks::core_foundation::cf_string::CFStringRef;
use crate::frameworks::core_foundation::cf_url::CFURLRef;
use crate::frameworks::core_foundation::{CFIndex, CFOptionFlags, CFTypeRef};
use crate::mem::{MutPtr, MutVoidPtr};
use crate::objc::{nil, objc_classes, ClassExports, TrivialHostObject};
use crate::Environment;

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/CFNetwork.framework/CFNetwork",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[CONSTANTS],
    function_exports: &[FUNCTIONS],
};

/// `CFHTTPMessageRef`, opaque to the app.
type CFHTTPMessageRef = CFTypeRef;
/// `CFReadStreamRef`, opaque to the app.
type CFReadStreamRef = CFTypeRef;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// Both are CFType-based: the app releases the results of the "Create" functions
// below with CFRelease(), which sends -release.
@implementation _tapHLE_CFHTTPMessage: NSObject
@end

@implementation _tapHLE_CFReadStream: NSObject
@end

};

fn CFHTTPMessageCreateRequest(
    env: &mut Environment,
    _allocator: CFAllocatorRef,
    _request_method: CFStringRef,
    _url: CFURLRef,
    _http_version: CFStringRef,
) -> CFHTTPMessageRef {
    let isa = env
        .objc
        .get_known_class("_tapHLE_CFHTTPMessage", &mut env.mem);
    env.objc
        .alloc_object(isa, Box::new(TrivialHostObject), &mut env.mem)
}

fn CFHTTPMessageSetHeaderFieldValue(
    _env: &mut Environment,
    _message: CFHTTPMessageRef,
    _header_field: CFStringRef,
    _value: CFStringRef,
) {
    // The request is never actually sent, so its headers are not retained.
}

fn CFHTTPMessageSetBody(_env: &mut Environment, _message: CFHTTPMessageRef, _body: CFDataRef) {
    // As above: the body is discarded.
}

fn CFHTTPMessageCopyHeaderFieldValue(
    _env: &mut Environment,
    _message: CFHTTPMessageRef,
    _header_field: CFStringRef,
) -> CFStringRef {
    nil
}

fn CFReadStreamCreateForHTTPRequest(
    env: &mut Environment,
    _allocator: CFAllocatorRef,
    _request: CFHTTPMessageRef,
) -> CFReadStreamRef {
    let isa = env
        .objc
        .get_known_class("_tapHLE_CFReadStream", &mut env.mem);
    env.objc
        .alloc_object(isa, Box::new(TrivialHostObject), &mut env.mem)
}

fn CFReadStreamSetProperty(
    _env: &mut Environment,
    _stream: CFReadStreamRef,
    _property_name: CFStringRef,
    _property_value: CFTypeRef,
) -> bool {
    // Accept every property (SSL settings, etc.); none of them matter for a
    // stream that will not open.
    true
}

fn CFReadStreamCopyProperty(
    _env: &mut Environment,
    _stream: CFReadStreamRef,
    _property_name: CFStringRef,
) -> CFTypeRef {
    nil
}

fn CFReadStreamSetClient(
    _env: &mut Environment,
    _stream: CFReadStreamRef,
    _stream_events: CFOptionFlags,
    _client_cb: GuestFunction,
    _client_context: MutVoidPtr,
) -> bool {
    true
}

fn CFReadStreamScheduleWithRunLoop(
    _env: &mut Environment,
    _stream: CFReadStreamRef,
    _run_loop: CFRunLoopRef,
    _run_loop_mode: CFStringRef,
) {
}

fn CFReadStreamUnscheduleFromRunLoop(
    _env: &mut Environment,
    _stream: CFReadStreamRef,
    _run_loop: CFRunLoopRef,
    _run_loop_mode: CFStringRef,
) {
}

fn CFReadStreamOpen(_env: &mut Environment, _stream: CFReadStreamRef) -> bool {
    // Report that the stream opened, even though tapHLE has no network stack
    // and no bytes will ever arrive. Returning true is deliberate: an SDK that
    // opens an async HTTP stream registers a run-loop client and keeps the
    // stream for the completion callback (which we simply never deliver, so the
    // request hangs pending and no telemetry is sent). Returning false instead
    // forces callers down a synchronous-open-failure path, which in EA's IPSP
    // SDK double-releases the request URL string and crashes — a latent bug
    // that a real device rarely triggers because a synchronous open failure is
    // rare.
    true
}

fn CFReadStreamRead(
    _env: &mut Environment,
    _stream: CFReadStreamRef,
    _buffer: MutPtr<u8>,
    _buffer_length: CFIndex,
) -> CFIndex {
    // End of stream: there is nothing to read from a stream that never opened.
    0
}

fn CFReadStreamClose(_env: &mut Environment, _stream: CFReadStreamRef) {}

fn CFReadStreamCopyError(_env: &mut Environment, _stream: CFReadStreamRef) -> CFTypeRef {
    nil
}

pub const CONSTANTS: ConstantExports = &[
    ("_kCFHTTPVersion1_1", HostConstant::NSString("HTTP/1.1")),
    (
        "_kCFStreamPropertyHTTPResponseHeader",
        HostConstant::NSString("kCFStreamPropertyHTTPResponseHeader"),
    ),
    (
        "_kCFStreamPropertySSLSettings",
        HostConstant::NSString("kCFStreamPropertySSLSettings"),
    ),
    (
        "_kCFStreamSSLLevel",
        HostConstant::NSString("kCFStreamSSLLevel"),
    ),
    (
        "_kCFStreamSSLAllowsExpiredCertificates",
        HostConstant::NSString("kCFStreamSSLAllowsExpiredCertificates"),
    ),
    (
        "_kCFStreamSSLAllowsExpiredRoots",
        HostConstant::NSString("kCFStreamSSLAllowsExpiredRoots"),
    ),
    (
        "_kCFStreamSSLAllowsAnyRoot",
        HostConstant::NSString("kCFStreamSSLAllowsAnyRoot"),
    ),
    (
        "_kCFStreamSSLValidatesCertificateChain",
        HostConstant::NSString("kCFStreamSSLValidatesCertificateChain"),
    ),
    (
        "_kCFStreamSSLPeerName",
        HostConstant::NSString("kCFStreamSSLPeerName"),
    ),
    (
        "_kCFStreamSocketSecurityLevelNegotiatedSSL",
        HostConstant::NSString("kCFStreamSocketSecurityLevelNegotiatedSSL"),
    ),
];

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CFHTTPMessageCreateRequest(_, _, _, _)),
    export_c_func!(CFHTTPMessageSetHeaderFieldValue(_, _, _)),
    export_c_func!(CFHTTPMessageSetBody(_, _)),
    export_c_func!(CFHTTPMessageCopyHeaderFieldValue(_, _)),
    export_c_func!(CFReadStreamCreateForHTTPRequest(_, _)),
    export_c_func!(CFReadStreamSetProperty(_, _, _)),
    export_c_func!(CFReadStreamCopyProperty(_, _)),
    export_c_func!(CFReadStreamSetClient(_, _, _, _)),
    export_c_func!(CFReadStreamScheduleWithRunLoop(_, _, _)),
    export_c_func!(CFReadStreamUnscheduleFromRunLoop(_, _, _)),
    export_c_func!(CFReadStreamOpen(_)),
    export_c_func!(CFReadStreamRead(_, _, _)),
    export_c_func!(CFReadStreamClose(_)),
    export_c_func!(CFReadStreamCopyError(_)),
];
