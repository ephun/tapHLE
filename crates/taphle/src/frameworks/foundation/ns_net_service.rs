/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSNetService` and `NSNetServiceBrowser` (Bonjour).
//!
//! tapHLE does not implement Bonjour, so no service is ever discovered and none
//! can be published. Rather than accept the calls silently — which leaves an
//! app waiting forever on a delegate callback that never comes — both classes
//! report the documented failure asynchronously, exactly as they would on a
//! device with no local network: `netServiceBrowser:didNotSearch:` and
//! `netService:didNotPublish:`.
//!
//! This is what a game's local-multiplayer menu is written to survive; single
//! player is unaffected.
//!
//! Resources:
//! - Apple's [NSNetServiceBrowser](https://developer.apple.com/documentation/foundation/nsnetservicebrowser)

use crate::frameworks::foundation::ns_string::get_static_str;
use crate::objc::{
    id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject, NSZonePtr,
};
use crate::Environment;

/// `NSNetServicesErrorDomain`. Named for completeness; the error dictionary
/// built below carries the code alone, which is what the delegate callbacks
/// here are observed to read.
#[allow(dead_code)]
pub const NSNetServicesErrorDomain: &str = "NSNetServicesErrorDomain";
/// `NSNetServicesErrorCode` key in the error dictionary.
const NSNetServicesErrorCode: &str = "NSNetServicesErrorCode";
/// `NSNetServicesNotFoundError`: the documented code for "not found", which is
/// the closest honest answer when there is no Bonjour at all.
const NSNetServicesNotFoundError: i32 = -72004;

#[derive(Default)]
struct NSNetServiceBrowserHostObject {
    /// Weak, as delegates always are.
    delegate: id,
}
impl HostObject for NSNetServiceBrowserHostObject {}

#[derive(Default)]
struct NSNetServiceHostObject {
    delegate: id,
    /// `NSString*`, retained.
    name: id,
    /// `NSString*`, retained.
    type_: id,
    /// `NSString*`, retained.
    domain: id,
}
impl HostObject for NSNetServiceHostObject {}

/// Build the `errorDictionary` both delegate callbacks take.
fn error_dictionary(env: &mut Environment) -> id {
    let key = get_static_str(env, NSNetServicesErrorCode);
    let code: id = msg_class![env; NSNumber numberWithInt:NSNetServicesNotFoundError];
    msg_class![env; NSDictionary dictionaryWithObject:code forKey:key]
}

/// Send a delegate method only if the delegate implements it, as Cocoa does for
/// optional protocol methods.
fn send_if_responds(env: &mut Environment, delegate: id, selector: &str, sender: id, info: id) {
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
    () = crate::objc::msg_send(env, (delegate, sel, sender, info));
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSNetServiceBrowser: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<NSNetServiceBrowserHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    this
}

- (id)delegate {
    env.objc.borrow::<NSNetServiceBrowserHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<NSNetServiceBrowserHostObject>(this).delegate = delegate;
}

- (())searchForServicesOfType:(id)_type // NSString*
                    inDomain:(id)_domain { // NSString*
    log_once!("NSNetServiceBrowser: Bonjour is not implemented, reporting that the search could not start");
    let delegate = env.objc.borrow::<NSNetServiceBrowserHostObject>(this).delegate;
    let info = error_dictionary(env);
    send_if_responds(env, delegate, "netServiceBrowser:didNotSearch:", this, info);
}

- (())searchForBrowsableDomains {
    let delegate = env.objc.borrow::<NSNetServiceBrowserHostObject>(this).delegate;
    let info = error_dictionary(env);
    send_if_responds(env, delegate, "netServiceBrowser:didNotSearch:", this, info);
}
- (())searchForRegistrationDomains {
    let delegate = env.objc.borrow::<NSNetServiceBrowserHostObject>(this).delegate;
    let info = error_dictionary(env);
    send_if_responds(env, delegate, "netServiceBrowser:didNotSearch:", this, info);
}

- (())stop {
    // Nothing is running, but a delegate that tracks state expects to be told.
    let delegate = env.objc.borrow::<NSNetServiceBrowserHostObject>(this).delegate;
    if delegate == nil {
        return;
    }
    let Some(sel) = env.objc.lookup_selector("netServiceBrowserDidStopSearch:") else {
        return;
    };
    let responds: bool = msg![env; delegate respondsToSelector:sel];
    if responds {
        () = crate::objc::msg_send(env, (delegate, sel, this));
    }
}

// Scheduling is accepted so the usual setup sequence works; there is nothing to
// schedule because no search ever runs.
- (())scheduleInRunLoop:(id)_run_loop forMode:(id)_mode {
}
- (())removeFromRunLoop:(id)_run_loop forMode:(id)_mode {
}

@end

@implementation NSNetService: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<NSNetServiceHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithDomain:(id)domain // NSString*
                type:(id)type_ // NSString*
                name:(id)name { // NSString*
    retain(env, domain);
    retain(env, type_);
    retain(env, name);
    let host_object = env.objc.borrow_mut::<NSNetServiceHostObject>(this);
    host_object.domain = domain;
    host_object.type_ = type_;
    host_object.name = name;
    this
}

- (id)initWithDomain:(id)domain // NSString*
                type:(id)type_ // NSString*
                name:(id)name // NSString*
                port:(i32)_port {
    msg![env; this initWithDomain:domain type:type_ name:name]
}

- (())dealloc {
    let &NSNetServiceHostObject { name, type_, domain, .. } = env.objc.borrow(this);
    release(env, name);
    release(env, type_);
    release(env, domain);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)delegate {
    env.objc.borrow::<NSNetServiceHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<NSNetServiceHostObject>(this).delegate = delegate;
}

- (id)name {
    env.objc.borrow::<NSNetServiceHostObject>(this).name
}
- (id)type {
    env.objc.borrow::<NSNetServiceHostObject>(this).type_
}
- (id)domain {
    env.objc.borrow::<NSNetServiceHostObject>(this).domain
}
- (id)hostName {
    nil
}
- (id)addresses {
    // Never resolved, so there are no addresses. An empty array rather than nil
    // matches what a service that resolved to nothing would report.
    msg_class![env; NSArray array]
}

- (())publish {
    log_once!("NSNetService: Bonjour is not implemented, reporting that publishing failed");
    let delegate = env.objc.borrow::<NSNetServiceHostObject>(this).delegate;
    let info = error_dictionary(env);
    send_if_responds(env, delegate, "netService:didNotPublish:", this, info);
}

- (())resolveWithTimeout:(f64)_timeout {
    let delegate = env.objc.borrow::<NSNetServiceHostObject>(this).delegate;
    let info = error_dictionary(env);
    send_if_responds(env, delegate, "netService:didNotResolve:", this, info);
}
- (())resolve {
    () = msg![env; this resolveWithTimeout:0.0];
}

- (())stop {
    let delegate = env.objc.borrow::<NSNetServiceHostObject>(this).delegate;
    if delegate == nil {
        return;
    }
    let Some(sel) = env.objc.lookup_selector("netServiceDidStop:") else {
        return;
    };
    let responds: bool = msg![env; delegate respondsToSelector:sel];
    if responds {
        () = crate::objc::msg_send(env, (delegate, sel, this));
    }
}

- (())scheduleInRunLoop:(id)_run_loop forMode:(id)_mode {
}
- (())removeFromRunLoop:(id)_run_loop forMode:(id)_mode {
}

@end

};
