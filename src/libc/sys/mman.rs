/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::abi::DotDotDot;
use crate::dyld::FunctionExports;
use crate::environment::Environment;
use crate::export_c_func;
use crate::libc::errno::{set_errno, EINVAL, ENOMEM, ENOTSUP};
use crate::libc::posix_io;
use crate::libc::posix_io::{off_t, FileDescriptor, SEEK_SET};
use crate::mem::{ConstPtr, GuestUSize, MutVoidPtr, PAGE_SIZE_ALIGN_MASK};
use std::collections::HashMap;

#[allow(dead_code)]
const MAP_FILE: i32 = 0x0000;
const MAP_FIXED: i32 = 0x0010;
const MAP_ANON: i32 = 0x1000;

const PROT_NONE: i32 = 0x00;
const PROT_READ: i32 = 0x01;
const PROT_WRITE: i32 = 0x02;

/// What `mmap` returns on failure: `(void *)-1`, not null.
fn map_failed() -> MutVoidPtr {
    MutVoidPtr::from_bits(!0)
}

#[derive(Default)]
pub struct State {
    /// Keeping track of `mmap` allocations
    mmap_allocations: HashMap<MutVoidPtr, GuestUSize>,
}

/// Whether `[addr, addr + len)` lies wholly inside a mapping `mmap` has already
/// handed out.
fn within_existing_mapping(state: &State, addr: MutVoidPtr, len: GuestUSize) -> bool {
    if addr.is_null() || len == 0 {
        return false;
    }
    let start = addr.to_bits();
    let Some(end) = start.checked_add(len) else {
        return false;
    };
    state.mmap_allocations.iter().any(|(&base, &size)| {
        let base = base.to_bits();
        start >= base && end <= base.saturating_add(size)
    })
}

/// For files, our implementation of mmap is really simple:
/// it's just load entirety of file in memory!
fn mmap(
    env: &mut Environment,
    addr: MutVoidPtr,
    len: GuestUSize,
    prot: i32,
    flags: i32,
    fd: FileDescriptor,
    offset: off_t,
) -> MutVoidPtr {
    // TODO: handle errno properly
    set_errno(env, 0);

    log_dbg!(
        "mmap({:?}, {}, {}, {}, {}, {})",
        addr,
        len,
        prot,
        flags,
        fd,
        offset
    );

    // A MAP_FIXED mapping over memory this process already mapped is a
    // *re*-mapping, not a new allocation. Boehm's garbage collector — the one
    // inside Mono, and therefore inside every Unity game — releases memory
    // with `mmap(addr, len, PROT_NONE, MAP_FIXED | MAP_ANON, ...)` over its own
    // heap, and takes it back the same way, because that keeps the address
    // reserved while telling the kernel the contents are gone. On a real kernel
    // MAP_FIXED replaces whatever is mapped there; tapHLE's allocator instead
    // refuses an address it has already handed out, so the collector saw
    // MAP_FAILED and called `ABORT("mmap(PROT_NONE) failed")`, terminating the
    // app. In one measured case that happened while loading a level.
    //
    // Answering with the same address models the replacement. tapHLE has no
    // per-page protection to apply, and leaving the bytes untouched is the
    // conservative choice: the collector treats remapped memory as
    // uninitialised, so keeping stale contents is allowed, whereas zeroing a
    // range that some *other* mapping shares would destroy live data.
    if flags & MAP_FIXED != 0 && within_existing_mapping(&env.libc_state.mman, addr, len) {
        log_dbg!(
            "mmap: MAP_FIXED re-map of {:?}+{}, already mapped",
            addr,
            len
        );
        return addr;
    }

    // A mapping that cannot be satisfied is an ordinary runtime outcome, not a
    // programming error: mmap is specified to return MAP_FAILED and set errno,
    // and callers are written to check for it. Aborting instead killed apps
    // that would have coped — asking for a specific address is a hint that
    // tapHLE's address space often cannot honour, because its layout is not the
    // device's.
    let allocate = |env: &mut Environment| -> Option<MutVoidPtr> {
        if addr.is_null() {
            return env.mem.vm_alloc(None, len).ok();
        }
        match env.mem.vm_alloc(Some(addr.to_bits()), len) {
            Ok(ptr) => Some(ptr),
            // MAP_FIXED means "exactly here or fail", so it gets no fallback.
            Err(_) if flags & MAP_FIXED != 0 => None,
            // Without MAP_FIXED the address is only a hint, and mmap may place
            // the mapping anywhere when it cannot be honoured. Every reason it
            // could not be counts, which is the part that was missing: a hint
            // landing in free space too small for the request fails as
            // `NoSpace`, not `AddressUnavailable`, and only the latter used to
            // fall back. So a mapping could be refused with gigabytes of the
            // address space unused, purely because of where the caller pointed.
            //
            // That shows up as a hang rather than a crash. An allocator inside
            // a managed runtime treats a failed mapping as back-pressure and
            // retries, so the guest spins on the same doomed request forever
            // and the app sits frozen on whatever frame it last drew.
            Err(_) => {
                let ptr = env.mem.vm_alloc(None, len).ok()?;
                log!("Warning: mmap could not allocate at hint {addr:?}, allocated at {ptr:?}",);
                Some(ptr)
            }
        }
    };
    let Some(ptr) = allocate(env) else {
        log!("Warning: mmap({addr:?}, {len}) failed, returning MAP_FAILED");
        set_errno(env, ENOMEM);
        return map_failed();
    };

    assert!(ptr.to_bits() & PAGE_SIZE_ALIGN_MASK == 0);

    if (flags & MAP_ANON) != 0 {
        // The mapping is anonymous, so there is nothing to read in. Darwin
        // ignores the descriptor entirely in this case rather than requiring
        // it to be -1, and real code does pass a live one — aborting here
        // killed apps making a perfectly ordinary allocation.
        if fd != -1 {
            log_dbg!("mmap: MAP_ANON with fd {}, ignoring the descriptor", fd);
        }
    } else {
        let new_offset = posix_io::lseek(env, fd, offset, SEEK_SET);
        assert_eq!(new_offset, offset);

        let read = posix_io::read(env, fd, ptr, len);
        assert_eq!(read as u32, len);
    };

    assert!(!env.libc_state.mman.mmap_allocations.contains_key(&ptr));
    env.libc_state.mman.mmap_allocations.insert(ptr, len);

    ptr
}

fn munmap(env: &mut Environment, addr: MutVoidPtr, len: GuestUSize) -> i32 {
    // TODO: handle errno properly
    set_errno(env, 0);

    log_dbg!("munmap({:?}, {})", addr, len);

    if len == 0 {
        set_errno(env, EINVAL);
        // TODO: should we clear allocations for `addr` here too?
        log!("Warning: munmap({:?}, {}) failed, returning -1", addr, len);
        return -1;
    }
    assert_eq!(
        *env.libc_state.mman.mmap_allocations.get(&addr).unwrap(),
        len
    );
    env.mem.vm_free(addr, len);
    env.libc_state.mman.mmap_allocations.remove(&addr);
    0 // success
}

fn madvise(env: &mut Environment, addr: MutVoidPtr, len: GuestUSize, advice: i32) -> i32 {
    log!("TODO: madvise({:?}, {}, {}) -> -1", addr, len, advice);
    set_errno(env, ENOTSUP);
    -1
}

fn shm_open(env: &mut Environment, name: ConstPtr<u8>, oflag: i32, _dots: DotDotDot) -> i32 {
    log!(
        "TODO: shm_open({:?} '{:?}', {}, ...) -> -1",
        name,
        env.mem.cstr_at_utf8(name),
        oflag
    );
    set_errno(env, EINVAL);
    -1
}

/// tapHLE has no per-page protection: every page of guest memory is readable
/// and writable for as long as it is mapped. So there is nothing to apply, and
/// the only question is what to tell the caller.
///
/// Reporting failure was the wrong answer. A caller asking to make memory
/// *more* accessible — `PROT_READ | PROT_WRITE`, which is the common case —
/// gets exactly what it asked for, because the page already is. Returning -1
/// then describes tapHLE's own bookkeeping rather than the guest's memory, and
/// callers act on it: Boehm's collector, inside Mono and therefore inside every
/// Unity game, prints `Mprotect remapping failed` and calls `abort()`. In Cubed
/// measured case that ended the app part-way through a session.
///
/// A request to make memory *less* accessible is the case tapHLE cannot honour,
/// and there is nothing better to return for it either. Refusing does not
/// protect the page; it only kills apps that would have coped. So this
/// succeeds, and logs what could not be enforced, because a guard page that
/// never faults can turn a guest bug into confusing behaviour much later.
fn mprotect(env: &mut Environment, addr: MutVoidPtr, len: GuestUSize, prot: i32) -> i32 {
    set_errno(env, 0);
    if prot == PROT_NONE || prot & !(PROT_READ | PROT_WRITE) != 0 {
        log_dbg!(
            "mprotect({:?}, {}, {}): tapHLE has no page protection, reporting success anyway",
            addr,
            len,
            prot
        );
    }
    0 // success
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(mmap(_, _, _, _, _, _)),
    export_c_func!(munmap(_, _)),
    export_c_func!(madvise(_, _, _)),
    export_c_func!(shm_open(_, _, _)),
    export_c_func!(mprotect(_, _, _)),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::Ptr;

    fn state_with(base: u32, size: GuestUSize) -> State {
        let mut state = State::default();
        state.mmap_allocations.insert(Ptr::from_bits(base), size);
        state
    }

    #[test]
    fn a_subrange_of_an_existing_mapping_is_recognised() {
        let state = state_with(0x1f00000, 0x100000);
        // The exact call Boehm's GC_unmap made in a measured case.
        assert!(within_existing_mapping(
            &state,
            Ptr::from_bits(0x1f2e000),
            176128
        ));
        // Whole-range and start-aligned cases are re-mappings too.
        assert!(within_existing_mapping(
            &state,
            Ptr::from_bits(0x1f00000),
            0x100000
        ));
    }

    #[test]
    fn an_unmapped_or_overrunning_range_is_not_recognised() {
        let state = state_with(0x1f00000, 0x100000);
        // Below the mapping.
        assert!(!within_existing_mapping(
            &state,
            Ptr::from_bits(0x1000000),
            0x1000
        ));
        // Starts inside but runs past the end.
        assert!(!within_existing_mapping(
            &state,
            Ptr::from_bits(0x1ff0000),
            0x100000
        ));
        // Degenerate requests never count, so they still take the normal path.
        assert!(!within_existing_mapping(&state, Ptr::null(), 0x1000));
        assert!(!within_existing_mapping(
            &state,
            Ptr::from_bits(0x1f00000),
            0
        ));
    }
}
