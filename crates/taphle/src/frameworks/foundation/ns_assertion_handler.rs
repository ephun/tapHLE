/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::frameworks::foundation::ns_string;
use crate::frameworks::foundation::NSUInteger;
use crate::objc::{id, msg, msg_class, nil, objc_classes, release, ClassExports, SEL};

const THREAD_DICTIONARY_KEY: &str = "NSAssertionHandler";

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSAssertionHandler: NSObject

+ (id)currentHandler {
    let thread: id = msg_class![env; NSThread currentThread];
    let thread_dictionary: id = msg![env; thread threadDictionary];
    let key = ns_string::get_static_str(env, THREAD_DICTIONARY_KEY);
    let handler: id = msg![env; thread_dictionary objectForKey:key];
    if handler != nil {
        return handler;
    }

    let handler: id = msg![env; this new];
    let (): () = msg![env; thread_dictionary setObject:handler forKey:key];
    release(env, handler);
    handler
}

// These methods normally raise an NSInternalInconsistencyException. tapHLE
// does not yet implement Objective-C exception delivery, so preserve the
// diagnostic while allowing an app to continue through its release checks.
- (())handleFailureInMethod:(SEL)method
                     object:(id)_object
                       file:(id)file
                 lineNumber:(NSUInteger)line
                description:(id)format, ...args {
    let description = ns_string::with_format(env, format, args.start());
    log!(
        "NSAssertionHandler: assertion in {:?} at {}:{}: {}",
        method,
        ns_string::to_rust_string(env, file),
        line,
        description
    );
}

- (())handleFailureInFunction:(id)function
                         file:(id)file
                   lineNumber:(NSUInteger)line
                  description:(id)format, ...args {
    let description = ns_string::with_format(env, format, args.start());
    log!(
        "NSAssertionHandler: assertion in {} at {}:{}: {}",
        ns_string::to_rust_string(env, function),
        ns_string::to_rust_string(env, file),
        line,
        description
    );
}

@end

};
