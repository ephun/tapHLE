/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
#include <SDL.h>
#include <SDL_main.h>
#include <SDL_syswm.h>
#include <stdlib.h>
#include <TargetConditionals.h>
#import <CommonCrypto/CommonDigest.h>
#import <UIKit/UIKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>

#ifdef main
#undef main
#endif

extern int tapHLE_iOS_main(int argc, char *argv[]);

typedef NS_ENUM(NSInteger, TapHLEPickerPurpose) {
    TapHLEPickerPurposeImportApps,
    TapHLEPickerPurposeExportFolder,
};

@interface TapHLEDocumentPickerDelegate : NSObject <UIDocumentPickerDelegate>
@property(nonatomic) TapHLEPickerPurpose purpose;
@end

static TapHLEDocumentPickerDelegate *document_picker_delegate;

static dispatch_queue_t import_queue(void) {
    static dispatch_queue_t queue;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        queue = dispatch_queue_create("org.taphle.document-import", DISPATCH_QUEUE_SERIAL);
    });
    return queue;
}

static UIWindow *active_window(void) {
    for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
        if (scene.activationState != UISceneActivationStateForegroundActive ||
            ![scene isKindOfClass:UIWindowScene.class]) continue;
        UIWindowScene *window_scene = (UIWindowScene *)scene;
        for (UIWindow *window in window_scene.windows)
            if (window.isKeyWindow) return window;
        for (UIWindow *window in window_scene.windows)
            if (!window.isHidden && window.alpha > 0) return window;
    }
    return nil;
}

static UIViewController *presenting_view_controller(void) {
    UIWindow *window = active_window();
    UIViewController *controller = window.rootViewController;
    while (controller) {
        if (controller.presentedViewController) {
            controller = controller.presentedViewController;
        } else if ([controller isKindOfClass:UINavigationController.class]) {
            controller = ((UINavigationController *)controller).visibleViewController;
        } else if ([controller isKindOfClass:UITabBarController.class]) {
            controller = ((UITabBarController *)controller).selectedViewController;
        } else {
            break;
        }
    }
    return controller;
}

static int on_main_thread(int (^operation)(void)) {
    if (NSThread.isMainThread) return operation();
    __block int result = 0;
    dispatch_sync(dispatch_get_main_queue(), ^{ result = operation(); });
    return result;
}

static NSURL *documents_url(void) {
    return [NSFileManager.defaultManager URLsForDirectory:NSDocumentDirectory
                                                inDomains:NSUserDomainMask].firstObject;
}

const char *tapHLE_ios_documents_path(void) {
    static char *path;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        path = strdup(documents_url().fileSystemRepresentation);
    });
    return path;
}

static void push_drop_file(NSURL *url) {
    SDL_Event event = {0};
    event.type = SDL_DROPFILE;
    event.drop.file = SDL_strdup(url.fileSystemRepresentation);
    if (!event.drop.file) {
        NSLog(@"tapHLE document import could not enqueue %@", url.path);
        return;
    }
    if (SDL_PushEvent(&event) != 1) {
        SDL_free(event.drop.file);
        NSLog(@"tapHLE document import could not enqueue %@: %s", url.path,
              SDL_GetError());
    }
}

static NSData *file_digest(NSURL *url, NSError **error) {
    NSFileHandle *file = [NSFileHandle fileHandleForReadingFromURL:url error:error];
    if (!file) return nil;
    CC_SHA256_CTX context;
    CC_SHA256_Init(&context);
    for (;;) {
        NSData *data = [file readDataUpToLength:1024 * 1024 error:error];
        if (!data) {
            [file closeFile];
            return nil;
        }
        if (data.length == 0) break;
        CC_SHA256_Update(&context, data.bytes, (CC_LONG)data.length);
    }
    [file closeFile];
    unsigned char bytes[CC_SHA256_DIGEST_LENGTH];
    CC_SHA256_Final(bytes, &context);
    return [NSData dataWithBytes:bytes length:sizeof(bytes)];
}

static NSURL *destination_for_digest(NSURL *folder, NSData *digest) {
    const unsigned char *bytes = digest.bytes;
    NSMutableString *name =
        [NSMutableString stringWithCapacity:CC_SHA256_DIGEST_LENGTH * 2];
    for (NSUInteger index = 0; index < digest.length; ++index)
        [name appendFormat:@"%02x", bytes[index]];
    return [folder URLByAppendingPathComponent:
        [name stringByAppendingPathExtension:@"ipa"]];
}

@implementation TapHLEDocumentPickerDelegate
- (void)documentPicker:(UIDocumentPickerViewController *)controller
    didPickDocumentsAtURLs:(NSArray<NSURL *> *)urls {
    if (self.purpose == TapHLEPickerPurposeImportApps) {
        NSArray<NSURL *> *sources = [urls copy];
        NSMutableIndexSet *scoped_sources = [NSMutableIndexSet indexSet];
        [sources enumerateObjectsUsingBlock:^(NSURL *source, NSUInteger index,
                                               BOOL *stop) {
            if ([source startAccessingSecurityScopedResource])
                [scoped_sources addIndex:index];
        }];
        TapHLEDocumentPickerDelegate *delegate = self;
        dispatch_async(import_queue(), ^{
            @autoreleasepool {
                NSFileManager *files = NSFileManager.defaultManager;
                NSURL *apps = [documents_url() URLByAppendingPathComponent:@"apps"
                                                                isDirectory:YES];
                NSMutableArray<NSURL *> *destinations = [NSMutableArray array];
                NSError *directory_error = nil;
                if (![files createDirectoryAtURL:apps withIntermediateDirectories:YES
                                      attributes:nil error:&directory_error]) {
                    NSLog(@"tapHLE document import could not create %@: %@", apps.path,
                          directory_error);
                } else {
                    for (NSURL *source in sources) {
                        NSError *error = nil;
                        NSData *digest = file_digest(source, &error);
                        if (!digest) {
                            NSLog(@"tapHLE document import could not read %@: %@",
                                  source.path, error);
                            continue;
                        }
                        NSURL *destination = destination_for_digest(apps, digest);
                        BOOL is_directory = NO;
                        BOOL exists = [files fileExistsAtPath:destination.path
                                                 isDirectory:&is_directory];
                        if (exists && !is_directory) {
                            [destinations addObject:destination];
                        } else if (exists) {
                            NSLog(@"tapHLE document import destination is not a file: %@",
                                  destination.path);
                        } else if ([files copyItemAtURL:source toURL:destination
                                                 error:&error]) {
                            [destinations addObject:destination];
                        } else {
                            NSLog(@"tapHLE document import could not copy %@: %@",
                                  source.path, error);
                        }
                    }
                }
                [scoped_sources enumerateIndexesUsingBlock:^(NSUInteger index,
                                                               BOOL *stop) {
                    [sources[index] stopAccessingSecurityScopedResource];
                }];
                dispatch_async(dispatch_get_main_queue(), ^{
                    for (NSURL *destination in destinations)
                        push_drop_file(destination);
                    if (document_picker_delegate == delegate)
                        document_picker_delegate = nil;
                });
            }
        });
        return;
    } else {
        NSURL *destination = urls.firstObject;
        if (destination)
            NSLog(@"tapHLE Apps folder exported to %@", destination.path);
        else
            NSLog(@"tapHLE Apps folder export returned no destination");
    }
    document_picker_delegate = nil;
}

- (void)documentPickerWasCancelled:(UIDocumentPickerViewController *)controller {
    document_picker_delegate = nil;
}
@end

static int present_import_picker(void) {
    UIViewController *presenter = presenting_view_controller();
    if (!presenter || document_picker_delegate) return 0;
    UTType *ipa = [UTType typeWithFilenameExtension:@"ipa"] ?: UTTypeData;
    UIDocumentPickerViewController *picker =
        [[UIDocumentPickerViewController alloc]
            initForOpeningContentTypes:@[ipa] asCopy:YES];
    document_picker_delegate = [TapHLEDocumentPickerDelegate new];
    document_picker_delegate.purpose = TapHLEPickerPurposeImportApps;
    picker.delegate = document_picker_delegate;
    picker.allowsMultipleSelection = YES;
    [presenter presentViewController:picker animated:YES completion:nil];
    return 1;
}

int tapHLE_ios_present_app_picker(void) {
    return on_main_thread(^int { return present_import_picker(); });
}

int tapHLE_ios_export_folder(const char *path) {
    if (!path) return 0;
    NSURL *folder = [NSURL fileURLWithFileSystemRepresentation:path
                                                   isDirectory:YES relativeToURL:nil];
    return on_main_thread(^int {
        BOOL is_directory = NO;
        if (![NSFileManager.defaultManager fileExistsAtPath:folder.path
                                                isDirectory:&is_directory] || !is_directory)
            return 0;
        UIViewController *presenter = presenting_view_controller();
        if (!presenter || document_picker_delegate) return 0;
        UIDocumentPickerViewController *picker =
            [[UIDocumentPickerViewController alloc]
                initForExportingURLs:@[folder] asCopy:YES];
        document_picker_delegate = [TapHLEDocumentPickerDelegate new];
        document_picker_delegate.purpose = TapHLEPickerPurposeExportFolder;
        picker.delegate = document_picker_delegate;
        [presenter presentViewController:picker animated:YES completion:nil];
        return 1;
    });
}

int tapHLE_ios_open_url(const char *text) {
    if (!text) return 0;
    NSString *string = [NSString stringWithUTF8String:text];
    NSURL *url = string ? [NSURL URLWithString:string] : nil;
    if (!url) return 0;
    return on_main_thread(^int {
        UIApplication *application = UIApplication.sharedApplication;
        if (![application canOpenURL:url]) return 0;
        [application openURL:url options:@{} completionHandler:^(BOOL success) {
            if (!success) NSLog(@"tapHLE could not open URL %@", url);
        }];
        return 1;
    });
}

// The pinned SDL UIKit backend gives each GL context its own view. Making
// an older context current does not make its view visible again. Keep this
// adapter to SDL's own classes here; these are not private Apple APIs.
@protocol TapHLESDLContext
@property(nonatomic, readonly) UIView *sdlView;
@end
@protocol TapHLESDLView
- (void)setSDLWindow:(SDL_Window *)window;
@end

int tapHLE_iOS_drawable_bindings(unsigned int *framebuffer, unsigned int *renderbuffer) {
    SDL_SysWMinfo info;
    SDL_VERSION(&info.version);
    SDL_Window *window = SDL_GL_GetCurrentWindow();
    if (!window || !SDL_GetWindowWMInfo(window, &info) ||
        info.subsystem != SDL_SYSWM_UIKIT) return 0;
    id<TapHLESDLContext> context = (__bridge id)SDL_GL_GetCurrentContext();
    UIView *view = context.sdlView;
    if (info.info.uikit.window.rootViewController.view != view) {
        [(id<TapHLESDLView>)view setSDLWindow:NULL];
        [(id<TapHLESDLView>)view setSDLWindow:window];
        if (!SDL_GetWindowWMInfo(window, &info)) return 0;
    }
    *framebuffer = info.info.uikit.framebuffer;
    *renderbuffer = info.info.uikit.colorbuffer;
    return *framebuffer != 0 && *renderbuffer != 0;
}

void tapHLE_iOS_safe_area(float *top, float *right, float *bottom, float *left) {
    UIEdgeInsets insets = active_window().safeAreaInsets;
    *top = insets.top;
    *right = insets.right;
    *bottom = insets.bottom;
    *left = insets.left;
}

int main(int argc, char *argv[]) {
#if TARGET_OS_SIMULATOR
    // OpenAL Soft reads its backend selection before UIKit starts SDL. The
    // Intel simulator has no usable CoreAudio output, so select its null
    // backend before any framework can initialize OpenAL.
    setenv("ALSOFT_DRIVERS", "null", 0);
#endif
    return tapHLE_iOS_main(argc, argv);
}
