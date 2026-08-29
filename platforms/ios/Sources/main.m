/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
#include <SDL.h>
#include <SDL_main.h>
#include <stdlib.h>
#include <TargetConditionals.h>
#import <UIKit/UIKit.h>

#ifdef main
#undef main
#endif

extern int tapHLE_iOS_main(int argc, char *argv[]);

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
