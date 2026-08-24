/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSNotificationCenter`.

use super::ns_notification::NSNotificationName;
use super::ns_string;

use crate::abi::CallFromHost;
use crate::objc::{
    block_invoke_function, id, msg, msg_class, msg_send, nil, objc_classes, release, retain,
    ClassExports, HostObject, NSZonePtr, SEL,
};
use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    default_center: Option<id>,
}

#[derive(Clone)]
struct Observer {
    observer: id,
    /// The message to send, or `None` when this registration is a block. In
    /// that case `observer` is the block itself, which is both what gets
    /// invoked and the token `-removeObserver:` is given back.
    selector: Option<SEL>,
    object: id,
    /// Identity of this individual registration, unique within its centre.
    ///
    /// Posting a notification has to copy the observer list, because delivering
    /// it runs guest code that may register or remove observers. The centre
    /// does not retain observers, so a copied entry can outlive the object it
    /// names. This lets the poster ask whether a copied entry is still
    /// registered before it messages the observer.
    registration: u64,
}

struct NSNotificationCenterHostObject {
    observers: HashMap<Option<Cow<'static, str>>, Vec<Observer>>,
    next_registration: u64,
}
impl HostObject for NSNotificationCenterHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSNotificationCenter: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSNotificationCenterHostObject {
        observers: HashMap::new(),
        next_registration: 0,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)defaultCenter {
    if let Some(c) = env.framework_state.foundation.ns_notification_center.default_center {
        c
    } else {
        let new: id = msg![env; this new];
        env.framework_state.foundation.ns_notification_center.default_center = Some(new);
        new
    }
}

- (())dealloc {
    let host_obj = env.objc.borrow_mut::<NSNotificationCenterHostObject>(this);
    let observers = std::mem::take(&mut host_obj.observers);
    for observer in observers.values().flatten() {
        release(env, observer.object);
    }
    env.objc.dealloc_object(this, &mut env.mem);
}

- (())addObserver:(id)observer
         selector:(SEL)selector
             name:(NSNotificationName)name
           object:(id)object {
    let name = if name != nil {
        // Usually a static string, so no real copy will happen
        Some(ns_string::to_rust_string(env, name))
    } else {
        None
    };

    log_dbg!(
        "[(NSNotificationCenter*){:?} addObserver:{:?} selector:{:?} name:{:?} object:{:?}",
        this,
        observer,
        selector,
        name,
        object,
    );

    // When adding an observer, only the object is retained so it doesn't get
    // deallocated before the notification is delivered. Some apps, such as
    // Dungeon Hunter 2, rely on this being the case.
    // The observer is not retained to avoid retain cycles.
    // https://stackoverflow.com/a/36582937
    // While not explicitly stated by the documentation, there's a paragraph
    // that hints at this behavior:
    // "If your app targets iOS 9.0 and later or macOS 10.11 and later, you do
    // not need to unregister an observer that you created with this function.
    // If you forget or are unable to remove an observer, the system cleans up
    // the next time it would have posted to it."
    // https://developer.apple.com/documentation/foundation/notificationcenter/addobserver(_:selector:name:object:)?language=objc
    // Implying that prior to these versions, it's unsafe to not remove an
    // observer. It's been observed that some apps expect and rely on this
    // behavior, such as Marmalade SDK games that use the Movie Player
    // (Pandemonium and COD Zombies, for example).

    retain(env, object);

    let host_obj = env.objc.borrow_mut::<NSNotificationCenterHostObject>(this);
    let registration = host_obj.next_registration;
    host_obj.next_registration += 1;
    host_obj.observers.entry(name).or_default().push(Observer {
        observer,
        selector: Some(selector),
        object,
        registration,
    });
}

// The block-based registration. Unlike the selector form there is no observer
// object, so the returned token *is* the registration's identity — the caller
// keeps it solely to hand back to -removeObserver:.
//
// The block is copied, because a stack block stops being valid as soon as the
// registering function returns, and this one is invoked long afterwards.
//
// The queue argument is ignored: tapHLE delivers notifications synchronously on
// the posting thread, and it has no separate main-queue scheduling to hop onto.
- (id)addObserverForName:(NSNotificationName)name
                  object:(id)object
                   queue:(id)_queue
              usingBlock:(id)block {
    let name = if name != nil {
        Some(ns_string::to_rust_string(env, name))
    } else {
        None
    };

    let block: id = msg![env; block copy];
    retain(env, object);

    let host_obj = env.objc.borrow_mut::<NSNotificationCenterHostObject>(this);
    let registration = host_obj.next_registration;
    host_obj.next_registration += 1;
    host_obj.observers.entry(name).or_default().push(Observer {
        observer: block,
        selector: None,
        object,
        registration,
    });
    block
}

- (())removeObserver:(id)observer {
    msg![env; this removeObserver:observer name:nil object:nil]
}

- (())removeObserver:(id)observer
                name:(NSNotificationName)name
              object:(id)object {
    assert!(observer != nil); // TODO

    let name = if name == nil {
        None
    } else {
        // Usually a static string, so no real copy will happen
        Some(ns_string::to_rust_string(env, name))
    };

    log_dbg!(
        "[(NSNotificationCenter*){:?} removeObserver:{:?} name:{:?} object:{:?}",
        this,
        observer,
        name,
        object,
    );

    // TODO: is this the correct behaviour, can an observer be registered
    // several times?
    let mut removed_observers = Vec::new();

    let host_obj = env.objc.borrow_mut::<NSNotificationCenterHostObject>(this);
    if name.is_some() {
        let Some(observers) = host_obj.observers.get_mut(&name) else {
            return;
        };
        remove_observers_internal(observers, &mut removed_observers, observer, object);
    } else {
        for observers in host_obj.observers.values_mut() {
            remove_observers_internal(observers, &mut removed_observers, observer, object);
        }
    };

    for removed_observer in removed_observers {
        release(env, removed_observer.object);
    }
}

- (())postNotification:(id)notification {
    log_dbg!(
        "[(NSNotificationCenter*){:?} postNotification:{:?}]",
        this,
        notification,
    );

    let name: id = msg![env; notification name];
    // Usually a static string, so no real copy will happen
    let name = ns_string::to_rust_string(env, name);

    let notification_poster: id = msg![env; notification object];

    log_dbg!("Notification is a {:?} posted by {:?}", name, notification_poster);

    // Delivering a notification runs guest code that may register or remove
    // observers, so the lists have to be copied before anything is messaged.
    // Each copied entry remembers which list it came from: a registration never
    // moves between them, so that is where its identity is looked up again
    // below.
    let named_key = Some(name);
    let nameless_key = None;
    let host_obj = env.objc.borrow::<NSNotificationCenterHostObject>(this);
    let mut observers: Vec<(bool, Observer)> = host_obj
        .observers
        .get(&named_key)
        .map(|observers| observers.iter().map(|o| (true, o.clone())).collect())
        .unwrap_or_default();
    if let Some(nameless_observers) = host_obj.observers.get(&nameless_key) {
        observers.extend(nameless_observers.iter().map(|o| (false, o.clone())));
    }
    for (
        named,
        Observer {
            observer,
            selector,
            object,
            registration,
        },
    ) in observers
    {
        // The object argument is a filter for which notification sources the
        // observer is interested in.
        if object != nil && notification_poster != object {
            continue;
        }

        // An observer messaged earlier in this loop may have removed this one,
        // directly or by tearing down whatever owned it. Since the centre does
        // not retain observers, the copy taken above can name a deallocated
        // object, and messaging it would be a use-after-free. Real
        // NSNotificationCenter does not deliver a notification to an observer
        // that was removed while that notification was being posted, which is
        // what makes unregistering in a teardown method safe, so skip it.
        let key = if named { &named_key } else { &nameless_key };
        let host_obj = env.objc.borrow::<NSNotificationCenterHostObject>(this);
        let still_registered = host_obj
            .observers
            .get(key)
            .is_some_and(|observers| observers.iter().any(|o| o.registration == registration));
        if !still_registered {
            log_dbg!(
                "Observer {:?} was removed while {:?} was being posted, not delivering {:?}",
                observer,
                notification,
                selector.map(|selector| selector.as_str(&env.mem).to_string()),
            );
            continue;
        }

        // In some cases, observer could be removed during the
        // processing of the notification, effectively releasing it.
        // (This has been observed in a shipped app)
        // We need to retain it for correctness.
        retain(env, observer);
        match selector {
            Some(selector) => {
                log_dbg!(
                    "Notification {:?} observed, sending {:?} message to {:?}",
                    notification,
                    selector.as_str(&env.mem),
                    observer
                );
                // Signature should be `- (void)notification:(NSNotification
                // *)notif`.
                let _: () = msg_send(env, (observer, selector, notification));
            }
            None => {
                log_dbg!(
                    "Notification {:?} observed, invoking block {:?}",
                    notification,
                    observer
                );
                // `void (^)(NSNotification *)`, invoked as the block ABI
                // requires: the block itself is the first argument.
                let invoke = block_invoke_function(env, observer);
                let _: () = invoke.call_from_host(env, (observer, notification));
            }
        }
        release(env, observer);
    }
}
- (())postNotificationName:(NSNotificationName)name
                    object:(id)object {
    msg![env; this postNotificationName:name
                                 object:object
                               userInfo:nil]
}
- (())postNotificationName:(NSNotificationName)name
                    object:(id)object
                  userInfo:(id)user_info { // NSDictionary*
    let notification: id = msg_class![env; NSNotification alloc];
    let notification: id = msg![env; notification initWithName:name
                                                        object:object
                                                      userInfo:user_info];
    let _: () = msg![env; this postNotification:notification];
    release(env, notification);
}

@end

};

/// A helper function to populate `removed_observers` with observers
/// removed from `observers` based on `observer` and `object` criteria.
fn remove_observers_internal(
    observers: &mut Vec<Observer>,
    removed_observers: &mut Vec<Observer>,
    observer: id,
    object: id,
) {
    let mut i = 0;
    while i < observers.len() {
        if observers[i].observer == observer && (object == nil || object == observers[i].object) {
            removed_observers.push(observers.swap_remove(i));
        } else {
            i += 1;
        }
    }
}
