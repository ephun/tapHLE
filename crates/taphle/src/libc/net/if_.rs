/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `net/if.h`

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::libc::errno::{set_errno, EINVAL, ENXIO};
use crate::mem::{ConstPtr, Ptr};
use crate::Environment;

// TODO: struct definition
#[allow(non_camel_case_types)]
struct if_nameindex {}

fn if_nameindex(_env: &mut Environment) -> ConstPtr<if_nameindex> {
    // TODO: implement
    Ptr::null()
}

fn interface_index(name: &str) -> Option<u32> {
    // Present a small, deterministic iPhone-like interface set. Do not leak
    // the host computer's adapter names or configuration into the guest.
    match name {
        "lo0" => Some(1),
        "en0" => Some(2),
        "pdp_ip0" => Some(3),
        _ => None,
    }
}

fn if_nametoindex(env: &mut Environment, name: ConstPtr<u8>) -> u32 {
    let Ok(name) = env.mem.cstr_at_utf8(name) else {
        set_errno(env, EINVAL);
        return 0;
    };
    let index = interface_index(name);
    log_dbg!("if_nametoindex({name:?}) -> {index:?}");
    match index {
        Some(index) => index,
        None => {
            set_errno(env, ENXIO);
            0
        }
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(if_nameindex()),
    export_c_func!(if_nametoindex(_)),
];

#[cfg(test)]
mod tests {
    use super::interface_index;

    #[test]
    fn interface_names_are_deterministic_and_private() {
        assert_eq!(interface_index("lo0"), Some(1));
        assert_eq!(interface_index("en0"), Some(2));
        assert_eq!(interface_index("pdp_ip0"), Some(3));
        assert_eq!(interface_index("Ethernet 7"), None);
    }
}
