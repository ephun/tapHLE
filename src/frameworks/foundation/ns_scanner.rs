/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The `NSScanner` class.

use crate::frameworks::foundation::ns_string::from_u16_vec;
use crate::frameworks::foundation::{unichar, NSRange, NSUInteger};
use crate::mem::MutPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use crate::Environment;

// TODO: Speed up by optimizing for internal subclasses
#[derive(Default, Clone)]
struct NSScannerHostObject {
    /// NSCharacterSet *, characters to be skipped
    to_be_skipped: id,
    /// NSString *, should always be immutable since it's copied
    string: id,
    /// Length is cached since it is immutable.
    len: NSUInteger,
    pos: NSUInteger,
}
impl HostObject for NSScannerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSScanner: NSObject

+ (id)scannerWithString:(id)string {
    let new: id = msg![env; this alloc];
    let new = msg![env; new initWithString:string];
    autorelease(env, new)
}

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSScanner might be subclassed by something which needs
    // allocWithZone: to have the normal behaviour. Unimplemented: call
    // superclass alloc then.
    assert!(this == env.objc.get_known_class("NSScanner", &mut env.mem));
    msg_class![env; _tapHLE_NSScanner allocWithZone:zone]
}

@end

// Our private subclass that is the single implementation of NSScanner for
// the time being.
@implementation _tapHLE_NSScanner: NSScanner

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSScannerHostObject::default());
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithString:(id)string { // NSString *
    assert!(string != nil);
    let string: id = msg![env; string copy]; // Same behaviour as simulator
    let len: NSUInteger = msg![env; string length];
    let default_set = msg_class![env; NSCharacterSet whitespaceAndNewlineCharacterSet];
    retain(env, default_set);
    *env.objc.borrow_mut(this) = NSScannerHostObject {
        to_be_skipped: default_set,
        string,
        len,
        pos: 0
    };
    this
}

- (())dealloc {
    let &NSScannerHostObject {
        to_be_skipped,
        string,
        ..
    } = env.objc.borrow(this);
    release(env, string);
    release(env, to_be_skipped);
    env.objc.dealloc_object(this, &mut env.mem);
}

- (())setCharactersToBeSkipped:(id)to_be_skipped { // NSCharacterSet *
    let old_to_be_skipped = env.objc.borrow::<NSScannerHostObject>(this).to_be_skipped;
    env.objc.borrow_mut::<NSScannerHostObject>(this).to_be_skipped = to_be_skipped;
    retain(env, to_be_skipped);
    release(env, old_to_be_skipped);
}
- (id)charactersToBeSkipped {
    env.objc.borrow::<NSScannerHostObject>(this).to_be_skipped
}

- (NSUInteger)scanLocation {
    env.objc.borrow::<NSScannerHostObject>(this).pos
}

- (())setScanLocation:(NSUInteger)loc {
    let host = env.objc.borrow_mut::<NSScannerHostObject>(this);
    assert!(loc <= host.len); // TODO: raise NSRangeException
    host.pos = loc;
}

- (bool)isAtEnd {
    skip_characters(env, this);
    let NSScannerHostObject { len, pos, .. } = env.objc.borrow::<NSScannerHostObject>(this);
    len == pos
}

- (bool)scanUpToCharactersFromSet:(id)cset intoString:(MutPtr<id>)str {
    skip_characters(env, this);

    let NSScannerHostObject { to_be_skipped, string, len, mut pos } = env.objc.borrow::<NSScannerHostObject>(this).clone();
    if pos >= len {
        // Does nothing (same as simulator)
        return false;
    }
    let first_scan: unichar = msg![env; string characterAtIndex:pos];
    if msg![env; cset characterIsMember:first_scan] {
        // Does nothing (same as simulator)
        return false;
    }
    let mut chars = vec![first_scan];
    pos += 1;
    while pos < len {
        let curr = msg![env; string characterAtIndex:pos];
        if msg![env; cset characterIsMember:curr] {
            break
        }
        pos += 1;
        chars.push(curr);
    }
    if !str.is_null() {
        let out = from_u16_vec(env, chars);
        autorelease(env, out);
        env.mem.write(str, out);
    }

    *env.objc.borrow_mut::<NSScannerHostObject>(this) = NSScannerHostObject { to_be_skipped, string, len, pos };
    true
}

- (bool)scanCharactersFromSet:(id)cset intoString:(MutPtr<id>)str {
    let inv_cset: id = msg![env; cset invertedSet];
    msg![env; this scanUpToCharactersFromSet:inv_cset intoString:str]
}

- (bool)scanHexInt:(MutPtr<u32>)result {
    skip_characters(env, this);

    let NSScannerHostObject { to_be_skipped, string, len, pos } = env.objc.borrow::<NSScannerHostObject>(this).clone();

    // An optional "0x" or "0X" prefix is consumed only when hex digits follow
    // it, so that "0xzz" scans nothing at all rather than leaving the scanner
    // part way through a prefix that led nowhere.
    let mut scan = pos;
    if len - scan >= 2 {
        let second = scan + 1;
        let c0: unichar = msg![env; string characterAtIndex:scan];
        let c1: unichar = msg![env; string characterAtIndex:second];
        if c0 == u16::from(b'0') && (c1 == u16::from(b'x') || c1 == u16::from(b'X')) {
            scan += 2;
        }
    }

    // NSScanner saturates rather than wrapping or refusing: a run of digits too
    // long for 32 bits still scans, and reports UINT_MAX.
    let digits_start = scan;
    let mut value: u32 = 0;
    let mut overflowed = false;
    while scan < len {
        let c: unichar = msg![env; string characterAtIndex:scan];
        let Some(digit) = char::from_u32(u32::from(c)).and_then(|c| c.to_digit(16)) else {
            break;
        };
        match value.checked_mul(16).and_then(|v| v.checked_add(digit)) {
            Some(v) => value = v,
            None => overflowed = true,
        }
        scan += 1;
    }

    if scan == digits_start {
        // Nothing to scan. The scan location does not move and the caller is
        // told so, which is how an app asks whether a string is a hex number.
        return false;
    }

    if !result.is_null() {
        env.mem.write(result, if overflowed { u32::MAX } else { value });
    }
    *env.objc.borrow_mut::<NSScannerHostObject>(this) = NSScannerHostObject { to_be_skipped, string, len, pos: scan };
    true
}

- (bool)scanUpToString:(id)stop_string // NSString *
            intoString:(MutPtr<id>)result { // NSString **
    skip_characters(env, this);

    let NSScannerHostObject { to_be_skipped, string, len, pos } = std::mem::take(env.objc.borrow_mut::<NSScannerHostObject>(this));

    let stop_len: NSUInteger = msg![env; stop_string length];
    // A stop string with nothing in it is already there wherever the scanner
    // is, so nothing is scanned.
    let found = if stop_len == 0 {
        Some(pos)
    } else {
        find_string(env, string, len, pos, stop_string)
    };

    let scan_len = match found {
        Some(location) => location - pos,
        None => len - pos,
    };

    // "Returns YES if the receiver scanned any characters, otherwise NO." A
    // scan that moved nowhere - because the stop string is right here, or
    // because there is nothing left of the string at all - scanned no
    // characters, and saying otherwise is how a loop that reads until the
    // scanner stops finding things never ends. One game's level load sat at
    // the end of a seven-character string twelve million times.
    if scan_len == 0 {
        *env.objc.borrow_mut::<NSScannerHostObject>(this) = NSScannerHostObject { to_be_skipped, string, len, pos };
        return false;
    }
    assert!(pos + scan_len <= len);
    *env.objc.borrow_mut::<NSScannerHostObject>(this) = NSScannerHostObject { to_be_skipped, string, len, pos: pos + scan_len };

    if !result.is_null() {
        let range = NSRange { location: pos, length: scan_len };
        let copy: id = msg![env; string substringWithRange:range];
        log_dbg!("scanned up to {}", pos + scan_len);
        // Note: substring is already autoreleased
        env.mem.write(result, copy);
    }
    true
}

- (bool)scanString:(id)scan_string // NSString *
        intoString:(MutPtr<id>)result { // NSString **
    skip_characters(env, this);

    let NSScannerHostObject { to_be_skipped, string, len, pos } = std::mem::take(env.objc.borrow_mut::<NSScannerHostObject>(this));

    let scan_len: NSUInteger = msg![env; scan_string length];
    if pos + scan_len > len || !has_string_at(env, string, pos, scan_string) {
        *env.objc.borrow_mut::<NSScannerHostObject>(this) = NSScannerHostObject { to_be_skipped, string, len, pos };
        return false;
    }
    *env.objc.borrow_mut::<NSScannerHostObject>(this) = NSScannerHostObject { to_be_skipped, string, len, pos: pos + scan_len };

    if !result.is_null() {
        let copy: id = msg![env; scan_string copy];
        autorelease(env, copy);
        env.mem.write(result, copy);
    }
    true
}

- (bool)scanInt:(MutPtr<i32>)result {
    skip_characters(env, this);

    let NSScannerHostObject { to_be_skipped, string, len, pos } = std::mem::take(env.objc.borrow_mut::<NSScannerHostObject>(this));

    // Only the run of characters a number can be written with is read, so this
    // costs the length of the number rather than the length of everything
    // after it.
    let mut digits = String::new();
    let mut scan = pos;
    while scan < len {
        let c: unichar = msg![env; string characterAtIndex:scan];
        let Some(c) = char::from_u32(u32::from(c)) else {
            break;
        };
        if !c.is_ascii_digit() && c != '+' && c != '-' {
            break;
        }
        digits.push(c);
        scan += 1;
    }
    if digits.is_empty() {
        log_dbg!("scanInt: no valid int found at {}", pos);
        *env.objc.borrow_mut::<NSScannerHostObject>(this) = NSScannerHostObject { to_be_skipped, string, len, pos };
        return false;
    }

    if !result.is_null() {
        // TODO: handle over/underflow properly
        let res = digits.parse().unwrap_or(0);
        log_dbg!("scanInt: from '{}' -> {}", digits, res);
        env.mem.write(result, res);
    }

    *env.objc.borrow_mut::<NSScannerHostObject>(this) = NSScannerHostObject { to_be_skipped, string, len, pos: scan };
    true
}

// NSInteger is 32-bit on the guest ABI, so this is the same scan and the same
// out-parameter width as -scanInt:. On a 64-bit runtime the two differ and this
// would need its own implementation; the forwarding is correct here and only
// here.
- (bool)scanInteger:(MutPtr<i32>)result {
    msg![env; this scanInt:result]
}

@end

};

/// Whether `string` has `needle` at `at`, compared code unit by code unit.
///
/// The point is what this does not do. The obvious way to answer the question
/// is to take everything from `at` onwards and ask whether it starts with
/// `needle`, and that copies the whole of the rest of the string. A scanner
/// asks once per token, so the copy makes reading a file cost the square of
/// its length, and each copy is autoreleased, so the memory is held until the
/// pool drains. One game's level load grew to nine gigabytes that way and
/// never finished.
fn has_string_at(env: &mut Environment, string: id, at: NSUInteger, needle: id) -> bool {
    let needle_len: NSUInteger = msg![env; needle length];
    for offset in 0..needle_len {
        let index = at + offset;
        let a: unichar = msg![env; string characterAtIndex:index];
        let b: unichar = msg![env; needle characterAtIndex:offset];
        if a != b {
            return false;
        }
    }
    true
}

/// The index of the first occurrence of `needle` in `string` at or after
/// `from`, or [None]. Compares in place, for the reason in [has_string_at].
fn find_string(
    env: &mut Environment,
    string: id,
    len: NSUInteger,
    from: NSUInteger,
    needle: id,
) -> Option<NSUInteger> {
    let needle_len: NSUInteger = msg![env; needle length];
    if needle_len > len {
        return None;
    }
    (from..=(len - needle_len)).find(|&at| has_string_at(env, string, at, needle))
}

// Helper functions, skips characters from `charactersToBeSkipped` set
fn skip_characters(env: &mut Environment, scanner: id) {
    let &NSScannerHostObject {
        to_be_skipped,
        string,
        len,
        mut pos,
    } = env.objc.borrow::<NSScannerHostObject>(scanner);
    loop {
        if pos >= len {
            break;
        }
        let c: unichar = msg![env; string characterAtIndex:pos];
        if msg![env; to_be_skipped characterIsMember:c] {
            pos += 1;
        } else {
            break;
        }
    }
    env.objc.borrow_mut::<NSScannerHostObject>(scanner).pos = pos;
}
