//! Windows: embed the app icon and version info in animationcraft.exe.
//! (Other platforms take the icon from the window at runtime / the package.)
fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon/animationcraft.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon/animationcraft.ico");
        res.set("ProductName", "AnimationCraft");
        res.set("FileDescription", "AnimationCraft animation editor");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
