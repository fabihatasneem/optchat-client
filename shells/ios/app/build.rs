// iOS: the QR scanner (native/qr_scanner.m, Objective-C on AVFoundation) compiled into the app binary.
fn main() {
    println!("cargo:rerun-if-changed=native/qr_scanner.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("ios") {
        return;
    }
    cc::Build::new().file("native/qr_scanner.m").flag("-fobjc-arc").flag("-fmodules").compile("optchat_native");
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    println!("cargo:rustc-link-lib=framework=UIKit");
}
