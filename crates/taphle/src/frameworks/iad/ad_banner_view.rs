/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `ADBannerView`.
//!
//! A banner that never fills. See the module comment in the parent for why
//! that is the honest behavior rather than a stub.

use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::frameworks::uikit::ui_view::UIViewHostObject;
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_class, msg_super, nil, objc_classes, release,
    retain, ClassExports, NSZonePtr,
};

#[derive(Default)]
struct ADBannerViewHostObject {
    superclass: UIViewHostObject,
    /// Weak, as delegates always are.
    delegate: id,
    /// `NSString*`, retained.
    current_content_size_identifier: id,
    /// `NSSet*`, retained.
    required_content_size_identifiers: id,
    /// Whether the delegate has already been told this banner failed. The
    /// notification is sent once, the first time the app looks at the banner.
    failure_reported: bool,
}
impl_HostObject_with_superclass!(ADBannerViewHostObject);

/// Tell the delegate, once, that no ad arrived.
///
/// Apple's guide has apps hide the banner in this callback, so an app that
/// never receives it can leave an empty banner occupying part of the screen.
fn report_failure_once(env: &mut crate::Environment, this: id) {
    let host_object = env.objc.borrow_mut::<ADBannerViewHostObject>(this);
    if host_object.failure_reported {
        return;
    }
    host_object.failure_reported = true;
    let delegate = host_object.delegate;
    if delegate == nil {
        return;
    }
    let responds: bool = {
        let sel = env
            .objc
            .lookup_selector("bannerView:didFailToReceiveAdWithError:")
            .unwrap();
        msg![env; delegate respondsToSelector:sel]
    };
    if !responds {
        return;
    }
    // ADErrorDomain / ADErrorInventoryUnavailable: the documented code for
    // "there is no ad to show", which is permanently true now.
    let domain = get_static_str(env, ADErrorDomain);
    let error: id = msg_class![env; NSError alloc];
    let error: id =
        msg![env; error initWithDomain:domain code:(ADErrorInventoryUnavailable) userInfo:nil];
    () = msg![env; delegate bannerView:this didFailToReceiveAdWithError:error];
    release(env, error);
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation ADBannerView: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<ADBannerViewHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (())dealloc {
    let &ADBannerViewHostObject {
        current_content_size_identifier,
        required_content_size_identifiers,
        ..
    } = env.objc.borrow(this);
    release(env, current_content_size_identifier);
    release(env, required_content_size_identifiers);
    msg_super![env; this dealloc]
}

- (id)delegate {
    env.objc.borrow::<ADBannerViewHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<ADBannerViewHostObject>(this).delegate = delegate;
    // An app typically sets the delegate right after creating the banner, so
    // this is the first moment the failure can be delivered.
    report_failure_once(env, this);
}

- (id)currentContentSizeIdentifier {
    env.objc.borrow::<ADBannerViewHostObject>(this).current_content_size_identifier
}
- (())setCurrentContentSizeIdentifier:(id)identifier { // NSString*
    retain(env, identifier);
    let host_object = env.objc.borrow_mut::<ADBannerViewHostObject>(this);
    let old = std::mem::replace(&mut host_object.current_content_size_identifier, identifier);
    release(env, old);
}

- (id)requiredContentSizeIdentifiers {
    env.objc.borrow::<ADBannerViewHostObject>(this).required_content_size_identifiers
}
- (())setRequiredContentSizeIdentifiers:(id)identifiers { // NSSet*
    retain(env, identifiers);
    let host_object = env.objc.borrow_mut::<ADBannerViewHostObject>(this);
    let old = std::mem::replace(&mut host_object.required_content_size_identifiers, identifiers);
    release(env, old);
}

// No ad ever loads, so these are constant. An app that polls bannerLoaded
// instead of implementing the delegate method still sees a consistent answer.
- (bool)bannerLoaded {
    report_failure_once(env, this);
    false
}
- (bool)isBannerLoaded {
    msg![env; this bannerLoaded]
}
- (bool)bannerViewActionInProgress {
    false
}
- (bool)isBannerViewActionInProgress {
    false
}

- (())cancelBannerViewAction {
    // Nothing can be in progress.
}

@end

};

pub const ADErrorDomain: &str = "ADErrorDomain";
/// `ADError` case for "no ad available", which is now permanent.
pub const ADErrorInventoryUnavailable: i32 = 2;

pub const ADBannerContentSizeIdentifier320x50: &str = "IAdAdTypeBanner";
pub const ADBannerContentSizeIdentifier480x32: &str = "IAdAdTypeLandscapeBanner";
pub const ADBannerContentSizeIdentifierPortrait: &str = "320x50";
pub const ADBannerContentSizeIdentifierLandscape: &str = "480x32";

pub const CONSTANTS: ConstantExports = &[
    ("_ADErrorDomain", HostConstant::NSString(ADErrorDomain)),
    (
        "_ADBannerContentSizeIdentifier320x50",
        HostConstant::NSString(ADBannerContentSizeIdentifier320x50),
    ),
    (
        "_ADBannerContentSizeIdentifier480x32",
        HostConstant::NSString(ADBannerContentSizeIdentifier480x32),
    ),
    (
        "_ADBannerContentSizeIdentifierPortrait",
        HostConstant::NSString(ADBannerContentSizeIdentifierPortrait),
    ),
    (
        "_ADBannerContentSizeIdentifierLandscape",
        HostConstant::NSString(ADBannerContentSizeIdentifierLandscape),
    ),
];
