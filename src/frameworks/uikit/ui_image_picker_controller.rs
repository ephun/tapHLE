/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIImagePickerController`

use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::foundation::NSInteger;
use crate::objc::{id, objc_classes, ClassExports};

type UIImagePickerControllerSourceType = NSInteger;

/// Keys of the info dictionary a picker hands its delegate. No source is
/// available here, so no such dictionary is ever built — but apps reference
/// these keys while merely *setting up* a picker, and an unbound key is a null
/// pointer they dereference before ever presenting anything.
const UIImagePickerControllerMediaType: &str = "UIImagePickerControllerMediaType";
const UIImagePickerControllerOriginalImage: &str = "UIImagePickerControllerOriginalImage";
const UIImagePickerControllerEditedImage: &str = "UIImagePickerControllerEditedImage";
const UIImagePickerControllerCropRect: &str = "UIImagePickerControllerCropRect";
const UIImagePickerControllerMediaURL: &str = "UIImagePickerControllerMediaURL";
const UIImagePickerControllerReferenceURL: &str = "UIImagePickerControllerReferenceURL";

pub const CONSTANTS: ConstantExports = &[
    (
        "_UIImagePickerControllerMediaType",
        HostConstant::NSString(UIImagePickerControllerMediaType),
    ),
    (
        "_UIImagePickerControllerOriginalImage",
        HostConstant::NSString(UIImagePickerControllerOriginalImage),
    ),
    (
        "_UIImagePickerControllerEditedImage",
        HostConstant::NSString(UIImagePickerControllerEditedImage),
    ),
    (
        "_UIImagePickerControllerCropRect",
        HostConstant::NSString(UIImagePickerControllerCropRect),
    ),
    (
        "_UIImagePickerControllerMediaURL",
        HostConstant::NSString(UIImagePickerControllerMediaURL),
    ),
    (
        "_UIImagePickerControllerReferenceURL",
        HostConstant::NSString(UIImagePickerControllerReferenceURL),
    ),
];

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// TODO: should extend UINavigationController, which extends
//       UIViewController.
@implementation UIImagePickerController: UIViewController

+ (bool)isSourceTypeAvailable:(UIImagePickerControllerSourceType)_type {
    // For now, simply claim no sources are available.
    // TODO: support some sources.
    false
}

- (())setDelegate:(id)_delegate {
    // TODO
}

@end

};
