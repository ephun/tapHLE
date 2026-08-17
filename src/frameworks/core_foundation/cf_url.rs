/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFURL`.
//!
//! This is toll-free bridged to `NSURL` in Apple's implementation. Here it is
//! the same type.

use super::cf_allocator::{kCFAllocatorDefault, CFAllocatorRef};
use super::CFIndex;
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_foundation::cf_string::{
    kCFStringEncodingASCII, kCFStringEncodingISOLatin1, kCFStringEncodingMacRoman,
    kCFStringEncodingUTF8, CFStringConvertEncodingToNSStringEncoding, CFStringEncoding,
    CFStringRef,
};
use crate::frameworks::foundation::ns_string::{
    from_rust_string, get_static_str, to_rust_string, NSUTF8StringEncoding,
};
use crate::frameworks::foundation::NSUInteger;
use crate::mem::{ConstPtr, MutPtr, Ptr};
use crate::objc::{id, msg, msg_class, nil, release};
use crate::Environment;
use encoding_rs::MACINTOSH;

pub type CFURLRef = super::CFTypeRef;

type CFURLPathStyle = CFIndex;
const kCFURLPOSIXPathStyle: CFURLPathStyle = 0;
#[allow(dead_code)]
const kCFURLHFSPathStyle: CFURLPathStyle = 1;
#[allow(dead_code)]
const kCFURLWindowsPathStyle: CFURLPathStyle = 2;

pub fn CFURLGetFileSystemRepresentation(
    env: &mut Environment,
    url: CFURLRef,
    resolve_against_base: bool,
    buffer: MutPtr<u8>,
    buffer_size: CFIndex,
) -> bool {
    if resolve_against_base {
        // this function usually called to resolve resources from the main
        // bundle
        // thus, the url should already be an absolute path name
        // TODO: use absoluteURL instead once implemented
        let path = msg![env; url path];
        // TODO: avoid copy
        assert!(to_rust_string(env, path).starts_with('/'));
    }
    let buffer_size: NSUInteger = buffer_size.try_into().unwrap();

    msg![env; url getFileSystemRepresentation:buffer
                                    maxLength:buffer_size]
}

pub fn CFURLCreateFromFileSystemRepresentation(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    buffer: ConstPtr<u8>,
    buffer_size: CFIndex,
    is_directory: bool,
) -> CFURLRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented

    let buffer_size: NSUInteger = buffer_size.try_into().unwrap();

    let string: id = msg_class![env; NSString alloc];
    let string: id = msg![env; string initWithBytes:buffer
                                             length:buffer_size
                                           encoding:NSUTF8StringEncoding];

    let url: id = msg_class![env; NSURL alloc];
    let res = msg![env; url initFileURLWithPath:string isDirectory:is_directory];
    release(env, string);
    res
}

fn CFURLCreateWithBytes(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    url_bytes: ConstPtr<u8>,
    length: CFIndex,
    encoding: CFStringEncoding,
    base_url: CFURLRef,
) -> CFURLRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    if !base_url.is_null() {
        log!("TODO: CFURLCreateWithBytes() with a base URL; ignoring the base");
    }

    // The encoding is whatever the caller says. It used to be asserted to be
    // ASCII, which ended any app that built a URL from UTF-8 bytes — the
    // ordinary choice, and what a URL containing an escape or a non-ASCII
    // character needs.
    // TODO: interpret percent escape sequences using encoding as well
    let encoding = CFStringConvertEncodingToNSStringEncoding(env, encoding);
    let length: NSUInteger = length.try_into().unwrap();

    if length == 0 {
        return Ptr::null();
    }

    let string: id = msg_class![env; NSString alloc];
    let string: id = msg![env; string initWithBytes:url_bytes
                                             length:length
                                           encoding:encoding];

    // A URL with a scheme is an absolute URL and has to be parsed as one;
    // anything else is a file path. Treating everything as a path aborted on
    // the first `http://` an app built this way.
    let has_scheme = to_rust_string(env, string).contains("://");
    let url: id = msg_class![env; NSURL alloc];
    let res = if has_scheme {
        msg![env; url initWithString:string]
    } else {
        msg![env; url initFileURLWithPath:string]
    };
    release(env, string);
    res
}

/// The same as [CFURLCreateWithBytes] but resolved against a base URL, and with
/// a flag saying whether the bytes are already percent-escaped.
///
/// With no base URL the two are the same call, and that is the case apps reach
/// here on: a URL built from bytes that turn out to be absolute already.
fn CFURLCreateAbsoluteURLWithBytes(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    url_bytes: ConstPtr<u8>,
    length: CFIndex,
    encoding: CFStringEncoding,
    base_url: CFURLRef,
    _use_compatibility_mode: bool,
) -> CFURLRef {
    if !base_url.is_null() {
        log!("TODO: CFURLCreateAbsoluteURLWithBytes() with a base URL; ignoring the base");
    }
    CFURLCreateWithBytes(env, allocator, url_bytes, length, encoding, Ptr::null())
}

fn CFURLCreateWithFileSystemPath(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    file_path: CFStringRef,
    style: CFURLPathStyle,
    is_directory: bool,
) -> CFURLRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    assert_eq!(style, kCFURLPOSIXPathStyle);
    let url: id = msg_class![env; NSURL alloc];
    msg![env; url initFileURLWithPath:file_path isDirectory:is_directory]
}

fn CFURLCreateWithString(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    url_string: CFStringRef,
    base_url: CFURLRef,
) -> CFURLRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    assert!(base_url.is_null()); // TODO
    let url: id = msg_class![env; NSURL alloc];
    msg![env; url initWithString:url_string]
}

fn is_rfc2396_url_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(
            character,
            '-' | '_'
                | '.'
                | '!'
                | '~'
                | '*'
                | '\''
                | '('
                | ')'
                | ';'
                | '/'
                | '?'
                | ':'
                | '@'
                | '&'
                | '='
                | '+'
                | '$'
                | ','
                | '%'
                | '#'
        )
}

fn bytes_for_percent_escape(character: char, encoding: CFStringEncoding) -> Option<Vec<u8>> {
    match encoding {
        kCFStringEncodingUTF8 => {
            let mut buffer = [0; 4];
            Some(character.encode_utf8(&mut buffer).as_bytes().to_vec())
        }
        kCFStringEncodingASCII => character.is_ascii().then(|| vec![character as u8]),
        kCFStringEncodingISOLatin1 => {
            let codepoint = character as u32;
            (codepoint <= u8::MAX as u32).then(|| vec![codepoint as u8])
        }
        kCFStringEncodingMacRoman => {
            let mut buffer = [0; 4];
            let string = character.encode_utf8(&mut buffer);
            let (bytes, _, had_errors) = MACINTOSH.encode(string);
            (!had_errors).then(|| bytes.into_owned())
        }
        _ => unimplemented!("Percent escapes with CFStringEncoding {encoding:#x}"),
    }
}

fn add_percent_escapes(
    original: &str,
    characters_to_leave: &str,
    legal_characters_to_escape: &str,
    encoding: CFStringEncoding,
) -> Option<String> {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut result = String::with_capacity(original.len());
    for character in original.chars() {
        let should_escape = !characters_to_leave.contains(character)
            && (!is_rfc2396_url_character(character)
                || legal_characters_to_escape.contains(character));
        if !should_escape {
            result.push(character);
            continue;
        }
        for byte in bytes_for_percent_escape(character, encoding)? {
            result.push('%');
            result.push(HEX[(byte >> 4) as usize] as char);
            result.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    Some(result)
}

fn CFURLCreateStringByAddingPercentEscapes(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    original_string: CFStringRef,
    characters_to_leave_unescaped: CFStringRef,
    legal_url_characters_to_be_escaped: CFStringRef,
    encoding: CFStringEncoding,
) -> CFStringRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default());

    let original = to_rust_string(env, original_string);
    let characters_to_leave = if characters_to_leave_unescaped.is_null() {
        ""
    } else {
        &to_rust_string(env, characters_to_leave_unescaped)
    };
    let legal_characters_to_escape = if legal_url_characters_to_be_escaped.is_null() {
        ""
    } else {
        &to_rust_string(env, legal_url_characters_to_be_escaped)
    };
    let Some(result) = add_percent_escapes(
        &original,
        characters_to_leave,
        legal_characters_to_escape,
        encoding,
    ) else {
        return Ptr::null();
    };
    from_rust_string(env, result)
}

pub fn CFURLCopyPathExtension(env: &mut Environment, url: CFURLRef) -> CFStringRef {
    let path = msg![env; url path];
    let ext = msg![env; path pathExtension];
    msg![env; ext copy]
}

fn CFURLCopyFileSystemPath(
    env: &mut Environment,
    url: CFURLRef,
    style: CFURLPathStyle,
) -> CFStringRef {
    assert_eq!(style, kCFURLPOSIXPathStyle);
    let path: CFStringRef = msg![env; url path];
    msg![env; path copy]
}

/// `CFURLCreateStringByReplacingPercentEscapesUsingEncoding`.
///
/// `chars_to_leave_escaped` names characters whose escapes should survive
/// decoding, which is how a caller decodes a whole URL without destroying the
/// delimiters inside a component. An empty or null string means decode
/// everything, and that is what every observed caller passes.
fn CFURLCreateStringByReplacingPercentEscapesUsingEncoding(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    origin_string: CFStringRef,
    chars_to_leave_escaped: CFStringRef,
    encoding: CFStringEncoding,
) -> CFStringRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    if origin_string.is_null() {
        return Ptr::null();
    }
    if !chars_to_leave_escaped.is_null() {
        let length: NSUInteger = msg![env; chars_to_leave_escaped length];
        // Honouring this means decoding selectively, which nothing seen needs.
        // Saying so beats silently decoding the delimiters the caller asked to
        // keep, which would corrupt the URL it is taking apart.
        assert!(
            length == 0,
            "CFURLCreateStringByReplacingPercentEscapesUsingEncoding() with \
             characters to leave escaped is not implemented"
        );
    }
    let encoding = CFStringConvertEncodingToNSStringEncoding(env, encoding);
    let decoded: CFStringRef =
        msg![env; origin_string stringByReplacingPercentEscapesUsingEncoding:encoding];
    if decoded == nil {
        // The CF function returns NULL where the Objective-C one returns nil,
        // and its callers check.
        return Ptr::null();
    }
    msg![env; decoded copy]
}

/// `CFURLCreateStringByReplacingPercentEscapes` — the same thing with the
/// encoding fixed to UTF-8.
fn CFURLCreateStringByReplacingPercentEscapes(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    origin_string: CFStringRef,
    chars_to_leave_escaped: CFStringRef,
) -> CFStringRef {
    CFURLCreateStringByReplacingPercentEscapesUsingEncoding(
        env,
        allocator,
        origin_string,
        chars_to_leave_escaped,
        kCFStringEncodingUTF8,
    )
}

fn CFURLCreateCopyAppendingPathComponent(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    url: CFURLRef,
    path_component: CFStringRef,
    is_directory: bool,
) -> CFURLRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    let new_url =
        msg![env; url URLByAppendingPathComponent:path_component isDirectory:is_directory];
    msg![env; new_url copy]
}

fn CFURLCreateCopyDeletingLastPathComponent(
    env: &mut Environment,
    allocator: CFAllocatorRef,
    url: CFURLRef,
) -> CFURLRef {
    assert!(allocator == kCFAllocatorDefault || env.mem.read(allocator).is_system_default()); // unimplemented
    let new_url = msg![env; url URLByDeletingLastPathComponent];
    msg![env; new_url copy]
}

fn CFURLHasDirectoryPath(env: &mut Environment, url: CFURLRef) -> bool {
    assert!(!url.is_null());

    let path = msg![env; url path];
    if msg![env; path isEqual:(get_static_str(env, "//"))] {
        // Special case
        return false;
    }
    // Note: cannot use `lastPathComponent` here!
    let components: id = msg![env; path pathComponents];
    let count: NSUInteger = msg![env; components count];
    if count == 0 {
        return false;
    }
    let last: id = msg![env; components objectAtIndex:(count - 1)];
    msg![env; last isEqual:(get_static_str(env, "/"))]
        || msg![env; last isEqual:(get_static_str(env, "."))]
        || msg![env; last isEqual:(get_static_str(env, ".."))]
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CFURLGetFileSystemRepresentation(_, _, _, _)),
    export_c_func!(CFURLCreateFromFileSystemRepresentation(_, _, _, _)),
    export_c_func!(CFURLCreateWithBytes(_, _, _, _, _)),
    export_c_func!(CFURLCreateAbsoluteURLWithBytes(_, _, _, _, _, _)),
    export_c_func!(CFURLCreateWithFileSystemPath(_, _, _, _)),
    export_c_func!(CFURLCreateWithString(_, _, _)),
    export_c_func!(CFURLCreateStringByAddingPercentEscapes(_, _, _, _, _)),
    export_c_func!(CFURLCreateStringByReplacingPercentEscapesUsingEncoding(
        _,
        _,
        _,
        _
    )),
    export_c_func!(CFURLCreateStringByReplacingPercentEscapes(_, _, _)),
    export_c_func!(CFURLCopyPathExtension(_)),
    export_c_func!(CFURLCopyFileSystemPath(_, _)),
    export_c_func!(CFURLCreateCopyAppendingPathComponent(_, _, _, _)),
    export_c_func!(CFURLCreateCopyDeletingLastPathComponent(_, _)),
    export_c_func!(CFURLHasDirectoryPath(_)),
];

#[cfg(test)]
mod tests {
    use super::add_percent_escapes;
    use crate::frameworks::core_foundation::cf_string::{
        kCFStringEncodingASCII, kCFStringEncodingUTF8,
    };

    #[test]
    fn percent_escapes_illegal_and_requested_url_characters() {
        assert_eq!(
            add_percent_escapes("hello world/path?é=1", "", "/?", kCFStringEncodingUTF8),
            Some("hello%20world%2Fpath%3F%C3%A9=1".to_owned())
        );
    }

    #[test]
    fn percent_escapes_respect_leave_list_and_existing_escapes() {
        assert_eq!(
            add_percent_escapes("hello world%20", " ", "", kCFStringEncodingUTF8),
            Some("hello world%20".to_owned())
        );
    }

    #[test]
    fn percent_escapes_reject_unrepresentable_ascii() {
        assert_eq!(
            add_percent_escapes("é", "", "", kCFStringEncodingASCII),
            None
        );
    }
}
