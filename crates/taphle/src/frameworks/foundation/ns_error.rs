/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::foundation::{ns_string, NSInteger};
use crate::objc::{
    autorelease, id, msg, nil, objc_classes, release, retain, ClassExports, HostObject, NSZonePtr,
};

/// `NSString*`
pub type NSErrorDomain = id;

pub const NSCocoaErrorDomain: &str = "NSCocoaErrorDomain";
pub const NSOSStatusErrorDomain: &str = "NSOSStatusErrorDomain";
/// The remaining error domains Foundation declares. Nothing here raises errors
/// in them yet, but apps reference the symbols to compare against an error's
/// domain, and an unbound one is a null pointer they dereference to do it.
pub const NSPOSIXErrorDomain: &str = "NSPOSIXErrorDomain";
pub const NSMachErrorDomain: &str = "NSMachErrorDomain";

// `NSUnderlyingErrorKey` is exported from foundation.rs, with Apple's actual
// value "NSUnderlyingError" rather than the symbol's own name.
const NSFilePathErrorKey: &str = "NSFilePathErrorKey";
const NSStringEncodingErrorKey: &str = "NSStringEncodingErrorKey";
const NSLocalizedRecoverySuggestionErrorKey: &str = "NSLocalizedRecoverySuggestionErrorKey";

const NSLocalizedDescriptionKey: &str = "NSLocalizedDescriptionKey";
const NSLocalizedFailureReasonErrorKey: &str = "NSLocalizedFailureReasonErrorKey";

pub const NSFileReadNoSuchFileError: NSInteger = 260;
/// The generic "could not write it" code. Cocoa has finer-grained ones (no
/// permission, volume full, …) but the guest filesystem does not distinguish
/// them, so reporting a specific cause would be inventing one.
pub const NSFileWriteUnknownError: NSInteger = 512;

struct ErrorHostObject {
    domain: NSErrorDomain,
    code: NSInteger,
    user_info: id,
}
impl HostObject for ErrorHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// TODO: Return errors in all methods that are supposed to do it.
@implementation NSError: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(ErrorHostObject {
        domain: nil,
        code: 0,
        user_info: nil
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)errorWithDomain:(NSErrorDomain)domain
                 code:(NSInteger)code
             userInfo:(id)user_info {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithDomain:domain code:code userInfo:user_info];
    autorelease(env, new)
}

- (id)initWithDomain:(NSErrorDomain)domain
                code:(NSInteger)code
            userInfo:(id)user_info {
    retain(env, domain);
    retain(env, user_info);
    let host_object = env.objc.borrow_mut::<ErrorHostObject>(this);
    host_object.domain = domain;
    host_object.code = code;
    host_object.user_info = user_info;
    this
}

// NSObject's -description is what NSLog("%@") and every diagnostic path reach
// for, and NSError's is documented to be the localized description. Without it
// an app logging an error it had handled perfectly well died on an unrecognised
// selector instead.
- (id)description {
    msg![env; this localizedDescription]
}

- (id)localizedDescription {
    let user_info =  env.objc.borrow::<ErrorHostObject>(this).user_info;
    let key = ns_string::get_static_str(env, NSLocalizedDescriptionKey);
    let localized = msg![env; user_info objectForKey:key];
    if localized != nil {
        return localized;
    }
    let &ErrorHostObject{ domain, code, .. } = env.objc.borrow(this);
    let domain = ns_string::to_rust_string(env, domain);
    let error_str = format!("Error Domain={} Code={}", domain, code);
    // TODO: cache the result?
    let res = ns_string::from_rust_string(env, error_str);
    autorelease(env, res)
}

- (id)localizedFailureReason {
    let user_info =  env.objc.borrow::<ErrorHostObject>(this).user_info;
    let key = ns_string::get_static_str(env, NSLocalizedFailureReasonErrorKey);
    msg![env; user_info objectForKey:key]
}

- (())dealloc {
    let &ErrorHostObject{ domain, user_info, .. } = env.objc.borrow(this);
    release(env, domain);
    release(env, user_info);

    env.objc.dealloc_object(this, &mut env.mem);
}

- (NSInteger)code {
    env.objc.borrow::<ErrorHostObject>(this).code
}

- (NSErrorDomain)domain {
    env.objc.borrow::<ErrorHostObject>(this).domain
}

- (id)userInfo {
    env.objc.borrow::<ErrorHostObject>(this).user_info
}

@end

};

pub const CONSTANTS: ConstantExports = &[
    (
        "_NSLocalizedDescriptionKey",
        HostConstant::NSString(NSLocalizedDescriptionKey),
    ),
    (
        "_NSLocalizedFailureReasonErrorKey",
        HostConstant::NSString(NSLocalizedFailureReasonErrorKey),
    ),
    (
        "_NSCocoaErrorDomain",
        HostConstant::NSString(NSCocoaErrorDomain),
    ),
    (
        "_NSOSStatusErrorDomain",
        HostConstant::NSString(NSOSStatusErrorDomain),
    ),
    (
        "_NSPOSIXErrorDomain",
        HostConstant::NSString(NSPOSIXErrorDomain),
    ),
    (
        "_NSMachErrorDomain",
        HostConstant::NSString(NSMachErrorDomain),
    ),
    (
        "_NSFilePathErrorKey",
        HostConstant::NSString(NSFilePathErrorKey),
    ),
    (
        "_NSStringEncodingErrorKey",
        HostConstant::NSString(NSStringEncodingErrorKey),
    ),
    (
        "_NSLocalizedRecoverySuggestionErrorKey",
        HostConstant::NSString(NSLocalizedRecoverySuggestionErrorKey),
    ),
];
