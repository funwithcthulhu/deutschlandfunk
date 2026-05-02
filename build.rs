fn main() {
    slint_build::compile("ui/app-window.slint").expect("failed to compile Slint UI");

    // Tell cargo to rebuild whenever the icon, slint UI, or this script changes,
    // so the embedded Windows icon stays in sync with the asset on disk.
    println!("cargo:rerun-if-changed=ui/app-window.slint");
    println!("cargo:rerun-if-changed=assets/deutschlandfunk.ico");
    println!("cargo:rerun-if-changed=assets/deutschlandfunk.png");
    println!("cargo:rerun-if-changed=build.rs");

    // Embed the app icon into the Windows executable
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/deutschlandfunk.ico");
        res.compile().expect("failed to compile Windows resources");
    }
}
