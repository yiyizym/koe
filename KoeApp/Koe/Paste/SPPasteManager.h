#import <Foundation/Foundation.h>

@interface SPPasteManager : NSObject

/// Simulate Cmd+V paste via CGEvent injection.
/// The completion block is called after a short delay to allow the paste to take effect.
- (void)simulatePasteWithCompletion:(void (^)(void))completion;

@end
