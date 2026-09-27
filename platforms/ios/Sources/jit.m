/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
#import <UIKit/UIKit.h>
#include "jit_memory.h"
#include <TargetConditionals.h>
#include <dlfcn.h>
#include <errno.h>
#include <libkern/OSCacheControl.h>
#include <mach/mach.h>
#include <mach/vm_map.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/sysctl.h>
#include <unistd.h>

// The protocol ABI is documented by StikDebug/StikJIT's INTEGRATION.md.
// This host deliberately uses the separate StikDebug app, not an embedded
// debugger/helper extension. All coordinator calls run on the UIKit thread.
#if !TARGET_OS_SIMULATOR && defined(__aarch64__)
extern int csops(pid_t, unsigned int, void *, size_t);
extern CFTypeRef SecTaskCreateFromSelf(CFAllocatorRef);
extern CFTypeRef SecTaskCopyValueForEntitlement(CFTypeRef, CFStringRef, CFErrorRef *);

static const size_t pool_size = 128 * 1024 * 1024;
static void *write_pool, *execute_pool;
static bool leased, ready, waiting, script_required, request_opened;
static NSUInteger generation;
static NSTimeInterval deadline;
static char last_error[512];

static void record_status(const char *message) {
    NSLog(@"tapHLE JIT: %s", message);
    NSURL *documents = [NSFileManager.defaultManager URLsForDirectory:NSDocumentDirectory inDomains:NSUserDomainMask].firstObject;
    NSString *text = [NSString stringWithFormat:@"%@ pid=%d\n%s\n", NSDate.date, getpid(), message];
    [text writeToURL:[documents URLByAppendingPathComponent:@"jit-status.txt"]
        atomically:YES encoding:NSUTF8StringEncoding error:nil];
}

static int fail(const char *message) {
    snprintf(last_error, sizeof(last_error), "%s", message);
    waiting = false;
    record_status(last_error);
    return -1;
}

static bool debugged(void) {
    uint32_t flags = 0;
    return csops(getpid(), 0, &flags, sizeof(flags)) == 0 &&
           (flags & 0x10000000) != 0; // CS_DEBUGGED persists after detach.
}

static bool attached(void) {
    struct kinfo_proc info = {0};
    size_t size = sizeof(info);
    int mib[] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()};
    return sysctl(mib, 4, &info, &size, NULL, 0) == 0 &&
           size == sizeof(info) && (info.kp_proc.p_flag & P_TRACED) != 0;
}

static bool task_allowed(void) {
    CFTypeRef task = SecTaskCreateFromSelf(kCFAllocatorDefault);
    if (!task) return false;
    CFTypeRef value = SecTaskCopyValueForEntitlement(task, CFSTR("get-task-allow"), NULL);
    bool allowed = value && CFEqual(value, kCFBooleanTrue);
    if (value) CFRelease(value);
    CFRelease(task);
    return allowed;
}

// Query the same IORegistry property as StikJIT's ProcessInfo+TXM.swift.
// Unknown must not silently select the older attachment-only path.
static int txm_present(void) {
    if (@available(iOS 26.0, *)) {
        void *library = dlopen("/System/Library/Frameworks/IOKit.framework/IOKit", RTLD_LAZY);
        if (!library) return -1;
        mach_port_t (*entry_from_path)(mach_port_t, const char *) = dlsym(library, "IORegistryEntryFromPath");
        CFTypeRef (*property)(mach_port_t, CFStringRef, CFAllocatorRef, uint32_t) = dlsym(library, "IORegistryEntryCreateCFProperty");
        kern_return_t (*release)(mach_port_t) = dlsym(library, "IOObjectRelease");
        int result = -1;
        if (entry_from_path && property && release) {
            mach_port_t entry = entry_from_path(MACH_PORT_NULL, "IODeviceTree:/chosen/memory-map");
            if (entry) {
                CFTypeRef keys = property(entry, CFSTR("IORegistryEntryPropertyKeys"), kCFAllocatorDefault, 0);
                if (keys && CFGetTypeID(keys) == CFArrayGetTypeID()) {
                    result = CFArrayContainsValue(keys, CFRangeMake(0, CFArrayGetCount(keys)), CFSTR("TXM")) ? 1 : 0;
                }
                if (keys) CFRelease(keys);
                release(entry);
            }
        }
        dlclose(library);
        return result;
    }
    return 0;
}

__attribute__((noinline, optnone, naked))
static void *prepare_region(void *address, size_t length) {
    __asm__("mov x16, #1\n brk #0xf00d\n ret\n");
}
__attribute__((noinline, optnone, naked))
static void detach_script(void) {
    __asm__("mov x16, #0\n brk #0xf00d\n ret\n");
}

static int prepare_pool(void) {
    TapHLEJITMemory pool = {.size = pool_size};
    char error[512];
    record_status("Preparing executable and writable JIT memory views.");
    if (!tapHLE_jit_memory_prepare(&pool,
        script_required ? prepare_region : NULL,
        script_required ? detach_script : NULL, error)) return fail(error);
    write_pool = pool.write;
    execute_pool = pool.execute;
    ready = true;
    waiting = false;
    record_status(script_required ? "Ready: 128 MiB JIT pool prepared with universal.js (TXM)." : "Ready: 128 MiB JIT pool prepared after debugger attachment.");
    return 1;
}

int tapHLE_ios_jit_begin(void) {
    if (ready) return 1;
    if (waiting) return 0;
    last_error[0] = 0;
    if (!task_allowed()) return fail("This installation lacks get-task-allow. Reinstall tapHLE with a sideloading method that preserves it.");
    int txm = txm_present();
    if (txm < 0) return fail("Could not determine device JIT requirements (TXM). Check the device console before retrying.");
    script_required = txm == 1;
    if (!script_required && debugged()) return prepare_pool();
    NSURLComponents *url = [NSURLComponents new];
    url.scheme = @"stikdebug";
    url.host = @"enable-jit";
    NSString *bundle = NSBundle.mainBundle.bundleIdentifier;
    if (!bundle) return fail("Could not determine tapHLE's installed bundle identifier.");
    NSMutableArray *items = [NSMutableArray arrayWithArray:@[
        [NSURLQueryItem queryItemWithName:@"bundle-id" value:bundle],
        [NSURLQueryItem queryItemWithName:@"pid" value:[NSString stringWithFormat:@"%d", getpid()]]]];
    if (script_required) [items addObject:[NSURLQueryItem queryItemWithName:@"script-name" value:@"universal.js"]];
    url.queryItems = items;
    if (![UIApplication.sharedApplication canOpenURL:url.URL])
        return fail("Install StikDebug and complete its pairing, Developer Mode, and LocalDevVPN setup, then try Play again.");
    record_status(script_required ? "Requesting StikDebug with universal.js for this process (TXM)." : "Requesting StikDebug attachment for this process.");
    waiting = true;
    request_opened = false;
    deadline = NSProcessInfo.processInfo.systemUptime + 120;
    NSUInteger request = ++generation;
    [UIApplication.sharedApplication openURL:url.URL options:@{} completionHandler:^(BOOL success) {
        if (request != generation || !waiting) return;
        if (!success) fail("iOS could not open StikDebug. Open it manually and check its setup.");
        else request_opened = true;
    }];
    return 0;
}

int tapHLE_ios_jit_poll(void) {
    if (ready) return 1;
    if (!waiting) return -1;
    if (NSProcessInfo.processInfo.systemUptime > deadline)
        return fail("JIT preparation timed out. Check StikDebug's log, pairing and VPN, then try Play again.");
    if (!request_opened || UIApplication.sharedApplication.applicationState != UIApplicationStateActive)
        return 0;
    if (!debugged() || (script_required && !attached())) return 0;
    return prepare_pool();
}
void tapHLE_ios_jit_cancel(void) { waiting = false; ++generation; }
const char *tapHLE_ios_jit_error(void) { return last_error; }
const char *tapHLE_ios_jit_acquire(size_t size, void **write, void **execute) {
    if (!ready) return "JIT is not prepared. Return to the library and enable JIT with StikDebug.";
    if (leased) return "Another guest CPU already owns the JIT pool.";
    if (size > pool_size) return "The guest CPU requested more than the prepared JIT pool.";
    leased = true;
    *write = write_pool;
    *execute = execute_pool;
    return NULL;
}
void tapHLE_ios_jit_release(void) { leased = false; }
#else
int tapHLE_ios_jit_begin(void) { return 1; }
int tapHLE_ios_jit_poll(void) { return 1; }
void tapHLE_ios_jit_cancel(void) {}
const char *tapHLE_ios_jit_error(void) { return ""; }
#endif
