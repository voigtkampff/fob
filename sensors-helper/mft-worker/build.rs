fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("worker.rc", embed_resource::NONE);
    }
    println!("cargo:rerun-if-changed=worker.rc");
    println!("cargo:rerun-if-changed=../../src-tauri/fob.manifest");
}
