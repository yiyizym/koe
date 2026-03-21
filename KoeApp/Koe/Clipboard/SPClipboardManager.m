#import "SPClipboardManager.h"
#import <AppKit/AppKit.h>

@interface SPClipboardManager ()

@property (nonatomic, strong) NSArray<NSPasteboardItem *> *backedUpItems;
@property (nonatomic, assign) NSInteger backedUpChangeCount;
@property (nonatomic, assign) NSInteger writtenChangeCount;

@end

@implementation SPClipboardManager

- (void)backup {
    NSPasteboard *pb = [NSPasteboard generalPasteboard];
    self.backedUpChangeCount = pb.changeCount;

    // Deep copy current pasteboard items
    NSMutableArray<NSPasteboardItem *> *items = [NSMutableArray array];
    for (NSPasteboardItem *item in pb.pasteboardItems) {
        NSPasteboardItem *copy = [[NSPasteboardItem alloc] init];
        for (NSString *type in item.types) {
            NSData *data = [item dataForType:type];
            if (data) {
                [copy setData:data forType:type];
            }
        }
        [items addObject:copy];
    }
    self.backedUpItems = items;
}

- (void)writeText:(NSString *)text {
    NSPasteboard *pb = [NSPasteboard generalPasteboard];
    [pb clearContents];
    [pb setString:text forType:NSPasteboardTypeString];
    self.writtenChangeCount = pb.changeCount;
}

- (void)scheduleRestoreAfterDelay:(NSUInteger)delayMs {
    if (!self.backedUpItems) return;

    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delayMs * NSEC_PER_MSEC)),
                   dispatch_get_main_queue(), ^{
        [self restoreIfUnchanged];
    });
}

- (void)restoreIfUnchanged {
    NSPasteboard *pb = [NSPasteboard generalPasteboard];

    // Only restore if the clipboard hasn't been modified since we wrote to it
    if (pb.changeCount != self.writtenChangeCount) {
        NSLog(@"[Koe] Clipboard changed since write, skipping restore");
        return;
    }

    if (!self.backedUpItems || self.backedUpItems.count == 0) {
        return;
    }

    [pb clearContents];
    [pb writeObjects:self.backedUpItems];
    self.backedUpItems = nil;
    NSLog(@"[Koe] Clipboard restored");
}

@end
