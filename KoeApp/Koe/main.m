#import <Cocoa/Cocoa.h>

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        NSApplication *app = [NSApplication sharedApplication];
        // AppDelegate is set via Info.plist NSPrincipalClass + NSMainNibFile,
        // or we set it manually here.
        // Load the delegate class
        Class delegateClass = NSClassFromString(@"SPAppDelegate");
        if (delegateClass) {
            id delegate = [[delegateClass alloc] init];
            [app setDelegate:delegate];
        }
        [app run];
    }
    return 0;
}
