/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! SCNetworkReachability

use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_foundation::cf_allocator::{kCFAllocatorDefault, CFAllocatorRef};
use crate::frameworks::core_foundation::CFTypeRef;
use crate::frameworks::foundation::ns_run_loop;
use crate::libc::sys::socket::sockaddr;
use crate::mem::{ConstPtr, MutPtr, MutVoidPtr, Ptr};
use crate::objc::{id, msg, msg_class, nil, objc_classes, Class, ClassExports, HostObject};
use crate::Environment;
use std::net::SocketAddrV4;

type SCNetworkReachabilityFlags = u32;
const kSCNetworkReachabilityFlagsReachable: SCNetworkReachabilityFlags = 1 << 1;
const kSCNetworkReachabilityFlagsIsDirect: SCNetworkReachabilityFlags = 1 << 17;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// SCNetworkReachabilityRef is not explicitly stated to be CFType-based type,
// but the result of "Create" or "Copy" functions here is expected to be
// released with CFRelease().
@implementation _tapHLE_SCNetworkReachability: NSObject

// Deliver the callback a freshly scheduled target gets on a real device. It is
// queued on the run loop rather than called from
// [SCNetworkReachabilityScheduleWithRunLoop] so the caller finishes scheduling
// before its own callback re-enters it, which is the order a device produces.
- (())tapHLE_deliverInitialReachability {
    let &SCNetworkReachabilityHostObject { callout, context, .. } = env.objc.borrow(this);
    let Some(callout) = callout else {
        return;
    };
    // The context's `info` is the second word of SCNetworkReachabilityContext,
    // after its `version`. A null context means no user pointer.
    let info: MutVoidPtr = if context.is_null() {
        Ptr::null()
    } else {
        env.mem.read(context.cast::<MutVoidPtr>() + 1)
    };
    let flags = current_flags(env);
    log_dbg!(
        "Delivering initial reachability callback {:?}({:?}, flags {:#x}, info {:?})",
        callout, this, flags, info
    );
    () = callout.call_from_host(env, (this, flags, info));
}

@end

};

struct SCNetworkReachabilityHostObject {
    address: Option<SocketAddrV4>,
    /// `SCNetworkReachabilityCallBack`, set by
    /// [SCNetworkReachabilitySetCallback].
    callout: Option<GuestFunction>,
    /// The `SCNetworkReachabilityContext *` that came with it, unretained: this
    /// never outlives the scheduling call that reads it.
    context: MutVoidPtr,
}
impl HostObject for SCNetworkReachabilityHostObject {}

/// The flags tapHLE reports, in one place so the polled answer and the
/// callback's answer cannot disagree.
fn current_flags(env: &Environment) -> SCNetworkReachabilityFlags {
    if env.options.network_access {
        kSCNetworkReachabilityFlagsReachable | kSCNetworkReachabilityFlagsIsDirect
    } else {
        0
    }
}

// See comment for `_tapHLE_SCNetworkReachability` class
type SCNetworkReachabilityRef = CFTypeRef;

fn SCNetworkReachabilityCreateWithName(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    name: ConstPtr<u8>,
) -> SCNetworkReachabilityRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    if env
        .bundle
        .bundle_identifier()
        .starts_with("com.chillingo.cuttherope")
        && env.mem.cstr_at_utf8(name).unwrap() == "chillingo-crystal.appspot.com"
    {
        log!("Applying game-specific hack for Cut the Rope: SCNetworkReachabilityCreateWithName(\"chillingo-crystal.appspot.com\") returns NULL");
        return Ptr::null();
    }
    let isa = env
        .objc
        .get_known_class("_tapHLE_SCNetworkReachability", &mut env.mem);
    let res = env.objc.alloc_object(
        isa,
        Box::new(SCNetworkReachabilityHostObject {
            address: None, // TODO
            callout: None,
            context: Ptr::null(),
        }),
        &mut env.mem,
    );
    log!(
        "TODO: SCNetworkReachabilityCreateWithName({:?}, {:?} {:?}) -> {:?}",
        allocator,
        name,
        env.mem.cstr_at_utf8(name),
        res
    );
    res
}

fn SCNetworkReachabilityCreateWithAddress(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    address: ConstPtr<sockaddr>,
) -> SCNetworkReachabilityRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    let isa = env
        .objc
        .get_known_class("_tapHLE_SCNetworkReachability", &mut env.mem);
    let address_val = env.mem.read(address);
    let res = env.objc.alloc_object(
        isa,
        Box::new(SCNetworkReachabilityHostObject {
            address: Some(address_val.to_sockaddr_v4()),
            callout: None,
            context: Ptr::null(),
        }),
        &mut env.mem,
    );
    log_dbg!(
        "SCNetworkReachabilityCreateWithAddress({:?}, {:?} ({})) -> {:?}",
        allocator,
        address,
        address_val.to_sockaddr_v4(),
        res
    );
    res
}

/// Answer a reachability query the way a device with no network answers it.
///
/// The return value and the flags mean different things. `false` is "the query
/// failed, I do not know", and it leaves the caller's `flags` untouched —
/// uninitialized stack, usually. `true` with no `Reachable` bit is "I do know,
/// and you are offline". A real device in airplane mode gives the second
/// answer, never the first.
///
/// This mattered because reporting failure hangs SDKs rather than failing them.
/// Chillingo's Crystal asks whether it is online before starting a session; on
/// "I do not know" it neither proceeds nor gives up, so the splash screen it
/// presents over the game is never dismissed, and being a full-screen modal it
/// swallows every touch. That is what stopped The Jim and Frank Mysteries HD at
/// its main menu, and a game-specific hack in
/// [SCNetworkReachabilityCreateWithName] was already papering over the same
/// hang in Cut the Rope.
fn SCNetworkReachabilityGetFlags(
    env: &mut Environment,
    target: SCNetworkReachabilityRef,
    flags: MutPtr<SCNetworkReachabilityFlags>,
) -> bool {
    if !env.options.network_access {
        log_dbg!(
            "Network access is disabled, SCNetworkReachabilityGetFlags({:?}, {:?}) -> true, not reachable",
            target,
            flags
        );
        env.mem.write(flags, 0);
        return true;
    }

    let target_class: Class = msg![env; target class];
    assert_eq!(
        target_class,
        env.objc
            .get_known_class("_tapHLE_SCNetworkReachability", &mut env.mem)
    );
    let host_object = env.objc.borrow::<SCNetworkReachabilityHostObject>(target);
    if let Some(addr) = host_object.address {
        if addr.ip().is_link_local() {
            log_dbg!(
                "SCNetworkReachabilityGetFlags({:?}, {:?}) -> true",
                target,
                flags
            );
            // Those corresponds to local WiFi connection on a real iOS device
            // TODO: actually check for the connectivity
            // (but do we _really_ need it?)
            let out_flags =
                kSCNetworkReachabilityFlagsReachable | kSCNetworkReachabilityFlagsIsDirect;
            env.mem.write(flags, out_flags);
            return true;
        }
    }
    log!(
        "TODO: SCNetworkReachabilityGetFlags({:?}, {:?}) -> false",
        target,
        flags
    );
    false
}

/// Accept a reachability callback registration, which is never called back.
///
/// Refusing the registration is the wrong answer for the same reason returning
/// failure from [SCNetworkReachabilityGetFlags] is. An SDK that cannot read the
/// state synchronously will wait to be told when it changes, so a refusal here
/// closes the second door after the first, and it waits forever.
///
/// Accepting and never calling back is not a stub: on a device whose
/// reachability never changes, a correct implementation delivers nothing
/// either. The callback fires on transitions, and tapHLE has none to report.
fn SCNetworkReachabilitySetCallback(
    env: &mut Environment,
    target: SCNetworkReachabilityRef,
    callout: GuestFunction, // SCNetworkReachabilityCallBack
    context: MutVoidPtr,    // SCNetworkReachabilityContext *
) -> bool {
    let target_class: Class = msg![env; target class];
    assert_eq!(
        target_class,
        env.objc
            .get_known_class("_tapHLE_SCNetworkReachability", &mut env.mem)
    );
    log_dbg!(
        "SCNetworkReachabilitySetCallback({:?}, {:?}, {:?}) -> TRUE",
        target,
        callout,
        context
    );
    let host_object = env
        .objc
        .borrow_mut::<SCNetworkReachabilityHostObject>(target);
    host_object.callout = if callout.to_ptr().is_null() {
        None
    } else {
        Some(callout)
    };
    host_object.context = context;
    true
}

/// Scheduling a target makes a device deliver the current state once, promptly,
/// without waiting for anything to change. SDKs lean on that: they register,
/// schedule, and then wait to be told where they stand rather than polling.
///
/// tapHLE has no state changes to report, so this one delivery is the whole of
/// the callback's life — but omitting it is not "no news", it is silence, and
/// an SDK that is waiting to be told cannot distinguish silence from a slow
/// network. Chillingo's Crystal waits exactly there.
fn SCNetworkReachabilityScheduleWithRunLoop(
    env: &mut Environment,
    target: SCNetworkReachabilityRef,
    _run_loop: CFTypeRef,      // CFRunLoopRef
    _run_loop_mode: CFTypeRef, // CFStringRef
) -> bool {
    if env
        .objc
        .borrow::<SCNetworkReachabilityHostObject>(target)
        .callout
        .is_none()
    {
        return true;
    }
    let selector = env
        .objc
        .lookup_selector("tapHLE_deliverInitialReachability")
        .unwrap();
    // The main run loop, for the same reason NSURLConnection uses it: tapHLE
    // pumps only that one, and these targets are commonly scheduled from a
    // worker thread whose run loop never runs.
    let run_loop: id = msg_class![env; NSRunLoop mainRunLoop];
    ns_run_loop::add_perform_request(env, run_loop, target, selector, nil, Some(0.0), false);
    true
}

fn SCNetworkReachabilityUnscheduleFromRunLoop(
    _env: &mut Environment,
    _target: SCNetworkReachabilityRef,
    _run_loop: CFTypeRef,      // CFRunLoopRef
    _run_loop_mode: CFTypeRef, // CFStringRef
) -> bool {
    true
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(SCNetworkReachabilityCreateWithName(_, _)),
    export_c_func!(SCNetworkReachabilityCreateWithAddress(_, _)),
    export_c_func!(SCNetworkReachabilityGetFlags(_, _)),
    export_c_func!(SCNetworkReachabilitySetCallback(_, _, _)),
    export_c_func!(SCNetworkReachabilityScheduleWithRunLoop(_, _, _)),
    export_c_func!(SCNetworkReachabilityUnscheduleFromRunLoop(_, _, _)),
];
