// Embeds the app icon in the Windows .exe. Without it Windows shows its generic icon on the
// window, taskbar and shortcuts (issue #8). Other targets need nothing here.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=assets/app/sitevet.rc");
        println!("cargo:rerun-if-changed=assets/app/icon.ico");
        embed_resource::compile("assets/app/sitevet.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("the Windows icon resource compiles");
    }
}
