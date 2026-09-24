/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
#include "system_headers.h"
#include <stdio.h>

// The open test SDK does not carry OpenGLES headers.
@interface EAGLContext : NSObject
- (id)initWithAPI:(NSUInteger)api;
- (id)initWithAPI:(NSUInteger)api sharegroup:(id)group;
- (id)sharegroup;
+ (BOOL)setCurrentContext:(EAGLContext *)context;
@end
extern void glGenFramebuffersOES(int, unsigned *);
extern void glGenRenderbuffersOES(int, unsigned *);
extern void glBindFramebufferOES(unsigned, unsigned);
extern void glBindRenderbufferOES(unsigned, unsigned);
extern void glRenderbufferStorageOES(unsigned, unsigned, int, int);
extern void glFramebufferRenderbufferOES(unsigned, unsigned, unsigned, unsigned);
extern void glGetFramebufferAttachmentParameterivOES(unsigned, unsigned, unsigned,
                                                     int *);
extern unsigned glCheckFramebufferStatusOES(unsigned);
extern unsigned char glIsFramebufferOES(unsigned);
extern unsigned char glIsRenderbufferOES(unsigned);
extern void glDeleteFramebuffersOES(int, const unsigned *);
extern void glDeleteRenderbuffersOES(int, const unsigned *);
extern void glGetIntegerv(unsigned, int *);
extern void glGetFloatv(unsigned, float *);
extern void glGetBooleanv(unsigned, unsigned char *);
extern unsigned glGetError(void);
extern void glClearColor(float, float, float, float);
extern void glClear(unsigned);
extern void glReadPixels(int, int, int, int, unsigned, unsigned, void *);

#define FB 0x8D40
#define RB 0x8D41
#define FB_BINDING 0x8CA6
#define RB_BINDING 0x8CA7
#define COLOR_ATTACHMENT 0x8CE0
#define OBJECT_NAME 0x8CD1
#define REQUIRE(condition)                                                      \
  do {                                                                          \
    if (!(condition)) {                                                         \
      printf("GLES object isolation failed at line %d: %s\n", __LINE__,          \
             #condition);                                                       \
      return 1;                                                                 \
    }                                                                           \
  } while (0)

static int test_names(unsigned api) {
  EAGLContext *first = [[EAGLContext alloc] initWithAPI:api];
  REQUIRE(first != nil);
  REQUIRE([EAGLContext setCurrentContext:first]);
  int value = -1;
  glGetIntegerv(FB_BINDING, &value);
  REQUIRE(value == 0);
  glGetIntegerv(RB_BINDING, &value);
  REQUIRE(value == 0);
  unsigned fb = 0, rb = 0;
  glGenFramebuffersOES(1, &fb);
  glGenRenderbuffersOES(1, &rb);
  REQUIRE(fb == 1 && rb == 1);
  REQUIRE(!glIsFramebufferOES(fb) && !glIsRenderbufferOES(rb));
  glBindFramebufferOES(FB, fb);
  glBindRenderbufferOES(RB, rb);
  glRenderbufferStorageOES(RB, 0x8058, 2, 2); // RGBA8
  glFramebufferRenderbufferOES(FB, COLOR_ATTACHMENT, RB, rb);
  REQUIRE(glCheckFramebufferStatusOES(FB) == 0x8CD5);
  glGetFramebufferAttachmentParameterivOES(FB, COLOR_ATTACHMENT, OBJECT_NAME,
                                           &value);
  REQUIRE(value == (int)rb);
  float float_value = 0;
  unsigned char bool_value = 0;
  glGetFloatv(RB_BINDING, &float_value);
  glGetBooleanv(FB_BINDING, &bool_value);
  REQUIRE(float_value == 1 && bool_value == 1);
  glClearColor(1, 0, 0, 1);
  glClear(0x4000);
  unsigned char pixel[4] = {0};
  glReadPixels(0, 0, 1, 1, 0x1908, 0x1401, pixel);
  REQUIRE(pixel[0] == 255 && pixel[1] == 0 && pixel[2] == 0);

  EAGLContext *second =
      [[EAGLContext alloc] initWithAPI:api sharegroup:[first sharegroup]];
  REQUIRE([EAGLContext setCurrentContext:second]);
  glGetIntegerv(FB_BINDING, &value);
  REQUIRE(value == 0);
  REQUIRE(glIsFramebufferOES(fb) && glIsRenderbufferOES(rb));
  glBindRenderbufferOES(RB, rb);
  glDeleteRenderbuffersOES(1, &rb);
  glGetIntegerv(RB_BINDING, &value);
  REQUIRE(value == 0 && !glIsRenderbufferOES(rb));
  REQUIRE([EAGLContext setCurrentContext:first]);
  // Deletion does not unbind an object from another context.
  glGetIntegerv(RB_BINDING, &value);
  REQUIRE(value == (int)rb);
  glGetFramebufferAttachmentParameterivOES(FB, COLOR_ATTACHMENT, OBJECT_NAME,
                                           &value);
  REQUIRE(value == (int)rb);
  glBindRenderbufferOES(RB, 0);
  glDeleteFramebuffersOES(1, &fb);
  glGetIntegerv(FB_BINDING, &value);
  REQUIRE(value == 0 && !glIsFramebufferOES(fb));
  REQUIRE(glGetError() == 0);
  [EAGLContext setCurrentContext:nil];
  [second release];
  [first release];
  return 0;
}

int TestApp_gles_tests_main(void) {
  NSAutoreleasePool *pool = [NSAutoreleasePool new];
  int passed = 0;
  for (unsigned api = 1; api <= 2; api++) {
    int result = test_names(api);
    printf("GLES %u object isolation: %s\n", api, result ? "FAIL" : "OK");
    passed += result == 0;
  }
  printf("Passed %d out of 2 tests\n", passed);
  [pool drain];
  return passed == 2 ? 0 : 1;
}
