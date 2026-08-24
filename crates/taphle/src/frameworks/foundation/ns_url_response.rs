/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSURLResponse` and `NSHTTPURLResponse`.
//!
//! tapHLE has no network, so it never produces one of these itself: a
//! connection reports that there is no internet connection instead. They exist
//! because apps name them anyway — to check a class is there before using it,
//! to declare a variable, and to build one by hand in their own tests and
//! offline paths — and a missing class ends the app at the first mention.
//!
//! What is stored is what an app reads back after making one. Nothing here
//! invents a response: an app that builds one gets its own values, and the
//! defaults are the neutral ones for a response that carries nothing.

use super::{ns_dictionary, ns_string, NSInteger};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};

struct NSURLResponseHostObject {
    /// `NSURL*`, retained.
    url: id,
    /// `NSString*`, retained.
    mime_type: id,
    expected_content_length: i64,
    /// `NSString*`, retained.
    text_encoding_name: id,
    status_code: NSInteger,
    /// `NSDictionary*`, retained.
    header_fields: id,
}
impl Default for NSURLResponseHostObject {
    fn default() -> Self {
        NSURLResponseHostObject {
            url: nil,
            mime_type: nil,
            // -1 is what a response with no Content-Length reports, and is the
            // value every caller already tests for.
            expected_content_length: -1,
            text_encoding_name: nil,
            status_code: 0,
            header_fields: nil,
        }
    }
}
impl HostObject for NSURLResponseHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSURLResponse: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<NSURLResponseHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithURL:(id)url // NSURL*
         MIMEType:(id)mime_type // NSString*
expectedContentLength:(NSInteger)expected_content_length
 textEncodingName:(id)text_encoding_name { // NSString*
    retain(env, url);
    retain(env, mime_type);
    retain(env, text_encoding_name);
    let host_object = env.objc.borrow_mut::<NSURLResponseHostObject>(this);
    host_object.url = url;
    host_object.mime_type = mime_type;
    host_object.expected_content_length = expected_content_length as i64;
    host_object.text_encoding_name = text_encoding_name;
    this
}

- (id)URL {
    env.objc.borrow::<NSURLResponseHostObject>(this).url
}
- (id)MIMEType {
    env.objc.borrow::<NSURLResponseHostObject>(this).mime_type
}
- (i64)expectedContentLength {
    env.objc.borrow::<NSURLResponseHostObject>(this).expected_content_length
}
- (id)textEncodingName {
    env.objc.borrow::<NSURLResponseHostObject>(this).text_encoding_name
}

// The last path component of the URL, which is what UIKit's own implementation
// falls back to when a response carries no filename.
- (id)suggestedFilename {
    let url = env.objc.borrow::<NSURLResponseHostObject>(this).url;
    if url == nil {
        return nil;
    }
    msg![env; url lastPathComponent]
}

- (())dealloc {
    let &NSURLResponseHostObject {
        url, mime_type, text_encoding_name, header_fields, ..
    } = env.objc.borrow(this);
    release(env, url);
    release(env, mime_type);
    release(env, text_encoding_name);
    release(env, header_fields);
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

@implementation NSHTTPURLResponse: NSURLResponse

+ (id)localizedStringForStatusCode:(NSInteger)status_code {
    let text = match status_code {
        200 => "no error",
        404 => "not found",
        500 => "internal server error",
        _ => "unknown status",
    };
    let string = ns_string::from_rust_string(env, text.to_string());
    autorelease(env, string)
}

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<NSURLResponseHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

// The header dictionary is copied rather than retained, as Foundation's
// initialisers do, so a caller mutating its dictionary afterwards does not
// change the response.
- (id)initWithURL:(id)url // NSURL*
       statusCode:(NSInteger)status_code
      HTTPVersion:(id)_http_version // NSString*
     headerFields:(id)header_fields { // NSDictionary*
    retain(env, url);
    let header_fields: id = if header_fields == nil {
        nil
    } else {
        msg![env; header_fields copy]
    };
    let host_object = env.objc.borrow_mut::<NSURLResponseHostObject>(this);
    host_object.url = url;
    host_object.status_code = status_code;
    host_object.header_fields = header_fields;
    this
}

- (NSInteger)statusCode {
    env.objc.borrow::<NSURLResponseHostObject>(this).status_code
}

// An empty dictionary rather than nil: callers subscript this without checking,
// and a response with no headers is an ordinary thing to have.
- (id)allHeaderFields {
    let header_fields = env.objc.borrow::<NSURLResponseHostObject>(this).header_fields;
    if header_fields != nil {
        return header_fields;
    }
    let empty = ns_dictionary::dict_from_keys_and_objects(env, &[]);
    autorelease(env, empty)
}

@end

};

/// Build an `NSHTTPURLResponse` for a URL and status, for use by any part of
/// tapHLE that has to hand one to a delegate.
#[allow(dead_code)]
pub fn http_response(env: &mut crate::Environment, url: id, status_code: NSInteger) -> id {
    let response: id = msg_class![env; NSHTTPURLResponse alloc];
    let response: id = msg![env; response initWithURL:url
                                          statusCode:status_code
                                         HTTPVersion:nil
                                        headerFields:nil];
    autorelease(env, response)
}
