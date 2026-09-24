"""Exercise the actual iOS memory allocator on Darwin without a device debugger."""
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]


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
