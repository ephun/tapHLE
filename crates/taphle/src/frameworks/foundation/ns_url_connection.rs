/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSURLConnection`.

use super::{ns_run_loop, ns_string, NSInteger};
use crate::dyld::{ConstantExports, HostConstant};
use crate::environment::Environment;
use crate::mem::MutPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, msg_send, nil, objc_classes, release, retain, ClassExports,
    HostObject, NSZonePtr,
};
use std::borrow::Cow;

const NSURLErrorDomain: &str = "NSURLErrorDomain";
/// `userInfo` key on the errors this domain produces. Nothing here populates it
/// yet, but apps reference the symbol to read it out of an error, and an
/// unbound one is a null pointer they will dereference.
const NSErrorFailingURLStringKey: &str = "NSErrorFailingURLStringKey";

pub const CONSTANTS: ConstantExports = &[
    (
        "_NSURLErrorDomain",
        HostConstant::NSString(NSURLErrorDomain),
    ),
    (
        "_NSErrorFailingURLStringKey",
        HostConstant::NSString(NSErrorFailingURLStringKey),
    ),
];

/// Our helper type, Foundation just uses ints.
type NSURLErrorCode = NSInteger;
const NSURLErrorNotConnectedToInternet: NSURLErrorCode = -1009;

struct NSURLConnectionHostObject {
    /// `NSURLRequest*`, owned.
    request: id,
    /// The delegate, retained for the lifetime of the connection as
    /// `NSURLConnection` documents, and released when it ends.
    delegate: id,
    cancelled: bool,
}
impl HostObject for NSURLConnectionHostObject {}

/// Drop the connection's reference to its delegate, once.
fn release_delegate(env: &mut Environment, this: id) {
    let delegate = std::mem::replace(
        &mut env
            .objc
            .borrow_mut::<NSURLConnectionHostObject>(this)
            .delegate,
        nil,
    );
    release(env, delegate);
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSURLConnection: NSObject

+ (id)sendSynchronousRequest:(id)request // NSURLRequest *
           returningResponse:(MutPtr<id>)response // NSURLResponse **
                       error:(MutPtr<id>)out_error { // NSError **
    log!(
        "TODO: [NSURLConnection sendSynchronousRequest:{:?} ('{}') response:{:?} error:{:?}] -> nil",
        request,
        url_string_from_request(env, request),
        response,
        out_error,
    );
    if !response.is_null() {
        env.mem.write(response, nil);
    }
    if !out_error.is_null() {
        let domain = ns_string::get_static_str(env, NSURLErrorDomain);
        let error = msg_class![env; NSError alloc];
        // TODO: fill userInfo
        let error = msg![env; error initWithDomain:domain code:NSURLErrorNotConnectedToInternet userInfo:nil];
        autorelease(env, error);
        env.mem.write(out_error, error);
    }
    nil
}

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSURLConnectionHostObject {
        request: nil,
        delegate: nil,
        cancelled: false,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (bool)canHandleRequest:(id)_request {
    // Whether the URL loading system understands the scheme, not whether the
    // network is up. It does; the connection is what fails.
    true
}

+ (id)connectionWithRequest:(id)request // NSURLRequest *
                   delegate:(id)delegate {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithRequest:request delegate:delegate];
    autorelease(env, new)
}

- (id)initWithRequest:(id)request // NSURLRequest *
             delegate:(id)delegate {
    msg![env; this initWithRequest:request delegate:delegate startImmediately:true]
}

- (id)initWithRequest:(id)request // NSURLRequest *
             delegate:(id)delegate
     startImmediately:(bool)start_immediately {
    // A nil request is the one case where a device also returns nil.
    if request == nil {
        release(env, this);
        return nil;
    }

    log_dbg!(
        "[(NSURLConnection *){:?} initWithRequest:{:?} ('{}') delegate:{:?} startImmediately:{}]",
        this,
        request,
        url_string_from_request(env, request),
        delegate,
        start_immediately,
    );

    // Apple's copies the request; tapHLE's NSURLRequest is not NSCopying, and
    // retaining is equivalent here because nothing reads the request after the
    // failure is composed. Copy it if a real transport ever lands.
    retain(env, request);
    retain(env, delegate);
    *env.objc.borrow_mut(this) = NSURLConnectionHostObject {
        request,
        delegate,
        cancelled: false,
    };

    if start_immediately {
        () = msg![env; this start];
    }

    this
}

// The delegate callback must not run inside -start: a caller that writes
// `conn = [[NSURLConnection alloc] initWith...]` has not yet assigned `conn`
// when the callback would fire, and delegates routinely compare the connection
// they are handed against their stored one. Deferring by a run-loop turn is
// also what a real connection does, since no transport completes
// synchronously.
- (())start {
    let &NSURLConnectionHostObject { request, .. } = env.objc.borrow(this);
    log_dbg!(
        "[(NSURLConnection *){:?} start] ('{}'), will fail: no network",
        this,
        url_string_from_request(env, request),
    );
    env.objc.borrow_mut::<NSURLConnectionHostObject>(this).cancelled = false;
    let selector = env
        .objc
        .lookup_selector("tapHLE_failBecauseOffline")
        .unwrap();
    // Apple's schedules on the calling thread's run loop. tapHLE only pumps the
    // main one — a guest background thread's run loop runs only if that thread
    // runs it — and analytics and leaderboard SDKs habitually start connections
    // from worker threads. Queueing there would recreate, on those threads, the
    // silent hang this method exists to remove, so the failure is delivered on
    // the main run loop instead.
    let run_loop: id = msg_class![env; NSRunLoop mainRunLoop];
    ns_run_loop::add_perform_request(env, run_loop, this, selector, nil, Some(0.0), false);
}

- (())cancel {
    log_dbg!("[(NSURLConnection *){:?} cancel]", this);
    // Cancelling forbids any further delegate message, so the pending failure
    // has to be suppressed rather than merely unscheduled: the run loop may
    // already be holding the request.
    env.objc.borrow_mut::<NSURLConnectionHostObject>(this).cancelled = true;
    release_delegate(env, this);
}

- (())scheduleInRunLoop:(id)_run_loop forMode:(id)_mode {
}
- (())unscheduleFromRunLoop:(id)_run_loop forMode:(id)_mode {
}

// Deliver the failure a device with no network delivers.
- (())tapHLE_failBecauseOffline {
    let &NSURLConnectionHostObject { cancelled, delegate, request } = env.objc.borrow(this);
    if cancelled || delegate == nil {
        return;
    }

    let domain = ns_string::get_static_str(env, NSURLErrorDomain);
    let error: id = msg_class![env; NSError alloc];
    let error: id = msg![env; error initWithDomain:domain
                                              code:NSURLErrorNotConnectedToInternet
                                          userInfo:nil];
    autorelease(env, error);

    log_dbg!(
        "[(NSURLConnection *){:?} ('{}')] failing delegate {:?} with NSURLErrorNotConnectedToInternet",
        this,
        url_string_from_request(env, request),
        delegate,
    );

    if let Some(selector) = env.objc.lookup_selector("connection:didFailWithError:") {
        let responds: bool = msg![env; delegate respondsToSelector:selector];
        if responds {
            () = msg_send(env, (delegate, selector, this, error));
        }
    }

    // NSURLConnection holds its delegate only until the connection ends, and
    // this one has ended. Keeping it would leak every delegate that outlives
    // its connection, which for a retry loop is all of them.
    release_delegate(env, this);
}

- (())dealloc {
    let &NSURLConnectionHostObject { request, delegate, .. } = env.objc.borrow(this);
    release(env, request);
    release(env, delegate);
    env.objc.dealloc_object(this, &mut env.mem)
}

@end


// NSURLProtocol is the hook apps install to intercept or fake network requests,
// and registering one is the first thing an offline-capable app does. Nothing
// here performs requests, so nothing consults the registry — but refusing to
// let an app register stopped eleven of them in a 1501-app survey before they
// reached their own code.
@implementation NSURLProtocol: NSObject

+ (bool)registerClass:(id)protocol_class {
    log_dbg!("TODO: [NSURLProtocol registerClass:{:?}] accepted but never consulted", protocol_class);
    true
}

+ (())unregisterClass:(id)protocol_class {
    log_dbg!("TODO: [NSURLProtocol unregisterClass:{:?}]", protocol_class);
}

// Asked of a *subclass* to decide whether it wants a request. The base class
// answering NO is correct: NSURLProtocol itself handles nothing.
+ (bool)canInitWithRequest:(id)_request {
    false
}

+ (id)canonicalRequestForRequest:(id)request {
    request
}

+ (id)propertyForKey:(id)_key inRequest:(id)_request {
    nil
}
+ (())setProperty:(id)_value forKey:(id)_key inRequest:(id)_request {
}
+ (())removePropertyForKey:(id)_key inRequest:(id)_request {
}

@end

};

fn url_string_from_request(env: &mut Environment, request: id) -> Cow<'static, str> {
    if request == nil {
        Cow::from("(null)")
    } else {
        let url = msg![env; request URL];
        let ns_string = msg![env; url absoluteString];
        ns_string::to_rust_string(env, ns_string)
    }
}
