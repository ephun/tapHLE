/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `ifaddrs.h` (interface addresses)
//!
//! Apps reach for this to answer one of two questions: what is my own IP
//! address, and is there a network at all. The interfaces reported here are
//! synthetic and deterministic, matching the set [crate::libc::net::if_]
//! indexes and the `en0` that [crate::libc::sysctl] describes. Exposing the
//! host computer's real adapters would be both a privacy leak and a source of
//! run-to-run variation in a game's behaviour.

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::libc::errno::set_errno;
use crate::libc::sys::socket::sockaddr;
use crate::mem::{MutPtr, MutVoidPtr, Ptr, SafeRead};
use crate::Environment;

/// Darwin's 32-bit `struct ifaddrs`: seven words, all pointers except the
/// flags.
#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
#[allow(non_camel_case_types)]
pub struct ifaddrs {
    ifa_next: MutPtr<ifaddrs>,
    ifa_name: MutPtr<u8>,
    ifa_flags: u32,
    ifa_addr: MutPtr<sockaddr>,
    ifa_netmask: MutPtr<sockaddr>,
    /// The broadcast address on a broadcast interface, or the peer's address on
    /// a point-to-point one; the union is `ifa_broadaddr` under its other name.
    ifa_dstaddr: MutPtr<sockaddr>,
    ifa_data: MutVoidPtr,
}
unsafe impl SafeRead for ifaddrs {}

// Interface flags from net/if.h, the same subset src/libc/sysctl.rs reports for
// en0 through the routing socket.
const IFF_UP: u32 = 0x1;
const IFF_BROADCAST: u32 = 0x2;
const IFF_LOOPBACK: u32 = 0x8;
const IFF_RUNNING: u32 = 0x40;
const IFF_SIMPLEX: u32 = 0x800;
const IFF_MULTICAST: u32 = 0x8000;

/// One synthetic interface: everything needed to build its `ifaddrs` node.
struct Interface {
    name: &'static [u8],
    flags: u32,
    address: [u8; 4],
    netmask: [u8; 4],
    /// Broadcast address, for the interfaces that have one.
    broadcast: Option<[u8; 4]>,
}

/// The interface list, in the order `getifaddrs` reports it.
///
/// A device has `lo0` and, on WiFi, `en0`; `pdp_ip0` exists only on a cellular
/// connection and is deliberately absent, because the reachability
/// implementation in [crate::frameworks::system_configuration] tells guests
/// the network is direct rather than WWAN, and the two answers should not
/// contradict each other.
///
/// The `en0` address is a fixed private one. It is not the host's, and a game
/// that publishes it to a peer is publishing something that will not route —
/// which is the honest state of affairs, since tapHLE's sockets live on the
/// host's stack rather than on this fictional interface.
fn interfaces() -> [Interface; 2] {
    [
        Interface {
            name: b"lo0",
            flags: IFF_UP | IFF_LOOPBACK | IFF_RUNNING | IFF_MULTICAST,
            address: [127, 0, 0, 1],
            netmask: [255, 0, 0, 0],
            broadcast: None,
        },
        Interface {
            name: b"en0",
            flags: IFF_UP | IFF_BROADCAST | IFF_RUNNING | IFF_SIMPLEX | IFF_MULTICAST,
            address: [192, 168, 1, 2],
            netmask: [255, 255, 255, 0],
            broadcast: Some([192, 168, 1, 255]),
        },
    ]
}

/// Allocate a `sockaddr_in` for an IPv4 address with no port.
///
/// `struct sockaddr_in` and `struct sockaddr` are the same 16 bytes, and
/// [sockaddr::from_ipv4_parts] already writes that layout: `sin_len`,
/// `sin_family`, `sin_port`, then `sin_addr`.
fn alloc_sockaddr(env: &mut Environment, octets: [u8; 4]) -> MutPtr<sockaddr> {
    env.mem
        .alloc_and_write(sockaddr::from_ipv4_parts(octets, 0))
}

/// Read the device's network interfaces into a freshly allocated linked list.
///
/// The caller owns the list and must release it with [freeifaddrs]; that
/// ownership is why every piece of it — nodes, names and addresses — is a
/// separate guest allocation rather than one block.
fn getifaddrs(env: &mut Environment, ifap: MutPtr<MutPtr<ifaddrs>>) -> i32 {
    set_errno(env, 0);

    if ifap.is_null() {
        // The real call faults here. Refusing is closer to that than writing
        // through a null pointer, and an app that does this has a bug tapHLE
        // should not hide by succeeding.
        log!("getifaddrs() called with a null out-parameter; reporting failure");
        return -1;
    }

    // Built back to front so each node can point at the one after it.
    let mut head: MutPtr<ifaddrs> = Ptr::null();
    for interface in interfaces().iter().rev() {
        let name = env.mem.alloc_and_write_cstr(interface.name);
        let address = alloc_sockaddr(env, interface.address);
        let netmask = alloc_sockaddr(env, interface.netmask);
        let dstaddr = match interface.broadcast {
            Some(broadcast) => alloc_sockaddr(env, broadcast),
            None => Ptr::null(),
        };
        head = env.mem.alloc_and_write(ifaddrs {
            ifa_next: head,
            ifa_name: name,
            ifa_flags: interface.flags,
            ifa_addr: address,
            ifa_netmask: netmask,
            ifa_dstaddr: dstaddr,
            ifa_data: Ptr::null(),
        });
    }

    env.mem.write(ifap, head);
    log_dbg!("getifaddrs({:?}) => 0, list at {:?}", ifap, head);
    0
}

/// Release a list from [getifaddrs].
///
/// A null argument is a no-op. That is not a defensive guard: the canonical
/// Apple sample for finding the device's own address declares its list pointer,
/// calls `getifaddrs`, and calls this unconditionally afterwards, so the null
/// case is reached whenever the query fails. 314 of the 1192 apps in the
/// import-demand catalogue import this function, and before it existed every
/// one of them would have died on that cleanup line.
///
/// Darwin allocates the whole list as one block and frees it with a single
/// `free`, whereas [getifaddrs] here allocates each piece separately, so this
/// walks the list. The visible difference is narrow but real: an app that calls
/// `free` on the list head directly instead of calling this leaks the names and
/// addresses rather than releasing everything.
fn freeifaddrs(env: &mut Environment, ifa: MutPtr<ifaddrs>) {
    let mut node = ifa;
    while !node.is_null() {
        let ifaddrs {
            ifa_next,
            ifa_name,
            ifa_addr,
            ifa_netmask,
            ifa_dstaddr,
            ..
        } = env.mem.read(node);
        for allocation in [
            ifa_name.cast(),
            ifa_addr.cast(),
            ifa_netmask.cast(),
            ifa_dstaddr.cast(),
        ] {
            let allocation: MutVoidPtr = allocation;
            if !allocation.is_null() {
                env.mem.free(allocation);
            }
        }
        env.mem.free(node.cast());
        node = ifa_next;
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(getifaddrs(_)),
    export_c_func!(freeifaddrs(_)),
];

#[cfg(test)]
mod tests {
    use super::{ifaddrs, interfaces, IFF_BROADCAST, IFF_LOOPBACK};
    use crate::mem::guest_size_of;

    #[test]
    fn ifaddrs_uses_the_32_bit_darwin_layout() {
        // Seven words. A wrong size would make the guest read the next node's
        // fields as this one's.
        assert_eq!(guest_size_of::<ifaddrs>(), 28);
    }

    #[test]
    fn the_interface_list_is_loopback_then_en0() {
        let interfaces = interfaces();
        assert_eq!(interfaces[0].name, b"lo0");
        assert_eq!(interfaces[0].address, [127, 0, 0, 1]);
        assert!(interfaces[0].flags & IFF_LOOPBACK != 0);
        assert!(interfaces[0].broadcast.is_none());

        assert_eq!(interfaces[1].name, b"en0");
        assert!(interfaces[1].flags & IFF_LOOPBACK == 0);
        assert!(interfaces[1].flags & IFF_BROADCAST != 0);
        // A broadcast interface must supply the broadcast address, since that
        // is what the ifa_dstaddr union means when IFF_BROADCAST is set.
        assert!(interfaces[1].broadcast.is_some());
    }

    #[test]
    fn no_reported_address_is_the_hosts() {
        // The whole set is fixed constants; this states the property so that
        // adding a host lookup here has to argue with a failing test.
        for interface in interfaces() {
            assert!(
                interface.address[0] == 127 || interface.address == [192, 168, 1, 2],
                "unexpected address {:?}",
                interface.address
            );
        }
    }
}
