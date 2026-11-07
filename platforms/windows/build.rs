fn main() {
    println!("cargo:rerun-if-changed=bikey.def");
    println!("cargo:rerun-if-changed=bikey.rc");
    println!("cargo:rerun-if-changed=res");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc");
    if windows && msvc {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo");
        println!("cargo:rustc-cdylib-link-arg=/DEF:{dir}\\bikey.def");
        embed_resources();
    }
}

#[cfg(windows)]
fn embed_resources() {
    embed_resource::compile("bikey.rc", embed_resource::NONE)
        .manifest_required()
        .expect("compile icons with rc.exe");

    // Version details come from Cargo.toml so they never drift. Code signing
    // (SignPath) checks the product name and version on every signed binary.
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("set by cargo"));
    let version_rc = out.join("bikey-version.rc");
    std::fs::write(&version_rc, version_resource()).expect("write bikey-version.rc");
    embed_resource::compile(&version_rc, embed_resource::NONE)
        .manifest_required()
        .expect("compile version info with rc.exe");
}

#[cfg(windows)]
fn version_resource() -> String {
    let var = |name: &str| std::env::var(name).expect("set by cargo");
    let (major, minor, patch) = (
        var("CARGO_PKG_VERSION_MAJOR"),
        var("CARGO_PKG_VERSION_MINOR"),
        var("CARGO_PKG_VERSION_PATCH"),
    );
    let version = var("CARGO_PKG_VERSION");
    let authors = var("CARGO_PKG_AUTHORS").replace(':', ", ");
    // 0x40004 = VOS_NT_WINDOWS32, 0x2 = VFT_DLL; written as numbers so the
    // file needs no Windows SDK headers.
    format!(
        r#"1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3F
FILEFLAGS 0x0
FILEOS 0x40004
FILETYPE 0x2
FILESUBTYPE 0x0
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", "{authors}"
            VALUE "FileDescription", "bikey Vietnamese input method"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "bikey"
            VALUE "LegalCopyright", "MIT OR Apache-2.0"
            VALUE "OriginalFilename", "bikey.dll"
            VALUE "ProductName", "bikey"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#
    )
}

#[cfg(not(windows))]
fn embed_resources() {}
