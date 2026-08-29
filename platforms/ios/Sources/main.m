/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
#include <SDL.h>
#include <SDL_main.h>
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
    return tapHLE_iOS_main(argc, argv);
}
