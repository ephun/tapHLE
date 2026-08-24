/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `Mach-O` related functions.

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstVoidPtr, GuestUSize, MutPtr, Ptr};
use crate::Environment;

fn get_end(env: &mut Environment) -> u32 {
    // Assume app binary is the first.
    // From https://www.manpagez.com/man/3/get_end/
    // `In a Mach-O file <...> get_end returns the first address after
    // the last segment in the executable`
    // It was confirmed on a real device with the TestApp binary.
    env.bins[0].last_segment_end
}

fn get_etext(env: &mut Environment) -> u32 {
    // Assume app binary is the first.
    let app_sections = &env.bins[0].sections;
    assert_eq!(
        app_sections
            .iter()
            .filter(|s| s.name.to_uppercase() == "__TEXT")
            .count(),
        1
    );
    let text_section = app_sections
        .iter()
        .find(|s| s.name.to_uppercase() == "__TEXT")
        .unwrap();
    text_section.next_section_addr()
}

/// `int _NSGetExecutablePath(char *buf, uint32_t *bufsize)` from
/// `<mach-o/dyld.h>`. Copies the executable's path (NUL-terminated) into `buf`.
/// Apps use it to locate their own bundle. On success it returns 0 and leaves
/// `*bufsize` unchanged; if the buffer is too small it returns -1 and stores
/// the required size (including the NUL) in `*bufsize`, matching the real dyld.
fn _NSGetExecutablePath(env: &mut Environment, buf: MutPtr<u8>, bufsize: MutPtr<u32>) -> i32 {
    let path = env.bundle.executable_path().as_str().to_string();
    let bytes = path.as_bytes();
    let needed = bytes.len() as u32 + 1;
    let available = env.mem.read(bufsize);
    if needed > available {
        env.mem.write(bufsize, needed);
        return -1;
    }
    let dst = env.mem.bytes_at_mut(buf, needed);
    dst[..bytes.len()].copy_from_slice(bytes);
    dst[bytes.len()] = 0;
    0
}

/// `_dyld_image_count` — how many Mach-O images are loaded.
///
/// tapHLE loads the executable and its dylibs into one list, which is exactly
/// what dyld reports, so this is that list's length.
fn _dyld_image_count(env: &mut Environment) -> u32 {
    env.bins.len().try_into().unwrap()
}

/// `_dyld_get_image_header` — the `mach_header` of the image at `image_index`.
///
/// The header is the first thing in the `__TEXT` segment, so the segment's load
/// address is the header's address. Callers use it to walk load commands, most
/// often to find a section or read the UUID.
///
/// An out-of-range index returns NULL, which is what dyld does and what a
/// caller looping until NULL depends on.
fn _dyld_get_image_header(env: &mut Environment, image_index: u32) -> ConstVoidPtr {
    match env.bins.get(image_index as usize) {
        Some(bin) => Ptr::from_bits(bin.text_segment_base),
        None => Ptr::null(),
    }
}

/// `_dyld_get_image_vmaddr_slide` — how far an image was moved from its
/// preferred load address by ASLR.
///
/// tapHLE loads each image at the address its Mach-O headers ask for, so the
/// slide is genuinely zero rather than merely unknown. Code calling this is
/// normally converting a link-time address into a runtime one, and zero is the
/// correct conversion here.
fn _dyld_get_image_vmaddr_slide(_env: &mut Environment, _image_index: u32) -> GuestUSize {
    0
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(_dyld_get_image_vmaddr_slide(_)),
    export_c_func!(_dyld_image_count()),
    export_c_func!(_dyld_get_image_header(_)),
    export_c_func!(get_end()),
    export_c_func!(get_etext()),
    export_c_func!(_NSGetExecutablePath(_, _)),
];
