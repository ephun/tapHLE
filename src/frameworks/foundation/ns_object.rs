/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSObject`, the root of most class hierarchies in Objective-C.
//!
//! Resources:
//! - Apple's [Advanced Memory Management Programming Guide](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/MemoryMgmt/Articles/MemoryMgmt.html)
//!   explains how reference counting works. Note that we are interested in what
//!   it calls "manual retain-release", not ARC.
//! - Apple's [Key-Value Coding Programming Guide](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/KeyValueCoding/SearchImplementation.html)
//!   explains the algorithms `setValue:forKey:` and `valueForKey:` should
//!   follow.
//!
//! See also: [crate::objc], especially the `objects` module.

use super::ns_string::{from_rust_string, to_rust_string};
use super::{ns_dictionary, NSTimeInterval, NSUInteger};
use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::frameworks::foundation::ns_run_loop::{
    add_perform_request, cancel_all_perform_requests_for_target, cancel_perform_requests,
};
use crate::frameworks::foundation::ns_thread::detach_new_thread_inner;
use crate::libc::semaphore::{host_destroy_semaphore, sem_wait};
use crate::mem::{ConstVoidPtr, MutVoidPtr, Ptr};
use crate::objc::{
    autorelease, id, msg, msg_class, msg_send, msg_send_no_type_checking, nil, objc_classes,
    retain, Class, ClassExports, NSZonePtr, ObjC, TrivialHostObject, IMP, SEL,
};
use crate::Environment;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSObject

+ (id)alloc {
    msg![env; this allocWithZone:(MutVoidPtr::null())]
}
+ (id)allocWithZone:(NSZonePtr)_zone { // struct _NSZone*
    log_dbg!("[{:?} allocWithZone:]", this);
    env.objc.alloc_object(this, Box::new(TrivialHostObject), &mut env.mem)
}

+ (id)new {
    let new_object: id = msg![env; this alloc];
    msg![env; new_object init]
}

+ (Class)class {
    this
}
+ (Class)superclass {
    env.objc.class_get_superclass(this)
}
+ (bool)isSubclassOfClass:(Class)class {
    env.objc.class_is_subclass_of(this, class)
}

// See the instance method section for the normal versions of these.
+ (id)retain {
    this // classes are not refcounted
}
+ (())release {
    // classes are not refcounted
}
+ (())autorelease {
    // classes are not refcounted
}

+ (bool)instancesRespondToSelector:(SEL)selector {
    env.objc.class_has_method(this, selector)
}

+ (ConstVoidPtr)instanceMethodForSelector:(SEL)selector {
    match env.objc.class_get_method_implementation(this, selector) {
        Some(IMP::Guest(imp)) => imp.to_ptr(),
        Some(IMP::Host(_)) => {
            log_once!("TODO: NSObject instanceMethodForSelector: for a host implementation");
            ConstVoidPtr::null()
        }
        None => ConstVoidPtr::null(),
    }
}

+ (())cancelPreviousPerformRequestsWithTarget:(id)target selector:(SEL)selector object:(id)arg {
    let run_loop: id = msg_class![env; NSRunLoop currentRunLoop];
    cancel_perform_requests(env, run_loop, target, selector, arg);
}

+ (())cancelPreviousPerformRequestsWithTarget:(id)target {
    let run_loop: id = msg_class![env; NSRunLoop currentRunLoop];
    cancel_all_perform_requests_for_target(env, run_loop, target);
}

+ (bool)accessInstanceVariablesDirectly {
    true
}

+ (id)description {
    let name = env.objc.get_class_name(this);
    let str = from_rust_string(env, name.to_string());
    autorelease(env, str)
}

+ (id)debugDescription {
    msg![env; this description]
}

+ (id)instanceMethodSignatureForSelector:(SEL)sel {
    // Host implementations do not yet carry Objective-C type encodings, and
    // asking about an unimplemented selector is valid. Both cases have no
    // method signature rather than being an error.
    let Some(sig) = env.objc.class_get_method_signature(this, sel).copied() else {
        return nil;
    };
    log_dbg!("instanceMethodSignatureForSelector: '{}' -> {:?}", sel.as_str(&env.mem), env.mem.cstr_at_utf8(sig));
    msg_class![env; NSMethodSignature signatureWithObjCTypes:sig]
}

+ (())initialize {
    // Do nothing
}

// Real NSObject implements +load, and a guest class's own +load routinely ends
// with [super load]. Without this that super-send aborted the app.
+ (())load {
    // Do nothing
}

- (id)init {
    this
}

// Minimal key-value observing. Registration is accepted so apps that observe
// optional/background state (e.g. social-SDK device status) do not crash, but
// change notifications are not delivered: tapHLE does not swizzle setters for
// automatic KVO, and manual willChange/didChange notifications are not modeled.
- (())addObserver:(id)_observer
       forKeyPath:(id)_key_path
          options:(NSUInteger)_options
          context:(MutVoidPtr)_context {
    log_dbg!("TODO: ignoring KVO addObserver: on {:?}", this);
}
- (())removeObserver:(id)_observer
          forKeyPath:(id)_key_path {
    log_dbg!("TODO: ignoring KVO removeObserver: on {:?}", this);
}
- (())removeObserver:(id)_observer
          forKeyPath:(id)_key_path
             context:(MutVoidPtr)_context {
    log_dbg!("TODO: ignoring KVO removeObserver:context: on {:?}", this);
}

// `self` is used by code generated for some Objective-C property accessors.
// It is inherited by every NSObject subclass and simply returns the receiver.
- (id)self {
    this
}


- (NSUInteger)retainCount {
    env.objc.get_refcount(this).into()
}

- (id)retain {
    log_dbg!("[{:?} retain]", this);
    env.objc.increment_refcount(this);
    this
}
- (())release {
    log_dbg!("[{:?} release]", this);
    if env.objc.decrement_refcount(this) {
        () = msg![env; this dealloc];
    }
}
- (id)autorelease {
    () = msg_class![env; NSAutoreleasePool addObject:this];
    this
}

- (())dealloc {
    log_dbg!("[{:?} dealloc]", this);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (Class)class {
    ObjC::read_isa(this, &env.mem)
}
- (bool)isMemberOfClass:(Class)class {
    let this_class: Class = msg![env; this class];
    class == this_class
}
- (bool)isKindOfClass:(Class)class {
    let this_class: Class = msg![env; this class];
    env.objc.class_is_subclass_of(this_class, class)
}

- (NSUInteger)hash {
    this.to_bits()
}

// To not confuse with isEqualTo:, which is
// a category of NSWhoseSpecifier!
// Reference https://nshipster.com/equality
- (bool)isEqual:(id)other {
    this == other
}

// TODO: Instance description and debugDescription.
// This is not hard to add, but before adding a fallback implementation of it,
// we should make sure all the Foundation classes' overrides of it are there,
// to prevent weird behavior.
// TODO: localized description methods also? (not sure if NSObject has them)

// Helper for NSCopying
- (id)copy {
    msg![env; this copyWithZone:(MutVoidPtr::null())]
}

// Helper for NSMutableCopying
- (id)mutableCopy {
    msg![env; this mutableCopyWithZone:(MutVoidPtr::null())]
}

// NSKeyValueCoding
// https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/KeyValueCoding/SearchImplementation.html
- (())setValue:(id)value
       forKey:(id)key { // NSString*
    let key_string = to_rust_string(env, key); // TODO: avoid copy?
    assert!(key_string.is_ascii()); // TODO: do we have to handle non-ASCII keys?
    let camel_case_key_string = format!("{}{}", key_string.as_bytes()[0].to_ascii_uppercase() as char, &key_string[1..]);

    let class = msg![env; this class];

    // When the value is a boxed scalar (NSNumber/NSValue) but the target
    // accessor takes a plain scalar (e.g. -[... setVolume:(double)]), KVC
    // unwraps the box and passes the scalar by value.
    // `kvc_set_unwrapped_scalar` consults the setter's type encoding and
    // handles that; an object-typed setter falls through to receive the boxed
    // value unchanged.
    let value_is_boxed_scalar = if value == nil {
        false
    } else {
        let value_class = msg![env; value class];
        let ns_value_class = env.objc.get_known_class("NSValue", &mut env.mem);
        env.objc.class_is_subclass_of(value_class, ns_value_class)
    };

    // Look for the first accessor named set<Key>: or _set<Key>, in that order.
    // If found, invoke it with the input value (or unwrapped value, as needed)
    // and finish.
    let setter = env
        .objc
        .lookup_selector(&format!("set{camel_case_key_string}:"))
        .filter(|&sel| env.objc.class_has_method(class, sel))
        .or_else(|| {
            env.objc
                .lookup_selector(&format!("_set{camel_case_key_string}:"))
                .filter(|&sel| env.objc.class_has_method(class, sel))
        });
    if crate::log::debug_enabled_for(module_path!()) {
        let cls: Class = msg![env; this class];
        let cls_name = env.objc.get_class_name(cls).to_string();
        let found: String = match setter {
            Some(sel) => sel.as_str(&env.mem).to_string(),
            None => "<none — falling back to ivar>".to_string(),
        };
        log_dbg!("KVC set '{}' on {} -> {}", key_string, cls_name, found);
    }
    if let Some(sel) = setter {
        // nil only means something to an object-typed setter. For any other
        // type there is no value to write, so Apple's documented behaviour is
        // to invoke setNilValueForKey: instead.
        if value == nil && !matches!(kvc_setter_arg_type(env, this, sel), None | Some(b'@' | b'#')) {
            let sel = env.objc.lookup_selector("setNilValueForKey:").unwrap();
            () = msg_send(env, (this, sel, key));
            return;
        }
        if value_is_boxed_scalar && kvc_set_unwrapped_scalar(env, this, sel, value) {
            return;
        }
        () = msg_send(env, (this, sel, value));
        return;
    }

    // TODO: a boxed scalar written into a scalar ivar directly (no accessor)
    // still needs unwrapping via the ivar's type encoding; the direct-ivar path
    // below only handles object-typed ivars.

    // If no simple accessor is found, and if the class method
    // accessInstanceVariablesDirectly returns YES, look for an instance
    // variable with a name like _<key>, _is<Key>, <key>, or is<Key>,
    // in that order.
    // If found, set the variable directly with the input value
    // (or unwrapped value) and finish.
    let sel = env.objc.lookup_selector("accessInstanceVariablesDirectly").unwrap();
    let accessInstanceVariablesDirectly = msg_send(env, (class, sel));
    if accessInstanceVariablesDirectly {
        if let Some(ivar_ptr) = env.objc.object_lookup_ivar(&env.mem, this, &format!("_{key_string}"))
            .or_else(|| env.objc.object_lookup_ivar(&env.mem, this, &format!("_is{camel_case_key_string}")))
            .or_else(|| env.objc.object_lookup_ivar(&env.mem, this, &format!("{key_string}")))
            .or_else(|| env.objc.object_lookup_ivar(&env.mem, this, &format!("is{camel_case_key_string}"))
        ) {
            retain(env, value);
            env.mem.write(ivar_ptr.cast(), value);
            return;
        }
    }

    // Upon finding no accessor or instance variable,
    // invoke setValue:forUndefinedKey:.
    // This raises an exception by default, but a subclass of NSObject
    // may provide key-specific behavior.
    let sel = env.objc.lookup_selector("setValue:forUndefinedKey:").unwrap();
    () = msg_send(env, (this, sel, value, key));
}

- (id)valueForKey:(id)key { // NSString*
    let key_string = to_rust_string(env, key); // TODO: avoid copy?
    assert!(key_string.is_ascii()); // TODO: do we have to handle non-ASCII keys?
    let camel_case_key_string = format!("{}{}", key_string.as_bytes()[0].to_ascii_uppercase() as char, &key_string[1..]);

    let class = msg![env; this class];

    // Look for the first accessor named get<Key>, <key>, is<Key> or _<key>, in
    // that order. If found, invoke it and return its result, boxed in an
    // NSNumber if the accessor does not return an object.
    let getter = [
        format!("get{camel_case_key_string}"),
        key_string.to_string(),
        format!("is{camel_case_key_string}"),
        format!("_{key_string}"),
    ]
    .iter()
    .find_map(|name| {
        env.objc
            .lookup_selector(name)
            .filter(|&sel| env.objc.class_has_method(class, sel))
    });
    if let Some(sel) = getter {
        return kvc_get_boxed_value(env, this, sel);
    }

    // TODO: the to-many accessor patterns (countOf<Key> together with
    // objectIn<Key>AtIndex: or <key>AtIndexes:, and countOf<Key> together with
    // enumeratorOf<Key> and memberOf<Key>:), which return a proxy collection.

    // If no simple accessor is found, and if the class method
    // accessInstanceVariablesDirectly returns YES, look for an instance
    // variable with a name like _<key>, _is<Key>, <key>, or is<Key>,
    // in that order, and return its value.
    let sel = env.objc.lookup_selector("accessInstanceVariablesDirectly").unwrap();
    let accessInstanceVariablesDirectly = msg_send(env, (class, sel));
    if accessInstanceVariablesDirectly {
        // TODO: ivar type encodings are not recorded (see `ivar_t` handling in
        // objc::properties), so a scalar ivar cannot be boxed here and is read
        // as if it were an object. `setValue:forKey:` has the same limitation
        // in the same position.
        if let Some(ivar_ptr) = env.objc.object_lookup_ivar(&env.mem, this, &format!("_{key_string}"))
            .or_else(|| env.objc.object_lookup_ivar(&env.mem, this, &format!("_is{camel_case_key_string}")))
            .or_else(|| env.objc.object_lookup_ivar(&env.mem, this, &format!("{key_string}")))
            .or_else(|| env.objc.object_lookup_ivar(&env.mem, this, &format!("is{camel_case_key_string}"))
        ) {
            return env.mem.read(ivar_ptr.cast());
        }
    }

    // Upon finding no accessor or instance variable,
    // invoke valueForUndefinedKey:.
    // This raises an exception by default, but a subclass of NSObject
    // may provide key-specific behavior.
    let sel = env.objc.lookup_selector("valueForUndefinedKey:").unwrap();
    msg_send(env, (this, sel, key))
}

// Apple's KVC implementation obtains each requested value through
// `valueForKey:` and represents nil values with the NSNull singleton.
// See https://developer.apple.com/documentation/objectivec/nsobject-swift.class/dictionarywithvalues%28forkeys%3A%29.
- (id)dictionaryWithValuesForKeys:(id)keys { // NSArray<NSString *> *
    let count: NSUInteger = msg![env; keys count];
    let null: id = msg_class![env; NSNull null];
    let mut entries = Vec::with_capacity(count as usize);

    for index in 0..count {
        let key: id = msg![env; keys objectAtIndex:index];
        let value: id = msg![env; this valueForKey:key];
        let value = if value == nil { null } else { value };
        entries.push((key, value));
    }

    let dictionary = ns_dictionary::dict_from_keys_and_objects(env, &entries);
    autorelease(env, dictionary)
}

// The write direction of `dictionaryWithValuesForKeys:`, and Apple implements
// it the same way round: every pair goes through `setValue:forKey:`, so a class
// that overrides that accessor sees these too.
//
// `NSNull` is how a property list or dictionary spells "no value", and it is
// translated back to nil here rather than being assigned as an object — that
// asymmetry is deliberate and matches the read direction above.
- (())setValuesForKeysWithDictionary:(id)dictionary { // NSDictionary*
    let keys: id = msg![env; dictionary allKeys];
    let count: NSUInteger = msg![env; keys count];
    let null: id = msg_class![env; NSNull null];

    for index in 0..count {
        let key: id = msg![env; keys objectAtIndex:index];
        let value: id = msg![env; dictionary objectForKey:key];
        let value = if value == null { nil } else { value };
        () = msg![env; this setValue:value forKey:key];
    }
}

- (id)valueForUndefinedKey:(id)key { // NSString*
    // TODO: Raise NSUnknownKeyException
    let class: Class = ObjC::read_isa(this, &env.mem);
    let class_name_string = env.objc.get_class_name(class).to_owned(); // TODO: Avoid copying
    let key_string = to_rust_string(env, key);
    panic!("Object {:?} of class {:?} ({:?}) does not have a getter for {} ({:?})\
        \nAvailable selectors: {}\nAvailable ivars: {}",
        this, class_name_string, class, key_string, key,
        env.objc.debug_all_class_selectors_as_strings(&env.mem, class).join(", "),
        env.objc.debug_all_class_ivars_as_strings(class).join(", "));
}

- (())setNilValueForKey:(id)key { // NSString*
    // TODO: Raise NSInvalidArgumentException
    let class: Class = ObjC::read_isa(this, &env.mem);
    let class_name_string = env.objc.get_class_name(class).to_owned(); // TODO: Avoid copying
    let key_string = to_rust_string(env, key);
    panic!("Object {:?} of class {:?} ({:?}) was asked to set nil for {}, \
        which is not an object-typed property ({:?})",
        this, class_name_string, class, key_string, key);
}

- (())setValue:(id)_value
forUndefinedKey:(id)key { // NSString*
    // Foundation raises NSUnknownKeyException, which an app can catch and
    // routinely does: setting an unknown key is how nib loading tolerates an
    // outlet the class no longer declares. tapHLE cannot raise, and aborting
    // turned that survivable mismatch into a dead app. Log what was missing —
    // that is the useful half of the exception — and carry on.
    let class: Class = ObjC::read_isa(this, &env.mem);
    let class_name_string = env.objc.get_class_name(class).to_owned();
    let key_string = to_rust_string(env, key);
    log!(
        "Warning: {:?} of class {:?} has no setter or ivar for the key {:?}, which Foundation would raise NSUnknownKeyException for; ignoring it",
        this, class_name_string, key_string
    );
}

- (())willChangeValueForKey:(id)_key { // NSString *
    log_once!("TODO: NSObject willChangeValueForKey:");
}
- (())didChangeValueForKey:(id)_key { // NSString *
    log_once!("TODO: NSObject didChangeValueForKey:");
}

- (bool)respondsToSelector:(SEL)selector {
    env.objc.object_has_method(&env.mem, this, selector)
}

- (id)methodSignatureForSelector:(SEL)sel {
    let class = ObjC::read_isa(this, &env.mem);
    let Some(sig) = env.objc.class_get_method_signature(class, sel).copied() else {
        return nil;
    };
    log_dbg!("methodSignatureForSelector: '{}' -> {:?}", sel.as_str(&env.mem), env.mem.cstr_at_utf8(sig));
    msg_class![env; NSMethodSignature signatureWithObjCTypes:sig]
}

- (ConstVoidPtr)methodForSelector:(SEL)selector {
    match env.objc.object_get_method_implementation(&env.mem, this, selector) {
        // A guest IMP is already a guest-callable function pointer, including
        // its ARM/Thumb mode bit, so it can be returned directly.
        Some(IMP::Guest(imp)) => imp.to_ptr(),
        // Host methods are Rust functions rather than guest code. Returning a
        // usable IMP for one needs a guest-callable trampoline, which tapHLE
        // does not create yet. Returning null is safer than exposing a host
        // address that guest code could try to branch to.
        Some(IMP::Host(_)) => {
            log_once!("TODO: NSObject methodForSelector: for a host implementation");
            ConstVoidPtr::null()
        }
        None => ConstVoidPtr::null(),
    }
}

- (id)performSelector:(SEL)sel {
    assert!(!sel.is_null());
    msg_send_no_type_checking(env, (this, sel))
}

- (id)performSelector:(SEL)sel
           withObject:(id)o1 {
    assert!(!sel.is_null());
    msg_send_no_type_checking(env, (this, sel, o1))
}

- (id)performSelector:(SEL)sel
           withObject:(id)o1
           withObject:(id)o2 {
    assert!(!sel.is_null());
    msg_send_no_type_checking(env, (this, sel, o1, o2))
}

- (())performSelectorInBackground:(SEL)sel
                       withObject:(id)arg {
    detach_new_thread_inner(env, sel, this, arg, /* tolerate_type_mismatch: */ true)
}

- (())performSelector:(SEL)sel withObject:(id)arg afterDelay:(NSTimeInterval)delay {
    let run_loop: id = msg_class![env; NSRunLoop currentRunLoop];
    add_perform_request(env, run_loop, this, sel, arg, Some(delay), false);
}

- (())performSelector:(SEL)sel
              onThread:(id)thread // NSThread*
             withObject:(id)arg
          waitUntilDone:(bool)wait {
    assert!(!sel.is_null());
    let current_thread: id = msg_class![env; NSThread currentThread];
    if thread != current_thread {
        // tapHLE cannot yet map an arbitrary NSThread object back to its run
        // loop. Running the selector immediately preserves forward progress
        // and matches the existing inline dispatch compatibility
        // model, though it does not provide true cross-thread scheduling.
        log_once!("TODO: performSelector:onThread: is executing immediately instead of switching NSThread");
    }
    log_dbg!(
        "performSelector:{} onThread:{:?} withObject:{:?} waitUntilDone:{}",
        sel.as_str(&env.mem),
        thread,
        arg,
        wait,
    );
    if sel.as_str(&env.mem).ends_with(':') {
        () = msg_send(env, (this, sel, arg));
    } else {
        assert!(arg.is_null());
        () = msg_send(env, (this, sel));
    }
}

- (())performSelectorOnMainThread:(SEL)sel withObject:(id)arg waitUntilDone:(bool)wait {
    log_dbg!("performSelectorOnMainThread:{} withObject:{:?} waitUntilDone:{}", sel.as_str(&env.mem), arg, wait);
    if wait && env.current_thread == 0 {
        if sel.as_str(&env.mem).ends_with(':') {
            () = msg_send(env, (this, sel, arg));
        } else {
            assert!(arg.is_null());
            () = msg_send(env, (this, sel));
        }
        return;
    }
    if env.bundle.bundle_identifier().starts_with("com.gameloft.POP") && (sel == env.objc.lookup_selector("startMovie:").unwrap() || sel == env.objc.lookup_selector("stopMovie").unwrap()) && wait {
        log!("Applying game-specific hack for PoP: WW: ignoring performSelectorOnMainThread:SEL({}) waitUntilDone:true", sel.as_str(&env.mem));
        return;
    }
    if env.bundle.bundle_identifier().starts_with("com.gameloft.Asphalt5") && (sel == env.objc.lookup_selector("startMovie:").unwrap() || sel == env.objc.lookup_selector("stopMovie:").unwrap()) && wait {
        log!("Applying game-specific hack for Asphalt5: ignoring performSelectorOnMainThread:SEL({}) waitUntilDone:true", sel.as_str(&env.mem));
        return;
    }
    if env.bundle.bundle_identifier().starts_with("com.gameloft.SplinterCell") && sel == env.objc.lookup_selector("startMovie:").unwrap() && wait {
        log!("Applying game-specific hack for SplinterCell: ignoring performSelectorOnMainThread:SEL({}) waitUntilDone:true", sel.as_str(&env.mem));
        return;
    }
    if env.bundle.bundle_identifier().starts_with("com.gameloft.AssassinsCreed") && sel == env.objc.lookup_selector("moviePlayerInit:").unwrap() && wait {
        log!("Applying game-specific hack for AssassinsCreed: ignoring performSelectorOnMainThread:SEL(moviePlayerInit:) waitUntilDone:true");
        return;
    }
    if env.bundle.bundle_identifier().starts_with("com.gameloft.Ferrari") && wait {
        if sel == env.objc.lookup_selector("startMovie:").unwrap() {
            log!("Applying game-specific hack for Ferrari GT: ignoring performSelectorOnMainThread:SEL({}) waitUntilDone:true", sel.as_str(&env.mem));
            return;
        }
        if sel == env.objc.lookup_selector("initTextInput:").unwrap() || sel == env.objc.lookup_selector("removeTextField:").unwrap() {
            log!("Applying game-specific hack for Ferrari GT: performing performSelectorOnMainThread:SEL({}) waitUntilDone:true on thread {}", sel.as_str(&env.mem), env.current_thread);
            () = msg_send(env, (this, sel, arg));
            return;
        }
    }
    if env.bundle.bundle_identifier().starts_with("com.gameloft.HOS2") && wait {
        if sel == env.objc.lookup_selector("loadMovie:").unwrap() || sel == env.objc.lookup_selector("sendGameInfo").unwrap() || sel == env.objc.lookup_selector("setStatusBar:").unwrap() {
            log!("Applying game-specific hack for HOS2: performing performSelectorOnMainThread:SEL({}) waitUntilDone:true on thread {}", sel.as_str(&env.mem), env.current_thread);
            if sel.as_str(&env.mem).ends_with(':') {
                () = msg_send(env, (this, sel, arg));
            } else {
                assert!(arg.is_null());
                () = msg_send(env, (this, sel));
            }
            return;
        }
        if sel == env.objc.lookup_selector("startMovie:").unwrap() || sel == env.objc.lookup_selector("stopMovie:").unwrap() {
            log!("Applying game-specific hack for HOS2: ignoring performSelectorOnMainThread:SEL({}) waitUntilDone:true", sel.as_str(&env.mem));
            return;
        }
    }

    let run_loop: id = msg_class![env; NSRunLoop mainRunLoop];
    let sem = add_perform_request(env, run_loop, this, sel, arg, None, wait);
    if wait {
        sem_wait(env, sem);
        host_destroy_semaphore(env, sem);
    }
}

// UINibLoadingAdditions protocol
- (())awakeFromNib {
    // no-op
}

@end

};

/// Key-Value Coding helper: if `setter` takes a plain scalar argument, unwrap
/// the boxed scalar `value` (an `NSNumber`/`NSValue`) to that type and invoke
/// the setter, returning `true`. Returns `false` when the setter takes an
/// object (or a type we do not unwrap yet), so the caller passes the boxed
/// value through unchanged.
///
/// The argument type comes from the setter's Objective-C method type encoding,
/// which is authoritative for the wire type: e.g. `float` and `double` occupy a
/// different number of argument registers, so choosing the wrong one would
/// place the value incorrectly even when the `NSNumber`'s own `objCType`
/// differs.
fn kvc_set_unwrapped_scalar(env: &mut Environment, this: id, setter: SEL, value: id) -> bool {
    let Some(arg_type) = kvc_setter_arg_type(env, this, setter) else {
        return false;
    };

    // See `impl GuestArg` in abi.rs: scalar arguments (including `f32`/`f64`)
    // are passed in the core argument registers, so unwrapping to the matching
    // Rust type places the value the same way the guest compiler emitted the
    // call.
    match arg_type {
        b'f' => {
            let v: f32 = msg![env; value floatValue];
            () = msg_send(env, (this, setter, v));
        }
        b'd' => {
            let v: f64 = msg![env; value doubleValue];
            () = msg_send(env, (this, setter, v));
        }
        // `c`/`B` cover BOOL/char/_Bool; a boxed 0/1 is the common case.
        b'c' | b'B' => {
            let v: bool = msg![env; value boolValue];
            () = msg_send(env, (this, setter, v));
        }
        b'i' | b'l' | b's' => {
            let v: i32 = msg![env; value intValue];
            () = msg_send(env, (this, setter, v));
        }
        b'I' | b'L' | b'S' => {
            let v: u32 = msg![env; value unsignedIntValue];
            () = msg_send(env, (this, setter, v));
        }
        b'q' | b'Q' => {
            let v: i64 = msg![env; value longLongValue];
            () = msg_send(env, (this, setter, v));
        }
        // '@' (object), '{' (struct), '^' (pointer), etc.: not a scalar we
        // unwrap; let the caller pass the boxed value through.
        _ => return false,
    }
    true
}

/// Key-Value Coding helper: the Objective-C type encoding of a unary setter's
/// value argument, or `None` when the method records no signature (which means
/// a host-implemented method: see `class_get_method_signature`).
fn kvc_setter_arg_type(env: &Environment, this: id, setter: SEL) -> Option<u8> {
    let class = ObjC::read_isa(this, &env.mem);
    let signature = *env.objc.class_get_method_signature(class, setter)?;
    let signature = env.mem.cstr_at_utf8(signature).ok()?;
    // A method type encoding is <return><self@><cmd:><arg…> with a numeric byte
    // offset after each type. Dropping the digits leaves the ordered type
    // tokens; a unary setter's value argument is the fourth token (return,
    // self, _cmd, value).
    let tokens: Vec<u8> = signature.bytes().filter(|b| !b.is_ascii_digit()).collect();
    let mut i = 3;
    // Skip any type qualifiers (const, in, out, …) that may precede the type.
    while tokens
        .get(i)
        .is_some_and(|b| matches!(b, b'r' | b'n' | b'N' | b'o' | b'O' | b'R' | b'V'))
    {
        i += 1;
    }
    tokens.get(i).copied()
}

/// Key-Value Coding helper: invoke `getter` and return its result as an object,
/// boxing it in an `NSNumber` when the accessor returns a plain scalar.
///
/// This is the inverse of [kvc_set_unwrapped_scalar] and reads the return type
/// from the same authoritative source, the method's Objective-C type encoding.
/// A missing or unreadable encoding means a host-implemented method, which
/// returns an object.
fn kvc_get_boxed_value(env: &mut Environment, this: id, getter: SEL) -> id {
    let class = ObjC::read_isa(this, &env.mem);
    let return_type = env
        .objc
        .class_get_method_signature(class, getter)
        .copied()
        .and_then(|signature| env.mem.cstr_at_utf8(signature).ok())
        .and_then(|signature| {
            // A method type encoding is <return><self@><cmd:><arg…> with a
            // numeric byte offset after each type. Dropping the digits leaves
            // the ordered type tokens, the first of which is the return type.
            let tokens: Vec<u8> = signature.bytes().filter(|b| !b.is_ascii_digit()).collect();
            let mut i = 0;
            // Skip any type qualifiers (const, in, out, …) that may precede it.
            while tokens
                .get(i)
                .is_some_and(|b| matches!(b, b'r' | b'n' | b'N' | b'o' | b'O' | b'R' | b'V'))
            {
                i += 1;
            }
            tokens.get(i).copied()
        })
        // Host methods have no recorded signature (see
        // `class_get_method_signature`), and a KVC accessor implemented in the
        // host is an object-returning Foundation getter.
        .unwrap_or(b'@');

    // See `impl GuestRet` in abi.rs: scalar results (including `f32`/`f64`)
    // come back in the core result registers, so reading the matching Rust type
    // takes the value from where the guest compiler put it.
    match return_type {
        b'@' | b'#' => msg_send(env, (this, getter)),
        b'f' => {
            let v: f32 = msg_send(env, (this, getter));
            msg_class![env; NSNumber numberWithFloat:v]
        }
        b'd' => {
            let v: f64 = msg_send(env, (this, getter));
            msg_class![env; NSNumber numberWithDouble:v]
        }
        // `c`/`B` cover BOOL/char/_Bool. A signed char that is not a BOOL is
        // indistinguishable here; Apple's KVC boxes it the same way.
        b'c' | b'B' => {
            let v: bool = msg_send(env, (this, getter));
            msg_class![env; NSNumber numberWithBool:v]
        }
        b'i' | b'l' | b's' => {
            let v: i32 = msg_send(env, (this, getter));
            msg_class![env; NSNumber numberWithInt:v]
        }
        b'I' | b'L' | b'S' => {
            let v: u32 = msg_send(env, (this, getter));
            msg_class![env; NSNumber numberWithUnsignedInt:v]
        }
        b'q' | b'Q' => {
            let v: i64 = msg_send(env, (this, getter));
            msg_class![env; NSNumber numberWithLongLong:v]
        }
        b'v' => nil,
        // '{' (struct) would need NSValue boxing through the struct-return
        // calling convention, and '^' (pointer) needs valueWithPointer:.
        // Neither is reached yet, and guessing would corrupt the result.
        _ => unimplemented!(
            "Key-value coding getter {:?} on class {:?} returns unsupported type {:?}",
            getter.as_str(&env.mem),
            env.objc.get_class_name(class),
            return_type as char,
        ),
    }
}

/// Keys of the change dictionary passed to a key-value observer.
///
/// tapHLE accepts KVO registration but never delivers a change, so nothing here
/// builds such a dictionary. The symbols still have to exist: an app that
/// observes anything references them to read the change out, and an unbound one
/// is a null pointer it dereferences to do so.
const NSKeyValueChangeKindKey: &str = "NSKeyValueChangeKindKey";
const NSKeyValueChangeNewKey: &str = "NSKeyValueChangeNewKey";
const NSKeyValueChangeOldKey: &str = "NSKeyValueChangeOldKey";
const NSKeyValueChangeIndexesKey: &str = "NSKeyValueChangeIndexesKey";
const NSKeyValueChangeNotificationIsPriorKey: &str = "NSKeyValueChangeNotificationIsPriorKey";

/// `NSAllocateObject(Class, extraBytes, zone)` — Foundation's own allocation
/// entry point, which class methods call instead of `+alloc` when they were
/// compiled against the C API. The extra bytes are for trailing storage a class
/// declares beyond its ivars; nothing in tapHLE uses them, and a class that
/// wanted them would need host-side support anyway, so a non-zero request is
/// reported rather than silently under-allocated.
fn NSAllocateObject(
    env: &mut Environment,
    class: Class,
    extra_bytes: NSUInteger,
    zone: MutVoidPtr,
) -> id {
    if extra_bytes != 0 {
        log!(
            "TODO: NSAllocateObject() asked for {} extra bytes, which are not allocated",
            extra_bytes
        );
    }
    msg![env; class allocWithZone:zone]
}

fn NSDeallocateObject(env: &mut Environment, object: id) {
    () = msg![env; object dealloc];
}

/// `NSDefaultMallocZone` — the zone Foundation allocates from by default.
///
/// Zones were a way to place related allocations near each other, and have been
/// vestigial since well before this era: on iPhone OS every zone is the one
/// malloc heap. tapHLE has no zones at all, and its `allocWithZone:` ignores
/// the argument, so a zone here is a token an app passes back rather than
/// something it can act on.
///
/// The token is null, which is what Foundation itself accepts everywhere a zone
/// is taken and what `+alloc` already passes. Handing back a fabricated
/// non-null pointer would invite an app to dereference it.
fn NSDefaultMallocZone(_env: &mut Environment) -> MutVoidPtr {
    Ptr::null()
}

fn NSCreateZone(
    _env: &mut Environment,
    _start_size: NSUInteger,
    _granularity: NSUInteger,
    _can_free: bool,
) -> MutVoidPtr {
    // One heap, so a "new" zone is the same zone.
    Ptr::null()
}

fn NSRecycleZone(_env: &mut Environment, _zone: MutVoidPtr) {}

fn NSZoneFromPointer(_env: &mut Environment, _ptr: MutVoidPtr) -> MutVoidPtr {
    Ptr::null()
}

fn NSZoneMalloc(env: &mut Environment, _zone: MutVoidPtr, size: NSUInteger) -> MutVoidPtr {
    env.mem.alloc(size.max(1))
}

fn NSZoneCalloc(
    env: &mut Environment,
    _zone: MutVoidPtr,
    count: NSUInteger,
    size: NSUInteger,
) -> MutVoidPtr {
    env.mem.calloc(count.saturating_mul(size).max(1))
}

fn NSZoneRealloc(
    env: &mut Environment,
    _zone: MutVoidPtr,
    ptr: MutVoidPtr,
    size: NSUInteger,
) -> MutVoidPtr {
    if ptr.is_null() {
        return env.mem.alloc(size.max(1));
    }
    env.mem.realloc(ptr, size.max(1))
}

fn NSZoneFree(env: &mut Environment, _zone: MutVoidPtr, ptr: MutVoidPtr) {
    if !ptr.is_null() {
        env.mem.free(ptr);
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(NSDefaultMallocZone()),
    export_c_func!(NSCreateZone(_, _, _)),
    export_c_func!(NSRecycleZone(_)),
    export_c_func!(NSZoneFromPointer(_)),
    export_c_func!(NSZoneMalloc(_, _)),
    export_c_func!(NSZoneCalloc(_, _, _)),
    export_c_func!(NSZoneRealloc(_, _, _)),
    export_c_func!(NSZoneFree(_, _)),
    export_c_func!(NSAllocateObject(_, _, _)),
    export_c_func!(NSDeallocateObject(_)),
];

pub const CONSTANTS: ConstantExports = &[
    (
        "_NSKeyValueChangeKindKey",
        HostConstant::NSString(NSKeyValueChangeKindKey),
    ),
    (
        "_NSKeyValueChangeNewKey",
        HostConstant::NSString(NSKeyValueChangeNewKey),
    ),
    (
        "_NSKeyValueChangeOldKey",
        HostConstant::NSString(NSKeyValueChangeOldKey),
    ),
    (
        "_NSKeyValueChangeIndexesKey",
        HostConstant::NSString(NSKeyValueChangeIndexesKey),
    ),
    (
        "_NSKeyValueChangeNotificationIsPriorKey",
        HostConstant::NSString(NSKeyValueChangeNotificationIsPriorKey),
    ),
];
