/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The `NSString` class cluster, including `NSMutableString`.
//!
//! Resources:
//! - Apple's [String Programming Guide](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Strings/introStrings.html)

mod path_algorithms;

use super::ns_error::NSFileWriteUnknownError;
use super::ns_file_manager::write_error;
use super::ns_keyed_archiver::set_value_to_encode_for_current_key;
use super::{ns_array, ns_keyed_unarchiver};
use super::{
    unichar, NSComparisonResult, NSInteger, NSNotFound, NSOrderedAscending, NSOrderedDescending,
    NSOrderedSame, NSRange, NSUInteger,
};
use crate::abi::VaList;
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::_nib_archive_decoder;
use crate::frameworks::uikit::ui_font::{
    self, UILineBreakMode, UILineBreakModeWordWrap, UITextAlignment, UITextAlignmentLeft,
};
use crate::fs::GuestPath;
use crate::mach_o::MachO;
use crate::mem::{guest_size_of, ConstPtr, ConstVoidPtr, GuestUSize, Mem, MutPtr, Ptr, SafeRead};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, Class, ClassExports,
    HostObject, NSZonePtr, ObjC,
};
use crate::{fs, Environment};
use encoding_rs::{MACINTOSH, SHIFT_JIS, WINDOWS_1252};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use std::iter::Peekable;
use std::string::FromUtf16Error;

pub type NSStringEncoding = NSUInteger;
pub const NSASCIIStringEncoding: NSUInteger = 1;
pub const NSUTF8StringEncoding: NSUInteger = 4;
pub const NSISOLatin1StringEncoding: NSUInteger = 5;
pub const NSShiftJISStringEncoding: NSUInteger = 8;
pub const NSUnicodeStringEncoding: NSUInteger = 10;
pub const NSWindowsCP1252StringEncoding: NSUInteger = 12;
pub const NSMacOSRomanStringEncoding: NSUInteger = 30;
pub const NSUTF16StringEncoding: NSUInteger = NSUnicodeStringEncoding;
pub const NSUTF16BigEndianStringEncoding: NSUInteger = 0x90000100;
pub const NSUTF16LittleEndianStringEncoding: NSUInteger = 0x94000100;

pub type NSStringCompareOptions = NSUInteger;
pub const NSCaseInsensitiveSearch: NSUInteger = 1;
pub const NSLiteralSearch: NSUInteger = 2;
pub const NSBackwardsSearch: NSUInteger = 4;
pub const NSNumericSearch: NSUInteger = 64;

/// Encodings that C strings (null-terminated byte strings) can use.
const C_STRING_FRIENDLY_ENCODINGS: &[NSStringEncoding] = &[
    NSASCIIStringEncoding,
    NSUTF8StringEncoding,
    NSWindowsCP1252StringEncoding,
    NSMacOSRomanStringEncoding,
    NSISOLatin1StringEncoding,
];

pub const NSMaximumStringLength: NSUInteger = (i32::MAX - 1) as _;

#[derive(Default)]
pub struct State {
    static_str_pool: HashMap<&'static str, id>,
}
impl State {
    fn get(env: &mut Environment) -> &mut Self {
        &mut env.framework_state.foundation.ns_string
    }
}

/// Constant strings embedded in the app binary use this struct. The name is
/// according to Ghidra, the rest is guesswork.
#[allow(non_camel_case_types)]
struct cfstringStruct {
    _isa: Class,
    flags: u32,
    bytes: ConstPtr<u8>,
    length: NSUInteger,
}
unsafe impl SafeRead for cfstringStruct {}

type Utf16String = Vec<u16>;

/// Belongs to _tapHLE_NSString.
enum StringHostObject {
    Utf8(Cow<'static, str>),
    /// Not necessarily well-formed UTF-16: might contain unpaired surrogates.
    Utf16(Utf16String),
}
impl HostObject for StringHostObject {}
impl StringHostObject {
    /// Decode bytes in `encoding`, or `None` if they are not valid in it.
    ///
    /// `None` is not an error condition to report: `initWithData:encoding:` and
    /// its relatives are documented to return nil when the data cannot be
    /// converted, and apps do pass data that cannot be — a binary property list
    /// handed to the UTF-8 path is the case seen most. The caller returns nil
    /// and the app takes the branch it already has for that.
    fn decode(bytes: Cow<[u8]>, encoding: NSStringEncoding) -> Option<StringHostObject> {
        if bytes.is_empty() {
            return Some(StringHostObject::Utf8(Cow::Borrowed("")));
        }

        // TODO: error handling

        Some(match encoding {
            NSASCIIStringEncoding => {
                if !bytes.iter().all(|byte| byte.is_ascii()) {
                    return None;
                }
                // Safety: guaranteed by the check above
                let string = unsafe { String::from_utf8_unchecked(bytes.into_owned()) };
                StringHostObject::Utf8(Cow::Owned(string))
            }
            NSMacOSRomanStringEncoding | NSISOLatin1StringEncoding => {
                // TODO: support non ASCII symbols
                assert!(bytes.iter().all(|byte| byte.is_ascii()));
                // Safety: guaranteed by above assertion
                let string = unsafe { String::from_utf8_unchecked(bytes.into_owned()) };
                StringHostObject::Utf8(Cow::Owned(string))
            }
            NSUTF8StringEncoding => {
                let string = String::from_utf8(bytes.into_owned()).ok()?;
                StringHostObject::Utf8(Cow::Owned(string))
            }
            NSWindowsCP1252StringEncoding => {
                let (cow, encoding_used, had_errors) = WINDOWS_1252.decode(&bytes);
                assert_eq!(encoding_used, WINDOWS_1252);
                assert!(!had_errors);
                StringHostObject::Utf8(Cow::Owned(cow.into_owned()))
            }
            NSShiftJISStringEncoding => {
                let (cow, encoding_used, had_errors) = SHIFT_JIS.decode(&bytes);
                assert_eq!(encoding_used, SHIFT_JIS);
                assert!(!had_errors);
                log_dbg!("ShiftJIS decoded {:?}", cow);
                StringHostObject::Utf8(Cow::Owned(cow.to_string()))
            }
            NSUTF16StringEncoding
            | NSUTF16BigEndianStringEncoding
            | NSUTF16LittleEndianStringEncoding => {
                assert!(bytes.len().is_multiple_of(2));

                let is_big_endian = match encoding {
                    NSUTF16BigEndianStringEncoding => true,
                    NSUTF16LittleEndianStringEncoding => false,
                    NSUTF16StringEncoding => match &bytes[0..2] {
                        [0xFE, 0xFF] => true,
                        [0xFF, 0xFE] => false,
                        // Assuming NSUTF16LittleEndianStringEncoding if no BOM
                        // is present
                        // TODO: it seems that foundation can prefix string
                        // with BOM bytes?
                        _ => false,
                    },
                    _ => unreachable!(),
                };

                let units: Vec<u16> = if is_big_endian {
                    bytes
                        .chunks(2)
                        .map(|chunk| u16::from_be_bytes(chunk.try_into().unwrap()))
                        .collect()
                } else {
                    bytes
                        .chunks(2)
                        .map(|chunk| u16::from_le_bytes(chunk.try_into().unwrap()))
                        .collect()
                };

                // The mark is consumed, not kept: under
                // NSUTF16StringEncoding it is how the byte order was just
                // decided, so leaving it in would put an invisible character
                // at the front of every file that has one. The
                // explicitly-ordered encodings are different - nothing was
                // decided from it there, so it stays as the zero width
                // no-break space it nominally is.
                let units = match units.split_first() {
                    Some((0xFEFF, rest)) if encoding == NSUTF16StringEncoding => rest.to_vec(),
                    _ => units,
                };

                StringHostObject::Utf16(units)
            }
            _ => panic!("Unimplemented encoding: {encoding:#x}"),
        })
    }
    fn to_utf8(&self) -> Result<Cow<'static, str>, FromUtf16Error> {
        match self {
            StringHostObject::Utf8(utf8) => Ok(utf8.clone()),
            StringHostObject::Utf16(utf16) => Ok(Cow::Owned(String::from_utf16(utf16)?)),
        }
    }
    /// Mutate the object, converting to UTF-16 if the string was not already
    /// UTF-16. Returns a reference to the UTF-16 content and a boolean that is
    /// [true] if a conversion happened.
    fn convert_to_utf16_inplace(&mut self) -> (&mut Utf16String, bool) {
        let converted = match self {
            Self::Utf8(_) => {
                *self = Self::Utf16(self.iter_code_units().collect());
                true
            }
            Self::Utf16(_) => false,
        };
        let Self::Utf16(utf16) = self else {
            unreachable!();
        };
        (utf16, converted)
    }
    /// Iterate over the string as UTF-16 code units.
    fn iter_code_units(&self) -> CodeUnitIterator<'_> {
        match self {
            StringHostObject::Utf8(utf8) => CodeUnitIterator::Utf8(utf8.encode_utf16()),
            StringHostObject::Utf16(utf16) => CodeUnitIterator::Utf16(utf16.iter()),
        }
    }
}

enum CodeUnitIterator<'a> {
    Utf8(std::str::EncodeUtf16<'a>),
    Utf16(std::slice::Iter<'a, u16>),
}
impl Iterator for CodeUnitIterator<'_> {
    type Item = u16;

    fn next(&mut self) -> Option<u16> {
        match self {
            CodeUnitIterator::Utf8(iter) => iter.next(),
            CodeUnitIterator::Utf16(iter) => iter.next().copied(),
        }
    }
}
impl Clone for CodeUnitIterator<'_> {
    fn clone(&self) -> Self {
        match self {
            CodeUnitIterator::Utf8(iter) => CodeUnitIterator::Utf8(iter.clone()),
            CodeUnitIterator::Utf16(iter) => CodeUnitIterator::Utf16(iter.clone()),
        }
    }
}
impl CodeUnitIterator<'_> {
    /// If the sequence of code units in `prefix` is a prefix of `self`,
    /// return [Some] with `self` advanced past that prefix, otherwise [None].
    ///
    /// Code units comparison is done conditional to `case_insensitive` bool:
    /// if it's true, the code units are converted to chars first and compared
    /// as lowercase variants, otherwise the match is exact.
    fn strip_prefix(&self, prefix: &CodeUnitIterator, case_insensitive: bool) -> Option<Self> {
        let mut self_match = self.clone();
        let mut prefix_match = prefix.clone();
        loop {
            match prefix_match.next() {
                None => {
                    return Some(self_match);
                }
                Some(prefix_c) => {
                    let self_c = self_match.next();
                    if case_insensitive {
                        self_c?;
                        let (Some(a_c), Some(b_c)) = (
                            char::from_u32(self_c.unwrap() as u32),
                            char::from_u32(prefix_c as u32),
                        ) else {
                            panic!("Invalid chars in the strings!");
                        };
                        if !a_c.to_lowercase().eq(b_c.to_lowercase()) {
                            return None;
                        }
                    } else if self_c != Some(prefix_c) {
                        return None;
                    }
                }
            }
        }
    }
}

/// Helper for formatting methods. They can't call eachother currently due to
/// full vararg passthrough being missing.
pub fn with_format(env: &mut Environment, format: id, args: VaList) -> String {
    let format_string = to_rust_string(env, format);

    log_dbg!("Formatting {:?} ({:?})", format, format_string);

    let res = crate::libc::stdio::printf::printf_inner::<true, _>(
        env,
        |_, idx| {
            if idx as usize == format_string.len() {
                b'\0'
            } else {
                format_string.as_bytes()[idx as usize]
            }
        },
        args,
    );
    match String::from_utf8_lossy(&res) {
        Cow::Borrowed(str) => str.to_owned(),
        Cow::Owned(string) => {
            // TODO: Support UTF-16 printf directly
            log!(
                "Warning: invalid UTF-8 sequence replaced with U+FFFD in UTF-16 string formatting"
            );
            string
        }
    }
}

pub fn from_rust_ordering(ordering: std::cmp::Ordering) -> NSComparisonResult {
    match ordering {
        std::cmp::Ordering::Less => NSOrderedAscending,
        std::cmp::Ordering::Equal => NSOrderedSame,
        std::cmp::Ordering::Greater => NSOrderedDescending,
    }
}

/// Clear a caller's `NSError**` out-parameter.
///
/// These initialisers report failure by returning nil, which is what callers
/// check. tapHLE has no NSError detail worth inventing here, so the pointer is
/// set to nil rather than left holding stale memory — and, importantly, rather
/// than asserting that the caller did not ask for an error at all, which used
/// to abort any app that passed one.
fn report_no_error(env: &mut Environment, error: MutPtr<id>) {
    if !error.is_null() {
        env.mem.write(error, nil);
    }
}

/// The deprecated `initWithContentsOfFile:`/`initWithContentsOfURL:` take no
/// encoding and are documented to determine one themselves. Honour a byte order
/// mark when there is one, and otherwise assume the default C string encoding,
/// which is right for the ASCII and UTF-8 text these apps actually ship.
fn sniff_encoding(env: &mut Environment, bytes: &[u8]) -> NSStringEncoding {
    if bytes.len() > 1 && (bytes[..2] == [0xFE, 0xFF] || bytes[..2] == [0xFF, 0xFE]) {
        NSUTF16StringEncoding
    } else if bytes.len() > 2 && bytes[..3] == [0xEF, 0xBB, 0xBF] {
        NSUTF8StringEncoding
    } else {
        msg_class![env; NSString defaultCStringEncoding]
    }
}

/// Body of the deprecated one-argument `initWithContentsOfURL:`, shared by the
/// concrete string classes. They are siblings under NSString, so neither
/// inherits the other's implementations and both have to declare it.
fn init_with_contents_of_url(env: &mut Environment, this: id, url: id) -> id {
    if url == nil {
        release(env, this);
        return nil;
    }
    let data: id = msg_class![env; NSData dataWithContentsOfURL:url];
    if data == nil {
        release(env, this);
        return nil;
    }
    let bytes: ConstPtr<u8> = msg![env; data bytes];
    let length: NSUInteger = msg![env; data length];
    let prefix = env.mem.bytes_at(bytes, length.min(3)).to_vec();
    let encoding = sniff_encoding(env, &prefix);
    msg![env; this initWithData:data encoding:encoding]
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// NSString is an abstract class. A subclass must provide:
// - (NSUInteger)length;
// - (unichar)characterAtIndex:(NSUInteger)index;
// We can pick whichever subclass we want for the various alloc methods.
// For the time being, that will always be _tapHLE_NSString.
@implementation NSString: NSObject

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSString might be subclassed by something which needs allocWithZone:
    // to have the normal behaviour. Unimplemented: call superclass alloc then.
    assert!(this == env.objc.get_known_class("NSString", &mut env.mem));
    msg_class![env; _tapHLE_NSString allocWithZone:zone]
}

+ (id)string {
    let str: id = msg![env; this new];
    autorelease(env, str)
}

+ (id)stringWithString:(id)string { // NSString*
    // Foundation does not build a new string here: for an immutable receiver
    // it returns the argument itself, and `-copyWithZone:` above already
    // encodes that rule (retain for an immutable string, a real copy for a
    // mutable one). Going through it rather than alloc/init keeps the two
    // consistent.
    //
    // The identity matters, not just the allocation. An app that releases the
    // result without owning it — which several SDKs do to constant strings,
    // because on Apple's runtime a constant string's release is a no-op — kills
    // a freshly allocated copy instead and crashes when the pool drains. Tap
    // Tap Revenge 3's bundled comScore SDK does exactly this.
    //
    // NSMutableString overrides this; see below.
    let new: id = msg![env; string copy];
    autorelease(env, new)
}

+ (id)stringWithUTF8String:(ConstPtr<u8>)utf8_string {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithUTF8String:utf8_string];
    autorelease(env, new)
}

+ (id)stringWithCString:(ConstPtr<u8>)c_string {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCString:c_string];
    autorelease(env, new)
}

+ (id)stringWithCString:(ConstPtr<u8>)c_string length:(NSUInteger)length {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCString:c_string length:length];
    autorelease(env, new)
}

+ (id)stringWithCString:(ConstPtr<u8>)c_string
               encoding:(NSStringEncoding)encoding {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCString:c_string encoding:encoding];
    autorelease(env, new)
}

+ (id)stringWithContentsOfFile:(id)path { // NSString*
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithContentsOfFile:path];
    autorelease(env, new)
}

+ (id)stringWithContentsOfFile:(id)path // NSString*
                      encoding:(NSStringEncoding)encoding
                         error:(MutPtr<id>)error { // NSError**
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithContentsOfFile:path
                                              encoding:encoding
                                                 error:error];
    autorelease(env, new)
}

// The variant that asks the file what encoding it is in rather than being told.
// It is the modern replacement for the deprecated one-argument form, so an app
// that reads a text file it did not write itself usually calls this one.
+ (id)stringWithContentsOfFile:(id)path // NSString*
                  usedEncoding:(MutPtr<NSStringEncoding>)used_encoding
                         error:(MutPtr<id>)error { // NSError**
    if path == nil {
        report_no_error(env, error);
        return nil;
    }
    let path_string = to_rust_string(env, path).to_string();
    let Ok(bytes) = env.fs.read(GuestPath::new(&path_string)) else {
        report_no_error(env, error);
        return nil;
    };
    let encoding = sniff_encoding(env, &bytes);
    // Written before the string is built, and only on success, which is what
    // Apple documents: a caller reads this to decide how to write the file back
    // out again.
    if !used_encoding.is_null() {
        env.mem.write(used_encoding, encoding);
    }
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithContentsOfFile:path
                                              encoding:encoding
                                                 error:error];
    autorelease(env, new)
}

+ (id)stringWithContentsOfURL:(id)url { // NSURL*
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithContentsOfURL:url];
    autorelease(env, new)
}

+ (id)stringWithContentsOfURL:(id)url // NSURL*
                     encoding:(NSStringEncoding)encoding
                         error:(MutPtr<id>)error { // NSError**
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithContentsOfURL:url
                                             encoding:encoding
                                                error:error];
    autorelease(env, new)
}

+ (id)stringWithFormat:(id)format, // NSString*
                       ...args {
    let res = with_format(env, format, args.start());
    let res = from_rust_string(env, res);
    let res = autorelease(env, res);

    // This will return _tapHLE_NSString or _tapHLE_NSMutableString
    msg![env; this stringWithString:res]
}

+ (id)stringWithCharacters:(ConstPtr<unichar>)characters length:(NSUInteger)length {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCharacters:characters length:length];
    autorelease(env, new)
}

+ (id)pathWithComponents:(id)components {
    let count: NSUInteger = msg![env; components count];
    if count == 0 {
        return get_static_str(env, "");
    }
    let mut res = msg_class![env; NSString new];
    let enumerator: id = msg![env; components objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        let len: NSUInteger = msg![env; next length];
        if len == 0 {
            continue;
        }
        // FIXME: this leads to O(N^2) for N char string, but it should be O(N)
        res = msg![env; res stringByAppendingPathComponent:next];
    }
    log_dbg!("pathWithComponents: {} -> '{}'", {
        let desc = msg![env; components description];
        to_rust_string(env, desc)
    }, to_rust_string(env, res));
    res
}

+ (NSStringEncoding)defaultCStringEncoding {
    // I don't want to figure out what that is on all platforms, and the use
    // I've seen of this method was on ASCII strings, so let's just hardcode
    // UTF-8 and hope that works.
    NSUTF8StringEncoding
}

// NSCoding is a property of the NSString class cluster, not only of the
// immutable concrete class. Archived UITextView content can be an
// NSMutableString, which must decode through this inherited implementation.
//
// Encoding belongs here for the same reason, and for a while did not: it sat on
// _tapHLE_NSString, whose sibling _tapHLE_NSMutableString inherits nothing from
// it, so archiving a mutable string died on a missing selector. The Jim and
// Frank Mysteries hit that saving its state during startup.
- (())encodeWithCoder:(id)coder {
    let string = to_rust_string(env, this);
    assert!(string.as_bytes().iter().all(|byte| byte.is_ascii())); // TODO

    // TODO: use some kind of substitution instead?
    // See "Making Substitutions During Coding" in the doc https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Archiving/Articles/codingobjects.html
    set_value_to_encode_for_current_key(env, coder, plist::Value::String(string.to_string()));
}

- (id)initWithCoder:(id)coder {
    let class: Class = msg![env; coder class];
    let keyed_unarch_class: Class = msg_class![env; NSKeyedUnarchiver class];
    let nib_archive_class: Class = msg_class![env; _tapHLE_NIBArchiveDecoder class];
    let new_str = if env.objc.class_is_subclass_of(class, keyed_unarch_class) {
        ns_keyed_unarchiver::decode_current_string(env, coder)
    } else if env.objc.class_is_subclass_of(class, nib_archive_class) {
        _nib_archive_decoder::decode_current_string(env, coder)
    } else {
        unimplemented!();
    };
    release(env, this);
    new_str
}

- (id)initWithUTF8String:(ConstPtr<u8>)utf8_string {
    msg![env; this initWithCString:utf8_string encoding:NSUTF8StringEncoding]
}

- (id)initWithCString:(ConstPtr<u8>)c_string {
    let encoding: NSStringEncoding = msg_class![env; NSString defaultCStringEncoding];
    msg![env; this initWithCString:c_string encoding:encoding]
}

- (id)initWithCString:(ConstPtr<u8>)c_string length:(NSUInteger)len {
    let encoding: NSStringEncoding = msg_class![env; NSString defaultCStringEncoding];
    msg![env; this initWithBytes:c_string length:len encoding:encoding]
}

- (id)initWithCString:(ConstPtr<u8>)c_string
             encoding:(NSStringEncoding)encoding {
    assert!(C_STRING_FRIENDLY_ENCODINGS.contains(&encoding), "encoding {encoding}");
    let len: NSUInteger = env.mem.cstr_at(c_string).len().try_into().unwrap();
    msg![env; this initWithBytes:c_string length:len encoding:encoding]
}

- (id)dataUsingEncoding:(NSStringEncoding)encoding {
    msg![env; this dataUsingEncoding:encoding allowLossyConversion:false]
}

// These are the two methods that have to be overridden by subclasses, so these
// implementations don't have to care about foreign subclasses.
- (NSUInteger)length {
    let host_object = env.objc.borrow_mut::<StringHostObject>(this);

    // To know what length the string has in UTF-16, we need to convert it to
    // UTF-16. If `length` is used, it's likely other methods that operate on
    // UTF-16 code unit boundaries will also be used (e.g. `characterAt:`), so
    // persisting the UTF-16 version lets us potentially optimize future method
    // calls. This is a heuristic though and won't always be optimal.
    let (utf16, did_convert) = host_object.convert_to_utf16_inplace();
    if did_convert {
        log_dbg!("[{:?} length]: converted string to UTF-16", this);
    }

    utf16.len().try_into().unwrap()
}
- (u16)characterAtIndex:(NSUInteger)index {
    let host_object = env.objc.borrow_mut::<StringHostObject>(this);

    // The string has to be in UTF-16 to get O(1) rather than O(n) indexing, and
    // it's likely this method will be called many times, so converting it to
    // UTF-16 as early as possible and persisting that representation is
    // probably best for performance. This is a heuristic though and won't
    // always be optimal.
    let (utf16, did_convert) = host_object.convert_to_utf16_inplace();
    if did_convert {
        log_dbg!("[{:?} characterAtIndex:{:?}]: converted string to UTF-16", this, index);
    }

    // TODO: raise exception instead of panicking?
    utf16[index as usize]
}

- (NSUInteger)lengthOfBytesUsingEncoding:(NSStringEncoding)encoding {
    // Zero is Foundation's answer for a string that cannot be represented in
    // the encoding, deliberately the same answer as for an empty string: the
    // caller allocates nothing and converts nothing.
    match encode_string(env, this, encoding) {
        Some(bytes) => bytes.len().try_into().unwrap(),
        None => 0,
    }
}

- (NSRange)rangeOfString:(id)search_string {
    msg![env; this rangeOfString:search_string options:0u32]
}

- (NSRange)rangeOfString:(id)search_string
                 options:(NSStringCompareOptions)options { // NSString *
    log_dbg!(
        "[(NSString *){} rangeOfString:{} options:{}]",
        to_rust_string(env, this), to_rust_string(env, search_string), options
    );
    let len: NSUInteger = msg![env; this length];
    let len_search: NSUInteger = msg![env; search_string length];
    if len_search == 0 {
        return NSRange { location: NSNotFound as NSUInteger, length: 0 };
    }
    // TODO: other search options
    // TODO: OR'ing of options
    match options {
        // 0 is for default options, which is NSLiteralSearch
        NSLiteralSearch | 0 => {
            for i in 0..len {
                if is_match_at_position(env, this, search_string, i, len, len_search, |a, b| a == b) {
                    return NSRange { location: i, length: len_search }
                }
            }
        },
        NSCaseInsensitiveSearch => {
            let compare = |a, b| {
                let (Some(a_c), Some(b_c)) = (char::from_u32(a as u32), char::from_u32(b as u32)) else {
                    panic!("Invalid chars in the strings!");
                };
                a_c.to_lowercase().eq(b_c.to_lowercase())
            };
            for i in 0..len {
                if is_match_at_position(env, this, search_string, i, len, len_search, compare) {
                    return NSRange { location: i, length: len_search }
                }
            }
        },
        NSBackwardsSearch => {
            for i in (0..len).rev() {
                if is_match_at_position(env, this, search_string, i, len, len_search, |a, b| a == b) {
                    return NSRange { location: i, length: len_search }
                }
            }
        },
        _ => unimplemented!("options {}", options)
    }
    NSRange { location: NSNotFound as NSUInteger, length: 0 }
}

- (NSRange)rangeOfCharacterFromSet:(id)set { // NSCharacterSet *
    let length: NSUInteger = msg![env; this length];
    let range = NSRange { location: 0, length };
    msg![env; this rangeOfCharacterFromSet:set options:0u32 range:range]
}

- (NSRange)rangeOfCharacterFromSet:(id)set // NSCharacterSet *
                          options:(NSStringCompareOptions)options
                            range:(NSRange)search_range {
    let length: NSUInteger = msg![env; this length];
    let end = search_range.location.checked_add(search_range.length).unwrap();
    assert!(end <= length);
    // Character-set membership is unaffected by literal/case-insensitive flags;
    // only NSBackwardsSearch changes behaviour, selecting the last match rather
    // than the first. Scanning the whole range and keeping the last hit handles
    // both directions.
    let backwards = options & NSBackwardsSearch != 0;
    let mut found: Option<NSUInteger> = None;
    let mut idx = search_range.location;
    while idx < end {
        let c: u16 = msg![env; this characterAtIndex:idx];
        if msg![env; set characterIsMember:c] {
            found = Some(idx);
            if !backwards {
                break;
            }
        }
        idx += 1;
    }
    match found {
        Some(i) => NSRange { location: i, length: 1 },
        None => NSRange { location: NSNotFound as NSUInteger, length: 0 },
    }
}

- (id)description {
    this
}
// TODO: debugDescription, localized description (is that a thing for NSString?)

- (NSUInteger)hash {
    // TODO: avoid copying
    super::hash_helper(&to_rust_string(env, this))
}
- (bool)isEqual:(id)other {
    if this == other {
        return true;
    }
    let class: Class = msg_class![env; NSString class];
    if !msg![env; other isKindOfClass:class] {
        return false;
    }
    // TODO: avoid copying
    to_rust_string(env, this) == to_rust_string(env, other)
}
- (bool)isEqualToString:(id)other { // NSString*
    if this == other {
        return true;
    }
    if other == nil {
        return false;
    }
    // TODO: avoid copying
    to_rust_string(env, this) == to_rust_string(env, other)
}

- (bool)hasPrefix:(id)str { // NSString*
    // TODO: avoid copying
    let str = to_rust_string(env, str).to_string();
    to_rust_string(env, this).starts_with(&str)
}

- (bool)hasSuffix:(id)str { // NSString*
    // TODO: avoid copying
    let str = to_rust_string(env, str).to_string();
    to_rust_string(env, this).ends_with(&str)
}

// The two localized comparisons. tapHLE has no locale-aware collation, so each
// is its unlocalized counterpart, and the difference only shows on text a
// locale would order differently.
//
// Neither asserts that its operands are ASCII any more. They used to, and that
// turned an ordinary sort of ordinary text into a dead app — non-ASCII is not a
// programming error, it is a name with an accent in it. Comparing by code unit
// is the wrong *order* for such text, which is a defect worth fixing when
// collation exists; ending the app is not a better answer than the wrong order.
//
// TODO: use the current locale, and support compatibility equivalence in the
// Unicode standard. More info: https://www.objc.io/issues/9-strings/unicode/
- (NSComparisonResult)localizedCompare:(id)other { // NSString*
    log_once!("TODO: -[NSString localizedCompare:] ignores the locale");
    msg![env; this compare:other]
}

- (NSComparisonResult)localizedCaseInsensitiveCompare:(id)other { // NSString*
    log_once!("TODO: -[NSString localizedCaseInsensitiveCompare:] ignores the locale");
    msg![env; this caseInsensitiveCompare:other]
}

- (NSComparisonResult)compare:(id)other { // NSString*
    msg![env; this compare:other options:NSLiteralSearch]
}

- (NSComparisonResult)caseInsensitiveCompare:(id)other { //NSString*
    msg![env; this compare:other options:NSCaseInsensitiveSearch]
}

- (NSComparisonResult)compare:(id)other // NSString*
                      options:(NSStringCompareOptions)options
                        range:(NSRange)range {
    // TODO: avoid substring copying
    let substr = msg![env; this substringWithRange:range];
    msg![env; substr compare:other options:options]
}

- (NSComparisonResult)compare:(id)other options:(NSStringCompareOptions)mask { // NSString*
    fn ascii_number(iter: &mut Peekable<CodeUnitIterator>, leftmost_digit: char) -> u32 {
        let mut num = leftmost_digit.to_digit(10).unwrap();
        while let Some(a_digit_char) = iter.next_if(
            |&x| char::from_u32(x as u32).is_some_and(|y| y.is_ascii_digit())
        ) {
            num = num * 10 + char::from_u32(a_digit_char as u32).unwrap().to_digit(10).unwrap();
        }
        num
    }

    // Comparing against nil is a programming error Apple answers with an
    // exception, and tapHLE has nowhere to raise one, so the choice is between
    // ending the app and giving the comparison a defined answer. A defined
    // answer is far better: an app that looks up a setting that is not there
    // and compares the nil it got back is doing something harmless, and killing
    // it loses everything after that point. Any string sorts after nothing.
    if other == nil {
        log!("Warning: -[NSString compare:] with a nil argument; ordering it after nil");
        return NSOrderedDescending;
    }

    // TODO: support foreign subclasses (perhaps via a helper function that
    // copies the string first)
    let mut a_iter = env.objc.borrow::<StringHostObject>(this).iter_code_units().peekable();
    let mut b_iter = env.objc.borrow::<StringHostObject>(other).iter_code_units().peekable();

    // By default, no mask is a literal search
    let mask = if mask == 0 {
        NSLiteralSearch
    } else {
        mask
    };

    // TODO: OR'ing of compare options
    match mask {
        NSCaseInsensitiveSearch => {
            loop {
                let a_next = a_iter.next();
                let b_next = b_iter.next();
                let (Some(a_unit), Some(b_unit)) = (a_next, b_next) else {
                    return from_rust_ordering(a_next.cmp(&b_next));
                };
                let (Some(a_c), Some(b_c)) = (char::from_u32(a_unit as u32), char::from_u32(b_unit as u32)) else {
                    panic!("Invalid chars in the strings!");
                };

                let insensitive_order = a_c.to_lowercase().cmp(b_c.to_lowercase());
                if insensitive_order != std::cmp::Ordering::Equal {
                    return from_rust_ordering(insensitive_order);
                }
            }
        },
        NSLiteralSearch => {
            from_rust_ordering(a_iter.cmp(b_iter))
        },
        NSNumericSearch => {
            loop {
                let a_next = a_iter.next();
                let b_next = b_iter.next();
                let (Some(a_unit), Some(b_unit)) = (a_next, b_next) else {
                    return from_rust_ordering(a_next.cmp(&b_next));
                };
                let (Some(a_c), Some(b_c)) = (char::from_u32(a_unit as u32), char::from_u32(b_unit as u32)) else {
                    panic!("Invalid chars in the strings!");
                };

                if a_c.is_ascii_digit() && b_c.is_ascii_digit() {
                    let a_int = ascii_number(&mut a_iter, a_c);
                    let b_int = ascii_number(&mut b_iter, b_c);

                    let numeric_order = a_int.cmp(&b_int);
                    if numeric_order != std::cmp::Ordering::Equal {
                        return from_rust_ordering(numeric_order);
                    }
                } else {
                    let char_order = a_c.cmp(&b_c);
                    if char_order != std::cmp::Ordering::Equal {
                        return from_rust_ordering(char_order);
                    }
                }
            }
        },
        mask => unimplemented!("Other mask: {mask}"),
    }
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

// NSMutableCopying implementation
- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    let str_mut: id = msg_class![env; NSMutableString alloc];
    // TODO: use `initWithString:`
    let str_mut: id = msg![env; str_mut init];
    () = msg![env; str_mut setString:this];
    str_mut
}

- (bool)getFileSystemRepresentation:(MutPtr<u8>)buffer
                          maxLength:(NSUInteger)buffer_size {
    msg![env; this getCString:buffer
                    maxLength:buffer_size
                     encoding:NSUTF8StringEncoding]
}

- (bool)getCString:(MutPtr<u8>)buffer
         maxLength:(NSUInteger)buffer_size
          encoding:(NSStringEncoding)encoding {
    get_bytes_buffer_inner(env, this, buffer, buffer_size, encoding, true)
}
- (())getCString:(MutPtr<u8>)buffer {
    let encoding: NSStringEncoding = msg_class![env; NSString defaultCStringEncoding];

    // Prevent slice out-of-range error
    let length = (u32::MAX - buffer.to_bits()).min(NSMaximumStringLength);
    let res: bool = msg![env; this getCString:buffer
                                    maxLength:length
                                     encoding:encoding];
    assert!(res);
}

- (id)componentsSeparatedByString:(id)separator { // NSString*
    // TODO: support foreign subclasses (perhaps via a helper function that
    // copies the string first)
    let mut main_iter = env.objc.borrow::<StringHostObject>(this)
        .iter_code_units();
    let sep_iter = env.objc.borrow::<StringHostObject>(separator)
        .iter_code_units();

    // TODO: zero-length separator support
    assert!(sep_iter.clone().next().is_some());

    let mut components = Vec::<Utf16String>::new();
    let mut current_component: Utf16String = Vec::new();
    loop {
        if let Some(new_main_iter) = main_iter.strip_prefix(&sep_iter, /* case_insensitive: */ false) {
            // matched separator, end current component
            components.push(std::mem::take(&mut current_component));
            main_iter = new_main_iter;
        } else {
            // no separator match, extend the current component
            match main_iter.next() {
                Some(cur) => current_component.push(cur),
                None => break,
            }
        }
    }
    components.push(current_component);

    // TODO: For a foreign subclass of NSString, do we have to return that
    // subclass? The signature implies this isn't the case and it's probably not
    // worth the effort, but it's an interesting question.
    let class = env.objc.get_known_class("_tapHLE_NSString", &mut env.mem);

    let component_ns_strings = components.drain(..).map(|utf16| {
        let host_object = Box::new(StringHostObject::Utf16(utf16));
        env.objc.alloc_object(class, host_object, &mut env.mem)
    }).collect();
    let array = ns_array::from_vec(env, component_ns_strings);
    autorelease(env, array)
}

- (())getCharacters:(MutPtr<unichar>)buffer
              range:(NSRange)range {
    // TODO: avoid copying
    let ranged = msg![env; this substringWithRange:range];
    msg![env; ranged getCharacters:buffer]
}

- (())getCharacters:(MutPtr<unichar>)buffer {
    let host_object = env.objc.borrow_mut::<StringHostObject>(this);

    // this conversion maybe not most optimal heuristic
    let (utf16, did_convert) = host_object.convert_to_utf16_inplace();
    if did_convert {
        log_dbg!("[{:?} getCharacters:{:?}]: converted string to UTF-16", this, buffer);
    }

    let len: GuestUSize = guest_size_of::<unichar>() * utf16.len() as GuestUSize;
    let tmp_vec: Vec<u8> = utf16.iter().flat_map(|c| u16::to_le_bytes(*c)).collect();
    _ = env.mem.bytes_at_mut(buffer.cast(), len).write(tmp_vec.as_slice()).unwrap();
}

- (ConstPtr<u8>)cStringUsingEncoding:(NSStringEncoding)encoding {
    // TODO: avoid copying
    let string = to_rust_string(env, this);
    // TODO: other encodings
    let bytes: Vec<u8> = match encoding {
        NSASCIIStringEncoding | NSMacOSRomanStringEncoding | NSISOLatin1StringEncoding => {
            // TODO: properly support Mac OS Roman and ISO Latin 1 encodings.
            // The first 128 characters are identical to the ASCII
            assert!(string.as_bytes().iter().all(|byte| byte.is_ascii()));
            string.as_bytes().to_vec()
        },
        NSUTF8StringEncoding => {
            string.as_bytes().to_vec()
        },
        NSUTF16LittleEndianStringEncoding => string.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        _ => unimplemented!("{}", encoding),
    };
    let null_size: GuestUSize = match encoding {
        NSUTF8StringEncoding | NSASCIIStringEncoding | NSMacOSRomanStringEncoding | NSISOLatin1StringEncoding => 1,
        NSUTF16LittleEndianStringEncoding => 2,
        _ => unimplemented!()
    };
    let bytes_size = bytes.len() as GuestUSize;
    let total_size: GuestUSize = bytes_size + null_size;
    let c_string: MutPtr<u8> = env.mem.alloc(total_size).cast();
    _ = env.mem.bytes_at_mut(c_string, bytes_size).write(&bytes).unwrap();
    assert_eq!(env.mem.read(c_string + total_size - 1), b'\0');
    // NSData will handle releasing the string (it is autoreleased)
    let _: id = msg_class![env; NSData dataWithBytesNoCopy:(c_string.cast_void())
                                                    length:total_size];
    c_string.cast_const()
}

- (ConstPtr<u8>)cString {
    // TODO: use default C-string encoding of the current locale
    // TODO: raise NSCharacterConversionException if couldn't represent
    msg![env; this UTF8String]
}

- (ConstPtr<u8>)UTF8String {
    msg![env; this cStringUsingEncoding:NSUTF8StringEncoding]
}

- (id)substringToIndex:(NSUInteger)to {
    let mut res_utf16: Utf16String = Vec::with_capacity(to as usize);

    for_each_code_unit(env, this, |idx, c| {
        if idx < to {
            res_utf16.push(c);
        }
    });

    let res = msg_class![env; _tapHLE_NSString alloc];
    *env.objc.borrow_mut(res) = StringHostObject::Utf16(res_utf16);
    autorelease(env, res)
}

- (id)substringFromIndex:(NSUInteger)from {
    let mut res_utf16: Utf16String = Vec::with_capacity(from as usize);

    for_each_code_unit(env, this, |idx, c| {
        if idx >= from {
            res_utf16.push(c);
        }
    });

    let res = msg_class![env; _tapHLE_NSString alloc];
    *env.objc.borrow_mut(res) = StringHostObject::Utf16(res_utf16);
    autorelease(env, res)
}

- (id)stringByReplacingCharactersInRange:(NSRange)range
                              withString:(id)replacement { // NSString*
    // 425 of the 1192 distinct apps in the import-demand catalogue send this.
    // Built from the three substring primitives rather than by splicing UTF-16
    // directly, so it inherits their code-unit indexing: the range is in UTF-16
    // code units, which is what the caller's -length and -rangeOfString: gave
    // it.
    let length: NSUInteger = msg![env; this length];
    let end = range.location.checked_add(range.length).unwrap();
    assert!(end <= length, "range {:?} is outside a string of {}", range, length);

    let before: id = msg![env; this substringToIndex:(range.location)];
    let after: id = msg![env; this substringFromIndex:end];
    // A nil replacement is a deletion. Cocoa raises for it, but the range work
    // is already done and returning the receiver minus the range is closer to
    // what the caller meant than ending the app.
    let joined: id = if replacement == nil {
        log!("Warning: -[NSString stringByReplacingCharactersInRange:withString:nil], deleting the range");
        msg![env; before stringByAppendingString:after]
    } else {
        let with_replacement: id = msg![env; before stringByAppendingString:replacement];
        msg![env; with_replacement stringByAppendingString:after]
    };
    autorelease(env, joined)
}

- (id)stringByPaddingToLength:(NSUInteger)new_length
                   withString:(id)pad_string // NSString*
                  startingAtIndex:(NSUInteger)pad_index {
    // 487 apps send this. It both pads and truncates: a new length shorter than
    // the receiver is a truncation, which is how it gets used to fit a string
    // into a fixed-width field.
    let length: NSUInteger = msg![env; this length];
    if new_length <= length {
        let truncated: id = msg![env; this substringToIndex:new_length];
        return autorelease(env, truncated);
    }

    // The pad string is repeated, and `pad_index` says where in it to start -
    // not where in the receiver. Getting that backwards is the obvious mistake
    // here, and it is why the parameter is named for the pad.
    let pad_length: NSUInteger = if pad_string == nil {
        0
    } else {
        msg![env; pad_string length]
    };
    assert!(
        pad_length > 0,
        "-[NSString stringByPaddingToLength:withString:startingAtIndex:] needs a \
         non-empty pad string to reach {}",
        new_length
    );
    assert!(pad_index < pad_length);

    let mut padded: Utf16String = Vec::with_capacity(new_length as usize);
    for_each_code_unit(env, this, |_idx, c| padded.push(c));
    let mut pad_units: Utf16String = Vec::with_capacity(pad_length as usize);
    for_each_code_unit(env, pad_string, |_idx, c| pad_units.push(c));

    let mut cursor = pad_index as usize;
    while (padded.len() as NSUInteger) < new_length {
        padded.push(pad_units[cursor]);
        cursor = (cursor + 1) % pad_units.len();
    }

    let res = msg_class![env; _tapHLE_NSString alloc];
    *env.objc.borrow_mut(res) = StringHostObject::Utf16(padded);
    autorelease(env, res)
}

- (id)stringByTrimmingCharactersInSet:(id)set { // NSCharacterSet*
    let initial_length: NSUInteger = msg![env; this length];

    let mut res_start: NSUInteger = 0;
    let mut res_end = initial_length;

    while res_start < initial_length {
        let c: u16 = msg![env; this characterAtIndex:res_start];
        if msg![env; set characterIsMember:c] {
            res_start += 1;
        } else {
            break;
        }
    }

    while res_end > res_start {
        let c: u16 = msg![env; this characterAtIndex:(res_end - 1)];
        if msg![env; set characterIsMember:c] {
            res_end -= 1;
        } else {
            break;
        }
    }

    assert!(res_end >= res_start);
    let res_length = res_end - res_start;

    if res_length == initial_length {
        let ret = msg![env; this copy];
        autorelease(env, ret)
    } else {
        let range = NSRange{ location: res_start, length: res_length };
        let string: id = msg![env; this substringWithRange:range];
        string
    }
}

- (id)stringByReplacingOccurrencesOfString:(id)target // NSString*
                                withString:(id)replacement { // NSString*
    let length: NSUInteger = msg![env; this length];
    let range = NSRange { location: 0, length };
    msg![env; this stringByReplacingOccurrencesOfString:target
                                             withString:replacement
                                                options:0u32
                                                  range:range]
}

- (id)stringByReplacingOccurrencesOfString:(id)target // NSString*
                                withString:(id)replacement // NSString*
                                   options:(NSStringCompareOptions)options
                                     range:(NSRange)range {
    let loc = range.location;
    let len = range.length;
    let left: id = msg![env; this substringToIndex:loc];
    let middle: id = msg![env; this substringWithRange:range];
    let right: id = msg![env; this substringFromIndex:(loc + len)];
    let (new_middle, _) =
        string_by_replacing_occurrences_inner(env, middle, target, replacement, options);
    let res: id = msg![env; left stringByAppendingString:new_middle];
    msg![env; res stringByAppendingString:right]
}

- (id)stringByAppendingString:(id)other { // NSString*
    assert!(other != nil); // TODO: raise exception

    // TODO: ideally, don't convert to UTF-16 here
    let this_len: NSUInteger = msg![env; this length];
    let other_len: NSUInteger = msg![env; other length];
    let mut new_utf16 = Vec::with_capacity((this_len + other_len) as usize);
    for_each_code_unit(env, this, |_idx, c| {
        new_utf16.push(c);
    });
    for_each_code_unit(env, other, |_idx, c| {
        new_utf16.push(c);
    });

    // TODO: For a foreign subclass of NSString, do we have to return that
    // subclass? The signature implies this isn't the case and it's probably not
    // worth the effort, but it's an interesting question.
    let class = env.objc.get_known_class("_tapHLE_NSString", &mut env.mem);
    let host_object = Box::new(StringHostObject::Utf16(new_utf16));
    env.objc.alloc_object(class, host_object, &mut env.mem)
}

- (id)stringByAppendingFormat:(id)format, ...args {
    let new_string = with_format(env, format,  args.start());
    let new_string = from_rust_string(env, new_string);
    let new_string = msg![env; this stringByAppendingString:new_string];
    autorelease(env, new_string)
}

- (id)stringByDeletingLastPathComponent {
    let string = to_rust_string(env, this); // TODO: avoid copying
    let (res, _) = path_algorithms::split_last_path_component(&string);
    let new_string = from_rust_string(env, String::from(res));
    autorelease(env, new_string)
}

- (id)lastPathComponent {
    let string = to_rust_string(env, this); // TODO: avoid copying
    let (_, res) = path_algorithms::split_last_path_component(&string);
    let new_string = from_rust_string(env, String::from(res));
    autorelease(env, new_string)
}

- (id)pathComponents {
    let string = to_rust_string(env, this); // TODO: avoid copying
    let vec = path_algorithms::split_path_components(&string);
    let vec = vec.iter().map(|component| {
        from_rust_string(env, component.to_string())
    }).collect();
    let array = ns_array::from_vec(env, vec);
    autorelease(env, array)
}

- (id)stringByDeletingPathExtension {
    let string = to_rust_string(env, this); // TODO: avoid copying
    let (res, _) = path_algorithms::split_path_extension(&string);
    let new_string = from_rust_string(env, String::from(res));
    autorelease(env, new_string)
}

- (id)pathExtension {
    let string = to_rust_string(env, this); // TODO: avoid copying
    let (_, res) = path_algorithms::split_path_extension(&string);
    let new_string = from_rust_string(env, String::from(res));
    autorelease(env, new_string)
}

- (ConstPtr<u8>)fileSystemRepresentation {
    let file_manager: id = msg_class![env; NSFileManager defaultManager];
    // This behavior was confirmed on the iOS Simulator
    msg![env; file_manager fileSystemRepresentationWithPath:this]
}

- (id)stringByAddingPercentEscapesUsingEncoding:(NSStringEncoding)encoding {
    assert!(encoding == NSASCIIStringEncoding || encoding == NSUTF8StringEncoding); // TODO: other encodings
    let str = to_rust_string(env, this);
    // RFC 2396 leaves unreserved and reserved characters untouched. Escape
    // all other UTF-8 bytes, including '[' and ']', as iPhone OS does.
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut escaped = String::with_capacity(str.len());
    for &byte in str.as_bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~!*'();:@&=+$,/?%#".contains(&byte) {
            escaped.push(byte as char);
        } else {
            escaped.push('%');
            escaped.push(HEX[(byte >> 4) as usize] as char);
            escaped.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    let new = from_rust_string(env, escaped);
    autorelease(env, new)
}

- (id)stringByReplacingPercentEscapesUsingEncoding:(NSStringEncoding)encoding {
    // The inverse of -stringByAddingPercentEscapesUsingEncoding: above, and by
    // some distance the most-sent selector no tapHLE class implemented: 653
    // of the 1192 distinct apps in the import-demand catalogue send it,
    // because any app reading a value back out of a URL needs it.
    assert!(encoding == NSASCIIStringEncoding || encoding == NSUTF8StringEncoding); // TODO: other encodings
    let str = to_rust_string(env, this);
    match unescape_percent_escapes(&str) {
        Some(decoded) => {
            let new = from_rust_string(env, decoded);
            autorelease(env, new)
        }
        None => {
            // Cocoa returns nil when the escapes do not decode in the requested
            // encoding, and callers check for it, so this is the specified
            // answer rather than a failure to handle the input.
            log_dbg!(
                "-[NSString stringByReplacingPercentEscapesUsingEncoding:] => nil, \
                 {:?} does not decode",
                str
            );
            nil
        }
    }
}

- (id)stringByAppendingPathComponent:(id)component { // NSString*
    // Cocoa raises NSInvalidArgumentException for a nil component. tapHLE
    // cannot raise into guest code, and aborting the emulator is a far worse
    // answer than the app's own mistake deserves, so treat nil as "nothing to
    // append" and return the receiver. Log it, because a nil here usually
    // means an earlier lookup silently failed and that is the real bug.
    if component == nil {
        log!("Warning: -[NSString stringByAppendingPathComponent:nil], returning the receiver unchanged");
        return this;
    }
    // TODO: avoid copying
    let base_str = to_rust_string(env, this);
    let component_str = to_rust_string(env, component);
    let res = path_algorithms::string_by_appending_path_component(&base_str, &component_str);
    log_dbg!("'{}' + '{}' -> '{}'", base_str, component_str, res);
    let new_string = from_rust_string(env, res);
    autorelease(env, new_string)
}

- (id)stringByAppendingPathExtension:(id)extension { // NSString*
    // See the note on -stringByAppendingPathComponent:.
    if extension == nil {
        log!("Warning: -[NSString stringByAppendingPathExtension:nil], returning the receiver unchanged");
        return this;
    }
    // FIXME: handle edge cases like trailing '/' (may differ from Rust!)
    let mut combined = to_rust_string(env, this).into_owned();
    // TODO: avoid copying
    let extension_string = to_rust_string(env, extension);
    if !extension_string.is_empty(){
        combined.push('.');
        combined.push_str(&extension_string);
    }

    let new_string = from_rust_string(env, combined);
    autorelease(env, new_string)
}

- (id)stringByExpandingTildeInPath {
    let path = to_rust_string(env, this);

    let new_path_str = if let Some(new_path) = path.strip_prefix('~') {
        // ~ and anything up until the first / is stripped
        // This was confirmed using a test app on iOS
        // Examples (of what is placed after home directory):
        //  "~"            -> ""
        //  "~/"           -> ""
        //  "~user"        -> ""
        //  "~/Documents"  -> "/Documents"
        //  "~foo/bar"     -> "/bar"
        //  "~~foo/bar"    -> "/bar"
        let within_home_dir = new_path.split_once('/').map(|x| x.1).unwrap_or("");

        let guest_path = env.fs.home_directory().join(within_home_dir);
        let resolved = fs::resolve_path(&guest_path, None);
        format!("/{}", resolved.join("/"))
    } else {
        // If called on a path with no leading ~ do nothing
        path.to_string()
    };

    log_dbg!("[(NSString *){:?} stringByExpandingTildeInPath] {} -> {}", this, path, new_path_str);

    let new_string = from_rust_string(env, new_path_str);
    autorelease(env, new_string)
}

- (id)stringByStandardizingPath {
    let expanded: id = msg![env; this stringByExpandingTildeInPath];
    let path = to_rust_string(env, expanded); // TODO: avoid copying
    // TODO: Removing an initial component of "/private/var/automount",
    //       "/var/automount”, or "/private” from the path
    assert!(!path.starts_with("/private"));
    assert!(!path.starts_with("/var/automount"));
    // Removing a trailing slash from the last component.
    let path = path_algorithms::trim_trailing_slashes(&path);
    let new_path_str = if path.starts_with('/') {
        // For absolute paths, resolve_path already reduces empty components
        // ("//"), current-directory references ("/./") and parent references
        // ("/.."), which is exactly the reduction this method must perform.
        // Note: while we are using an fs function, it's just string
        // manipulation here.
        let resolved = fs::resolve_path(GuestPath::new(path), None);
        format!("/{}", resolved.join("/"))
    } else {
        // For relative paths, reduce empty components and current-directory
        // references. Apple leaves a leading ".." unresolved for these, so we
        // do not pop parent references here.
        path.split('/')
            .filter(|component| !component.is_empty() && *component != ".")
            .collect::<Vec<_>>()
            .join("/")
    };
    log_dbg!("[(NSString *){:?} stringByStandardizingPath] {} -> {}", this, to_rust_string(env, this), new_path_str);
    let new_string = from_rust_string(env, new_path_str);
    autorelease(env, new_string)
}

- (id)stringsByAppendingPaths:(id)paths {
    let count: NSUInteger = msg![env; paths count];
    let mut_arr: id = msg_class![env; NSMutableArray new];
    for i in 0..count {
        let path: id = msg![env; paths objectAtIndex:i];
        let new: id = msg![env; this stringByAppendingPathComponent:path];
        () = msg![env; mut_arr addObject:new];
    }
    let arr = msg![env; mut_arr copy];
    release(env, mut_arr);
    autorelease(env, arr)
}

// These come from a category in UIKit (UIStringDrawing).
// TODO: Implement categories so we can completely move the code to UIFont.
// TODO: More `sizeWithFont:` variants
- (CGSize)sizeWithFont:(id)font { // UIFont*
    // TODO: avoid copy
    let text = to_rust_string(env, this);
    ui_font::size_with_font(env, font, &text, None)
}
// The `forWidth:` measurement, whose drawing counterpart
// -drawAtPoint:forWidth:withFont:lineBreakMode: is already below. UIKit keeps
// this family on one line and lets the line break mode decide what happens to
// the text that does not fit, where the `constrainedToSize:` family wraps into
// a box. Both reach the same width-and-mode calculation here, which is exactly
// what the drawing counterpart does — so a string measured this way and then
// drawn that way agrees with itself, which is the property a caller is relying
// on when it uses the two together.
- (CGSize)sizeWithFont:(id)font // UIFont*
              forWidth:(CGFloat)width
         lineBreakMode:(UILineBreakMode)line_break_mode {
    // TODO: avoid copy
    let text = to_rust_string(env, this);
    // Only the width is consulted; the height stands for "as tall as it needs
    // to be", which is what a single line asks for.
    let size = CGSize { width, height: CGFloat::INFINITY };
    ui_font::size_with_font(env, font, &text, Some((size, line_break_mode)))
}
- (CGSize)sizeWithFont:(id)font // UIFont*
     constrainedToSize:(CGSize)size {
    msg![env; this sizeWithFont:font
              constrainedToSize:size
                  lineBreakMode:UILineBreakModeWordWrap]
}
- (CGSize)sizeWithFont:(id)font // UIFont*
     constrainedToSize:(CGSize)size
         lineBreakMode:(UILineBreakMode)line_break_mode {
    // TODO: avoid copy
    let text = to_rust_string(env, this);
    ui_font::size_with_font(env, font, &text, Some((size, line_break_mode)))
}

- (CGSize)drawAtPoint:(CGPoint)point
             withFont:(id)font { // UIFont*
    // TODO: avoid copy
    let text = to_rust_string(env, this);
    ui_font::draw_at_point(env, font, &text, point, None)
}

- (CGSize)drawAtPoint:(CGPoint)point
             forWidth:(CGFloat)width
             withFont:(id)font // UIFont*
        lineBreakMode:(UILineBreakMode)line_break_mode {
    // TODO: avoid copy
    let text = to_rust_string(env, this);
    ui_font::draw_at_point(env, font, &text, point, Some((width, line_break_mode)))
}

- (CGSize)drawInRect:(CGRect)rect
            withFont:(id)font { // UIFont*
    msg![env; this drawInRect:rect
                     withFont:font
                lineBreakMode:UILineBreakModeWordWrap
                    alignment:UITextAlignmentLeft]
}
- (CGSize)drawInRect:(CGRect)rect
            withFont:(id)font // UIFont*
       lineBreakMode:(UILineBreakMode)line_break_mode {
    msg![env; this drawInRect:rect
                     withFont:font
                lineBreakMode:line_break_mode
                    alignment:UITextAlignmentLeft]
}
- (CGSize)drawInRect:(CGRect)rect
            withFont:(id)font // UIFont*
       lineBreakMode:(UILineBreakMode)line_break_mode
           alignment:(UITextAlignment)align {
    // TODO: avoid copy
    let text = to_rust_string(env, this);
    ui_font::draw_in_rect(env, font, &text, rect, line_break_mode, align)
}

- (bool)writeToFile:(id)path // NSString*
         atomically:(bool)use_aux_file {
    let encoding: NSStringEncoding = msg_class![env; NSString defaultCStringEncoding];
    let error: MutPtr<id> = Ptr::null();
    msg![env; this writeToFile:path atomically:use_aux_file encoding:encoding error:error]
}

- (bool)writeToFile:(id)path // NSString*
         atomically:(bool)use_aux_file
           encoding:(NSStringEncoding)encoding
              error:(MutPtr<id>)error { // NSError**
    assert!(encoding == NSUTF8StringEncoding || encoding == NSASCIIStringEncoding);

    let string = to_rust_string(env, this);
    let c_string = env.mem.alloc_and_write_cstr(string.as_bytes());
    // This should not include a NULL terminator!
    let length: NSUInteger = string.len().try_into().unwrap();
    // NSData will handle releasing the string (it is autoreleased)
    let data: id = msg_class![env; NSData dataWithBytesNoCopy:(c_string.cast_void())
                                                    length:length];

    // TODO: write extended attributes about text encoding
    let success: bool = msg![env; data writeToFile:path atomically:use_aux_file];
    if !success {
        // Writing into a directory that does not exist is the ordinary way this
        // fails, and an app that asked for an NSError** wants to be told so
        // rather than have the emulator stop.
        write_error(env, error, NSFileWriteUnknownError);
    }
    success
}

- (f32)floatValue {
    float_value_common(env, this)
}
- (f64)doubleValue {
    float_value_common(env, this)
}

- (NSInteger)integerValue {
    msg![env; this intValue]
}
- (i32)intValue {
    let st = to_rust_string(env, this);
    let st = st.trim_start();
    let mut cutoff = st.len();
    for (i, c) in st.char_indices() {
        if !c.is_ascii_digit() && c != '+' && c != '-' {
            cutoff = i;
            break;
        }
    }
    // TODO: handle over/underflow properly
    st[..cutoff].parse().unwrap_or(0)
}

- (id)lowercaseString {
    // TODO: check if rust methods are consistent with ObjC one
    let str = to_rust_string(env, this).to_lowercase();
    let res = from_rust_string(env, str);
    autorelease(env, res)
}

- (id)uppercaseString {
    // TODO: check if rust methods are consistent with ObjC one
    let str = to_rust_string(env, this).to_uppercase();
    let res = from_rust_string(env, str);
    autorelease(env, res)
}

@end

// NSMutableString is an abstract class. Apple's contract is that a subclass
// provides everything NSString provides, plus the single mutation primitive:
// - (void)replaceCharactersInRange:(NSRange)range withString:(NSString)string;
// Here that primitive is written once, in terms of `setString:` and the
// substring methods, so every mutator below — and any subclass — gets it from
// this class rather than having to reimplement it.
// Note that it inherits from NSString, so we must ensure we override any
// default methods that would be inappropriate for mutability.
@implementation NSMutableString: NSString

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSMutableString might be subclassed by something
    // which needs allocWithZone: to have the normal behaviour.
    // Unimplemented: call superclass alloc then.
    assert!(this == env.objc.get_known_class("NSMutableString", &mut env.mem));
    msg_class![env; _tapHLE_NSMutableString allocWithZone:zone]
}

// NSString's version returns the argument itself when it can, which would hand
// back an immutable string to a caller that asked NSMutableString for one.
+ (id)stringWithString:(id)string { // NSString*
    let new: id = msg![env; string mutableCopy];
    autorelease(env, new)
}

+ (id)stringWithCapacity:(NSUInteger)capacity {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCapacity:capacity];
    autorelease(env, new)
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    let new: id = msg_class![env; NSString alloc];
    msg![env; new initWithString:this]
}

- (())replaceCharactersInRange:(NSRange)range
                    withString:(id)a_string { // NSString*
    assert_ne!(a_string, nil);

    let length: NSUInteger = msg![env; this length];
    let location = range.location;
    assert!(location <= length);
    assert!(range.length <= length - location);

    // Replacing a range with the empty string is a deletion and replacing an
    // empty range is an insertion, so this one method covers both: keep what is
    // on either side of the range and put the new text between them.
    let left: id = msg![env; this substringToIndex:location];
    let right: id = msg![env; this substringFromIndex:(location + range.length)];
    let res: id = msg![env; left stringByAppendingString:a_string];
    let res: id = msg![env; res stringByAppendingString:right];
    () = msg![env; this setString:res];
}

- (())appendString:(id)a_string { // NSString*
    assert_ne!(a_string, nil);
    // TODO: this is inefficient? append in place instead
    let new: id = msg![env; this stringByAppendingString:a_string];
    () = msg![env; this setString:new];
}

- (())deleteCharactersInRange:(NSRange)range {
    let location = range.location;
    let length = range.length;

    let left: id = if location == 0 {
        get_static_str(env, "")
    } else {
        let left_range = NSRange {
            location: 0,
            length: location,
        };
        msg![env; this substringWithRange:left_range]
    };

    let idx_after_removal = location + length;
    let lenght_str: NSUInteger = msg![env; this length];
    let right: id = if idx_after_removal == lenght_str {
        get_static_str(env, "")
    } else {
        let right_range = NSRange {
            location: idx_after_removal,
            length: lenght_str - idx_after_removal,
        };
        msg![env; this substringWithRange:right_range]
    };

    let res: id = msg![env; left stringByAppendingString:right];
    () = msg![env; this setString:res];
}

- (())insertString:(id)a_string // NSString*
           atIndex:(NSUInteger)index {
    // Concatenate the text before `index`, the inserted string, and the text
    // after `index`. Inefficient (rebuilds the whole string) but matches the
    // primitive-based approach of the other mutators above.
    let length: NSUInteger = msg![env; this length];
    let left: id = if index == 0 {
        get_static_str(env, "")
    } else {
        let left_range = NSRange { location: 0, length: index };
        msg![env; this substringWithRange:left_range]
    };
    let right: id = if index == length {
        get_static_str(env, "")
    } else {
        let right_range = NSRange { location: index, length: length - index };
        msg![env; this substringWithRange:right_range]
    };
    let res: id = msg![env; left stringByAppendingString:a_string];
    let res: id = msg![env; res stringByAppendingString:right];
    () = msg![env; this setString:res];
}

- (NSUInteger)replaceOccurrencesOfString:(id)target // NSString*
                              withString:(id)replacement // NSString*
                                 options:(NSStringCompareOptions)options
                                   range:(NSRange)range {
    assert_ne!(target, nil);
    assert_ne!(replacement, nil);

    let total_length: NSUInteger = msg![env; this length];
    let location = range.location;
    let length = range.length;
    assert!(location <= total_length);
    assert!(length <= total_length - location);

    let left: id = msg![env; this substringToIndex:location];
    let middle: id = msg![env; this substringWithRange:range];
    let right: id = msg![env; this substringFromIndex:(location + length)];
    let (new_middle, replacements) =
        string_by_replacing_occurrences_inner(env, middle, target, replacement, options);
    let result: id = msg![env; left stringByAppendingString:new_middle];
    let result: id = msg![env; result stringByAppendingString:right];
    () = msg![env; this setString:result];
    replacements
}

@end

// Our private subclass that is the single implementation of NSString for the
// time being.
@implementation _tapHLE_NSString: NSString

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(StringHostObject::Utf8(Cow::Borrowed("")));
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

// TODO: more init methods

- (id)initWithData:(id)data // NSData *
          encoding:(NSStringEncoding)encoding {
    if data == nil {
        release(env, this);
        return nil;
    }
    let bytes: ConstVoidPtr = msg![env; data bytes];
    let bytes: ConstPtr<u8> = bytes.cast();
    let length: NSUInteger = msg![env; data length];
    let new = msg![env; this initWithBytes:bytes length:length encoding:encoding];
    log_dbg!("initWithData:encoding: {}", to_rust_string(env, new));
    new
}

- (id)initWithFormat:(id)format, // NSString*
                     ...args {
    init_with_format_inner(env, this, format, args.start())
}

- (id)initWithFormat:(id)format // NSString*
           arguments:(VaList)args {
    init_with_format_inner(env, this, format, args)
}

- (id)initWithBytes:(ConstPtr<u8>)bytes
             length:(NSUInteger)len
           encoding:(NSStringEncoding)encoding {
    let slice = env.mem.bytes_at(bytes, len);
    let Some(host_object) = StringHostObject::decode(Cow::Borrowed(slice), encoding) else {
        release(env, this);
        return nil;
    };

    *env.objc.borrow_mut(this) = host_object;

    this
}

// See the note on the same method in _tapHLE_NSMutableString. Both concrete
// classes need it: _tapHLE_NSString and _tapHLE_NSMutableString are siblings
// under NSString, so neither inherits the other's implementations.
- (id)initWithBytesNoCopy:(MutPtr<u8>)bytes
                   length:(NSUInteger)len
                 encoding:(NSStringEncoding)encoding
             freeWhenDone:(bool)free_when_done {
    let new: id = msg![env; this initWithBytes:(bytes.cast_const()) length:len encoding:encoding];
    if free_when_done {
        env.mem.free(bytes.cast());
    }
    new
}

- (id)initWithCharacters:(ConstPtr<unichar>)characters length:(NSUInteger)len {
    assert!(!characters.is_null());
    let num_bytes = len * 2;
    msg![env; this initWithBytes:(characters.cast::<u8>())
                          length:num_bytes
                        encoding:NSUTF16StringEncoding]
}

- (id)initWithString:(id)string { // NSString *
    // TODO: optimize for more common cases (or maybe just call copy?)
    let mut code_units = Vec::new();
    for_each_code_unit(env, string, |_, c| code_units.push(c));
    *env.objc.borrow_mut(this) = StringHostObject::Utf16(code_units);
    this
}

- (id)initWithContentsOfFile:(id)path { // NSString*
    if path == nil {
        return nil;
    }
    // TODO: avoid copy?
    let path = to_rust_string(env, path);
    let Ok(bytes) = env.fs.read(GuestPath::new(&path)) else {
        return nil;
    };
    let len = bytes.len();

    let encoding = if len > 1 && (bytes[..2] == [0xFE, 0xFF] || bytes[..2] == [0xFF, 0xFE]) {
        NSUTF16StringEncoding
    } else if len > 2 && bytes[..3] == [0xEF, 0xBB, 0xBF] {
        NSUTF8StringEncoding
    } else {
        msg_class![env; NSString defaultCStringEncoding]
    };

    let Some(host_object) = StringHostObject::decode(Cow::Owned(bytes), encoding) else {
        release(env, this);
        return nil;
    };
    *env.objc.borrow_mut(this) = host_object;
    this
}

- (id)initWithContentsOfFile:(id)path // NSString*
                    encoding:(NSStringEncoding)encoding
                       error:(MutPtr<id>)error { // NSError**
    // A nil path is not an error to raise on: there is nothing to read, so the
    // documented nil return is the answer. The single-argument variant already
    // did this; without it here, to_rust_string() below panicked on nil.
    if path == nil {
        report_no_error(env, error);
        release(env, this);
        return nil;
    }
    // TODO: avoid copy?
    let path = to_rust_string(env, path);
    let Ok(bytes) = env.fs.read(GuestPath::new(&path)) else {
        report_no_error(env, error);
        release(env, this);
        return nil;
    };

    let Some(host_object) = StringHostObject::decode(Cow::Owned(bytes), encoding) else {
        report_no_error(env, error);
        release(env, this);
        return nil;
    };
    *env.objc.borrow_mut(this) = host_object;
    this
}

- (id)initWithContentsOfURL:(id)url { // NSURL*
    init_with_contents_of_url(env, this, url)
}

- (id)initWithContentsOfURL:(id)url // NSURL*
                    encoding:(NSStringEncoding)encoding
                       error:(MutPtr<id>)error { // NSError**
    if url == nil {
        report_no_error(env, error);
        release(env, this);
        return nil;
    }
    let data: id = msg_class![env; NSData dataWithContentsOfURL:url];
    if data == nil {
        report_no_error(env, error);
        release(env, this);
        return nil;
    }
    // TODO: error handling for encoding
    msg![env; this initWithData:data encoding:encoding]
}

- (bool)isAbsolutePath {
    // TODO: avoid copy?
    let path = to_rust_string(env, this);
    path.starts_with('/') || path.starts_with('~')
}


- (bool)boolValue {
    let string = to_rust_string(env, this);
    let string = string.trim_start_matches(|c: char| {
        c.is_ascii_whitespace() || c == '-' || c == '+' || c == '0'
    });

    let matching_values = "YyTt123456789";
    string.chars()
        .next()
        .map(|c| matching_values.contains(c))
        .unwrap_or(false)
}

- (id)dataUsingEncoding:(NSStringEncoding)encoding
   allowLossyConversion:(bool)lossy {
    data_using_encoding_lossy_inner(env, this, encoding, lossy)
}

- (id)componentsSeparatedByCharactersInSet:(id)cset { // NSCharacterSet*
    let string = {
        let host_object = env.objc.borrow_mut::<StringHostObject>(this);
        let (orig_string, did_convert) = host_object.convert_to_utf16_inplace();
        if did_convert {
            log_dbg!("[{:?} componentsSeparatedByCharactersInSet]: converted string to UTF-16", this);
        }
        orig_string.clone()
    };

    let substrings: Vec<&[u16]> = {
        string.split(|&c| msg![env; cset characterIsMember:c]).collect()
    };

    let substrings: Vec<id> = substrings.into_iter().map(|substr| {
        from_u16_vec(env, substr.to_vec())
    }).collect();

    let res = ns_array::from_vec(env, substrings);
    autorelease(env, res)
}

- (id)substringWithRange:(NSRange)range {
    let host_object = env.objc.borrow_mut::<StringHostObject>(this);
    let (orig_string, did_convert) = host_object.convert_to_utf16_inplace();
    if did_convert {
        log_dbg!("[{:?} substringWithRange]: converted string to UTF-16", this);
    }
    let host_string =
        orig_string[(range.location as usize)..((range.location + range.length) as usize)].to_vec();
    let res = from_u16_vec(env, host_string);
    autorelease(env, res)
}

- (NSRange)lineRangeForRange:(NSRange)range {
    let host_object = env.objc.borrow_mut::<StringHostObject>(this);
    let (orig_string, did_convert) = host_object.convert_to_utf16_inplace();
    if did_convert {
        log_dbg!("[{:?} lineRangeForRange]: converted string to UTF-16", this);
    }
    let (start, end, _) = line_range_helper(orig_string, range, true, true, BreakKind::Line);
    NSRange { location: start, length: end - start }
}

- (())getLineStart:(MutPtr<NSUInteger>)start_ptr
               end:(MutPtr<NSUInteger>)end_ptr
       contentsEnd:(MutPtr<NSUInteger>)contents_end_ptr
          forRange:(NSRange)range {
    let host_object = env.objc.borrow_mut::<StringHostObject>(this);
    let (orig_string, did_convert) = host_object.convert_to_utf16_inplace();
    if did_convert {
        log_dbg!("[{:?} getLineStart]: converted string to UTF-16", this);
    }

    let get_start = !start_ptr.is_null();
    let get_end = !end_ptr.is_null() || !contents_end_ptr.is_null();
    let (start, end, contents_end) =
        line_range_helper(orig_string, range, get_start, get_end, BreakKind::Line);

    if !start_ptr.is_null() {
        env.mem.write(start_ptr, start);
    }

    if !end_ptr.is_null() {
        env.mem.write(end_ptr, end);
    }

    if !contents_end_ptr.is_null() {
        env.mem.write(contents_end_ptr, contents_end);
    }
}

// The paragraph pair. Both Carnivores games ask for a paragraph range while
// laying out their briefing text, and the missing selector ended them.
- (NSRange)paragraphRangeForRange:(NSRange)range {
    let host_object = env.objc.borrow_mut::<StringHostObject>(this);
    let (orig_string, did_convert) = host_object.convert_to_utf16_inplace();
    if did_convert {
        log_dbg!("[{:?} paragraphRangeForRange]: converted string to UTF-16", this);
    }
    let (start, end, _) =
        line_range_helper(orig_string, range, true, true, BreakKind::Paragraph);
    NSRange { location: start, length: end - start }
}

- (())getParagraphStart:(MutPtr<NSUInteger>)start_ptr
                    end:(MutPtr<NSUInteger>)end_ptr
            contentsEnd:(MutPtr<NSUInteger>)contents_end_ptr
               forRange:(NSRange)range {
    let host_object = env.objc.borrow_mut::<StringHostObject>(this);
    let (orig_string, did_convert) = host_object.convert_to_utf16_inplace();
    if did_convert {
        log_dbg!("[{:?} getParagraphStart]: converted string to UTF-16", this);
    }

    let get_start = !start_ptr.is_null();
    let get_end = !end_ptr.is_null() || !contents_end_ptr.is_null();
    let (start, end, contents_end) =
        line_range_helper(orig_string, range, get_start, get_end, BreakKind::Paragraph);

    if !start_ptr.is_null() {
        env.mem.write(start_ptr, start);
    }

    if !end_ptr.is_null() {
        env.mem.write(end_ptr, end);
    }

    if !contents_end_ptr.is_null() {
        env.mem.write(contents_end_ptr, contents_end);
    }
}
@end

// Specialised subclass for static-lifetime strings.
// See `get_static_str`.
@implementation _tapHLE_NSString_Static: _tapHLE_NSString

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(StringHostObject::Utf8(Cow::Borrowed("")));
    env.objc.alloc_static_object(this, host_object, &mut env.mem)
}

- (id) retain { this }
- (()) release {}
- (id) autorelease { this }

@end

// Specialised subclasses for static-lifetime strings from the guest app binary.
@implementation _tapHLE_NSString_CFConstantString_UTF8: _tapHLE_NSString_Static

- (ConstPtr<u8>)UTF8String {
    let cfstringStruct { bytes, .. } = env.mem.read(this.cast());

    bytes
}

@end

@implementation _tapHLE_NSString_CFConstantString_UTF16: _tapHLE_NSString_Static
@end

@implementation _tapHLE_NSMutableString: NSMutableString

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(StringHostObject::Utf8(Cow::Borrowed("")));
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithCapacity:(NSUInteger)_capacity {
    // TODO: capacity
    msg![env; this init]
}

// Reading a string from a file. Present on _tapHLE_NSString and needed here
// too: the two concrete classes are siblings under NSString, so neither
// inherits the other's implementations. This has now caught four separate
// methods; when adding anything NSString declares, add it to both.
- (id)initWithContentsOfFile:(id)path { // NSString*
    if path == nil {
        return nil;
    }
    let path = to_rust_string(env, path);
    let Ok(bytes) = env.fs.read(GuestPath::new(&path)) else {
        return nil;
    };
    let encoding = msg_class![env; NSString defaultCStringEncoding];
    let Some(host_object) = StringHostObject::decode(Cow::Owned(bytes), encoding) else {
        release(env, this);
        return nil;
    };
    *env.objc.borrow_mut(this) = host_object;
    this
}

- (id)initWithContentsOfURL:(id)url { // NSURL*
    init_with_contents_of_url(env, this, url)
}

- (id)initWithContentsOfURL:(id)url // NSURL*
                    encoding:(NSStringEncoding)encoding
                       error:(MutPtr<id>)error { // NSError**
    if url == nil {
        report_no_error(env, error);
        release(env, this);
        return nil;
    }
    let data: id = msg_class![env; NSData dataWithContentsOfURL:url];
    if data == nil {
        report_no_error(env, error);
        release(env, this);
        return nil;
    }
    msg![env; this initWithData:data encoding:encoding]
}

- (id)initWithContentsOfFile:(id)path // NSString*
                    encoding:(NSStringEncoding)encoding
                       error:(MutPtr<id>)error { // NSError**
    if path == nil {
        report_no_error(env, error);
        release(env, this);
        return nil;
    }
    let path = to_rust_string(env, path);
    let Ok(bytes) = env.fs.read(GuestPath::new(&path)) else {
        report_no_error(env, error);
        release(env, this);
        return nil;
    };
    let Some(host_object) = StringHostObject::decode(Cow::Owned(bytes), encoding) else {
        report_no_error(env, error);
        release(env, this);
        return nil;
    };
    *env.objc.borrow_mut(this) = host_object;
    this
}

- (id)initWithBytes:(ConstPtr<u8>)bytes
             length:(NSUInteger)len
           encoding:(NSStringEncoding)encoding {
    let slice = env.mem.bytes_at(bytes, len);
    let Some(host_object) = StringHostObject::decode(Cow::Borrowed(slice), encoding) else {
        release(env, this);
        return nil;
    };

    *env.objc.borrow_mut(this) = host_object;

    this
}

// The no-copy variant. tapHLE's NSString always owns decoded storage, so the
// bytes are copied regardless; what the caller is actually promising is that it
// will not touch the buffer again, and `freeWhenDone` transfers ownership of it
// to the string. Honour that by freeing it here, since nothing else will.
- (id)initWithBytesNoCopy:(MutPtr<u8>)bytes
                   length:(NSUInteger)len
                 encoding:(NSStringEncoding)encoding
             freeWhenDone:(bool)free_when_done {
    let new: id = msg![env; this initWithBytes:(bytes.cast_const()) length:len encoding:encoding];
    if free_when_done {
        env.mem.free(bytes.cast());
    }
    new
}

- (id)initWithFormat:(id)format, // NSString*
                     ...args {
    init_with_format_inner(env, this, format, args.start())
}

- (id)initWithFormat:(id)format // NSString*
           arguments:(VaList)args {
    init_with_format_inner(env, this, format, args)
}

- (id)initWithString:(id)string { // NSString*
    () = msg![env; this setString:string];
    this
}

- (id)dataUsingEncoding:(NSStringEncoding)encoding
   allowLossyConversion:(bool)lossy {
    data_using_encoding_lossy_inner(env, this, encoding, lossy)
}

// Appending, on the concrete class, writes into the string that is already
// there. NSMutableString's own implementation goes through the mutation
// primitive, which rebuilds the whole string from its two halves — correct for
// any subclass, and fine for the one-off edits it is usually asked for, but
// appending is the one that gets done in a loop. A game assembling a file a
// token at a time then pays for the whole string on every token, so the work
// and the memory grow with the square of the result: one level load spent
// eleven gigabytes on eleven thousand appends before it was killed.
- (())appendString:(id)a_string { // NSString*
    assert_ne!(a_string, nil);
    // Only what is being added is copied here.
    let addition = to_rust_string(env, a_string).into_owned();
    match env.objc.borrow_mut::<StringHostObject>(this) {
        StringHostObject::Utf8(utf8) => utf8.to_mut().push_str(&addition),
        StringHostObject::Utf16(utf16) => utf16.extend(addition.encode_utf16()),
    }
}

- (())appendFormat:(id)format, // NSString*
                   ...args {
    assert_ne!(format, nil);
    let res = with_format(env, format, args.start());
    match env.objc.borrow_mut::<StringHostObject>(this) {
        StringHostObject::Utf8(utf8) => utf8.to_mut().push_str(&res),
        StringHostObject::Utf16(utf16) => utf16.extend(res.encode_utf16()),
    }
}

- (())setString:(id)a_string { // NSString*
    assert_ne!(a_string, nil);
    let str = to_rust_string(env, a_string);
    let host_object = StringHostObject::Utf8(str);
    *env.objc.borrow_mut(this) = host_object;
}

- (id)substringWithRange:(NSRange)range {
    let host_object = env.objc.borrow_mut::<StringHostObject>(this);
    let (orig_string, did_convert) = host_object.convert_to_utf16_inplace();
    if did_convert {
        log_dbg!("[{:?} substringWithRange]: converted string to UTF-16", this);
    }
    let host_string =
        orig_string[(range.location as usize)..((range.location + range.length) as usize)].to_vec();
    let res = from_u16_vec(env, host_string);
    autorelease(env, res)
}

@end

};

/// This helper is used in `initWithFormat:` on our private subclasses
/// _tapHLE_NSString and _tapHLE_NSMutableString
fn init_with_format_inner(env: &mut Environment, this: id, format: id, args: VaList) -> id {
    let res = with_format(env, format, args);
    *env.objc.borrow_mut::<StringHostObject>(this) = StringHostObject::Utf8(res.into());
    this
}

/// This helper is used in `dataUsingEncoding:allowLossyConversion:` on our
/// private subclasses _tapHLE_NSString and _tapHLE_NSMutableString
fn data_using_encoding_lossy_inner(
    env: &mut Environment,
    this: id,
    encoding: NSStringEncoding,
    lossy: bool,
) -> id {
    if lossy {
        log!(
            "Warning: ignoring allowLossyConversion for '{}'",
            to_rust_string(env, this)
        );
    }
    let Some(bytes) = encode_string(env, this, encoding) else {
        // `dataUsingEncoding:` returns nil for a string it cannot convert,
        // and that is the branch the app already has written for it.
        // `encode_string` has already said why in the log.
        return nil;
    };
    // TODO: this includes the terminator in the NSData and Foundation does
    // not. Left as it was: it is a different defect from the assertion
    // replaced here, and apps have been measured against the behaviour.
    let c_string = env.mem.alloc_and_write_cstr(&bytes);
    let length: NSUInteger = (bytes.len() + 1).try_into().unwrap();

    msg_class![env; NSData dataWithBytesNoCopy:(c_string.cast_void()) length:length]
}

/// For use by [crate::dyld]: Handle static strings listed in the app binary.
/// Sets up host objects and updates `isa` fields
/// (`___CFConstantStringClassReference` is ignored by our dyld).
pub fn register_constant_strings(bin: &MachO, mem: &mut Mem, objc: &mut ObjC) {
    let Some(cfstrings) = bin.get_section("__cfstring") else {
        return;
    };

    assert!(cfstrings.size % guest_size_of::<cfstringStruct>() == 0);
    let base: ConstPtr<cfstringStruct> = Ptr::from_bits(cfstrings.addr);
    for i in 0..(cfstrings.size / guest_size_of::<cfstringStruct>()) {
        let cfstr_ptr = base + i;
        let cfstringStruct {
            _isa,
            flags,
            bytes,
            length,
        } = mem.read(cfstr_ptr);

        // Constant CFStrings should (probably) only ever have flags 0x7c8 and
        // 0x7d0.
        // See https://lists.llvm.org/pipermail/cfe-dev/2008-August/002518.html
        let (host_object, class_name) = if flags == 0x7C8 {
            // ASCII
            let decoded = std::str::from_utf8(mem.bytes_at(bytes, length)).unwrap();

            (
                StringHostObject::Utf8(Cow::Owned(String::from(decoded))),
                "_tapHLE_NSString_CFConstantString_UTF8",
            )
        } else if flags == 0x7D0 {
            // UTF16 (length is in code units, not bytes)
            let decoded = mem
                .bytes_at(bytes, length * 2)
                .chunks(2)
                .map(|chunk| u16::from_le_bytes(chunk.try_into().unwrap()))
                .collect();

            (
                StringHostObject::Utf16(decoded),
                "_tapHLE_NSString_CFConstantString_UTF16",
            )
        } else {
            panic!("Bad CFTypeID for constant string: {flags:#x}");
        };

        objc.register_static_object(cfstr_ptr.cast().cast_mut(), Box::new(host_object));

        let new_isa = objc.get_known_class(class_name, mem);
        mem.write(cfstr_ptr.cast().cast_mut(), new_isa);
    }
}

/// Shortcut for host code: get an NSString corresponding to a `&'static str`,
/// which does not have to be released and is never deallocated.
pub fn get_static_str(env: &mut Environment, from: &'static str) -> id {
    if let Some(&existing) = State::get(env).static_str_pool.get(from) {
        existing
    } else {
        let new = msg_class![env; _tapHLE_NSString_Static alloc];
        *env.objc.borrow_mut(new) = StringHostObject::Utf8(Cow::Borrowed(from));
        State::get(env).static_str_pool.insert(from, new);
        new
    }
}

/// Shortcut for host code, roughly equivalent to
/// `[[NSString alloc] initWithUTF8String:]` in the proper API.
pub fn from_rust_string(env: &mut Environment, from: String) -> id {
    let string: id = msg_class![env; _tapHLE_NSString alloc];
    let host_object: &mut StringHostObject = env.objc.borrow_mut(string);
    *host_object = StringHostObject::Utf8(Cow::Owned(from));
    string
}

/// Shortcut for host code, roughly equivalent to
/// `[[NSMutableString alloc] initWithUTF8String:]` in the proper API.
pub fn mutable_from_rust_string(env: &mut Environment, from: String) -> id {
    let string: id = msg_class![env; _tapHLE_NSMutableString alloc];
    let host_object: &mut StringHostObject = env.objc.borrow_mut(string);
    *host_object = StringHostObject::Utf8(Cow::Owned(from));
    string
}

/// Shortcut for host code, allocs and inits with the given u16 vec.
pub fn from_u16_vec(env: &mut Environment, from: Vec<u16>) -> id {
    let string: id = msg_class![env; _tapHLE_NSString alloc];
    let host_object: &mut StringHostObject = env.objc.borrow_mut(string);
    *host_object = StringHostObject::Utf16(from);
    string
}

/// Shortcut for host code, provides a view of a string in UTF-8.
/// Warning: This may panic if the string is not valid UTF-16!
///
/// TODO: Try to avoid allocating a new String in more cases.
///
/// TODO: Try to avoid converting from UTF-16 in more cases.
pub fn to_rust_string(env: &mut Environment, string: id) -> Cow<'static, str> {
    // TODO: handle foreign subclasses of NSString
    env.objc
        .borrow_mut::<StringHostObject>(string)
        .to_utf8()
        .unwrap()
}

/// Shortcut for host code, calls a callback once for each UTF-16 code-unit in a
/// string. This is equivalent to a for loop using the `length` and
/// `characterAtIndex:` methods, but much more efficient.
pub fn for_each_code_unit<F>(env: &mut Environment, string: id, mut f: F)
where
    F: FnMut(NSUInteger, u16),
{
    // TODO: handle foreign subclasses of NSString
    let mut idx: NSUInteger = 0;
    env.objc
        .borrow::<StringHostObject>(string)
        .iter_code_units()
        .for_each(|c| {
            f(idx, c);
            idx += 1;
        });
}

/// Helper function for `rangeOfString:options:` method
/// Note: this implementation is linear
fn is_match_at_position<F: Fn(u16, u16) -> bool>(
    env: &mut Environment,
    the_string: id,
    search_string: id,
    start: NSUInteger,
    len: NSUInteger,
    len_search: NSUInteger,
    compare_fn: F,
) -> bool {
    (0..len_search).all(|j| {
        let curr: NSUInteger = start + j;
        if curr < len {
            let a_c: u16 = msg![env; the_string characterAtIndex:curr];
            let b_c: u16 = msg![env; search_string characterAtIndex:j];
            compare_fn(a_c, b_c)
        } else {
            false
        }
    })
}

/// Helper function for shared `doubleValue` and `floatValue` implementations.
fn float_value_common<F: std::str::FromStr + Default>(env: &mut Environment, string: id) -> F {
    let st = to_rust_string(env, string);
    let st = st.trim_start();
    let mut cutoff = st.len();
    for (i, c) in st.char_indices() {
        if !c.is_ascii_digit() && c != '.' && c != '+' && c != '-' {
            cutoff = i;
            break;
        }
    }
    // TODO: handle over/underflow properly
    st[..cutoff].parse().unwrap_or(Default::default())
}

/// Which of the two things a range can be extended to.
///
/// They differ only in the delimiters they stop at. Every paragraph break is
/// also a line break; LINE SEPARATOR and NEXT LINE break a line without
/// starting a new paragraph, which is the whole distinction.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum BreakKind {
    Line,
    Paragraph,
}

/// Helper function for lineRangeForRange:,
/// getLineStart:end:contentsEnd:forRange:, paragraphRangeForRange: and
/// getParagraphStart:end:contentsEnd:forRange:.
///
/// The two `get_[start/end]` arguments correspond to the start and
/// end/contentsEnd returns. If false is specified for a given argument, the
/// corresponding return values will not be calculated and set to 0.
fn line_range_helper(
    string: &Utf16String,
    range: NSRange,
    get_start: bool,
    get_end: bool,
    kind: BreakKind,
) -> (NSUInteger, NSUInteger, NSUInteger) {
    let NSRange {
        location: r_start,
        length,
    } = range;
    let r_end: usize = r_start.checked_add(length).unwrap().try_into().unwrap();
    let r_start: usize = r_start.try_into().unwrap();
    // All the line range functions are "counting the posts, not the fences", so
    // it's ok if r_end = length.
    let str_len = string.len();
    assert!(r_end <= str_len, "Range out of bounds!");

    let mut start_pos: usize = 0;
    if get_start {
        start_pos = r_start;
        while start_pos > 0 {
            let c: u16 = string[start_pos - 1];
            // What counts as a line delimiter is noted here:
            // https://developer.apple.com/documentation/foundation/nsstring/1415111-getlinestart?language=objc
            // There's some special handling for if we start in the
            // middle of a CRLF.
            match c {
                // 'LINE FEED (LF)' (\n), 'PARAGRAPH SEPARATOR'
                0x000A | 0x2029 => break,
                // 'NEXT LINE (NEL)', 'LINE SEPARATOR'. These end a line but not
                // a paragraph.
                0x0085 | 0x2028 if kind == BreakKind::Line => break,
                // 'CARRIAGE RETURN (CR)' (\r)
                0x000D => {
                    // If the first character is CR, and it is followed by an
                    // LF, then it's not counted as a line delimiter.
                    // (verified on simulator)
                    if start_pos == r_start && start_pos < str_len {
                        let after_cr: u16 = string[start_pos];
                        // 'LINE FEED (LF)' (\n)
                        if after_cr == 0x000A {
                            start_pos -= 1;
                            continue;
                        }
                    }
                    break;
                }
                _ => {}
            }
            start_pos -= 1;
        }
    }

    // There is very little extra cost for also getting contentsEnd if we're
    // getting end (or vice-versa), so they're combined into one argument.
    let mut end_pos = 0;
    let mut cend_pos = 0;
    if get_end {
        // We want to include the entire line that covers the last char
        // in [r_start, r_end).
        cend_pos = if length > 0 { r_end - 1 } else { r_start };
        while cend_pos < str_len {
            let c: u16 = string[cend_pos];
            // See above about what counts as a line delimiter.
            // There's more understandable handling for CRLF here as well.
            match c {
                // 'PARAGRAPH SEPARATOR'
                0x2029 => {
                    end_pos = cend_pos + 1;
                    break;
                }
                // 'NEXT LINE (NEL)', 'LINE SEPARATOR', which end a line but not
                // a paragraph.
                0x0085 | 0x2028 if kind == BreakKind::Line => {
                    end_pos = cend_pos + 1;
                    break;
                }
                // 'LINE FEED (LF)' (\n),
                0x000A => {
                    // If this is the first character checked, then we also need
                    // to check back for a CR.
                    if cend_pos > 0 && string[cend_pos - 1] == 0x000D {
                        cend_pos -= 1;
                        end_pos = cend_pos + 2;
                    } else {
                        end_pos = cend_pos + 1;
                    }
                    break;
                }
                // 'CARRIAGE RETURN (CR)' (\r)
                0x000D => {
                    // Check if next character exists and is LF.
                    if cend_pos < str_len - 1 {
                        let after_cr: u16 = string[cend_pos + 1];
                        // 'LINE FEED (LF)' (\n)
                        if after_cr == 0x000A {
                            end_pos = cend_pos + 2;
                            break;
                        }
                    }
                    end_pos = cend_pos + 1;
                    break;
                }
                _ => {}
            }
            cend_pos += 1;
        }
        if cend_pos == str_len {
            end_pos = cend_pos
        }
    }

    (
        start_pos.try_into().unwrap(),
        end_pos.try_into().unwrap(),
        cend_pos.try_into().unwrap(),
    )
}

#[cfg(test)]
mod ns_string_tests {
    use super::*;
    #[test]
    fn linerange_tests() {
        let range = |x, y| NSRange {
            location: x,
            length: y,
        };
        let str1: Utf16String = "abcd\nab".encode_utf16().collect();
        assert!(line_range_helper(&str1, range(5, 1), true, true, BreakKind::Line) == (5, 7, 7));
        assert!(line_range_helper(&str1, range(4, 1), true, true, BreakKind::Line) == (0, 5, 4));

        let str2: Utf16String = "abc\r".encode_utf16().collect();
        assert!(line_range_helper(&str2, range(4, 0), true, true, BreakKind::Line) == (4, 4, 4));
        assert!(line_range_helper(&str2, range(3, 1), true, true, BreakKind::Line) == (0, 4, 3));

        let str3: Utf16String = "abc\r\nab".encode_utf16().collect();
        assert!(line_range_helper(&str3, range(4, 0), true, true, BreakKind::Line) == (0, 5, 3));
        assert!(line_range_helper(&str3, range(4, 1), true, true, BreakKind::Line) == (0, 5, 3));
        assert!(line_range_helper(&str3, range(6, 1), true, true, BreakKind::Line) == (5, 7, 7));
        assert!(line_range_helper(&str3, range(4, 2), true, true, BreakKind::Line) == (0, 7, 7));

        let str4: Utf16String = "\r\n".encode_utf16().collect();
        assert!(line_range_helper(&str4, range(1, 0), true, true, BreakKind::Line) == (0, 2, 0));
        assert!(line_range_helper(&str4, range(1, 1), true, true, BreakKind::Line) == (0, 2, 0));
        assert!(line_range_helper(&str4, range(0, 0), true, true, BreakKind::Line) == (0, 2, 0));

        let str5: Utf16String = "abcd\na\n".encode_utf16().collect();
        assert!(line_range_helper(&str5, range(6, 1), true, true, BreakKind::Line) == (5, 7, 6));
        assert!(line_range_helper(&str5, range(4, 1), true, true, BreakKind::Line) == (0, 5, 4));
    }

    /// A paragraph stops where a line does, minus the two characters that end
    /// a line without ending a paragraph. Each case is checked both ways so
    /// the difference is the thing being asserted.
    #[test]
    fn paragraphrange_tests() {
        let range = |x, y| NSRange {
            location: x,
            length: y,
        };

        // LINE SEPARATOR: a line break, not a paragraph break, so a paragraph
        // range reaches straight through it.
        let sep: Utf16String = "ab\u{2028}cd".encode_utf16().collect();
        assert!(line_range_helper(&sep, range(0, 1), true, true, BreakKind::Line) == (0, 3, 2));
        assert!(
            line_range_helper(&sep, range(0, 1), true, true, BreakKind::Paragraph) == (0, 5, 5)
        );

        // NEXT LINE behaves the same way.
        let nel: Utf16String = "ab\u{0085}cd".encode_utf16().collect();
        assert!(line_range_helper(&nel, range(0, 1), true, true, BreakKind::Line) == (0, 3, 2));
        assert!(
            line_range_helper(&nel, range(0, 1), true, true, BreakKind::Paragraph) == (0, 5, 5)
        );

        // PARAGRAPH SEPARATOR ends both.
        let par: Utf16String = "ab\u{2029}cd".encode_utf16().collect();
        assert!(line_range_helper(&par, range(0, 1), true, true, BreakKind::Line) == (0, 3, 2));
        assert!(
            line_range_helper(&par, range(0, 1), true, true, BreakKind::Paragraph) == (0, 3, 2)
        );

        // So do the ordinary ones, including CRLF as a single break.
        let crlf: Utf16String = "ab\r\ncd".encode_utf16().collect();
        assert!(
            line_range_helper(&crlf, range(0, 1), true, true, BreakKind::Paragraph) == (0, 4, 2)
        );
        assert!(
            line_range_helper(&crlf, range(4, 1), true, true, BreakKind::Paragraph) == (4, 6, 6)
        );

        let lf: Utf16String = "ab\ncd".encode_utf16().collect();
        assert!(line_range_helper(&lf, range(3, 1), true, true, BreakKind::Paragraph) == (3, 5, 5));
    }
}

/// The encoding a string is already held in, which is the one it converts to
/// most cheaply.
///
/// tapHLE keeps a string as either UTF-8 or UTF-16 and converts on demand, so
/// this is not a guess: it is what the host object is holding right now.
pub fn fastest_encoding(env: &mut Environment, string: id) -> NSStringEncoding {
    match env.objc.borrow::<StringHostObject>(string) {
        StringHostObject::Utf8(_) => NSUTF8StringEncoding,
        StringHostObject::Utf16(_) => NSUTF16StringEncoding,
    }
}

/// The encoding a string takes the least room in.
///
/// ASCII when every character fits in it, which is the case this question is
/// asked about, and UTF-8 otherwise — never UTF-16, which is larger than UTF-8
/// for everything an early iPhone app is likely to be holding.
pub fn smallest_encoding(env: &mut Environment, string: id) -> NSStringEncoding {
    if to_rust_string(env, string).is_ascii() {
        NSASCIIStringEncoding
    } else {
        NSUTF8StringEncoding
    }
}

/// Encode a string's characters as bytes in `encoding`: the mirror of
/// [StringHostObject::decode], and it answers the same way.
///
/// `None` means the string cannot be represented in that encoding, which is an
/// ordinary answer rather than an error. Foundation says so with `NO`, `nil` or
/// zero depending on the call, and every caller here has one of those to
/// return. It used to be an assertion instead, on the theory that an app asking
/// for an encoding tapHLE had not implemented was a bug to surface loudly. What
/// it actually surfaced was four apps dying during start-up — three versions of
/// Super Hexagon and Don't Look Back — over a string conversion whose failure
/// they were already written to handle.
pub fn encode_string(
    env: &mut Environment,
    string: id,
    encoding: NSStringEncoding,
) -> Option<Vec<u8>> {
    let string = to_rust_string(env, string);

    fn with_encoding_rs(encoding: &'static encoding_rs::Encoding, string: &str) -> Option<Vec<u8>> {
        // encoding_rs substitutes a numeric character reference for anything it
        // cannot map and reports that as an error. Substituting "&#1234;" into
        // what the app believes is its own text is worse than saying no, so the
        // error is passed on rather than absorbed.
        let (bytes, encoding_used, had_errors) = encoding.encode(string);
        (!had_errors && encoding_used == encoding).then(|| bytes.into_owned())
    }

    match encoding {
        NSUTF8StringEncoding => Some(string.into_owned().into_bytes()),
        NSASCIIStringEncoding => string.is_ascii().then(|| string.into_owned().into_bytes()),
        // Latin-1 is exactly the first 256 Unicode code points, one byte each,
        // so this needs no table: a character outside that range is the whole
        // of what the encoding cannot say.
        NSISOLatin1StringEncoding => string
            .chars()
            .map(|c| u8::try_from(u32::from(c)).ok())
            .collect(),
        NSMacOSRomanStringEncoding => with_encoding_rs(MACINTOSH, &string),
        NSWindowsCP1252StringEncoding => with_encoding_rs(WINDOWS_1252, &string),
        NSShiftJISStringEncoding => with_encoding_rs(SHIFT_JIS, &string),
        NSUTF16StringEncoding
        | NSUTF16BigEndianStringEncoding
        | NSUTF16LittleEndianStringEncoding => {
            // NSUnicodeStringEncoding is host-endian, and every host tapHLE
            // targets is little-endian, so it and the explicit little-endian
            // spelling produce the same bytes. No BOM is written: this is the
            // raw-bytes path, where the caller has told us the byte order.
            let big_endian = encoding == NSUTF16BigEndianStringEncoding;
            let mut bytes = Vec::with_capacity(string.len() * 2);
            for unit in string.encode_utf16() {
                bytes.extend_from_slice(&if big_endian {
                    unit.to_be_bytes()
                } else {
                    unit.to_le_bytes()
                });
            }
            Some(bytes)
        }
        _ => {
            log!(
                "TODO: encoding a string as NSStringEncoding {} is unimplemented; reporting the conversion as failed",
                encoding
            );
            None
        }
    }
}

/// Whether a NUL terminator in `encoding` is two bytes wide rather than one.
fn encoding_is_utf16(encoding: NSStringEncoding) -> bool {
    matches!(
        encoding,
        NSUTF16StringEncoding | NSUTF16BigEndianStringEncoding | NSUTF16LittleEndianStringEncoding
    )
}

/// Helper function to get bytes of a string in the specified NSStringEncoding.
///
/// `include_null_terminator` flag controls if NULL-terminator should be
/// included or not.
/// Return value specify if provided buffer was ok or too small.
/// (TODO: indicate error on conversion too)
/// In case of small buffer no data is written.
///
/// Right now this helper is used for `NSString getCString:maxLength:encoding:`
/// method, `CFStringGetPascalString` and `CFStringGetBytes` functions.
pub fn get_bytes_buffer_inner(
    env: &mut Environment,
    str: id, // NSString *
    buffer: MutPtr<u8>,
    buffer_size: NSUInteger,
    encoding: NSStringEncoding,
    include_null_terminator: bool,
) -> bool {
    let Some(mut src) = encode_string(env, str, encoding) else {
        // The same answer as a buffer that is too small: the caller is told the
        // bytes are not there, which is a case it already has to handle.
        return false;
    };
    if include_null_terminator {
        let nul_width = if encoding_is_utf16(encoding) { 2 } else { 1 };
        src.extend(std::iter::repeat_n(0u8, nul_width));
    }

    let dest = env.mem.bytes_at_mut(buffer, buffer_size);
    if dest.len() < src.len() {
        return false;
    }
    dest[..src.len()].copy_from_slice(&src);

    true
}

/// Helper function used by
/// `[NSString stringByReplacingOccurrencesOfString:withString:options:range:]`
/// method.
fn string_by_replacing_occurrences_inner(
    env: &mut Environment,
    source: id,      // NSString *
    target: id,      // NSString *
    replacement: id, // NSString *
    options: NSStringCompareOptions,
) -> (id, NSUInteger) {
    // TODO: support foreign subclasses (perhaps via a helper function that
    // copies the string first)
    let mut main_iter = env
        .objc
        .borrow::<StringHostObject>(source)
        .iter_code_units();
    let target_iter = env
        .objc
        .borrow::<StringHostObject>(target)
        .iter_code_units();
    let replacement_iter = env
        .objc
        .borrow::<StringHostObject>(replacement)
        .iter_code_units();

    // Zero-length target case
    if target_iter.clone().next().is_none() {
        let res = msg![env; source copy];
        return (autorelease(env, res), 0);
    }

    let case_insensitive = match options {
        0 => false, // No options mean literal match
        NSCaseInsensitiveSearch => true,
        _ => unimplemented!(),
    };

    let mut result: Utf16String = Vec::new();
    let mut replacements: NSUInteger = 0;
    loop {
        if let Some(new_main_iter) = main_iter.strip_prefix(&target_iter, case_insensitive) {
            // matched target, replace it
            result.extend(replacement_iter.clone());
            main_iter = new_main_iter;
            replacements += 1;
        } else {
            // no match, copy as normal
            match main_iter.next() {
                Some(cur) => result.push(cur),
                None => break,
            }
        }
    }

    // TODO: For a foreign subclass of NSString, do we have to return that
    // subclass? The signature implies this isn't the case and it's probably not
    // worth the effort, but it's an interesting question.
    let result_ns_string = msg_class![env; _tapHLE_NSString alloc];
    *env.objc.borrow_mut(result_ns_string) = StringHostObject::Utf16(result);
    (autorelease(env, result_ns_string), replacements)
}

/// Decode `%XX` escapes, or [None] if the result is not valid text.
///
/// Two behaviours here are not guesses and both matter:
///
/// A `%` that is not followed by two hexadecimal digits is passed through
/// literally rather than treated as an error. Strings containing a bare percent
/// sign are common - "100% complete" - and Cocoa returns them unchanged.
///
/// A `+` is left alone. Converting it to a space is form encoding, a different
/// specification, and doing it here would corrupt every base64 payload that
/// travels through a URL.
///
/// [None] is returned when the decoded bytes are not valid UTF-8, which is what
/// makes the Objective-C method return nil. That is a real answer callers test
/// for, not a shortcut.
pub fn unescape_percent_escapes(escaped: &str) -> Option<String> {
    let bytes = escaped.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let high = (bytes[i + 1] as char).to_digit(16);
            let low = (bytes[i + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                decoded.push((high * 16 + low) as u8);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod percent_escape_tests {
    use super::unescape_percent_escapes;

    #[test]
    fn escapes_decode_to_their_bytes() {
        assert_eq!(
            unescape_percent_escapes("a%20b%2Fc").as_deref(),
            Some("a b/c")
        );
        // Lower-case hex digits are as valid as upper-case ones.
        assert_eq!(unescape_percent_escapes("%2f").as_deref(), Some("/"));
    }

    #[test]
    fn multibyte_sequences_reassemble() {
        assert_eq!(
            unescape_percent_escapes("%C3%A9").as_deref(),
            Some("\u{e9}")
        );
    }

    #[test]
    fn a_bare_percent_sign_survives() {
        // "100% complete" is the case this protects: Cocoa passes a percent
        // that is not followed by two hex digits through unchanged.
        assert_eq!(
            unescape_percent_escapes("100% complete").as_deref(),
            Some("100% complete")
        );
        assert_eq!(unescape_percent_escapes("%").as_deref(), Some("%"));
        assert_eq!(unescape_percent_escapes("%2").as_deref(), Some("%2"));
        assert_eq!(unescape_percent_escapes("%zz").as_deref(), Some("%zz"));
    }

    #[test]
    fn a_plus_is_not_a_space() {
        // Converting it would be form encoding, and would corrupt base64.
        assert_eq!(unescape_percent_escapes("a+b").as_deref(), Some("a+b"));
    }

    #[test]
    fn undecodable_bytes_give_nothing() {
        // A lone continuation byte is not valid UTF-8, and the Objective-C
        // method returns nil for it.
        assert_eq!(unescape_percent_escapes("%80"), None);
        assert_eq!(unescape_percent_escapes("%C3"), None);
    }
}
