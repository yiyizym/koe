#import <Foundation/Foundation.h>

typedef void (^SPPermissionCheckCompletion)(BOOL micGranted, BOOL accessibilityGranted, BOOL inputMonitoringGranted);

@interface SPPermissionManager : NSObject

- (void)checkAllPermissionsWithCompletion:(SPPermissionCheckCompletion)completion;
- (BOOL)isMicrophoneGranted;
- (BOOL)isAccessibilityGranted;
- (BOOL)isInputMonitoringGranted;

@end
