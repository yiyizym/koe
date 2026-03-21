#import <Foundation/Foundation.h>

@protocol SPStatusBarDelegate <NSObject>
@optional
- (void)statusBarDidSelectReloadConfig;
- (void)statusBarDidSelectQuit;
@end

@interface SPStatusBarManager : NSObject

- (instancetype)initWithDelegate:(id<SPStatusBarDelegate>)delegate;

/// Update the status bar icon and status text.
/// state: "idle", "recording_hold", "recording_toggle", "connecting_asr",
///        "finalizing_asr", "correcting", "preparing_paste", "pasting", "error", "completed"
- (void)updateState:(NSString *)state;

@end
