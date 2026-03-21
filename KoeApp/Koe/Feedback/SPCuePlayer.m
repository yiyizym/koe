#import "SPCuePlayer.h"
#import <AppKit/AppKit.h>

@implementation SPCuePlayer

- (void)playStart {
    [self playSystemSound:@"Tink"];
}

- (void)playStop {
    [self playSystemSound:@"Pop"];
}

- (void)playError {
    [self playSystemSound:@"Basso"];
}

- (void)playSystemSound:(NSString *)name {
    NSSound *sound = [NSSound soundNamed:name];
    if (sound) {
        [sound play];
    }
}

@end
