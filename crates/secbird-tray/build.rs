fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=../../assets/secretarybird.ico");
    println!("cargo:rerun-if-changed=../../assets/tray.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("app.rc", embed_resource::NONE);
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
            panic!("The native DirectWrite bridge requires the Windows MSVC SDK");
        }
        println!("cargo:rerun-if-changed=src/native_editor/source_text_bridge.cpp");
        cc::Build::new()
            .cpp(true)
            .std("c++17")
            .flag("/EHs-c-")
            .flag("/GR-")
            .define("_HAS_EXCEPTIONS", "0")
            .define("_WIN32_WINNT", "0x0A00")
            .define("NTDDI_VERSION", "0x0A000003")
            .file("src/native_editor/source_text_bridge.cpp")
            .compile("source_text_bridge");
        for library in ["dwrite", "gdi32", "bcrypt"] {
            println!("cargo:rustc-link-lib={library}");
        }
    }
}
