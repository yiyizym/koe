#import "SPPasteManager.h"
#import <Carbon/Carbon.h>
#import <ApplicationServices/ApplicationServices.h>

@implementation SPPasteManager

- (void)simulatePasteWithCompletion:(void (^)(void))completion {
    // Small delay after clipboard write to ensure it's ready
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(50 * NSEC_PER_MSEC)),
                   dispatch_get_main_queue(), ^{
        [self performPaste];

        // Delay after paste to let the target app process it
        if (completion) {
            dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(100 * NSEC_PER_MSEC)),
                           dispatch_get_main_queue(), ^{
                completion();
            });
        }
    });
}

- (void)performPaste {
    CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStateHIDSystemState);
    if (!source) {
        NSLog(@"[Koe] Failed to create event source for paste");
        return;
    }

    // Key code for 'V' is 9 (kVK_ANSI_V)
    CGEventRef cmdDown = CGEventCreateKeyboardEvent(source, (CGKeyCode)kVK_ANSI_V, true);
    CGEventRef cmdUp = CGEventCreateKeyboardEvent(source, (CGKeyCode)kVK_ANSI_V, false);

    // Set the Command modifier
    CGEventSetFlags(cmdDown, kCGEventFlagMaskCommand);
    CGEventSetFlags(cmdUp, kCGEventFlagMaskCommand);

    // Post events
    CGEventPost(kCGHIDEventTap, cmdDown);
    CGEventPost(kCGHIDEventTap, cmdUp);

    CFRelease(cmdDown);
    CFRelease(cmdUp);
    CFRelease(source);

    NSLog(@"[Koe] Cmd+V simulated");
}

@end
