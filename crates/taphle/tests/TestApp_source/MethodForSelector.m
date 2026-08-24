#import "system_headers.h"

@interface MethodForSelectorTestClass : NSObject
@end

@implementation MethodForSelectorTestClass
- (int)guestMethod {
  return 0x544150;
}
@end

int test_MethodForSelector(void) {
  MethodForSelectorTestClass *instance = [MethodForSelectorTestClass new];
  SEL guest_method =
      NSSelectorFromString([NSString stringWithUTF8String:"guestMethod"]);
  int (*imp)(id, SEL) = (int (*)(id, SEL))[instance methodForSelector:guest_method];
  if (imp == NULL)
    return -1;
  if (imp(instance, guest_method) != 0x544150)
    return -2;

  SEL missing_method =
      NSSelectorFromString([NSString stringWithUTF8String:"missingMethod"]);
  if ([instance methodForSelector:missing_method] != NULL)
    return -3;

  return 0;
}
