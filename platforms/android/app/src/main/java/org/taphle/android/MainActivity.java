/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * Parts of this file are derived from SDL 2's Android project template, which
 * has a different license. Please see vendor/SDL/LICENSE.txt for details.
 */
package org.taphle.android;

import android.view.KeyEvent;

import org.libsdl.app.SDLActivity;

/**
 * A wrapper class over SDLActivity
 */

public class MainActivity extends SDLActivity {
    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        if (event.getKeyCode() == KeyEvent.KEYCODE_BACK) {
            if (event.getAction() == KeyEvent.ACTION_UP) {
                SDLActivity.nativeSendQuit();
            }
            return true;
        }
        return super.dispatchKeyEvent(event);
    }

    @Override
    public void onBackPressed() {
        SDLActivity.nativeSendQuit();
    }

    @Override
    protected String[] getLibraries() {
        return new String[]{
            "SDL2",
            "tapHLE_gui"
        };
    }
}
