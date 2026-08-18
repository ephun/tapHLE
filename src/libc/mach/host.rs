/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `mach_host.h` and some other related functions

#![allow(non_camel_case_types)]

use crate::dyld::FunctionExports;
use crate::libc::mach::arm::vm_types::vm_size_t;
use crate::libc::mach::core_types::{integer_t, natural_t};
use crate::libc::mach::port::mach_port_t;
use crate::libc::mach::thread_info::{
    kern_return_t, mach_msg_type_number_t, KERN_FAILURE, KERN_INVALID_ARGUMENT, KERN_SUCCESS,
};
use crate::mem::{guest_size_of, MutPtr, SafeRead, PAGE_SIZE};
use crate::{export_c_func, Environment};

type host_t = mach_port_t;
type host_name_port_t = host_t;
type host_flavor_t = natural_t;
type host_info_t = MutPtr<natural_t>;

// The value doesn't matter that much, only the fact that it's unique
// per host so we could assert against it in our code.
const MACH_HOST_SELF: host_name_port_t = 0x100c442e;

// Values taken from an iPod Touch 4 running iOS 6.1
// Used in host_statistics function (returned in vm_statistics)
// Also used to calcuate PHYSICAL_MEMORY (used by NSProcessInfo)
const FREE_COUNT: natural_t = 12897;
const ACTIVE_COUNT: natural_t = 0;
const INACTIVE_COUNT: natural_t = 0;
const WIRE_COUNT: natural_t = 0;

pub const PHYSICAL_MEMORY: natural_t =
    (FREE_COUNT + ACTIVE_COUNT + INACTIVE_COUNT + WIRE_COUNT) * PAGE_SIZE;

const HOST_BASIC_INFO: host_flavor_t = 1;
const HOST_VM_INFO: host_flavor_t = 2;

/// `CPU_TYPE_ARM` and `CPU_SUBTYPE_ARM_V7`, which is what tapHLE executes.
const CPU_TYPE_ARM: integer_t = 12;
const CPU_SUBTYPE_ARM_V7: integer_t = 9;

/// What `host_info(HOST_BASIC_INFO)` reports.
///
/// The first five fields are the whole of the structure as it was before Mac OS
/// X 10.5, and a caller built against those headers offers a five-word buffer.
/// Both shapes are still in use, so both are written; see the count handling in
/// `host_info` below.
#[repr(C, packed)]
struct host_basic_info {
    max_cpus: integer_t,
    avail_cpus: integer_t,
    memory_size: natural_t,
    cpu_type: integer_t,
    cpu_subtype: integer_t,
    cpu_threadtype: integer_t,
    physical_cpu: integer_t,
    physical_cpu_max: integer_t,
    logical_cpu: integer_t,
    logical_cpu_max: integer_t,
    max_mem: u64,
}
unsafe impl SafeRead for host_basic_info {}

/// The five-field structure a caller built against the older headers expects.
#[repr(C, packed)]
struct host_basic_info_old {
    max_cpus: integer_t,
    avail_cpus: integer_t,
    memory_size: natural_t,
    cpu_type: integer_t,
    cpu_subtype: integer_t,
}
unsafe impl SafeRead for host_basic_info_old {}

#[repr(C, packed)]
struct vm_statistics {
    free_count: natural_t,
    active_count: natural_t,
    inactive_count: natural_t,
    wire_count: natural_t,
    zero_fill_count: natural_t,
    reactivations: natural_t,
    pageins: natural_t,
    pageouts: natural_t,
    faults: natural_t,
    cow_faults: natural_t,
    lookups: natural_t,
    hits: natural_t,
    purgeable_count: natural_t,
    purges: natural_t,
    speculative_count: natural_t,
}
unsafe impl SafeRead for vm_statistics {}

fn mach_host_self(_env: &mut Environment) -> host_name_port_t {
    MACH_HOST_SELF
}

fn host_page_size(
    env: &mut Environment,
    host: host_t,
    out_page_size: MutPtr<vm_size_t>,
) -> kern_return_t {
    assert_eq!(host, MACH_HOST_SELF);
    env.mem.write(out_page_size, PAGE_SIZE);
    KERN_SUCCESS
}

fn host_statistics(
    env: &mut Environment,
    host: host_t,
    flavor: host_flavor_t,
    host_info_out: host_info_t,
    host_info_out_count: MutPtr<mach_msg_type_number_t>,
) -> kern_return_t {
    assert_eq!(host, MACH_HOST_SELF);
    if flavor != HOST_VM_INFO {
        log!(
            "TODO: host_statistics() with flavor {}, returning KERN_INVALID_ARGUMENT",
            flavor
        );
        return KERN_INVALID_ARGUMENT;
    }

    // The count is the size of the buffer the caller is offering, not a promise
    // that the caller's idea of `vm_statistics` matches ours. Unreal Engine 3
    // apps (Epic Citadel, Infinity Blade 2, Dark Meadow) pass 60 here, which is
    // the size in bytes rather than in words, and an app built against another
    // SDK revision of the structure passes that revision's word count. The
    // kernel copies out as much as both sides have room for and writes back how
    // much it wrote, so a caller offering more room than needed reads a short
    // reply instead of being refused.
    let out_size_available = env.mem.read(host_info_out_count);
    let out_size_expected = guest_size_of::<vm_statistics>() / guest_size_of::<natural_t>();
    if out_size_available < out_size_expected {
        log!(
            "host_statistics() reply buffer holds {} words but vm_statistics needs {}",
            out_size_available,
            out_size_expected
        );
        return KERN_FAILURE;
    }

    // Below values corresponds to a run of an iPod Touch 4 running iOS 6.1.
    // As tapHLE doesn't have a paging system (yet? never?),
    // those numbers are (mostly) meaningless.
    // In reality, this function is commonly used by apps to get
    // the amount of current free memory available.
    // Ace Combat Xi uses this function to allocate a pool of objects as big as
    // it can fit. A larger free count value means more allocations, making the
    // startup time longer.
    // TODO: approximate size of current memory allocations and return them?
    env.mem.write(
        host_info_out.cast(),
        vm_statistics {
            free_count: FREE_COUNT,
            active_count: ACTIVE_COUNT,
            inactive_count: INACTIVE_COUNT,
            wire_count: WIRE_COUNT,
            zero_fill_count: 0,
            reactivations: 0,
            pageins: 0,
            pageouts: 0,
            faults: 0,
            cow_faults: 0,
            lookups: 0,
            hits: 0,
            purgeable_count: 0,
            purges: 0,
            speculative_count: 0,
        },
    );
    // Report what was actually written, which is how a caller with a larger
    // buffer learns where our reply ends.
    env.mem.write(host_info_out_count, out_size_expected);
    KERN_SUCCESS
}

/// What the machine is: how many processors and how much memory.
///
/// Games ask during start-up to size a thread pool or a cache, and the question
/// had no implementation, so asking it ended the app — Crossy Road and Smashy
/// Road Wanted both stop here. One processor is reported because tapHLE runs
/// the guest on one, so an app that sizes a pool by this number gets a pool
/// that matches what it will actually be able to run.
fn host_info(
    env: &mut Environment,
    host: host_t,
    flavor: host_flavor_t,
    host_info_out: host_info_t,
    host_info_out_count: MutPtr<mach_msg_type_number_t>,
) -> kern_return_t {
    assert_eq!(host, MACH_HOST_SELF);
    if flavor != HOST_BASIC_INFO {
        log!(
            "TODO: host_info() with flavor {}, returning KERN_INVALID_ARGUMENT",
            flavor
        );
        return KERN_INVALID_ARGUMENT;
    }

    // As in host_statistics above, the count is the room the caller has, and
    // which shape of the structure it expects follows from it.
    let out_size_available = env.mem.read(host_info_out_count);
    let old_size = guest_size_of::<host_basic_info_old>() / guest_size_of::<natural_t>();
    let full_size = guest_size_of::<host_basic_info>() / guest_size_of::<natural_t>();

    if out_size_available < old_size {
        log!(
            "host_info() reply buffer holds {} words but host_basic_info needs at least {}",
            out_size_available,
            old_size
        );
        return KERN_FAILURE;
    }

    let written = if out_size_available < full_size {
        env.mem.write(
            host_info_out.cast(),
            host_basic_info_old {
                max_cpus: 1,
                avail_cpus: 1,
                memory_size: PHYSICAL_MEMORY,
                cpu_type: CPU_TYPE_ARM,
                cpu_subtype: CPU_SUBTYPE_ARM_V7,
            },
        );
        old_size
    } else {
        env.mem.write(
            host_info_out.cast(),
            host_basic_info {
                max_cpus: 1,
                avail_cpus: 1,
                memory_size: PHYSICAL_MEMORY,
                cpu_type: CPU_TYPE_ARM,
                cpu_subtype: CPU_SUBTYPE_ARM_V7,
                cpu_threadtype: 0,
                physical_cpu: 1,
                physical_cpu_max: 1,
                logical_cpu: 1,
                logical_cpu_max: 1,
                max_mem: PHYSICAL_MEMORY as u64,
            },
        );
        full_size
    };
    env.mem.write(host_info_out_count, written);
    KERN_SUCCESS
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(mach_host_self()),
    export_c_func!(host_info(_, _, _, _)),
    export_c_func!(host_page_size(_, _)),
    export_c_func!(host_statistics(_, _, _, _)),
];
