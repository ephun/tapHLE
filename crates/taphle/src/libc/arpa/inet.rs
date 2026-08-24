/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `arpa/inet.h` (Internet address manipulation routines)

use crate::libc::errno::{set_errno, EAFNOSUPPORT, ENOSPC};
use crate::libc::netdb::socklen_t;
use crate::libc::sys::socket::AF_INET;
use crate::mem::{ConstPtr, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr, Ptr, SafeRead};
use crate::{export_c_func, Environment};

use crate::dyld::FunctionExports;
use std::net::Ipv4Addr;

#[allow(non_camel_case_types)]
type in_addr_t = u32;

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
#[allow(non_camel_case_types)]
struct in_addr {
    s_addr: in_addr_t,
}
unsafe impl SafeRead for in_addr {}

/// The value `inet_addr` returns for an address it cannot parse. Note that this
/// is also the valid address 255.255.255.255; the C API does not distinguish
/// them, which is why `inet_aton` exists.
const INADDR_NONE: in_addr_t = 0xFFFF_FFFF;

/// Parse a dotted-quad. Returns `None` for anything malformed, which callers
/// must report rather than treat as fatal: passing an unparseable string to
/// these functions is an ordinary runtime occurrence, not a guest bug. Apps
/// routinely call them on hostnames or on empty strings to find out whether
/// they need a DNS lookup, and expect the documented failure return.
fn parse_ipv4(env: &Environment, str: ConstPtr<u8>) -> Option<Ipv4Addr> {
    env.mem.cstr_at_utf8(str).ok()?.parse().ok()
}

fn inet_addr(env: &mut Environment, str: ConstPtr<u8>) -> in_addr_t {
    let Some(address) = parse_ipv4(env, str) else {
        log_dbg!("inet_addr({:?}) => INADDR_NONE", str);
        return INADDR_NONE;
    };
    let res = u32::from_le_bytes(address.octets());
    log_dbg!("inet_addr({:?}) => {}", str, res);
    res
}

fn inet_ntop(
    env: &mut Environment,
    af: i32,
    src: ConstVoidPtr,
    dst: MutPtr<u8>,
    size: socklen_t,
) -> ConstPtr<u8> {
    if af != AF_INET {
        set_errno(env, EAFNOSUPPORT);
        return Ptr::null();
    }
    let addr_ptr: ConstPtr<in_addr> = src.cast();
    let addr = env.mem.read(addr_ptr);
    let ipv4_addr = Ipv4Addr::from_bits(u32::from_be(addr.s_addr));
    log_dbg!("inet_ntop: addr = {:?}", ipv4_addr);
    let binding = ipv4_addr.to_string();
    let addr_bytes = binding.as_bytes();
    let len: GuestUSize = addr_bytes.len().try_into().unwrap();
    if len >= size {
        set_errno(env, ENOSPC);
        return Ptr::null();
    }
    env.mem.bytes_at_mut(dst, len).copy_from_slice(addr_bytes);
    env.mem.write(dst + len, b'\0');
    dst.cast_const()
}

fn inet_pton(env: &mut Environment, af: i32, src: ConstPtr<u8>, dst: MutVoidPtr) -> i32 {
    if af != AF_INET {
        set_errno(env, EAFNOSUPPORT);
        return -1;
    }
    let Some(address) = parse_ipv4(env, src) else {
        log_dbg!("inet_pton({:?}) => 0 (not a valid address)", src);
        return 0; // not an error: the string simply is not an address
    };
    let addr = in_addr {
        s_addr: u32::from_le_bytes(address.octets()),
    };
    let addr_ptr: MutPtr<in_addr> = dst.cast();
    env.mem.write(addr_ptr, addr);
    1 // address was valid, success
}

/// `inet_aton` is the variant that reports failure separately from the value,
/// so unlike `inet_addr` it can accept 255.255.255.255.
fn inet_aton(env: &mut Environment, str: ConstPtr<u8>, addr_ptr: MutPtr<in_addr>) -> i32 {
    let Some(address) = parse_ipv4(env, str) else {
        log_dbg!("inet_aton({:?}) => 0 (not a valid address)", str);
        return 0;
    };
    if !addr_ptr.is_null() {
        let addr = in_addr {
            s_addr: u32::from_le_bytes(address.octets()),
        };
        env.mem.write(addr_ptr, addr);
    }
    1
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(inet_addr(_)),
    export_c_func!(inet_aton(_, _)),
    export_c_func!(inet_ntop(_, _, _, _)),
    export_c_func!(inet_pton(_, _, _)),
];
