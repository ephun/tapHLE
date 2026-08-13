//! Things from `NSObjCRuntime.h`.

use super::ns_string;
use crate::dyld::{export_c_func, FunctionExports};
use crate::objc::{id, nil, Class, SEL};
use crate::Environment;

fn NSStringFromSelector(env: &mut Environment, selector: SEL) -> id {
    // A null selector names nothing, and Foundation answers nil rather than
    // reading through it. This is not a defensive check: a null selector
    // reaches here on ordinary paths, because it is what an accessor for an
    // unset selector-typed property returns. The Jim and Frank Mysteries HD
    // takes exactly that route while starting a game, and dereferencing it
    // ended the app one tap after its menu became usable.
    if selector.is_null() {
        return nil;
    }
    // TODO: caching?
    let string = selector.as_str(&env.mem).to_string();
    ns_string::from_rust_string(env, string)
}

fn NSSelectorFromString(env: &mut Environment, string: id) -> SEL {
    // The mirror of the above, and nil for the same reason: there is no
    // selector named by no string.
    if string == nil {
        return SEL::null();
    }
    // TODO: avoid copy?
    let string = ns_string::to_rust_string(env, string);
    env.objc.register_host_selector(string.into(), &mut env.mem)
}

pub fn NSStringFromClass(env: &mut Environment, class: Class) -> id {
    if class.is_null() {
        return nil;
    }
    // TODO: caching?
    let string = env.objc.get_class_name(class).to_string();
    ns_string::from_rust_string(env, string)
}

fn NSClassFromString(env: &mut Environment, string: id) -> Class {
    if string == nil {
        return nil;
    }
    // TODO: avoid copy?
    let string = ns_string::to_rust_string(env, string);

    // Returning nil for an unknown class is both the documented behavior and
    // the whole point of this function: apps call it to find out whether a
    // class exists on the OS version they are running on, and branch to a
    // fallback when it does not. Glass Tower 3 probes for
    // GKLeaderboardViewController this way; panicking turned a supported
    // "Game Center is unavailable" path into an abort.
    //
    // The missing class is still worth knowing about, so say so loudly rather
    // than silently. If the app needed the class rather than merely probing
    // for it, that will surface as a later nil-receiver failure.
    let string = string.to_string();
    match env
        .objc
        .get_known_class_if_implemented(&string, &mut env.mem)
    {
        Some(class) => class,
        None => {
            log!("NSClassFromString(\"{}\") -> nil: no implementation of that class. This is correct if the app is probing for an OS feature.", string);
            nil
        }
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(NSStringFromSelector(_)),
    export_c_func!(NSSelectorFromString(_)),
    export_c_func!(NSClassFromString(_)),
    export_c_func!(NSStringFromClass(_)),
];
