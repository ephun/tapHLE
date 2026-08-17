/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `string.h`

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstPtr, ConstVoidPtr, GuestUSize, Mem, MutPtr, MutVoidPtr, Ptr};
use crate::Environment;
use std::cmp::Ordering;

use super::generic_char::GenericChar;

#[derive(Default)]
pub struct State {
    strtok: Option<MutPtr<u8>>,
}

fn strtok(env: &mut Environment, s: MutPtr<u8>, sep: ConstPtr<u8>) -> MutPtr<u8> {
    let s = if s.is_null() {
        let state = env.libc_state.string.strtok.unwrap();
        if state.is_null() {
            env.libc_state.string.strtok = None;
            return Ptr::null();
        }
        state
    } else {
        s
    };

    let sep = env.mem.cstr_at(sep);

    let mut token_start = s;
    loop {
        let c = env.mem.read(token_start);
        if c == b'\0' {
            env.libc_state.string.strtok = None;
            return Ptr::null();
        } else if sep.contains(&c) {
            token_start += 1;
        } else {
            break;
        }
    }

    let mut token_end = token_start;
    let next_token = loop {
        let c = env.mem.read(token_end);
        if sep.contains(&c) {
            env.mem.write(token_end, b'\0');
            break token_end + 1;
        } else if c == b'\0' {
            break Ptr::null();
        } else {
            token_end += 1;
        }
    };

    env.libc_state.string.strtok = Some(next_token);

    token_start
}

/// `strtok_r` — the reentrant `strtok`. The only difference is where the "rest
/// of the string" cursor lives: the caller owns it, so two interleaved
/// tokenisations do not corrupt each other. That is also why it cannot simply
/// delegate to `strtok`, whose cursor is global.
fn strtok_r(
    env: &mut Environment,
    s: MutPtr<u8>,
    sep: ConstPtr<u8>,
    saveptr: MutPtr<MutPtr<u8>>,
) -> MutPtr<u8> {
    let start = if s.is_null() {
        env.mem.read(saveptr)
    } else {
        s
    };
    if start.is_null() {
        return Ptr::null();
    }

    let separators = env.mem.cstr_at(sep).to_vec();

    // Skip leading separators; a string of nothing but separators has no token.
    let mut token_start = start;
    loop {
        let c = env.mem.read(token_start);
        if c == b'\0' {
            env.mem.write(saveptr, Ptr::null());
            return Ptr::null();
        } else if separators.contains(&c) {
            token_start += 1;
        } else {
            break;
        }
    }

    let mut token_end = token_start;
    let next_token = loop {
        let c = env.mem.read(token_end);
        if separators.contains(&c) {
            env.mem.write(token_end, b'\0');
            break token_end + 1;
        } else if c == b'\0' {
            break Ptr::null();
        } else {
            token_end += 1;
        }
    };

    env.mem.write(saveptr, next_token);
    token_start
}

// Functions shared with wchar.rs

fn bzero(env: &mut Environment, dest: MutVoidPtr, count: GuestUSize) {
    memset(env, dest, 0, count);
}
fn memset(env: &mut Environment, dest: MutVoidPtr, ch: i32, count: GuestUSize) -> MutVoidPtr {
    GenericChar::<u8>::memset(env, dest.cast(), ch as u8, count, GuestUSize::MAX).cast()
}
fn __memset_chk(
    env: &mut Environment,
    dest: MutVoidPtr,
    ch: i32,
    count: GuestUSize,
    dest_count: GuestUSize,
) -> MutVoidPtr {
    GenericChar::<u8>::memset(env, dest.cast(), ch as u8, count, dest_count).cast()
}
fn memset_pattern4(env: &mut Environment, b: MutVoidPtr, pattern4: ConstVoidPtr, len: GuestUSize) {
    memset_pattern_inner(env, b, pattern4, len, 4)
}
fn memset_pattern8(env: &mut Environment, b: MutVoidPtr, pattern8: ConstVoidPtr, len: GuestUSize) {
    memset_pattern_inner(env, b, pattern8, len, 8)
}
fn memset_pattern16(
    env: &mut Environment,
    b: MutVoidPtr,
    pattern16: ConstVoidPtr,
    len: GuestUSize,
) {
    memset_pattern_inner(env, b, pattern16, len, 16)
}
fn memset_pattern_inner(
    env: &mut Environment,
    b: MutVoidPtr,
    pattern: ConstVoidPtr,
    len: GuestUSize,
    pattern_len: GuestUSize,
) {
    assert!(matches!(pattern_len, 4 | 8 | 16));
    let mut tmp = [0; 16];
    tmp[..pattern_len as usize].copy_from_slice(env.mem.bytes_at(pattern.cast(), pattern_len));
    let mut target: MutPtr<u8> = b.cast();
    for _ in 0..(len / pattern_len) {
        env.mem
            .bytes_at_mut(target, pattern_len)
            .copy_from_slice(&tmp[..pattern_len as usize]);
        target += pattern_len;
    }
    for i in 0..(len % pattern_len) {
        let value = env.mem.read(pattern.cast() + i);
        env.mem.write(target, value);
        target += 1;
    }
}
fn memcpy(
    env: &mut Environment,
    dest: MutVoidPtr,
    src: ConstVoidPtr,
    size: GuestUSize,
) -> MutVoidPtr {
    GenericChar::<u8>::memcpy(env, dest.cast(), src.cast(), size, GuestUSize::MAX).cast()
}
fn __memcpy_chk(
    env: &mut Environment,
    dest: MutVoidPtr,
    src: ConstVoidPtr,
    size: GuestUSize,
    dest_size: GuestUSize,
) -> MutVoidPtr {
    GenericChar::<u8>::memcpy(env, dest.cast(), src.cast(), size, dest_size).cast()
}
fn memmove(
    env: &mut Environment,
    dest: MutVoidPtr,
    src: ConstVoidPtr,
    size: GuestUSize,
) -> MutVoidPtr {
    GenericChar::<u8>::memmove(env, dest.cast(), src.cast(), size, GuestUSize::MAX).cast()
}
fn __memmove_chk(
    env: &mut Environment,
    dest: MutVoidPtr,
    src: ConstVoidPtr,
    size: GuestUSize,
    dest_size: GuestUSize,
) -> MutVoidPtr {
    GenericChar::<u8>::memmove(env, dest.cast(), src.cast(), size, dest_size).cast()
}
fn memchr(env: &mut Environment, string: ConstVoidPtr, c: i32, size: GuestUSize) -> ConstVoidPtr {
    GenericChar::<u8>::memchr(env, string.cast(), c as u8, size).cast()
}
fn memcmp(env: &mut Environment, a: ConstVoidPtr, b: ConstVoidPtr, size: GuestUSize) -> i32 {
    GenericChar::<u8>::memcmp(env, a.cast(), b.cast(), size)
}
pub(crate) fn strlen(env: &mut Environment, s: ConstPtr<u8>) -> GuestUSize {
    GenericChar::<u8>::strlen(env, s)
}
pub(super) fn strcpy(env: &mut Environment, dest: MutPtr<u8>, src: ConstPtr<u8>) -> MutPtr<u8> {
    GenericChar::<u8>::strcpy(env, dest, src, GuestUSize::MAX)
}
fn __strcpy_chk(
    env: &mut Environment,
    dest: MutPtr<u8>,
    src: ConstPtr<u8>,
    size: GuestUSize,
) -> MutPtr<u8> {
    GenericChar::<u8>::strcpy(env, dest, src, size)
}
fn strcat(env: &mut Environment, dest: MutPtr<u8>, src: ConstPtr<u8>) -> MutPtr<u8> {
    GenericChar::<u8>::strcat(env, dest, src, GuestUSize::MAX)
}
fn __strcat_chk(
    env: &mut Environment,
    dest: MutPtr<u8>,
    src: ConstPtr<u8>,
    size: GuestUSize,
) -> MutPtr<u8> {
    GenericChar::<u8>::strcat(env, dest, src, size)
}
fn strspn(env: &mut Environment, s: ConstPtr<u8>, charset: ConstPtr<u8>) -> GuestUSize {
    GenericChar::<u8>::strspn(env, s, charset)
}
fn strcspn(env: &mut Environment, s: ConstPtr<u8>, charset: ConstPtr<u8>) -> GuestUSize {
    GenericChar::<u8>::strcspn(env, s, charset)
}
pub(crate) fn strncpy(
    env: &mut Environment,
    dest: MutPtr<u8>,
    src: ConstPtr<u8>,
    size: GuestUSize,
) -> MutPtr<u8> {
    GenericChar::<u8>::strncpy(env, dest, src, size, GuestUSize::MAX)
}
fn __strncpy_chk(
    env: &mut Environment,
    dest: MutPtr<u8>,
    src: ConstPtr<u8>,
    size: GuestUSize,
    dest_size: GuestUSize,
) -> MutPtr<u8> {
    GenericChar::<u8>::strncpy(env, dest, src, size, dest_size)
}
fn strsep(env: &mut Environment, stringp: MutPtr<MutPtr<u8>>, delim: ConstPtr<u8>) -> MutPtr<u8> {
    let orig = env.mem.read(stringp);
    if orig.is_null() {
        return Ptr::null();
    }
    let tmp = orig;
    let mut i = 0;
    loop {
        let c = env.mem.read(tmp + i);
        if c == b'\0' {
            env.mem.write(stringp, Ptr::null());
            break;
        }
        let mut j = 0;
        loop {
            let cc = env.mem.read(delim + j);
            if c == cc {
                env.mem.write(tmp + i, b'\0');
                env.mem.write(stringp, tmp + i + 1);
                return orig;
            }
            if cc == b'\0' {
                break;
            }
            j += 1;
        }
        i += 1;
    }
    orig
}
pub(crate) fn strdup(env: &mut Environment, src: ConstPtr<u8>) -> MutPtr<u8> {
    GenericChar::<u8>::strdup(env, src)
}
pub fn strcmp(env: &mut Environment, a: ConstPtr<u8>, b: ConstPtr<u8>) -> i32 {
    GenericChar::<u8>::strcmp(env, a, b)
}
fn strncmp(env: &mut Environment, a: ConstPtr<u8>, b: ConstPtr<u8>, n: GuestUSize) -> i32 {
    GenericChar::<u8>::strncmp(env, a, b, n)
}
fn strcasecmp(env: &mut Environment, a: ConstPtr<u8>, b: ConstPtr<u8>) -> i32 {
    // TODO: generalize to wide chars
    let mut offset = 0;
    loop {
        let char_a = env.mem.read(a + offset).to_ascii_lowercase();
        let char_b = env.mem.read(b + offset).to_ascii_lowercase();
        offset += 1;

        match char_a.cmp(&char_b) {
            Ordering::Less => return -1,
            Ordering::Greater => return 1,
            Ordering::Equal => {
                if char_a == u8::default() {
                    return 0;
                } else {
                    continue;
                }
            }
        }
    }
}
fn strncasecmp(env: &mut Environment, a: ConstPtr<u8>, b: ConstPtr<u8>, n: GuestUSize) -> i32 {
    // TODO: generalize to wide chars
    if n == 0 {
        return 0;
    }

    let mut offset = 0;
    loop {
        let char_a = env.mem.read(a + offset).to_ascii_lowercase();
        let char_b = env.mem.read(b + offset).to_ascii_lowercase();
        offset += 1;

        match char_a.cmp(&char_b) {
            Ordering::Less => return -1,
            Ordering::Greater => return 1,
            Ordering::Equal => {
                if offset == n || char_a == u8::default() {
                    return 0;
                } else {
                    continue;
                }
            }
        }
    }
}
fn strncat(env: &mut Environment, s1: MutPtr<u8>, s2: ConstPtr<u8>, n: GuestUSize) -> MutPtr<u8> {
    GenericChar::<u8>::strncat(env, s1, s2, n)
}
fn strstr(env: &mut Environment, string: ConstPtr<u8>, substring: ConstPtr<u8>) -> ConstPtr<u8> {
    GenericChar::<u8>::strstr(env, string, substring)
}
fn strchr(env: &mut Environment, path: ConstPtr<u8>, c: u8) -> ConstPtr<u8> {
    GenericChar::<u8>::strchr(env, path, c)
}
fn strrchr(env: &mut Environment, path: ConstPtr<u8>, c: u8) -> ConstPtr<u8> {
    GenericChar::<u8>::strrchr(env, path, c)
}
fn strpbrk(env: &mut Environment, s: ConstPtr<u8>, charset: ConstPtr<u8>) -> ConstPtr<u8> {
    GenericChar::<u8>::strpbrk(env, s, charset)
}
fn strlcpy(
    env: &mut Environment,
    dst: MutPtr<u8>,
    src: ConstPtr<u8>,
    size: GuestUSize,
) -> GuestUSize {
    GenericChar::<u8>::strlcpy(env, dst, src, size)
}

/// `strnstr`, the BSD bounded search: like [strstr], but it looks at no more
/// than `slen` characters of `s`.
///
/// The bound is on `s` only — `find` is still read to its terminator — and the
/// terminator of `s` still stops the search early if it comes first, so a
/// haystack shorter than `slen` is not read past its end. An empty `find`
/// matches at the start, as the specification requires and as [strstr] does.
///
/// There is no wide-character counterpart in Darwin, so unlike its neighbours
/// this is not routed through [GenericChar].
fn strnstr(
    env: &mut Environment,
    s: ConstPtr<u8>,
    find: ConstPtr<u8>,
    slen: GuestUSize,
) -> ConstPtr<u8> {
    strnstr_inner(&env.mem, s, find, slen)
}

/// The bounds arithmetic, split out so it can be tested against a bare [Mem]
/// rather than only through a running guest.
fn strnstr_inner(mem: &Mem, s: ConstPtr<u8>, find: ConstPtr<u8>, slen: GuestUSize) -> ConstPtr<u8> {
    if mem.read(find) == b'\0' {
        return s;
    }
    for start in 0..=slen {
        let mut i = 0;
        loop {
            let wanted = mem.read(find + i);
            if wanted == b'\0' {
                return s + start;
            }
            // Reading beyond the bound is what the bound exists to prevent, and
            // a match cannot complete past it either way.
            if start + i >= slen {
                return Ptr::null();
            }
            let candidate = mem.read(s + start + i);
            if candidate == b'\0' || candidate != wanted {
                break;
            }
            i += 1;
        }
    }
    Ptr::null()
}

/// `strlcat`, the BSD bounded concatenation.
///
/// The return value is the length it *would* have produced given room —
/// `strlen(dst) + strlen(src)` — not the length it did produce, which is how a
/// caller detects truncation. Getting that backwards is the classic misuse, so
/// it is worth being explicit: the result can exceed `size`.
///
/// If `dst` has no terminator within `size` bytes there is nothing to append
/// to, and the specification says to return `size + strlen(src)` and write
/// nothing.
fn strlcat(
    env: &mut Environment,
    dst: MutPtr<u8>,
    src: ConstPtr<u8>,
    size: GuestUSize,
) -> GuestUSize {
    strlcat_inner(&mut env.mem, dst, src, size)
}

/// The truncation arithmetic, split out for the same reason as
/// [strnstr_inner].
fn strlcat_inner(
    mem: &mut Mem,
    dst: MutPtr<u8>,
    src: ConstPtr<u8>,
    size: GuestUSize,
) -> GuestUSize {
    let mut used = 0;
    while used < size && mem.read(dst + used) != b'\0' {
        used += 1;
    }
    let mut src_len = 0;
    while mem.read(src + src_len) != b'\0' {
        src_len += 1;
    }
    if used == size {
        return size + src_len;
    }

    // One byte of the remainder belongs to the terminator.
    let room = size - used - 1;
    let copied = room.min(src_len);
    for i in 0..copied {
        let c = mem.read(src + i);
        mem.write(dst + used + i, c);
    }
    mem.write(dst + used + copied, b'\0');
    used + src_len
}

/// `bcopy`, the pre-standard copy that survives because BSD headers still
/// declare it.
///
/// Two things differ from [memcpy] and both matter: the arguments are in the
/// opposite order, and overlap is defined, so this is [memmove] rather than
/// [memcpy].
fn bcopy(env: &mut Environment, src: ConstVoidPtr, dest: MutVoidPtr, size: GuestUSize) {
    GenericChar::<u8>::memmove(env, dest.cast(), src.cast(), size, GuestUSize::MAX);
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(strtok(_, _)),
    export_c_func!(strtok_r(_, _, _)),
    export_c_func!(bzero(_, _)),
    // Functions shared with wchar.rs
    export_c_func!(memset(_, _, _)),
    export_c_func!(__memset_chk(_, _, _, _)),
    export_c_func!(memset_pattern4(_, _, _)),
    export_c_func!(memset_pattern8(_, _, _)),
    export_c_func!(memset_pattern16(_, _, _)),
    export_c_func!(memcpy(_, _, _)),
    export_c_func!(__memcpy_chk(_, _, _, _)),
    export_c_func!(memmove(_, _, _)),
    export_c_func!(__memmove_chk(_, _, _, _)),
    export_c_func!(memchr(_, _, _)),
    export_c_func!(memcmp(_, _, _)),
    export_c_func!(strlen(_)),
    export_c_func!(strcpy(_, _)),
    export_c_func!(__strcpy_chk(_, _, _)),
    export_c_func!(strcat(_, _)),
    export_c_func!(strspn(_, _)),
    export_c_func!(strcspn(_, _)),
    export_c_func!(__strcat_chk(_, _, _)),
    export_c_func!(strncpy(_, _, _)),
    export_c_func!(__strncpy_chk(_, _, _, _)),
    export_c_func!(strsep(_, _)),
    export_c_func!(strdup(_)),
    export_c_func!(strcmp(_, _)),
    export_c_func!(strncmp(_, _, _)),
    export_c_func!(strcasecmp(_, _)),
    export_c_func!(strncasecmp(_, _, _)),
    export_c_func!(strncat(_, _, _)),
    export_c_func!(strstr(_, _)),
    export_c_func!(strnstr(_, _, _)),
    export_c_func!(strlcat(_, _, _)),
    export_c_func!(bcopy(_, _, _)),
    export_c_func!(strchr(_, _)),
    export_c_func!(strrchr(_, _)),
    export_c_func!(strpbrk(_, _)),
    export_c_func!(strlcpy(_, _, _)),
];

#[cfg(test)]
mod tests {
    use super::{strlcat_inner, strnstr_inner};
    use crate::mem::{ConstPtr, GuestUSize, Mem, MutPtr, Ptr};

    /// Guest memory holding two C strings, at fixed addresses well clear of the
    /// null segment so a stray read of address zero cannot pass by accident.
    fn memory_with(first: &[u8], second: &[u8]) -> (Mem, MutPtr<u8>, MutPtr<u8>) {
        let mut mem = Mem::new();
        let first_ptr: MutPtr<u8> = Ptr::from_bits(0x10000);
        let second_ptr: MutPtr<u8> = Ptr::from_bits(0x20000);
        for (ptr, bytes) in [(first_ptr, first), (second_ptr, second)] {
            for (i, &byte) in bytes.iter().enumerate() {
                mem.write(ptr + i as GuestUSize, byte);
            }
            mem.write(ptr + bytes.len() as GuestUSize, b'\0');
        }
        (mem, first_ptr, second_ptr)
    }

    fn strnstr_at(haystack: &[u8], needle: &[u8], slen: GuestUSize) -> Option<GuestUSize> {
        let (mem, s, find) = memory_with(haystack, needle);
        let found = strnstr_inner(&mem, s.cast_const(), find.cast_const(), slen);
        (!found.is_null()).then(|| found.to_bits() - s.to_bits())
    }

    #[test]
    fn strnstr_finds_a_match_inside_the_bound() {
        assert_eq!(strnstr_at(b"abcdef", b"cd", 6), Some(2));
        assert_eq!(strnstr_at(b"abcdef", b"cd", 4), Some(2));
    }

    #[test]
    fn strnstr_refuses_a_match_that_would_cross_the_bound() {
        // "cd" starts inside the first three characters but does not finish
        // inside them, so the bound rules it out.
        assert_eq!(strnstr_at(b"abcdef", b"cd", 3), None);
        assert_eq!(strnstr_at(b"abcdef", b"ef", 5), None);
    }

    #[test]
    fn strnstr_stops_at_the_haystacks_terminator_before_the_bound() {
        // A bound far past the end must not read past the terminator into
        // whatever follows.
        assert_eq!(strnstr_at(b"abc", b"x", 4096), None);
    }

    #[test]
    fn strnstr_matches_an_empty_needle_at_the_start() {
        assert_eq!(strnstr_at(b"abc", b"", 0), Some(0));
    }

    fn strlcat_call(dst: &[u8], src: &[u8], size: GuestUSize) -> (GuestUSize, Vec<u8>) {
        let (mut mem, dst_ptr, src_ptr) = memory_with(dst, src);
        let wanted = strlcat_inner(&mut mem, dst_ptr, src_ptr.cast_const(), size);
        let mut result = Vec::new();
        let mut i = 0;
        loop {
            let byte = mem.read(dst_ptr + i);
            if byte == b'\0' {
                break;
            }
            result.push(byte);
            i += 1;
        }
        (wanted, result)
    }

    #[test]
    fn strlcat_appends_when_there_is_room() {
        assert_eq!(strlcat_call(b"ab", b"cd", 16), (4, b"abcd".to_vec()));
    }

    #[test]
    fn strlcat_truncates_and_reports_the_length_it_wanted() {
        // Five bytes hold "abcd" plus the terminator, so one character of "cde"
        // is dropped, and the return value is the untruncated length.
        assert_eq!(strlcat_call(b"ab", b"cde", 5), (5, b"abcd".to_vec()));
        // No room for any of it, but still terminated and still reporting.
        assert_eq!(strlcat_call(b"ab", b"cde", 3), (5, b"ab".to_vec()));
    }

    #[test]
    fn strlcat_writes_nothing_when_the_destination_has_no_terminator_in_range() {
        // size smaller than the existing content means there is no terminator
        // to append at, so the specification says write nothing and return size
        // + strlen(src).
        assert_eq!(strlcat_call(b"abcd", b"xy", 2), (4, b"abcd".to_vec()));
    }

    #[test]
    fn strnstr_returns_a_pointer_into_the_haystack_not_a_copy() {
        let (mem, s, find) = memory_with(b"hello world", b"world");
        let found: ConstPtr<u8> = strnstr_inner(&mem, s.cast_const(), find.cast_const(), 11);
        assert_eq!(found.to_bits(), s.to_bits() + 6);
    }
}
