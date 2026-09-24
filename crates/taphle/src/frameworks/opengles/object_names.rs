/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Guest framebuffer names must not depend on SDL's drawable allocations.

use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct ObjectNames {
    guest_to_host: BTreeMap<u32, u32>,
    host_to_guest: BTreeMap<u32, u32>,
}

impl ObjectNames {
    pub fn generate(&mut self, host: u32) -> u32 {
        if host == 0 {
            return 0;
        }
        let mut guest = 1;
        while self.guest_to_host.contains_key(&guest) {
            guest = guest.checked_add(1).expect("GL object namespace exhausted");
        }
        self.guest_to_host.insert(guest, host);
        self.host_to_guest.insert(host, guest);
        guest
    }

    pub fn host(&self, guest: u32) -> u32 {
        self.guest_to_host.get(&guest).copied().unwrap_or(0)
    }

    pub fn guest(&self, host: u32) -> u32 {
        self.host_to_guest.get(&host).copied().unwrap_or(0)
    }

    pub fn bind(&mut self, guest: u32, allocate: impl FnOnce() -> u32) -> u32 {
        if guest == 0 {
            return 0;
        }
        let host = *self.guest_to_host.entry(guest).or_insert_with(allocate);
        self.host_to_guest.insert(host, guest);
        host
    }

    pub fn remove(&mut self, guest: u32) -> u32 {
        // Another context or framebuffer may still reference a deleted object.
        // Keep its reverse name until the host reuses that name or the whole
        // sharegroup is destroyed.
        self.guest_to_host.remove(&guest).unwrap_or(0)
    }
}

#[derive(Default)]
pub(super) struct FramebufferObjects {
    pub framebuffers: ObjectNames,
    pub renderbuffers: ObjectNames,
}

#[derive(Default)]
pub(super) struct FramebufferBindings {
    pub framebuffer: u32,
    pub renderbuffer: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_drawables_do_not_shift_guest_names() {
        let mut names = FramebufferObjects::default();
        assert_eq!(names.framebuffers.generate(2), 1);
        assert_eq!(names.renderbuffers.generate(3), 1);
        assert_eq!(names.framebuffers.host(1), 2);
        assert_eq!(names.renderbuffers.host(1), 3);
        assert_eq!(names.renderbuffers.guest(3), 1);
        assert_eq!(names.renderbuffers.guest(1), 0);
    }

    #[test]
    fn explicit_names_deletion_and_zero_stay_isolated() {
        let mut names = ObjectNames::default();
        assert_eq!(names.bind(0, || panic!("must not allocate zero")), 0);
        assert_eq!(names.bind(7, || 42), 42);
        assert_eq!(names.bind(7, || panic!("must reuse binding")), 42);
        assert_eq!(names.generate(43), 1);
        assert_eq!(names.remove(99), 0);
        assert_eq!(names.remove(7), 42);
        assert_eq!(names.host(7), 0);
        assert_eq!(names.bind(7, || 44), 44);
        assert_eq!(names.guest(44), 7);
    }
}
