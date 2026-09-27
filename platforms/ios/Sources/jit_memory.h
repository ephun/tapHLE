/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
#pragma once
#include <stdbool.h>
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct {
    void *write;
    void *execute;
    size_t size;
} TapHLEJITMemory;
bool tapHLE_jit_memory_prepare(TapHLEJITMemory *,
    void *(*prepare_region)(void *, size_t), void (*detach_script)(void),
    char error[512]);
#ifdef __cplusplus
}
#endif
