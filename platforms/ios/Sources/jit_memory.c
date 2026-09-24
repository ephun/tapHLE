/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
#include "jit_memory.h"
#include <errno.h>
#include <libkern/OSCacheControl.h>
#include <mach/mach.h>
#include <mach/vm_map.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>

static bool has_protection(void *address, size_t length, vm_prot_t expected) {
    vm_address_t start = (vm_address_t)address;
    vm_size_t size = 0;
    vm_region_basic_info_data_64_t info;
    mach_msg_type_number_t count = VM_REGION_BASIC_INFO_COUNT_64;
    mach_port_t object = MACH_PORT_NULL;
    kern_return_t result = vm_region_64(mach_task_self(), &start, &size,
        VM_REGION_BASIC_INFO_64, (vm_region_info_t)&info, &count, &object);
    if (object) mach_port_deallocate(mach_task_self(), object);
    // vm_region_64 returns the region containing address, which can begin
    // earlier after adjacent mappings coalesce or a debugger supplies a slice.
    vm_address_t requested = (vm_address_t)address;
    return result == KERN_SUCCESS && start <= requested && size >= length &&
           requested - start <= size - length && info.protection == expected;
}
bool tapHLE_jit_memory_prepare(TapHLEJITMemory *pool,
    void *(*prepare_region)(void *, size_t), void (*detach_script)(void),
    char error[512]) {
    const size_t pool_size = pool->size;
    const bool script_required = prepare_region != NULL;
    void *execute = NULL, *write = NULL;
    error[0] = 0;
    // On TXM, request a new region from the debugger so a failed preparation
    // cannot be confused with returning our original non-null input address.
    if (script_required) {
        execute = prepare_region(NULL, pool_size);
        if (execute == MAP_FAILED) execute = NULL;
        if (!execute ||
            !has_protection(execute, pool_size, VM_PROT_READ | VM_PROT_EXECUTE)) {
            snprintf(error, 512, "StikDebug did not return a valid executable region. Check its universal.js log.");
        }
    } else {
        write = mmap(NULL, pool_size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
        if (write == MAP_FAILED) {
            snprintf(error, 512, "JIT mmap failed: %s", strerror(errno));
            write = NULL;
        }
    }
    if (!error[0]) {
        vm_address_t alias = 0;
        vm_prot_t current, maximum;
        void *source = script_required ? execute : write;
        kern_return_t result = vm_remap(mach_task_self(), &alias, pool_size, 0,
            VM_FLAGS_ANYWHERE, mach_task_self(), (vm_address_t)source, FALSE,
            &current, &maximum, VM_INHERIT_NONE);
        if (result != KERN_SUCCESS) {
            snprintf(error, 512, "JIT alias mapping failed: %s", mach_error_string(result));
        } else {
            if (script_required) write = (void *)alias;
            else execute = (void *)alias;
            int permissions = script_required ? (PROT_READ | PROT_WRITE) : (PROT_READ | PROT_EXEC);
            if (mprotect((void *)alias, pool_size, permissions) != 0) {
                snprintf(error, 512, "JIT memory protection failed: %s", strerror(errno));
            }
        }
    }
    if (!error[0] && (!has_protection(write, pool_size, VM_PROT_READ | VM_PROT_WRITE) ||
                     !has_protection(execute, pool_size, VM_PROT_READ | VM_PROT_EXECUTE))) {
        snprintf(error, 512, "JIT mappings do not have separate writable and executable permissions.");
    }
    if (!error[0]) {
        // Check alias coherence without executing unverified memory.
        *(volatile uint32_t *)write = 0xd65f03c0; // ARM64 ret
        sys_icache_invalidate(execute, sizeof(uint32_t));
        if (*(volatile uint32_t *)execute != 0xd65f03c0)
            snprintf(error, 512, "JIT writable and executable mappings are not coherent.");
    }
    if (script_required) detach_script();
    if (error[0]) {
        if (write) munmap(write, pool_size);
        if (execute) munmap(execute, pool_size);
        return false;
    }
    pool->write = write;
    pool->execute = execute;
    return true;
}
