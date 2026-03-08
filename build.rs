fn main() {
    // On Windows, embed the application icon into the executable
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/logo.ico");
        res.set("ProductName", "NotepadX");
        res.set(
            "FileDescription",
            "A fast, beautiful, cross-platform text editor",
        );
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.compile().expect("Failed to compile Windows resources");
    }
}
