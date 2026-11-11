fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("app_icon.ico");
        res.set("FileDescription", "The Scribe's Ledger - Novel Studio");
        res.set("ProductName", "The Scribe's Ledger");
        res.set("LegalCopyright", "Copyright (c) 2026");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to compile Windows resource: {}", e);
        }
    }
}
