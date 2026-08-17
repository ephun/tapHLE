/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `dlfcn.h` (`dlopen()` and friends)

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstPtr, ConstVoidPtr, MutVoidPtr, Ptr};
use crate::Environment;

const RTLD_DEFAULT: MutVoidPtr = Ptr::from_bits(-2 as _);

fn is_known_library(path: &str) -> bool {
    crate::dyld::DYLIB_LIST
        .iter()
        .any(|dylib| dylib.path == path || dylib.aliases.contains(&path))
}

fn dlopen(env: &mut Environment, path: ConstPtr<u8>, _mode: i32) -> MutVoidPtr {
    if path.is_null() {
        return RTLD_DEFAULT;
    }
    // TODO: dlopen() support for real dynamic libraries.
    // A library that cannot be opened is reported as a null handle, which is
    // dlopen's own contract and the only answer a caller is written to handle.
    // Apps use dlopen to ask whether an optional framework is present, so being
    // told no is a normal outcome rather than a failure of the app.
    let Ok(path_str) = env.mem.cstr_at_utf8(path) else {
        log!("dlopen() path is not valid UTF-8, returning NULL");
        return Ptr::null();
    };
    if !is_known_library(path_str) {
        log!(
            "dlopen({:?}) names a library tapHLE does not have, returning NULL",
            path_str
        );
        return Ptr::null();
    }
    // For convenience, use the path as the handle.
    // TODO: Find out whether the handle is truly opaque on iPhone OS, and if
    // not, where it points.
    path.cast_mut().cast()
}

fn dlsym(env: &mut Environment, handle: MutVoidPtr, symbol: ConstPtr<u8>) -> MutVoidPtr {
    // A handle dlopen() never issued, most often the NULL from a library it
    // could not open. There is nothing to look the symbol up in, and a null
    // result is what dlsym reports for a symbol it cannot find.
    if handle != RTLD_DEFAULT
        && !env
            .mem
            .cstr_at_utf8(handle.cast())
            .is_ok_and(is_known_library)
    {
        log!("dlsym() called with a handle no library owns, returning NULL");
        return Ptr::null();
    }
    // For some reason, the symbols passed to dlsym() don't have the leading _.
    let symbol = format!("_{}", env.mem.cstr_at_utf8(symbol).unwrap());

    // A null path passed to dlopen() produces RTLD_DEFAULT, whose lookup scope
    // includes the main executable and its loaded libraries. This is commonly
    // used by game engines to find callbacks compiled into the app itself.
    if handle == RTLD_DEFAULT {
        if let Some(addr) = env
            .bins
            .iter()
            .find_map(|bin| bin.exported_symbols.get(&symbol).copied())
        {
            return Ptr::from_bits(addr);
        }
    }

    // TODO: Symbol lookup should be scoped to the specific library requested,
    // where appropriate!
    let Ok(addr) = env
        .dyld
        .create_proc_address(&mut env.mem, &mut env.cpu, &symbol)
    else {
        log_dbg!("dlsym() could not resolve {symbol}");
        return Ptr::null();
    };
    Ptr::from_bits(addr.addr_with_thumb_bit())
}

fn dlclose(env: &mut Environment, handle: MutVoidPtr) -> i32 {
    assert!(
        handle == RTLD_DEFAULT || is_known_library(env.mem.cstr_at_utf8(handle.cast()).unwrap())
    );
    0 // success
}

// tapHLE does not yet expose host or guest image metadata for arbitrary
// addresses. Report lookup failure, which is the documented result when the
// address cannot be associated with a loaded image.
fn dladdr(_env: &mut Environment, _addr: ConstVoidPtr, _info: MutVoidPtr) -> i32 {
    0
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(dlopen(_, _)),
    export_c_func!(dlsym(_, _)),
    export_c_func!(dlclose(_)),
    export_c_func!(dladdr(_, _)),
];
