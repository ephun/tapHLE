/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Cross-platform memory management wrappers using the host's system calls.

/// Cross-platform memory allocation using the host's system calls.
/// Returns an address aligned to the guest's 4KB page boundaries.
///
/// - The function returns a raw pointer to allocated memory.
///   The caller is responsible for managing that memory.
/// - The returned pointer must be freed using the corresponding
///   [`free_memory`] call.
#[cfg(windows)]
pub(super) unsafe fn allocate_memory(size: usize) -> std::io::Result<*mut core::ffi::c_void> {
    use windows_sys::Win32::System::Memory::{
        VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE,
    };

    let ptr = unsafe {
        VirtualAlloc(
            std::ptr::null(),
            size,
            MEM_RESERVE | MEM_COMMIT,
            PAGE_READWRITE,
        )
    };

    if ptr.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    Ok(ptr)
}

#[cfg(unix)]
pub(super) unsafe fn allocate_memory(size: usize) -> std::io::Result<*mut core::ffi::c_void> {
    use libc::{mmap, sysconf, MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE};

    // Linux and Android account anonymous writable mappings against commit
    // limits unless MAP_NORESERVE is set. Guest memory covers the 32-bit address
    // space but remains sparse; pages are still faulted in on first write and
    // remain subject to host memory pressure.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const RESERVATION_FLAG: i32 = libc::MAP_NORESERVE;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const RESERVATION_FLAG: i32 = 0;

    const PAGE_SIZE: usize = crate::mem::PAGE_SIZE as usize;
    let host_page_size = unsafe { sysconf(libc::_SC_PAGESIZE) as usize };

    assert!(
        host_page_size >= PAGE_SIZE,
        "Hosts with smaller than 4KiB pages are not supported."
    );

    let ptr = unsafe {
        mmap(
            std::ptr::null_mut(),
            size,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS | RESERVATION_FLAG,
            -1,
            0,
        )
    };

    if ptr == libc::MAP_FAILED {
        return Err(std::io::Error::last_os_error());
    }
    Ok(ptr)
}

/// Cross-platform memory free using the host's system calls.
///
/// # Safety
/// - The address and size should match parameters and result of the
///   [`allocate_memory`] call.
#[cfg(windows)]
pub(super) unsafe fn free_memory(
    address: *mut core::ffi::c_void,
    _size: usize,
) -> std::io::Result<()> {
    use windows_sys::Win32::System::Memory::{VirtualFree, MEM_RELEASE};

    let res = unsafe { VirtualFree(address, 0, MEM_RELEASE) };

    if res == 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(unix)]
pub(super) unsafe fn free_memory(
    address: *mut core::ffi::c_void,
    size: usize,
) -> std::io::Result<()> {
    use libc::munmap;

    let res = unsafe { munmap(address, size) };

    if res == -1 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(all(
    test,
    target_pointer_width = "64",
    any(target_os = "linux", target_os = "android")
))]
mod tests {
    #[test]
    fn sparse_guest_address_space_can_be_reserved() {
        let size = std::mem::size_of::<crate::mem::Bytes>();
        let ptr = unsafe { super::allocate_memory(size) }.expect("reserve guest address space");
        let bytes = ptr.cast::<u8>();
        unsafe {
            *bytes = 0xA5;
            *bytes.add(size - crate::mem::PAGE_SIZE as usize) = 0x5A;
            assert_eq!(*bytes, 0xA5);
            assert_eq!(*bytes.add(size - crate::mem::PAGE_SIZE as usize), 0x5A);
            super::free_memory(ptr, size).expect("release guest address space");
        }
    }
}
