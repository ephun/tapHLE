/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::dyld::{export_c_func, FunctionExports};
use crate::libc::errno::{set_errno, EFAULT, EINVAL, ENOMEM};
use crate::mem::{ConstPtr, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr, Ptr, SafeRead};
use crate::{Environment, ThreadId};
use std::collections::HashMap;

// Darwin's 32-bit stack_t layout and flags come from sys/signal.h:
// https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/signal.h
const SS_ONSTACK: i32 = 0x0001;
const SS_DISABLE: i32 = 0x0004;
// Darwin advertises a larger MINSIGSTKSZ, but its sigaltstack syscall retains
// this older minimum for binary compatibility:
// https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_sig.c
const ENFORCED_MINSIGSTKSZ: GuestUSize = 8 * 1024;

// Signal numbers, also from sys/signal.h. Darwin inherits the BSD numbering,
// which differs from Linux's above SIGCHLD, so these are not interchangeable
// with the host's.
const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGQUIT: i32 = 3;
const SIGILL: i32 = 4;
const SIGTRAP: i32 = 5;
const SIGABRT: i32 = 6;
const SIGEMT: i32 = 7;
const SIGFPE: i32 = 8;
const SIGKILL: i32 = 9;
const SIGBUS: i32 = 10;
const SIGSEGV: i32 = 11;
const SIGSYS: i32 = 12;
const SIGPIPE: i32 = 13;
const SIGALRM: i32 = 14;
const SIGTERM: i32 = 15;
const SIGURG: i32 = 16;
const SIGSTOP: i32 = 17;
const SIGTSTP: i32 = 18;
const SIGCONT: i32 = 19;
const SIGCHLD: i32 = 20;
const SIGTTIN: i32 = 21;
const SIGTTOU: i32 = 22;
const SIGIO: i32 = 23;
const SIGXCPU: i32 = 24;
const SIGXFSZ: i32 = 25;
const SIGVTALRM: i32 = 26;
const SIGPROF: i32 = 27;
const SIGWINCH: i32 = 28;
const SIGINFO: i32 = 29;
const SIGUSR1: i32 = 30;
const SIGUSR2: i32 = 31;
/// `NSIG` is 32 and is one past the last valid signal, so this is the last one.
const NSIG_MINUS_ONE: i32 = 31;

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct stack_t {
    ss_sp: MutVoidPtr,
    ss_size: GuestUSize,
    ss_flags: i32,
}
unsafe impl SafeRead for stack_t {}

#[derive(Default)]
pub struct State {
    /// Alternate signal stacks are configured independently for each guest
    /// thread. Signal delivery itself is not implemented yet, so no entry can
    /// currently acquire [SS_ONSTACK].
    alternate_stacks: HashMap<ThreadId, stack_t>,
}

impl State {
    fn disabled_stack() -> stack_t {
        stack_t {
            ss_sp: Ptr::null(),
            ss_size: 0,
            ss_flags: SS_DISABLE,
        }
    }

    fn stack_for_thread(&self, thread: ThreadId) -> stack_t {
        self.alternate_stacks
            .get(&thread)
            .copied()
            .unwrap_or_else(Self::disabled_stack)
    }

    fn update_stack_for_thread(&mut self, thread: ThreadId, requested: stack_t) {
        let stored = if requested.ss_flags == SS_DISABLE {
            let previous = self.stack_for_thread(thread);
            stack_t {
                ss_sp: previous.ss_sp,
                ss_size: previous.ss_size,
                ss_flags: SS_DISABLE,
            }
        } else {
            requested
        };
        self.alternate_stacks.insert(thread, stored);
    }
}

fn validate_alternate_stack(stack: stack_t) -> Result<(), i32> {
    if stack.ss_flags & SS_ONSTACK != 0 || stack.ss_flags & !SS_DISABLE != 0 {
        return Err(EINVAL);
    }
    if stack.ss_flags == 0 && stack.ss_size < ENFORCED_MINSIGSTKSZ {
        return Err(ENOMEM);
    }
    Ok(())
}

fn sigaction(env: &mut Environment, signum: i32, act: ConstVoidPtr, old_act: MutVoidPtr) -> i32 {
    // TODO: handle errno properly
    set_errno(env, 0);

    log!("TODO: sigaction({:?}, {:?}, {:?})", signum, act, old_act);
    0
}

fn signal(env: &mut Environment, signum: i32, handler: MutVoidPtr) -> MutVoidPtr {
    // TODO: handle errno properly
    set_errno(env, 0);

    log!("TODO: signal({:?}, {:?})", signum, handler);
    Ptr::null()
}

fn sigprocmask(env: &mut Environment, how: i32, set: ConstVoidPtr, old_set: MutVoidPtr) -> i32 {
    // TODO: handle errno properly
    set_errno(env, 0);

    log!("TODO: sigprocmask({}, {:?}, {:?})", how, set, old_set);
    0
}

/// What Darwin does to a process that receives a signal with no handler
/// installed. From the table in `sys/signal.h`.
#[derive(Debug, PartialEq, Eq)]
enum DefaultAction {
    /// Terminate the process, with or without a core dump — the difference is
    /// invisible here.
    Terminate,
    /// Suspend the process until it is continued.
    Stop,
    /// Throw the signal away.
    Discard,
}

fn default_action(signal: i32) -> Option<DefaultAction> {
    Some(match signal {
        SIGURG | SIGCONT | SIGCHLD | SIGIO | SIGWINCH | SIGINFO => DefaultAction::Discard,
        SIGSTOP | SIGTSTP | SIGTTIN | SIGTTOU => DefaultAction::Stop,
        // Every other signal in the 1..NSIG range terminates. Listing the two
        // small sets and defaulting the rest is how the header reads, and it
        // means a signal number nobody thought about here still gets the
        // conservative answer rather than being silently discarded.
        1..=NSIG_MINUS_ONE => DefaultAction::Terminate,
        _ => return None,
    })
}

fn signal_name(signal: i32) -> &'static str {
    match signal {
        SIGHUP => "SIGHUP",
        SIGINT => "SIGINT",
        SIGQUIT => "SIGQUIT",
        SIGILL => "SIGILL",
        SIGTRAP => "SIGTRAP",
        SIGABRT => "SIGABRT",
        SIGEMT => "SIGEMT",
        SIGFPE => "SIGFPE",
        SIGKILL => "SIGKILL",
        SIGBUS => "SIGBUS",
        SIGSEGV => "SIGSEGV",
        SIGSYS => "SIGSYS",
        SIGPIPE => "SIGPIPE",
        SIGALRM => "SIGALRM",
        SIGTERM => "SIGTERM",
        SIGURG => "SIGURG",
        SIGSTOP => "SIGSTOP",
        SIGTSTP => "SIGTSTP",
        SIGCONT => "SIGCONT",
        SIGCHLD => "SIGCHLD",
        SIGTTIN => "SIGTTIN",
        SIGTTOU => "SIGTTOU",
        SIGIO => "SIGIO",
        SIGXCPU => "SIGXCPU",
        SIGXFSZ => "SIGXFSZ",
        SIGVTALRM => "SIGVTALRM",
        SIGPROF => "SIGPROF",
        SIGWINCH => "SIGWINCH",
        SIGINFO => "SIGINFO",
        SIGUSR1 => "SIGUSR1",
        SIGUSR2 => "SIGUSR2",
        _ => "an unknown signal",
    }
}

/// Send a signal to the calling process, which here is the guest app.
///
/// The default disposition is applied, because that is what tapHLE's signal
/// state can honestly report: [signal] and [sigaction] do not record the
/// handler they are given, so there is never one to run. On a device an app
/// that installs a handler and raises would reach it — a crash reporter does
/// exactly this — and reaching the terminate branch below instead is that gap
/// showing through, not a decision made here.
///
/// The common case by far is `abort()`, which is `raise(SIGABRT)` after
/// unblocking it, so the message says the same thing `abort` says.
fn raise(env: &mut Environment, signal: i32) -> i32 {
    match default_action(signal) {
        None => {
            set_errno(env, EINVAL);
            -1
        }
        Some(DefaultAction::Discard) => {
            log_dbg!(
                "raise({}) discarded: its default action is to ignore it",
                signal_name(signal)
            );
            0
        }
        Some(DefaultAction::Stop) => panic!(
            "The app raised {}, which would suspend it until something continued it. \
             tapHLE has nothing that would, so the app cannot make progress from here.",
            signal_name(signal)
        ),
        Some(DefaultAction::Terminate) => panic!(
            "The app raised {}, whose default action is to terminate the process, and no \
             handler is installed. This is the app deliberately terminating itself, not a \
             tapHLE failure - the reason is usually logged just above.",
            signal_name(signal)
        ),
    }
}

fn sigaltstack(env: &mut Environment, stack: ConstPtr<stack_t>, old_stack: MutPtr<stack_t>) -> i32 {
    let thread = env.current_thread;
    let previous = env.libc_state.signal.stack_for_thread(thread);
    if !old_stack.is_null() && !env.mem.write_fallible(old_stack, previous) {
        set_errno(env, EFAULT);
        return -1;
    }

    if stack.is_null() {
        return 0;
    }

    let Some(requested) = env.mem.read_fallible(stack) else {
        set_errno(env, EFAULT);
        return -1;
    };
    if let Err(errno) = validate_alternate_stack(requested) {
        set_errno(env, errno);
        return -1;
    }

    env.libc_state
        .signal
        .update_stack_for_thread(thread, requested);
    0
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(sigaction(_, _, _)),
    export_c_func!(signal(_, _)),
    export_c_func!(sigprocmask(_, _, _)),
    export_c_func!(sigaltstack(_, _)),
    export_c_func!(raise(_)),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_stack(size: GuestUSize) -> stack_t {
        stack_t {
            ss_sp: Ptr::from_bits(0x1000),
            ss_size: size,
            ss_flags: 0,
        }
    }

    #[test]
    fn default_signal_actions_follow_the_darwin_table() {
        // The two small non-terminating sets, spot-checked at their edges.
        assert_eq!(default_action(SIGCHLD), Some(DefaultAction::Discard));
        assert_eq!(default_action(SIGINFO), Some(DefaultAction::Discard));
        assert_eq!(default_action(SIGSTOP), Some(DefaultAction::Stop));
        assert_eq!(default_action(SIGTTOU), Some(DefaultAction::Stop));
        // Everything else in range terminates, including the ends of the range.
        assert_eq!(default_action(SIGHUP), Some(DefaultAction::Terminate));
        assert_eq!(default_action(SIGABRT), Some(DefaultAction::Terminate));
        assert_eq!(default_action(SIGUSR2), Some(DefaultAction::Terminate));
        // Out of range is an error, not a discard.
        assert_eq!(default_action(0), None);
        assert_eq!(default_action(32), None);
        assert_eq!(default_action(-1), None);
    }

    #[test]
    fn every_signal_in_range_has_a_name() {
        for signal in 1..=NSIG_MINUS_ONE {
            assert_ne!(
                signal_name(signal),
                "an unknown signal",
                "signal {signal} is unnamed"
            );
        }
    }

    #[test]
    fn alternate_signal_stack_uses_the_32_bit_darwin_layout() {
        assert_eq!(std::mem::size_of::<stack_t>(), 12);
    }

    #[test]
    fn alternate_signal_stack_validation_matches_darwin_flags_and_minimum() {
        assert_eq!(
            validate_alternate_stack(enabled_stack(ENFORCED_MINSIGSTKSZ)),
            Ok(())
        );
        assert_eq!(
            validate_alternate_stack(enabled_stack(ENFORCED_MINSIGSTKSZ - 1)),
            Err(ENOMEM)
        );

        let mut disabled = enabled_stack(0);
        disabled.ss_flags = SS_DISABLE;
        assert_eq!(validate_alternate_stack(disabled), Ok(()));

        let mut on_stack = enabled_stack(ENFORCED_MINSIGSTKSZ);
        on_stack.ss_flags = SS_ONSTACK;
        assert_eq!(validate_alternate_stack(on_stack), Err(EINVAL));
    }

    #[test]
    fn alternate_signal_stacks_are_disabled_by_default_and_per_thread() {
        let mut state = State::default();
        let initial = state.stack_for_thread(3);
        let initial_size = initial.ss_size;
        let initial_flags = initial.ss_flags;
        assert!(initial.ss_sp.is_null());
        assert_eq!(initial_size, 0);
        assert_eq!(initial_flags, SS_DISABLE);

        state.update_stack_for_thread(3, enabled_stack(ENFORCED_MINSIGSTKSZ));
        let thread_3_flags = state.stack_for_thread(3).ss_flags;
        let thread_4_flags = state.stack_for_thread(4).ss_flags;
        assert_eq!(thread_3_flags, 0);
        assert_eq!(thread_4_flags, SS_DISABLE);
    }

    #[test]
    fn disabling_preserves_the_previous_stack_address_and_size() {
        let mut state = State::default();
        let enabled = enabled_stack(ENFORCED_MINSIGSTKSZ);
        let enabled_sp = enabled.ss_sp;
        let enabled_size = enabled.ss_size;
        state.update_stack_for_thread(7, enabled);

        let requested_disable = stack_t {
            ss_sp: Ptr::from_bits(0xdeadbeef),
            ss_size: 1,
            ss_flags: SS_DISABLE,
        };
        state.update_stack_for_thread(7, requested_disable);

        let disabled = state.stack_for_thread(7);
        let disabled_sp = disabled.ss_sp;
        let disabled_size = disabled.ss_size;
        let disabled_flags = disabled.ss_flags;
        assert_eq!(disabled_sp, enabled_sp);
        assert_eq!(disabled_size, enabled_size);
        assert_eq!(disabled_flags, SS_DISABLE);
    }
}
