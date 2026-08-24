/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMusicPlayerController` etc.

use crate::{
    dyld::{ConstantExports, HostConstant},
    objc::{id, nil, objc_classes, ClassExports},
};

pub const MPMusicPlayerControllerNowPlayingItemDidChangeNotification: &str =
    "MPMusicPlayerControllerNowPlayingItemDidChangeNotification";
pub const MPMusicPlayerControllerPlaybackStateDidChangeNotification: &str =
    "MPMusicPlayerControllerPlaybackStateDidChangeNotification";
/// There is no music library or player state here, so none of the following is
/// ever posted or populated. They exist because an app registers for them, or
/// reads a media item's properties, during ordinary startup — and doing that
/// dereferences the symbol, so an unbound one crashes the app before it reaches
/// anything it could have degraded gracefully.
pub const MPMusicPlayerControllerVolumeDidChangeNotification: &str =
    "MPMusicPlayerControllerVolumeDidChangeNotification";
pub const MPMediaLibraryDidChangeNotification: &str = "MPMediaLibraryDidChangeNotification";
pub const MPMediaItemPropertyTitle: &str = "title";
pub const MPMediaItemPropertyArtist: &str = "artist";
pub const MPMediaItemPropertyAlbumTitle: &str = "albumTitle";
pub const MPMediaItemPropertyPlaybackDuration: &str = "playbackDuration";
pub const MPMediaItemPropertyAssetURL: &str = "assetURL";
pub const MPMediaPlaylistPropertyName: &str = "name";

/// `NSNotificationName` values.
pub const CONSTANTS: ConstantExports = &[
    (
        "_MPMusicPlayerControllerNowPlayingItemDidChangeNotification",
        HostConstant::NSString(MPMusicPlayerControllerNowPlayingItemDidChangeNotification),
    ),
    (
        "_MPMusicPlayerControllerPlaybackStateDidChangeNotification",
        HostConstant::NSString(MPMusicPlayerControllerPlaybackStateDidChangeNotification),
    ),
    (
        "_MPMusicPlayerControllerVolumeDidChangeNotification",
        HostConstant::NSString(MPMusicPlayerControllerVolumeDidChangeNotification),
    ),
    (
        "_MPMediaLibraryDidChangeNotification",
        HostConstant::NSString(MPMediaLibraryDidChangeNotification),
    ),
    (
        "_MPMediaItemPropertyTitle",
        HostConstant::NSString(MPMediaItemPropertyTitle),
    ),
    (
        "_MPMediaItemPropertyArtist",
        HostConstant::NSString(MPMediaItemPropertyArtist),
    ),
    (
        "_MPMediaItemPropertyAlbumTitle",
        HostConstant::NSString(MPMediaItemPropertyAlbumTitle),
    ),
    (
        "_MPMediaItemPropertyPlaybackDuration",
        HostConstant::NSString(MPMediaItemPropertyPlaybackDuration),
    ),
    (
        "_MPMediaItemPropertyAssetURL",
        HostConstant::NSString(MPMediaItemPropertyAssetURL),
    ),
    (
        "_MPMediaPlaylistPropertyName",
        HostConstant::NSString(MPMediaPlaylistPropertyName),
    ),
];

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMusicPlayerController: NSObject

+ (id)iPodMusicPlayer {
    log_dbg!(
        "TODO: [(MPMusicPlayerController*){:?} iPodMusicPlayer]",
        this
    );
    nil
}

+ (id)applicationMusicPlayer {
    log_dbg!(
        "TODO: [(MPMusicPlayerController*){:?} applicationMusicPlayer]",
        this
    );
    nil
}

@end

};
