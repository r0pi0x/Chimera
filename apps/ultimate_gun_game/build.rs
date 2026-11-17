//! Embeds the Windows executable's icon (Explorer, the pinned taskbar):
//! `apps/party_kit/assets/icon/app.ico`. A no-op unless the target is
//! Windows and the host has the resource step (a Windows host; a Linux
//! cross build gets no icon resource, the window icon still shows).

fn main() {
    println!("cargo:rerun-if-changed=../party_kit/assets/icon/app.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../party_kit/assets/icon/app.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icon resource not embedded: {e}");
        }
    }
}
