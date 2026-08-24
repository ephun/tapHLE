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
use crate::frameworks::core_foundation::cf_array::CFArrayRef;
use crate::frameworks::core_foundation::cf_data::CFDataRef;
use crate::frameworks::core_foundation::cf_dictionary::CFDictionaryRef;
use crate::frameworks::core_foundation::cf_run_loop::CFRunLoopRef;
use crate::frameworks::core_foundation::cf_string::CFStringRef;
use crate::frameworks::core_foundation::cf_url::CFURLRef;
use crate::frameworks::core_foundation::{CFIndex, CFOptionFlags, CFTypeRef};
use crate::frameworks::foundation::{ns_array, ns_dictionary, ns_string};
use crate::mem::{MutPtr, MutVoidPtr};
use crate::objc::{id, msg_class, nil, objc_classes, release, ClassExports, TrivialHostObject};
use crate::Environment;

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/CFNetwork.framework/CFNetwork",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[CONSTANTS],
    function_exports: &[FUNCTIONS],
};

/// Keys in the system proxy settings dictionary. These are the names the
/// configuration itself uses, which is what the `kCFNetworkProxies*` constants
/// below are exported as.
const PROXIES_HTTP_ENABLE: &str = "HTTPEnable";
const PROXIES_HTTP_PROXY: &str = "HTTPProxy";
const PROXIES_HTTP_PORT: &str = "HTTPPort";
const PROXIES_HTTPS_ENABLE: &str = "HTTPSEnable";
const PROXIES_HTTPS_PROXY: &str = "HTTPSProxy";
const PROXIES_HTTPS_PORT: &str = "HTTPSPort";
const PROXIES_AUTO_CONFIG_ENABLE: &str = "ProxyAutoConfigEnable";

/// Keys in one entry of the array `CFNetworkCopyProxiesForURL` returns, and the
/// one proxy type tapHLE ever reports.
const PROXY_TYPE_KEY: &str = "kCFProxyTypeKey";
const PROXY_TYPE_NONE: &str = "kCFProxyTypeNone";

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

/// Proxy configuration for the whole system, which here is the configuration
/// of a device that has no proxy set.
///
/// A device with nothing configured still answers with a dictionary; it just
/// says nothing is enabled. NULL is what it returns when it cannot read the
/// configuration at all, which is a different claim and one tapHLE has no
/// reason to make.
///
/// The three flags are spelled out rather than left absent. An SDK that reads
/// one and passes it straight to `CFBooleanGetValue` or `CFNumberGetValue`
/// without checking for NULL is the common shape of this code, and a dictionary
/// that answers the question it is asked costs nothing here.
fn CFNetworkCopySystemProxySettings(env: &mut Environment) -> CFDictionaryRef {
    let disabled: id = msg_class![env; NSNumber numberWithInt:0i32];
    let entries: Vec<(id, id)> = [
        PROXIES_HTTP_ENABLE,
        PROXIES_HTTPS_ENABLE,
        PROXIES_AUTO_CONFIG_ENABLE,
    ]
    .into_iter()
    .map(|key| (ns_string::from_rust_string(env, key.to_string()), disabled))
    .collect();

    let settings = ns_dictionary::dict_from_keys_and_objects(env, &entries);
    // The dictionary retains its keys, and this function's own references to
    // them are done with. The returned dictionary is +1, which is what a
    // "Copy" function owes its caller.
    for &(key, _) in &entries {
        release(env, key);
    }
    settings
}

/// Which proxies to use to reach a particular URL, which here is none of them.
///
/// The answer is an array of one dictionary, and a dictionary whose type is
/// `kCFProxyTypeNone` means "connect directly". That is what a device with no
/// proxy configured returns, and it is a real answer rather than a refusal —
/// an empty array means "there is no way to reach this at all", which is a
/// different thing to tell an app that is about to give up.
///
/// The `settings` argument is ignored because tapHLE has exactly one
/// configuration to report and `CFNetworkCopySystemProxySettings` above is
/// where it is written down.
fn CFNetworkCopyProxiesForURL(
    env: &mut Environment,
    _url: CFURLRef,
    _proxy_settings: CFDictionaryRef,
) -> CFArrayRef {
    let key = ns_string::from_rust_string(env, PROXY_TYPE_KEY.to_string());
    let none = ns_string::from_rust_string(env, PROXY_TYPE_NONE.to_string());
    // The app compares this value against its own `kCFProxyTypeNone`, which is
    // a different string object with the same contents: CFEqual says they are
    // equal, pointer comparison does not. Apple's own documentation tells
    // callers to use CFEqual, and no app seen so far does otherwise.
    let entry = ns_dictionary::dict_from_keys_and_objects(env, &[(key, none)]);
    release(env, key);
    release(env, none);

    // `from_vec` takes ownership of the references it is given rather than
    // retaining them, so the entry is handed over as-is and the array is the
    // +1 object this function returns.
    ns_array::from_vec(env, vec![entry])
}

pub const CONSTANTS: ConstantExports = &[
    ("_kCFHTTPVersion1_0", HostConstant::NSString("HTTP/1.0")),
    ("_kCFHTTPVersion1_1", HostConstant::NSString("HTTP/1.1")),
    // The proxy keys. An app that imports one of these and finds it null does
    // not get a diagnostic, it gets a null dereference: Super Hexagon reads
    // kCFNetworkProxiesHTTPEnable straight out of the settings dictionary and
    // dies on the load, which is why these are exported rather than left for
    // the first app that reads one.
    (
        "_kCFNetworkProxiesHTTPEnable",
        HostConstant::NSString(PROXIES_HTTP_ENABLE),
    ),
    (
        "_kCFNetworkProxiesHTTPProxy",
        HostConstant::NSString(PROXIES_HTTP_PROXY),
    ),
    (
        "_kCFNetworkProxiesHTTPPort",
        HostConstant::NSString(PROXIES_HTTP_PORT),
    ),
    ("_kCFProxyTypeKey", HostConstant::NSString(PROXY_TYPE_KEY)),
    ("_kCFProxyTypeNone", HostConstant::NSString(PROXY_TYPE_NONE)),
    (
        "_kCFProxyHostNameKey",
        HostConstant::NSString("kCFProxyHostNameKey"),
    ),
    (
        "_kCFProxyPortNumberKey",
        HostConstant::NSString("kCFProxyPortNumberKey"),
    ),
    (
        "_kCFProxyAutoConfigurationURLKey",
        HostConstant::NSString("kCFProxyAutoConfigurationURLKey"),
    ),
    (
        "_kCFStreamPropertyHTTPResponseHeader",
        HostConstant::NSString("kCFStreamPropertyHTTPResponseHeader"),
    ),
    // Setting a proxy on a stream names the host and port with the same keys
    // the system settings dictionary uses, which is what lets a caller pass
    // that dictionary straight through. The keys are shared here for the same
    // reason.
    (
        "_kCFStreamPropertyHTTPProxy",
        HostConstant::NSString("kCFStreamPropertyHTTPProxy"),
    ),
    (
        "_kCFStreamPropertyHTTPProxyHost",
        HostConstant::NSString(PROXIES_HTTP_PROXY),
    ),
    (
        "_kCFStreamPropertyHTTPProxyPort",
        HostConstant::NSString(PROXIES_HTTP_PORT),
    ),
    (
        "_kCFStreamPropertyHTTPSProxyHost",
        HostConstant::NSString(PROXIES_HTTPS_PROXY),
    ),
    (
        "_kCFStreamPropertyHTTPSProxyPort",
        HostConstant::NSString(PROXIES_HTTPS_PORT),
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
    export_c_func!(CFNetworkCopySystemProxySettings()),
    export_c_func!(CFNetworkCopyProxiesForURL(_, _)),
];
