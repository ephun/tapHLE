/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSURLRequest and NSMutableURLRequest`.

use super::{ns_string, NSTimeInterval, NSUInteger};
use crate::frameworks::foundation::ns_string::to_rust_string;
use crate::objc::{
    autorelease, id, nil, objc_classes, release, retain, ClassExports, HostObject, NSZonePtr,
};
use crate::{msg, msg_class, Environment};

type NSURLRequestCachePolicy = NSUInteger;
const NSURLRequestUseProtocolCachePolicy: NSURLRequestCachePolicy = 0;

struct NSURLRequestHostObject {
    /// `NSURL*`
    url: id,
    cache_policy: NSURLRequestCachePolicy,
    timeout_interval: NSTimeInterval,
    // Request components
    /// `NSString*`
    http_method: id,
    /// `NSData*`
    http_body: id,
    // Header fields
    /// `NSDictionary*`
    http_header_fields: id,
    /// The rest of the request's settings. tapHLE performs no I/O, so none of
    /// these can be acted on; they are stored because a request is a value an
    /// app configures and then reads back, and a setter that does not exist
    /// ends the app while one that quietly forgets makes it misreport itself.
    http_should_handle_cookies: bool,
    http_should_use_pipelining: bool,
    network_service_type: NSUInteger,
    /// `NSURL*`
    main_document_url: id,
    /// `NSInputStream*`
    http_body_stream: id,
}
impl HostObject for NSURLRequestHostObject {}

/// Give `new` the same request `old` describes.
///
/// The header dictionary is copied entry by entry rather than shared: it is the
/// part a caller adjusts after copying, and sharing it is what makes a copy
/// behave like an alias.
fn copy_request_fields(env: &mut Environment, old: id, new: id) {
    let &NSURLRequestHostObject {
        url,
        cache_policy,
        timeout_interval,
        http_method,
        http_body,
        http_header_fields,
        http_should_handle_cookies,
        http_should_use_pipelining,
        network_service_type,
        main_document_url,
        http_body_stream,
    } = env.objc.borrow(old);

    let url_copy: id = msg![env; url copy];
    let method_copy: id = msg![env; http_method copy];
    let body_copy: id = msg![env; http_body copy];

    let new_fields = env
        .objc
        .borrow::<NSURLRequestHostObject>(new)
        .http_header_fields;
    () = msg![env; new_fields addEntriesFromDictionary:http_header_fields];

    let new_obj = env.objc.borrow_mut::<NSURLRequestHostObject>(new);
    let old_url = std::mem::replace(&mut new_obj.url, url_copy);
    let old_method = std::mem::replace(&mut new_obj.http_method, method_copy);
    let old_body = std::mem::replace(&mut new_obj.http_body, body_copy);
    new_obj.cache_policy = cache_policy;
    new_obj.timeout_interval = timeout_interval;
    new_obj.http_should_handle_cookies = http_should_handle_cookies;
    new_obj.http_should_use_pipelining = http_should_use_pipelining;
    new_obj.network_service_type = network_service_type;
    let old_main_document = std::mem::replace(&mut new_obj.main_document_url, main_document_url);
    let old_body_stream = std::mem::replace(&mut new_obj.http_body_stream, http_body_stream);
    retain(env, main_document_url);
    retain(env, http_body_stream);
    release(env, old_main_document);
    release(env, old_body_stream);
    release(env, old_url);
    release(env, old_method);
    release(env, old_body);
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSURLRequest: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    // TODO: this should be mutable _only_ in the subclass
    // TODO: fill default headers
    let http_header_fields: id = msg_class![env; NSMutableDictionary new];
    let host_object = Box::new(NSURLRequestHostObject {
        url: nil,
        cache_policy: NSURLRequestUseProtocolCachePolicy,
        timeout_interval: 60.0,
        http_method: ns_string::get_static_str(env, "GET"),
        http_body: nil,
        http_header_fields,
        // NSURLRequest's documented defaults.
        http_should_handle_cookies: true,
        http_should_use_pipelining: false,
        network_service_type: 0, // NSURLNetworkServiceTypeDefault
        main_document_url: nil,
        http_body_stream: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)requestWithURL:(id)url {
    msg![env; this requestWithURL:url
                      cachePolicy:NSURLRequestUseProtocolCachePolicy
                  timeoutInterval:60.0]
}

+ (id)requestWithURL:(id)url
         cachePolicy:(NSURLRequestCachePolicy)cache_policy
     timeoutInterval:(NSTimeInterval)timeout_interval {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithURL:url
                                cachePolicy:cache_policy
                            timeoutInterval:timeout_interval];
    autorelease(env, new)
}

// The convenience initialiser, with the same defaults +requestWithURL: uses.
- (id)initWithURL:(id)url {
    msg![env; this initWithURL:url
                   cachePolicy:NSURLRequestUseProtocolCachePolicy
               timeoutInterval:60.0]
}

- (id)initWithURL:(id)url
        cachePolicy:(NSURLRequestCachePolicy)cache_policy
    timeoutInterval:(NSTimeInterval)timeout_interval {
    if url == nil {
        return nil;
    }
    let url_desc: id = msg![env; url description];
    log_dbg!(
        "[(NSURLRequest *){:?} initWithURL:{} cachePolicy:{} timeoutInterval:{}]",
        this,
        to_rust_string(env, url_desc),
        cache_policy,
        timeout_interval,
    );

    // A request is a value, not a connection: building one performs no I/O and
    // succeeds on a device in airplane mode exactly as it does on WiFi. tapHLE
    // used to return nil here when network access was off, which no real device
    // ever does, so no app has code for it — they carry the nil forward and
    // hand it to NSURLConnection, which then cannot report a failure against a
    // request that does not exist. Offline is modelled where it actually
    // happens, in NSURLConnection, which fails with
    // NSURLErrorNotConnectedToInternet.

    let url_copy = msg![env; url copy];
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).url = url_copy;
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).cache_policy = cache_policy;
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).timeout_interval = timeout_interval;

    this
}

- (id)URL {
    env.objc.borrow::<NSURLRequestHostObject>(this).url
}
- (id)HTTPBody {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_body
}
- (id)HTTPMethod {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_method
}
- (NSURLRequestCachePolicy)cachePolicy {
    env.objc.borrow::<NSURLRequestHostObject>(this).cache_policy
}
- (NSTimeInterval)timeoutInterval {
    env.objc.borrow::<NSURLRequestHostObject>(this).timeout_interval
}
- (id)allHTTPHeaderFields {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_header_fields
}
- (id)valueForHTTPHeaderField:(id)field { // NSString*
    let http_header_fields = env.objc.borrow::<NSURLRequestHostObject>(this).http_header_fields;
    msg![env; http_header_fields objectForKey:field]
}

- (bool)HTTPShouldHandleCookies {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_should_handle_cookies
}
- (bool)HTTPShouldUsePipelining {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_should_use_pipelining
}
- (NSUInteger)networkServiceType {
    env.objc.borrow::<NSURLRequestHostObject>(this).network_service_type
}
- (id)mainDocumentURL {
    env.objc.borrow::<NSURLRequestHostObject>(this).main_document_url
}
- (id)HTTPBodyStream {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_body_stream
}

// NSCopying and NSMutableCopying.
//
// A request is a value: SDKs take one they were handed, copy it, and adjust
// the copy's headers or method rather than mutating the caller's. Retaining
// instead of copying would make those adjustments visible through the
// original, and tapHLE's immutable class shares its host object with the
// mutable one, so retaining is not safe even for `copy`.
- (id)copyWithZone:(NSZonePtr)_zone {
    let new: id = msg_class![env; NSURLRequest alloc];
    copy_request_fields(env, this, new);
    new
}
- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    let new: id = msg_class![env; NSMutableURLRequest alloc];
    copy_request_fields(env, this, new);
    new
}

- (())dealloc {
    log_dbg!("[(NSURLRequest*){:?} dealloc]", this);
    let &NSURLRequestHostObject {
        url,
        http_method,
        http_body,
        http_header_fields,
        main_document_url,
        http_body_stream,
        ..
    } = env.objc.borrow(this);
    release(env, url);
    release(env, http_method);
    release(env, http_body);
    release(env, http_header_fields);
    release(env, main_document_url);
    release(env, http_body_stream);
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

@implementation NSMutableURLRequest: NSURLRequest

- (())setHTTPMethod:(id)http_method { // NSString *
    let http_method_copy = msg![env; http_method copy];

    let host_obj = env.objc.borrow_mut::<NSURLRequestHostObject>(this);
    let old_http_method = std::mem::replace(&mut host_obj.http_method, http_method_copy);
    release(env, old_http_method);
    // No need to retain http_method as we made a copy
}

- (())setHTTPBody:(id)http_body { // NSData *
    let http_body_copy = msg![env; http_body copy];

    let host_obj = env.objc.borrow_mut::<NSURLRequestHostObject>(this);
    let old_http_body = std::mem::replace(&mut host_obj.http_body, http_body_copy);
    release(env, old_http_body);
    // No need to retain http_body as we made a copy
}

// The cache policy and timeout are already stored by -initWithURL:...; these
// are the mutable subclass's setters for them.
- (())setCachePolicy:(NSURLRequestCachePolicy)cache_policy {
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).cache_policy = cache_policy;
}

- (())setTimeoutInterval:(NSTimeInterval)timeout_interval {
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).timeout_interval = timeout_interval;
}

- (())setAllHTTPHeaderFields:(id)fields { // NSDictionary*
    let http_header_fields = env.objc.borrow::<NSURLRequestHostObject>(this).http_header_fields;
    () = msg![env; http_header_fields removeAllObjects];
    if fields != nil {
        () = msg![env; http_header_fields addEntriesFromDictionary:fields];
    }
}

- (())setHTTPShouldHandleCookies:(bool)handle {
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).http_should_handle_cookies = handle;
}

- (())setHTTPShouldUsePipelining:(bool)pipeline {
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).http_should_use_pipelining = pipeline;
}

- (())setNetworkServiceType:(NSUInteger)service_type {
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).network_service_type = service_type;
}

- (())setMainDocumentURL:(id)url { // NSURL *
    let url_copy: id = msg![env; url copy];
    let host_obj = env.objc.borrow_mut::<NSURLRequestHostObject>(this);
    let old = std::mem::replace(&mut host_obj.main_document_url, url_copy);
    release(env, old);
}

- (())setHTTPBodyStream:(id)stream { // NSInputStream *
    retain(env, stream);
    let host_obj = env.objc.borrow_mut::<NSURLRequestHostObject>(this);
    let old = std::mem::replace(&mut host_obj.http_body_stream, stream);
    release(env, old);
}

- (())setURL:(id)url { // NSURL *
    let url_copy = msg![env; url copy];

    let host_obj = env.objc.borrow_mut::<NSURLRequestHostObject>(this);
    let old_url = std::mem::replace(&mut host_obj.url, url_copy);
    release(env, old_url);
    // No need to retain url_copy as we made a copy
}

- (())setValue:(id)value // NSString *
    forHTTPHeaderField:(id)field { // NSString *
    log_dbg!("[(NSURLRequest*){:?} setValue:'{}' forHTTPHeaderField:'{}']", this, to_rust_string(env, value), to_rust_string(env, field));
    let http_header_fields = env.objc.borrow_mut::<NSURLRequestHostObject>(this).http_header_fields;
    () = msg![env; http_header_fields setObject:value forKey:field];
}

- (())addValue:(id)value // NSString *
    forHTTPHeaderField:(id)field { // NSString *
    log_dbg!("[(NSURLRequest*){:?} addValue:'{}' forHTTPHeaderField:'{}']", this, to_rust_string(env, value), to_rust_string(env, field));
    let http_header_fields = env.objc.borrow_mut::<NSURLRequestHostObject>(this).http_header_fields;
    let existing: id = msg![env; http_header_fields objectForKey:field];
    assert_eq!(existing, nil); // TODO: append values with comma
    () = msg![env; http_header_fields setObject:value forKey:field];
}

@end

};
