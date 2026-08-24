/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSHTTPCookieStorage`.
//!
//! An in-memory cookie jar, held for the lifetime of the process and never
//! written to disk. tapHLE's networking does not send or receive cookies, so
//! nothing populates this by itself; what it provides is a store that behaves
//! consistently for an app that sets a cookie and reads it back, and a shared
//! instance that exists rather than aborting the app.
//!
//! Resources:
//! - Apple's [NSHTTPCookieStorage](https://developer.apple.com/documentation/foundation/nshttpcookiestorage)

use crate::frameworks::foundation::{ns_array, NSInteger};
use crate::objc::{
    id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
};

/// `NSHTTPCookieAcceptPolicy`.
type NSHTTPCookieAcceptPolicy = NSInteger;

#[derive(Default)]
pub struct State {
    shared: Option<id>,
}

#[derive(Default)]
struct NSHTTPCookieStorageHostObject {
    /// `NSHTTPCookie*`, each retained.
    cookies: Vec<id>,
    accept_policy: NSHTTPCookieAcceptPolicy,
}
impl HostObject for NSHTTPCookieStorageHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSHTTPCookieStorage: NSObject

+ (id)sharedHTTPCookieStorage {
    if let Some(shared) = env.framework_state.foundation.ns_http_cookie_storage.shared {
        return shared;
    }
    let host_object = Box::<NSHTTPCookieStorageHostObject>::default();
    let new = env.objc.alloc_object(this, host_object, &mut env.mem);
    env.framework_state.foundation.ns_http_cookie_storage.shared = Some(new);
    new
}

- (id)cookies {
    let cookies = env.objc.borrow::<NSHTTPCookieStorageHostObject>(this).cookies.clone();
    ns_array::from_vec(env, cookies)
}

// No cookie is ever associated with a URL, because nothing here parses
// Set-Cookie headers. An empty array is the honest answer and is what a caller
// on a fresh session would get anyway.
- (id)cookiesForURL:(id)_url { // NSURL*
    msg_class![env; NSArray array]
}

- (())setCookie:(id)cookie { // NSHTTPCookie*
    if cookie == nil {
        return;
    }
    retain(env, cookie);
    env.objc.borrow_mut::<NSHTTPCookieStorageHostObject>(this).cookies.push(cookie);
}

- (())deleteCookie:(id)cookie { // NSHTTPCookie*
    let cookies = &mut env.objc.borrow_mut::<NSHTTPCookieStorageHostObject>(this).cookies;
    let Some(index) = cookies.iter().position(|&c| c == cookie) else {
        return;
    };
    let cookie = cookies.remove(index);
    release(env, cookie);
}

- (())setCookies:(id)cookies // NSArray*
          forURL:(id)_url // NSURL*
 mainDocumentURL:(id)_main {
    let count: crate::frameworks::foundation::NSUInteger = msg![env; cookies count];
    for i in 0..count {
        let cookie: id = msg![env; cookies objectAtIndex:i];
        () = msg![env; this setCookie:cookie];
    }
}

- (NSHTTPCookieAcceptPolicy)cookieAcceptPolicy {
    env.objc.borrow::<NSHTTPCookieStorageHostObject>(this).accept_policy
}
- (())setCookieAcceptPolicy:(NSHTTPCookieAcceptPolicy)policy {
    env.objc.borrow_mut::<NSHTTPCookieStorageHostObject>(this).accept_policy = policy;
}

@end

};
