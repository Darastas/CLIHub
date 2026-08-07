fn main() {
    if cfg!(target_os = "windows") {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().unwrap();
    }
}
