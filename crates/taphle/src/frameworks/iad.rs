/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! iAd.
//!
//! The iAd network was retired by Apple in 2016, so no banner can ever load.
//! That is not a gap tapHLE can close, but it is a state every iAd app already
//! had to handle: `ADBannerView` reports failure through
//! `bannerView:didFailToReceiveAdWithError:` whenever there is no inventory or
//! no network, and apps respond by hiding the banner and carrying on. This
//! models exactly that permanently-unfilled banner.
//!
//! Resources:
//! - Apple's [iAd Programming Guide](https://developer.apple.com/library/archive/documentation/UserExperience/Conceptual/iAd_Guide/Introduction/Introduction.html)
//!   (archived).

mod ad_banner_view;

pub const DYLIB: crate::dyld::HostDylib = crate::dyld::HostDylib {
    path: "/System/Library/Frameworks/iAd.framework/iAd",
    aliases: &[],
    class_exports: &[ad_banner_view::CLASSES],
    constant_exports: &[ad_banner_view::CONSTANTS],
    function_exports: &[],
};
