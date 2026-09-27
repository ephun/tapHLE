/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
#pragma once

#include <cstddef>
#include <cstdint>
#include <libkern/OSCacheControl.h>
#include <stdexcept>

extern "C" const char *tapHLE_ios_jit_acquire(size_t, void **, void **);
extern "C" void tapHLE_ios_jit_release(void);

namespace oaknut {
// The host prepares this pool before constructing any guest CPU. Keep the
// executable mapping alive between runs: after the debugger detaches, another
// allocation would require another preparation session on TXM devices.
class CodeBlock {
public:
    explicit CodeBlock(std::size_t size) : m_size(size) {
        void *write = nullptr, *execute = nullptr;
        if (const char *error = tapHLE_ios_jit_acquire(size, &write, &execute))
            throw std::runtime_error(error);
        m_write = static_cast<std::uint32_t *>(write);
        m_execute = static_cast<std::uint32_t *>(execute);
    }
    ~CodeBlock() { tapHLE_ios_jit_release(); }
    CodeBlock(const CodeBlock &) = delete;
    CodeBlock &operator=(const CodeBlock &) = delete;
    std::uint32_t *ptr() const { return m_execute; }
    std::uint32_t *writable_ptr() const { return m_write; }
    // Both mappings retain their permissions. Never turn RX into RW: TXM
    // preparation applies to the executable view, not to a writable mapping.
    void protect() {}
    void unprotect() {}
    void invalidate(std::uint32_t *address, std::size_t size) {
        sys_icache_invalidate(address, size);
    }
    void invalidate_all() { invalidate(m_execute, m_size); }

private:
    std::uint32_t *m_write;
    std::uint32_t *m_execute;
    std::size_t m_size;
};
} // namespace oaknut
