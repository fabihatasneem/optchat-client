// OptChat's QR scanner (optchat_ui::platform::scan_qr on iOS): a full-screen AVCaptureSession camera view with a
// close button. Only codes the Rust side accepts (optchat_qr_valid: oc1 engine tokens) end the scan; anything else
// shows a hint and scanning goes on. The callback runs on the main thread once, with the text or NULL (closed).
#import <UIKit/UIKit.h>
#import <AVFoundation/AVFoundation.h>

typedef void (*optchat_qr_cb)(const char *text, void *ctx);
extern bool optchat_qr_valid(const char *text);

@interface OCScanController : UIViewController <AVCaptureMetadataOutputObjectsDelegate>
@property(nonatomic, strong) AVCaptureSession *session;
@property(nonatomic, strong) AVCaptureVideoPreviewLayer *preview;
@property(nonatomic, strong) UILabel *hint;
@property(nonatomic, strong) dispatch_queue_t sessionQueue;
@property(nonatomic) optchat_qr_cb cb;
@property(nonatomic) void *ctx;
@property(nonatomic) BOOL finished;
@end

@implementation OCScanController

- (void)viewDidLoad {
    [super viewDidLoad];
    self.sessionQueue = dispatch_queue_create("optchat.qr", DISPATCH_QUEUE_SERIAL);
    self.view.backgroundColor = UIColor.blackColor;
    self.view.accessibilityIdentifier = @"qr-scanner";

    UILabel *title = [UILabel new];
    title.text = @"Scan the engine's QR code";
    title.textColor = UIColor.whiteColor;
    title.font = [UIFont systemFontOfSize:17 weight:UIFontWeightSemibold];
    title.translatesAutoresizingMaskIntoConstraints = NO;
    [self.view addSubview:title];

    UILabel *hint = [UILabel new];
    hint.text = @"The QR code shown with a new token on the engine's admin page";
    hint.textColor = [UIColor colorWithWhite:1 alpha:0.75];
    hint.font = [UIFont systemFontOfSize:14];
    hint.numberOfLines = 0;
    hint.textAlignment = NSTextAlignmentCenter;
    hint.accessibilityIdentifier = @"qr-hint";
    hint.translatesAutoresizingMaskIntoConstraints = NO;
    [self.view addSubview:hint];
    self.hint = hint;

    UIButton *close = [UIButton buttonWithType:UIButtonTypeSystem];
    UIImageSymbolConfiguration *cfg = [UIImageSymbolConfiguration configurationWithPointSize:17 weight:UIImageSymbolWeightSemibold];
    [close setImage:[UIImage systemImageNamed:@"xmark" withConfiguration:cfg] forState:UIControlStateNormal];
    close.tintColor = UIColor.whiteColor;
    close.backgroundColor = [UIColor colorWithWhite:0.15 alpha:0.85];
    close.layer.cornerRadius = 22;
    close.accessibilityIdentifier = @"qr-close";
    close.accessibilityLabel = @"Close";
    close.translatesAutoresizingMaskIntoConstraints = NO;
    [close addTarget:self action:@selector(closeTapped) forControlEvents:UIControlEventTouchUpInside];
    [self.view addSubview:close];

    UILayoutGuide *safe = self.view.safeAreaLayoutGuide;
    [NSLayoutConstraint activateConstraints:@[
        [close.topAnchor constraintEqualToAnchor:safe.topAnchor constant:12],
        [close.trailingAnchor constraintEqualToAnchor:safe.trailingAnchor constant:-16],
        [close.widthAnchor constraintEqualToConstant:44],
        [close.heightAnchor constraintEqualToConstant:44],
        [title.centerYAnchor constraintEqualToAnchor:close.centerYAnchor],
        [title.centerXAnchor constraintEqualToAnchor:self.view.centerXAnchor],
        [hint.bottomAnchor constraintEqualToAnchor:safe.bottomAnchor constant:-28],
        [hint.leadingAnchor constraintEqualToAnchor:safe.leadingAnchor constant:24],
        [hint.trailingAnchor constraintEqualToAnchor:safe.trailingAnchor constant:-24],
    ]];

    AVAuthorizationStatus st = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeVideo];
    if (st == AVAuthorizationStatusAuthorized) {
        [self startCamera];
    } else if (st == AVAuthorizationStatusNotDetermined) {
        __weak OCScanController *weakSelf = self;
        [AVCaptureDevice requestAccessForMediaType:AVMediaTypeVideo completionHandler:^(BOOL granted) {
            dispatch_async(dispatch_get_main_queue(), ^{
                if (granted) [weakSelf startCamera]; else [weakSelf showDenied];
            });
        }];
    } else {
        [self showDenied];
    }
}

- (void)showDenied {
    self.hint.text = @"OptChat can't use the camera. Allow it in Settings › OptChat › Camera, or paste the token instead.";
}

- (void)startCamera {
    if (self.finished) return;
    AVCaptureDevice *dev = [AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeVideo];
    NSError *err = nil;
    AVCaptureDeviceInput *input = dev ? [AVCaptureDeviceInput deviceInputWithDevice:dev error:&err] : nil;
    AVCaptureSession *session = [AVCaptureSession new];
    if (!input || ![session canAddInput:input]) { self.hint.text = @"The camera is not available. Paste the token instead."; return; }
    [session addInput:input];
    AVCaptureMetadataOutput *out = [AVCaptureMetadataOutput new];
    if (![session canAddOutput:out]) { self.hint.text = @"The camera can't read QR codes here. Paste the token instead."; return; }
    [session addOutput:out];
    [out setMetadataObjectsDelegate:self queue:dispatch_get_main_queue()];
    out.metadataObjectTypes = @[ AVMetadataObjectTypeQRCode ];
    AVCaptureVideoPreviewLayer *preview = [AVCaptureVideoPreviewLayer layerWithSession:session];
    preview.videoGravity = AVLayerVideoGravityResizeAspectFill;
    preview.frame = self.view.layer.bounds;
    [self.view.layer insertSublayer:preview atIndex:0];
    self.preview = preview;
    self.session = session;
    [self orientPreview];
    dispatch_async(self.sessionQueue, ^{ [session startRunning]; });
}

- (void)orientPreview {
    AVCaptureConnection *c = self.preview.connection;
    UIInterfaceOrientation o = self.view.window.windowScene.interfaceOrientation;
    // no @available here: it needs compiler-rt's __isPlatformVersionAtLeast, which the Rust link doesn't bring
    if ([c respondsToSelector:@selector(setVideoRotationAngle:)]) {
        CGFloat a = o == UIInterfaceOrientationLandscapeLeft ? 180 : o == UIInterfaceOrientationLandscapeRight ? 0 : o == UIInterfaceOrientationPortraitUpsideDown ? 270 : 90;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability-new"
        if ([c isVideoRotationAngleSupported:a]) c.videoRotationAngle = a;
#pragma clang diagnostic pop
    } else {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
        if (c.isVideoOrientationSupported && o != UIInterfaceOrientationUnknown) c.videoOrientation = (AVCaptureVideoOrientation)o;
#pragma clang diagnostic pop
    }
}

- (void)viewDidLayoutSubviews {
    [super viewDidLayoutSubviews];
    self.preview.frame = self.view.layer.bounds;
    [self orientPreview];
}

- (void)captureOutput:(AVCaptureOutput *)output didOutputMetadataObjects:(NSArray<__kindof AVMetadataObject *> *)objects fromConnection:(AVCaptureConnection *)connection {
    if (self.finished) return;
    for (AVMetadataObject *o in objects) {
        if (![o isKindOfClass:[AVMetadataMachineReadableCodeObject class]]) continue;
        NSString *s = ((AVMetadataMachineReadableCodeObject *)o).stringValue;
        if (s && optchat_qr_valid(s.UTF8String)) { [self finish:s]; return; }
        self.hint.text = @"That QR code is not an OptChat engine token.";
    }
}

- (void)closeTapped { [self finish:nil]; }

- (void)finish:(NSString *)text {
    if (self.finished) return;
    self.finished = YES;
    AVCaptureSession *session = self.session;
    if (session) dispatch_async(self.sessionQueue, ^{ [session stopRunning]; });
    optchat_qr_cb cb = self.cb;
    void *ctx = self.ctx;
    [self.presentingViewController dismissViewControllerAnimated:YES completion:^{
        cb(text ? text.UTF8String : NULL, ctx);
    }];
}

- (BOOL)prefersStatusBarHidden { return YES; }
- (UIInterfaceOrientationMask)supportedInterfaceOrientations {
    return UIDevice.currentDevice.userInterfaceIdiom == UIUserInterfaceIdiomPad ? UIInterfaceOrientationMaskLandscape : UIInterfaceOrientationMaskPortrait;
}
@end

bool optchat_can_scan_qr(void) {
    return [AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeVideo] != nil;
}

static UIViewController *top_controller(void) {
    for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
        if (![scene isKindOfClass:[UIWindowScene class]]) continue;
        for (UIWindow *w in ((UIWindowScene *)scene).windows) {
            if (!w.isKeyWindow && ((UIWindowScene *)scene).windows.count > 1) continue;
            UIViewController *vc = w.rootViewController;
            while (vc.presentedViewController) vc = vc.presentedViewController;
            if (vc) return vc;
        }
    }
    return nil;
}

/// presents the scanner; false when there is nothing to present it on (the callback is then never called)
bool optchat_scan_qr(optchat_qr_cb cb, void *ctx) {
    UIViewController *top = top_controller();
    if (!top) return false;
    OCScanController *vc = [OCScanController new];
    vc.cb = cb;
    vc.ctx = ctx;
    vc.modalPresentationStyle = UIModalPresentationFullScreen;
    [top presentViewController:vc animated:YES completion:nil];
    return true;
}
