/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSOperation`, `NSInvocationOperation`, and `NSOperationQueue`.
//!
//! The queue currently executes work synchronously. This is less concurrent
//! than iPhone OS, but it preserves operation ordering and lifecycle semantics
//! while making the common operation-based loading pattern functional.

use super::{NSInteger, NSUInteger};
use crate::objc::{
    id, msg, msg_class, msg_send_no_type_checking, nil, objc_classes, release, retain,
    ClassExports, HostObject, NSZonePtr, SEL,
};

/// The `+mainQueue` singleton, which must be the same object every time an app
/// asks for it: apps compare against it to decide whether they are already on
/// the main queue.
#[derive(Default)]
pub struct State {
    /// `NSOperationQueue*`
    main_queue: Option<id>,
}

struct NSOperationHostObject {
    cancelled: bool,
    executing: bool,
    finished: bool,
    invocation: Option<OperationInvocation>,
}
impl HostObject for NSOperationHostObject {}

#[derive(Clone, Copy)]
enum OperationInvocation {
    TargetSelectorObject {
        target: id,
        selector: SEL,
        object: id,
        object_is_live: bool,
    },
    NSInvocation(id),
}

struct NSOperationQueueHostObject {
    max_concurrent_operation_count: NSInteger,
    suspended: bool,
    /// Operations added while suspended, retained until they run or are
    /// cancelled. `NSOperation*`.
    pending: Vec<id>,
}
impl HostObject for NSOperationQueueHostObject {}

fn alloc_operation(env: &mut crate::Environment, class: crate::objc::Class) -> id {
    let host_object = NSOperationHostObject {
        cancelled: false,
        executing: false,
        finished: false,
        invocation: None,
    };
    env.objc
        .alloc_object(class, Box::new(host_object), &mut env.mem)
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSOperation: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    alloc_operation(env, this)
}

- (())start {
    let cancelled = env.objc.borrow::<NSOperationHostObject>(this).cancelled;
    if cancelled {
        env.objc.borrow_mut::<NSOperationHostObject>(this).finished = true;
        return;
    }

    env.objc.borrow_mut::<NSOperationHostObject>(this).executing = true;
    () = msg![env; this main];
    let host = env.objc.borrow_mut::<NSOperationHostObject>(this);
    host.executing = false;
    host.finished = true;
}

- (())main {
    let invocation = env.objc.borrow::<NSOperationHostObject>(this).invocation;
    match invocation {
        Some(OperationInvocation::TargetSelectorObject {
            target,
            selector,
            object,
            ..
        }) => {
            () = msg_send_no_type_checking(env, (target, selector, object));
        }
        Some(OperationInvocation::NSInvocation(invocation)) => {
            () = msg![env; invocation invoke];
        }
        None => {}
    }
}

- (())cancel {
    env.objc.borrow_mut::<NSOperationHostObject>(this).cancelled = true;
}

- (bool)isCancelled {
    env.objc.borrow::<NSOperationHostObject>(this).cancelled
}

- (bool)isExecuting {
    env.objc.borrow::<NSOperationHostObject>(this).executing
}

- (bool)isFinished {
    env.objc.borrow::<NSOperationHostObject>(this).finished
}

- (bool)isConcurrent {
    false
}

- (bool)isReady {
    !env.objc.borrow::<NSOperationHostObject>(this).executing
}

- (())waitUntilFinished {
    // Operations are currently run synchronously, so there is nothing to wait for.
}

- (())dealloc {
    if let Some(invocation) = env.objc.borrow::<NSOperationHostObject>(this).invocation {
        match invocation {
            OperationInvocation::TargetSelectorObject {
                target,
                object,
                object_is_live,
                ..
            } => {
                release(env, target);
                if object_is_live {
                    release(env, object);
                }
            }
            OperationInvocation::NSInvocation(invocation) => release(env, invocation),
        }
    }
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

@implementation NSInvocationOperation: NSOperation

+ (id)allocWithZone:(NSZonePtr)_zone {
    alloc_operation(env, this)
}

- (id)initWithTarget:(id)target selector:(SEL)selector object:(id)object {
    retain(env, target);
    // Although this argument is declared as `id`, some early audio stacks use
    // it to carry a scalar buffer identifier. Forward that value unchanged,
    // but retain it only when it is a live Objective-C object.
    let object_is_live = env.objc.get_host_object(object).is_some();
    if object_is_live {
        retain(env, object);
    }
    env.objc.borrow_mut::<NSOperationHostObject>(this).invocation =
        Some(OperationInvocation::TargetSelectorObject {
            target,
            selector,
            object,
            object_is_live,
        });
    this
}

- (id)initWithInvocation:(id)invocation { // NSInvocation *
    retain(env, invocation);
    env.objc.borrow_mut::<NSOperationHostObject>(this).invocation =
        Some(OperationInvocation::NSInvocation(invocation));
    this
}

- (id)result {
    // Return values are not captured yet. Void invocation operations, which are
    // the common loading use case on early iPhone OS, are fully supported.
    nil
}

@end


@implementation NSOperationQueue: NSObject

// The queue bound to the main thread. tapHLE has no separate main-queue
// scheduling, so this is an ordinary queue that happens to be a singleton —
// operations added to it run the same way they do on any other. Apps reach for
// it constantly to hop work back to the main thread, which is why its absence
// stopped seven apps in a 1501-app survey before they reached their own code.
+ (id)mainQueue {
    if let Some(existing) = env.framework_state.foundation.ns_operation.main_queue {
        return existing;
    }
    let queue: id = msg![env; this new];
    env.framework_state.foundation.ns_operation.main_queue = Some(queue);
    queue
}

+ (id)currentQueue {
    msg_class![env; NSOperationQueue mainQueue]
}

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = NSOperationQueueHostObject {
        // NSOperationQueueDefaultMaxConcurrentOperationCount
        max_concurrent_operation_count: -1,
        suspended: false,
        pending: Vec::new(),
    };
    env.objc.alloc_object(this, Box::new(host_object), &mut env.mem)
}

- (())setMaxConcurrentOperationCount:(NSInteger)count {
    env.objc
        .borrow_mut::<NSOperationQueueHostObject>(this)
        .max_concurrent_operation_count = count;
}

- (NSInteger)maxConcurrentOperationCount {
    env.objc
        .borrow::<NSOperationQueueHostObject>(this)
        .max_concurrent_operation_count
}

- (())addOperation:(id)operation {
    retain(env, operation);
    if env.objc.borrow::<NSOperationQueueHostObject>(this).suspended {
        // A suspended queue accepts operations but starts none of them. Keep
        // the reference until it runs; -setSuspended:NO drains the list.
        env.objc
            .borrow_mut::<NSOperationQueueHostObject>(this)
            .pending
            .push(operation);
        return;
    }
    () = msg![env; operation start];
    release(env, operation);
}

// This queue runs operations synchronously in -addOperation:, so "suspended"
// is the only state in which one can be waiting.
- (bool)isSuspended {
    env.objc.borrow::<NSOperationQueueHostObject>(this).suspended
}

- (())setSuspended:(bool)suspended {
    let host_object = env.objc.borrow_mut::<NSOperationQueueHostObject>(this);
    let was_suspended = std::mem::replace(&mut host_object.suspended, suspended);
    if !was_suspended || suspended {
        return;
    }
    // Resuming: run everything that arrived while suspended, in order. Take
    // the list first, because starting an operation reenters guest code that
    // may add more.
    let pending = std::mem::take(
        &mut env.objc.borrow_mut::<NSOperationQueueHostObject>(this).pending,
    );
    for operation in pending {
        () = msg![env; operation start];
        release(env, operation);
    }
}

- (id)operations {
    // Completed synchronous operations leave the queue immediately, so only
    // ones held back by suspension are still here.
    let pending = env.objc.borrow::<NSOperationQueueHostObject>(this).pending.clone();
    let array: id = msg_class![env; NSMutableArray array];
    for operation in pending {
        () = msg![env; array addObject:operation];
    }
    array
}

- (NSUInteger)operationCount {
    env.objc.borrow::<NSOperationQueueHostObject>(this).pending.len() as NSUInteger
}

- (())cancelAllOperations {
    let pending = std::mem::take(
        &mut env.objc.borrow_mut::<NSOperationQueueHostObject>(this).pending,
    );
    for operation in pending {
        () = msg![env; operation cancel];
        release(env, operation);
    }
}

- (())waitUntilAllOperationsAreFinished {}

@end


};
