"""Test the iOS Dynarmic overlay and memory allocator without a device debugger."""
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]


class IOSJITSpinLockTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which("rustc") and shutil.which("c++"),
                         "requires rustc and a C++ compiler")
    def test_host_spin_lock_needs_no_jit_lease(self):
        package = ROOT / "crates/taphle/src/cpu/dynarmic_wrapper"
        vendor = ROOT / "vendor/dynarmic"
        relative = "src/dynarmic/common/spin_lock_arm64.cpp"
        original = (vendor / relative).read_text()
        with tempfile.TemporaryDirectory(prefix="taphle-spin-test-") as directory:
            folder = pathlib.Path(directory)
            source = folder / "source"
            # Minimal input tree for the real overlay builder, not a reimplementation.
            for name in [relative, "src/dynarmic/common/spin_lock.h",
                         "src/dynarmic/backend/arm64/address_space.cpp",
                         "externals/oaknut/include/oaknut/code_block.hpp"]:
                target = source / name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(vendor / name, target)
            builder = (package / "build.rs").read_text().split("fn ios_dynarmic_source", 1)[1]
            rust = folder / "overlay.rs"
            rust.write_text('use std::{env, path::Path};\n'
                            'fn rerun_if_changed(_: &Path) {}\n'
                            'fn ios_dynarmic_source' + builder + '\n'
                            'fn main() { let a: Vec<_> = env::args().collect(); '
                            'ios_dynarmic_source(Path::new(&a[1]), Path::new(&a[2])); }\n')
            subprocess.run(["rustc", "--edition=2021", str(rust), "-o", str(folder / "overlay")],
                           check=True, capture_output=True, text=True)
            subprocess.run([str(folder / "overlay"), str(source), str(package)],
                           env={**os.environ, "OUT_DIR": str(folder)}, check=True,
                           capture_output=True, text=True)
            overlay = folder / "ios-dynarmic-source"
            patched = (overlay / relative).read_text()
            emitters = original.split("void EmitSpinLockLock", 1)[1].split("namespace {", 1)[0]
            self.assertIn("void EmitSpinLockLock" + emitters, patched)
            self.assertEqual((overlay / "src/dynarmic/common/spin_lock.h").read_bytes(),
                             (vendor / "src/dynarmic/common/spin_lock.h").read_bytes())
            # Register aliases are irrelevant to the host lock; avoid pulling in
            # the full backend/Boost just to compile its two emitter helpers.
            abi = folder / "include/dynarmic/backend/arm64/abi.h"
            abi.parent.mkdir(parents=True)
            abi.write_text('#include <oaknut/oaknut.hpp>\n'
                           'namespace Dynarmic::Backend::Arm64 {\n'
                           'constexpr auto Wscratch0 = oaknut::util::W16;\n'
                           'constexpr auto Wscratch1 = oaknut::util::W17;\n}\n')
            cache = folder / "include/libkern/OSCacheControl.h"
            cache.parent.mkdir(parents=True)
            cache.write_text("#include <stddef.h>\n"
                             "static inline void sys_icache_invalidate(void *, size_t) {}\n")
            test = folder / "test.cpp"
            test.write_text(r'''
#include "dynarmic/common/spin_lock.h"
#include <cassert>
#include <cstddef>
#include <thread>
#include <vector>
extern "C" const char *tapHLE_ios_jit_acquire(size_t, void **, void **) {
    return "JIT not prepared: host spin lock must not request a lease";
}
extern "C" void tapHLE_ios_jit_release() { assert(false); }
static_assert(sizeof(Dynarmic::SpinLock) == sizeof(int));
static_assert(offsetof(Dynarmic::SpinLock, storage) == 0);
int main() {
    Dynarmic::SpinLock lock;
    assert(lock.storage == 0);
    lock.Lock(); assert(lock.storage == 1);
    lock.Unlock(); assert(lock.storage == 0);
    int count = 0;
    std::vector<std::thread> threads;
    for (int i = 0; i < 4; ++i) threads.emplace_back([&] {
        for (int j = 0; j < 10000; ++j) {
            lock.Lock(); ++count; lock.Unlock();
        }
    });
    for (auto &thread : threads) thread.join();
    assert(count == 40000 && lock.storage == 0);
}
''')
            compile_result = subprocess.run([
                "c++", "-std=c++20", "-pthread", "-Wall", "-Wextra", "-Werror",
                "-I" + str(folder / "include"), "-I" + str(overlay / "src"),
                "-I" + str(overlay / "externals/oaknut/include"),
                "-I" + str(vendor / "externals/oaknut/include"),
                str(overlay / relative), str(test), "-o", str(folder / "test")],
                capture_output=True, text=True)
            self.assertEqual(compile_result.returncode, 0, compile_result.stderr)
            result = subprocess.run([str(folder / "test")], capture_output=True,
                                    text=True, timeout=20, cwd=folder)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertNotIn("SpinLockImpl", patched)
            self.assertEqual((vendor / relative).read_text(), original)


class IOSJITCleanupTests(unittest.TestCase):
    def test_rejected_debugger_mapping_is_retained_for_cleanup(self):
        source = (ROOT / "platforms/ios/Sources/jit_memory.c").read_text()
        rejection = source.split('snprintf(error, 512, "StikDebug', 1)[1].split("}", 1)[0]
        self.assertNotIn("execute = NULL", rejection,
                         "rejected non-null mapping must survive until failure cleanup")
        self.assertIn("if (execute == MAP_FAILED) execute = NULL;", source)
        self.assertIn("if (execute) munmap(execute, pool_size);", source)


class IOSHostIntegrationTests(unittest.TestCase):
    def test_frontend_separates_ios_paths_from_external_urls(self):
        dialogs = (ROOT / "crates/gui/src/platform/dialogs.rs").read_text()
        process = (ROOT / "crates/gui/src/platform/process.rs").read_text()
        host = (ROOT / "platforms/ios/Sources/main.m").read_text()
        path_route = process.split("pub fn open_path", 1)[1].split("pub fn open_url", 1)[0]
        url_route = process.split("pub fn open_url", 1)[1]
        self.assertIn("tapHLE_ios_present_app_picker", dialogs)
        self.assertIn("tapHLE_ios_export_folder", path_route)
        self.assertNotIn("tapHLE_ios_open_url", path_route)
        self.assertIn("tapHLE_ios_open_url", url_route)
        self.assertIn("initForExportingURLs:@[folder] asCopy:YES", host)

    def test_document_pickers_present_on_an_active_scene_and_main_thread(self):
        host = (ROOT / "platforms/ios/Sources/main.m").read_text()
        self.assertNotIn(".keyWindow", host)
        self.assertIn("UISceneActivationStateForegroundActive", host)
        self.assertIn("window.isKeyWindow", host)
        self.assertIn("NSThread.isMainThread", host)
        self.assertIn("dispatch_sync(dispatch_get_main_queue()", host)

    def test_imports_release_scope_and_enqueue_only_owned_copies(self):
        host = (ROOT / "platforms/ios/Sources/main.m").read_text()
        callback = host.split("didPickDocumentsAtURLs:", 1)[1].split(
            "documentPickerWasCancelled:", 1)[0]
        copy_branch = callback.split("copyItemAtURL:source", 1)[1]
        self.assertIn("push_drop_file(destination)", copy_branch)
        self.assertNotIn("push_drop_file(source)", callback)
        self.assertIn("stopAccessingSecurityScopedResource", callback)
        self.assertNotIn("security_scoped_urls", host)

    def test_import_copy_is_async_and_completion_returns_to_main(self):
        host = (ROOT / "platforms/ios/Sources/main.m").read_text()
        callback = host.split("didPickDocumentsAtURLs:", 1)[1].split(
            "documentPickerWasCancelled:", 1)[0]
        background = callback.split("dispatch_async(import_queue()", 1)[1]
        completion = background.split("dispatch_async(dispatch_get_main_queue()", 1)[1]
        self.assertIn("copyItemAtURL:source", background)
        self.assertIn("push_drop_file(destination)", completion)
        self.assertIn("document_picker_delegate == delegate", completion)
        self.assertNotIn("document_picker_delegate = nil", background.split(
            "dispatch_async(dispatch_get_main_queue()", 1)[0])

    def test_import_destination_is_stable_for_identical_ipa_bytes(self):
        host = (ROOT / "platforms/ios/Sources/main.m").read_text()
        self.assertIn("CC_SHA256_DIGEST_LENGTH", host)
        self.assertIn("destination_for_digest", host)
        self.assertIn("stringByAppendingPathExtension:@\"ipa\"]", host)
        self.assertNotIn("available_destination", host)

    def test_filtered_drop_event_releases_its_filename(self):
        host = (ROOT / "platforms/ios/Sources/main.m").read_text()
        push = host.split("static void push_drop_file", 1)[1].split(
            "static NSURL *available_destination", 1)[0]
        self.assertIn("SDL_PushEvent(&event) != 1", push)
        self.assertIn("SDL_free(event.drop.file)", push)

    def test_ios_user_data_is_in_the_shared_documents_directory(self):
        paths = (ROOT / "crates/taphle/src/paths.rs").read_text()
        host = (ROOT / "platforms/ios/Sources/main.m").read_text()
        self.assertIn("tapHLE_ios_documents_path", paths)
        self.assertIn("NSDocumentDirectory", host)

    def test_stikdebug_request_and_package_prerequisites_are_explicit(self):
        source = (ROOT / "platforms/ios/Sources/jit.m").read_text()
        info = (ROOT / "platforms/ios/Config/Info.plist").read_text()
        entitlements = (ROOT / "platforms/ios/Config/TapHLE.entitlements").read_text()
        self.assertIn('queryItemWithName:@"bundle-id"', source)
        self.assertIn('queryItemWithName:@"pid"', source)
        self.assertIn('queryItemWithName:@"script-name" value:@"universal.js"', source)
        begin = source.split("int tapHLE_ios_jit_begin(void)", 1)[1].split(
            "int tapHLE_ios_jit_poll(void)", 1)[0]
        poll = source.split("int tapHLE_ios_jit_poll(void)", 1)[1].split(
            "void tapHLE_ios_jit_cancel", 1)[0]
        self.assertLess(begin.index("canOpenURL"), begin.index("openURL"))
        self.assertIn("completionHandler:^(BOOL success)", begin)
        self.assertIn("UIApplicationStateActive", poll)
        self.assertIn("!debugged()", poll)
        self.assertIn("<string>stikdebug</string>", info)
        self.assertIn("<key>get-task-allow</key>", entitlements)
        self.assertIn("<true/>", entitlements)


@unittest.skipUnless(sys.platform == "darwin" and shutil.which("clang++"),
                     "requires Darwin VM APIs and clang++")
class IOSJITMemoryTests(unittest.TestCase):
    def test_aliases_relocations_reuse_and_errors(self):
        source = r'''
#include "jit_memory.h"
#include "ios_code_block.hpp"
#include <oaknut/oaknut.hpp>
#include <cassert>
#include <cstring>
#include <sys/mman.h>

static TapHLEJITMemory pool = {nullptr, nullptr, 16384};
static bool leased = false, enabled = false;
static int detached = 0;
extern "C" const char *tapHLE_ios_jit_acquire(size_t size, void **w, void **x) {
    if (!enabled) return "not ready";
    if (leased) return "already leased";
    if (size > pool.size) return "too large";
    leased = true; *w = pool.write; *x = pool.execute; return nullptr;
}
extern "C" void tapHLE_ios_jit_release() { leased = false; }
static void *failed_prepare(void *, size_t) { return nullptr; }
static void *fake_debugger(void *address, size_t size) {
    assert(address == nullptr);
    return mmap(nullptr, size, PROT_READ | PROT_EXEC,
                MAP_PRIVATE | MAP_ANON, -1, 0);
}
static void detach() { ++detached; }
static void destroy_pool() {
    assert(munmap(pool.write, pool.size) == 0);
    assert(munmap(pool.execute, pool.size) == 0);
    pool.write = pool.execute = nullptr;
}
static void exercise() {
    enabled = true;
    for (int run = 0; run < 3; ++run) {
        oaknut::CodeBlock block(pool.size);
        assert(leased);
        bool rejected = false;
        try { oaknut::CodeBlock other(pool.size); }
        catch (const std::runtime_error &) { rejected = true; }
        assert(rejected);
        oaknut::CodeGenerator code(block.writable_ptr(), block.ptr());
        code.NOP();
        code.B(block.ptr() + 4);
        assert(block.ptr()[0] == 0xd503201f);
        assert(block.ptr()[1] == 0x14000003);
        // Repatch a branch using an executable address, as Dynarmic does.
        oaknut::CodeGenerator patch(block.writable_ptr(), block.ptr());
        patch.set_xptr(block.ptr() + 1);
        patch.B(block.ptr());
        assert(block.ptr()[1] == 0x17ffffff);
        block.invalidate_all();
    }
    assert(!leased);
    bool rejected = false;
    try { oaknut::CodeBlock large(pool.size + 4); }
    catch (const std::runtime_error &) { rejected = true; }
    assert(rejected && !leased);
}
int main() {
    bool rejected = false;
    try { oaknut::CodeBlock early(pool.size); }
    catch (const std::runtime_error &e) { rejected = !strcmp(e.what(), "not ready"); }
    assert(rejected);
    char error[512];
    assert(!tapHLE_jit_memory_prepare(&pool, failed_prepare, detach, error));
    assert(detached == 1 && strstr(error, "executable region"));
    assert(!pool.write && !pool.execute);
    assert(tapHLE_jit_memory_prepare(&pool, nullptr, nullptr, error));
    exercise(); destroy_pool();
    assert(tapHLE_jit_memory_prepare(&pool, fake_debugger, detach, error));
    assert(detached == 2);
    exercise(); destroy_pool();
}
'''
        self.compile_and_run(source)

    def test_debugger_region_can_be_inside_a_larger_mapping(self):
        source = r'''
#include "jit_memory.h"
#include <cassert>
#include <sys/mman.h>
static const size_t size = 16384;
static char *region;
static void *prepare(void *, size_t length) {
    assert(length == size);
    region = static_cast<char *>(mmap(nullptr, 3 * size, PROT_READ | PROT_EXEC,
                                     MAP_PRIVATE | MAP_ANON, -1, 0));
    assert(region != MAP_FAILED);
    return region + size;
}
static void detach() {}
int main() {
    TapHLEJITMemory pool = {nullptr, nullptr, size};
    char error[512];
    assert(tapHLE_jit_memory_prepare(&pool, prepare, detach, error));
    assert(pool.execute == region + size);
    assert(munmap(pool.write, size) == 0);
    assert(munmap(region, 3 * size) == 0);
}
'''
        self.compile_and_run(source)

    def test_rejected_debugger_mapping_is_unmapped(self):
        source = r'''
#include "jit_memory.h"
#include <cassert>
#include <cstring>
#include <sys/mman.h>
static void *region;
static int released = 0, detached = 0;
static void *prepare(void *, size_t size) {
    region = mmap(nullptr, size, PROT_READ | PROT_WRITE,
                  MAP_PRIVATE | MAP_ANON, -1, 0);
    assert(region != MAP_FAILED);
    return region;
}
static void *failed(void *, size_t) { return MAP_FAILED; }
static void detach() { ++detached; }
extern "C" int taphle_test_munmap(void *address, size_t size) {
    assert(address != MAP_FAILED && address == region);
    ++released;
    return munmap(address, size);
}
int main() {
    TapHLEJITMemory pool = {nullptr, nullptr, 16384};
    char error[512];
    assert(!tapHLE_jit_memory_prepare(&pool, prepare, detach, error));
    assert(strstr(error, "valid executable region"));
    assert(released == 1 && detached == 1);
    assert(!pool.write && !pool.execute);
    assert(!tapHLE_jit_memory_prepare(&pool, failed, detach, error));
    assert(released == 1 && detached == 2);
    assert(!pool.write && !pool.execute);
}
'''
        self.compile_and_run(source, ["-Dmunmap=taphle_test_munmap"])

    def test_permission_failure_returns_error(self):
        source = r'''
#include "jit_memory.h"
#include <cassert>
#include <cerrno>
#include <cstring>
extern "C" int taphle_test_mprotect(void *, size_t, int) {
    errno = EACCES; return -1;
}
int main() {
    TapHLEJITMemory pool = {nullptr, nullptr, 16384};
    char error[512];
    assert(!tapHLE_jit_memory_prepare(&pool, nullptr, nullptr, error));
    assert(strstr(error, "JIT memory protection failed"));
    assert(!pool.write && !pool.execute);
}
'''
        self.compile_and_run(source, ["-Dmprotect=taphle_test_mprotect"])

    def test_map_failure_returns_error(self):
        source = r'''
#include "jit_memory.h"
#include <cassert>
#include <cerrno>
#include <cstring>
#include <sys/mman.h>
extern "C" void *taphle_test_mmap(void *, size_t, int, int, int, off_t) {
    errno = ENOMEM; return MAP_FAILED;
}
int main() {
    TapHLEJITMemory pool = {nullptr, nullptr, 16384};
    char error[512];
    assert(!tapHLE_jit_memory_prepare(&pool, nullptr, nullptr, error));
    assert(strstr(error, "JIT mmap failed"));
    assert(!pool.write && !pool.execute);
}
'''
        self.compile_and_run(source, ["-Dmmap=taphle_test_mmap"])

    def compile_and_run(self, source, flags=()):
        with tempfile.TemporaryDirectory(prefix="taphle-jit-test-") as folder:
            folder = pathlib.Path(folder)
            test = folder / "test.cpp"
            test.write_text(source)
            native = ROOT / "platforms/ios/Sources"
            includes = ["-I" + str(native),
                        "-I" + str(ROOT / "crates/taphle/src/cpu/dynarmic_wrapper"),
                        "-I" + str(ROOT / "vendor/dynarmic/externals/oaknut/include")]
            subprocess.run(["clang", "-Wall", "-Wextra", "-Werror", *flags,
                            *includes, "-c", str(native / "jit_memory.c"),
                            "-o", str(folder / "memory.o")], check=True,
                           capture_output=True, text=True)
            subprocess.run(["clang++", "-std=c++20", "-Wall", "-Wextra", "-Werror",
                            *includes, str(test), str(folder / "memory.o"),
                            "-o", str(folder / "test")], check=True,
                           capture_output=True, text=True)
            subprocess.run([str(folder / "test")], check=True,
                           capture_output=True, text=True)
