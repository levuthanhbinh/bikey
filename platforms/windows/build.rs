fn main() {
    println!("cargo:rerun-if-changed=bikey.def");
    println!("cargo:rerun-if-changed=bikey.rc");
    println!("cargo:rerun-if-changed=res");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc");
    if windows && msvc {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo");
        println!("cargo:rustc-cdylib-link-arg=/DEF:{dir}\\bikey.def");
        embed_icons();
    }
}

#[cfg(windows)]
fn embed_icons() {
    embed_resource::compile("bikey.rc", embed_resource::NONE)
        .manifest_required()
        .expect("compile icons with rc.exe");
}

#[cfg(not(windows))]
fn embed_icons() {}
