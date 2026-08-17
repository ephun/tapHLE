/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIWebView`.

use crate::frameworks::foundation::ns_string::{get_static_str, to_rust_string};
use crate::frameworks::foundation::NSUInteger;
use crate::msg;
use crate::objc::{id, msg_super, nil, objc_classes, ClassExports};
use std::borrow::Cow;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIWebView: UIView

// NSCoding implementation
//
// A web view in a nib is just a UIView as far as tapHLE is concerned: there is
// no web engine behind it, so none of the keys a real UIWebView decodes
// (scaling, data detectors, delegate) would change anything. Decoding the
// UIView half and stopping there gives a correctly sized, correctly placed,
// blank view — which is what this class does when constructed in code too.
//
// The point is that it no longer aborts. A nib with a web view in it usually
// also holds the screen the app actually wants, and refusing to unarchive the
// whole thing over one view that was always going to be blank throws that away.
- (id)initWithCoder:(id)coder {
    msg_super![env; this initWithCoder:coder]
}

- (())setScalesPageToFit:(bool)_scales {
    // TODO
}
// Which kinds of text — phone numbers, addresses, links — the web engine
// should turn into tappable links by itself. There is no engine and no
// document, so there is nothing for the setting to apply to; it is accepted
// and dropped, like the other presentation settings above. An app configures
// this immediately after creating the view, so refusing it ends the app
// before the screen holding the web view is ever built.
- (())setDataDetectorTypes:(NSUInteger)_types {
    // TODO
}
- (())setDelegate:(id)_delegate {
    // TODO
}
- (())loadRequest:(id)request { // NSURLRequest*
    let url_string = if request != nil {
        let url = msg![env; request URL];
        let url_desc = msg![env; url description];
        to_rust_string(env, url_desc)
    } else {
        Cow::default()
    };
    log!("TODO: [(UIWebView*) {:?} loadRequest:{:?} ({})]", this, request, url_string);
}

// There is no engine to render the HTML into, so the string is logged and
// dropped. The view stays blank, which is what this class does with every other
// kind of content.
- (())loadHTMLString:(id)html // NSString*
             baseURL:(id)_base_url { // NSURL*
    let length: crate::frameworks::foundation::NSUInteger = if html == nil {
        0
    } else {
        msg![env; html length]
    };
    log!("TODO: [(UIWebView*) {:?} loadHTMLString:(NSString* of {} characters) baseURL:_]", this, length);
}

- (id)stringByEvaluatingJavaScriptFromString:(id)_script { // NSString*
    // tapHLE has no JavaScript engine and no loaded document, so evaluation
    // yields the empty string — which is also what UIWebView returns for a
    // script whose result has no string value.
    get_static_str(env, "")
}

@end

};
