#import "system_headers.h"

static int operation_value;

@interface TestOperation : NSOperation
@end

@implementation TestOperation
- (void)main {
  if (![self isExecuting])
    operation_value = -100;
  else
    operation_value = 1;
}
@end

@interface NSOperationInvocationTarget : NSObject
- (void)record:(id)object;
@end

@implementation NSOperationInvocationTarget
- (void)record:(id)object {
  if (object == self)
    operation_value = 2;
}
@end

int test_NSOperation(void) {
  TestOperation *operation = [TestOperation new];
  if (![operation isReady] || [operation isExecuting] ||
      [operation isFinished] || [operation isCancelled])
    return -1;

  NSOperationQueue *queue = [NSOperationQueue new];
  [queue setMaxConcurrentOperationCount:2];
  if ([queue maxConcurrentOperationCount] != 2)
    return -6;
  if ([TestOperation instanceMethodForSelector:@selector(main)] == NULL)
    return -7;
  [queue addOperation:operation];
  if (operation_value != 1 || [operation isExecuting] ||
      ![operation isFinished])
    return -2;
  if ([queue operationCount] != 0 || [[queue operations] count] != 0)
    return -3;

  NSOperationInvocationTarget *target = [NSOperationInvocationTarget new];
  NSInvocationOperation *invocation =
      [[NSInvocationOperation alloc] initWithTarget:target
                                          selector:@selector(record:)
                                            object:target];
  [queue addOperation:invocation];
  if (operation_value != 2 || ![invocation isFinished])
    return -4;

  // initWithInvocation: retains and executes an NSInvocation with arbitrary
  // arguments, instead of reducing it to the one-object convenience form.
  operation_value = 0;
  NSMethodSignature *signature =
      [NSMethodSignature signatureWithObjCTypes:"v12@0:4@8"];
  NSInvocation *message = [NSInvocation invocationWithMethodSignature:signature];
  [message setTarget:target];
  [message setSelector:@selector(record:)];
  id argument = target;
  [message setArgument:&argument atIndex:2];
  NSInvocationOperation *from_invocation =
      [[NSInvocationOperation alloc] initWithInvocation:message];
  [queue addOperation:from_invocation];
  if (operation_value != 2 || ![from_invocation isFinished])
    return -8;

  TestOperation *cancelled = [TestOperation new];
  [cancelled cancel];
  [queue addOperation:cancelled];
  if (![cancelled isCancelled] || ![cancelled isFinished] ||
      operation_value != 2)
    return -5;

  [cancelled release];
  [from_invocation release];
  [invocation release];
  [target release];
  [queue release];
  [operation release];
  return 0;
}
