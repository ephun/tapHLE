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
#import <UIKit/UIKit.h>

#ifdef main
#undef main
#endif

extern int tapHLE_iOS_main(int argc, char *argv[]);

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
    UIEdgeInsets insets = UIApplication.sharedApplication.keyWindow.safeAreaInsets;
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
