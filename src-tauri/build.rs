use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=NEOHTOP_ANALYTICS_ENDPOINT");
    // winperf: embed the sensor helper's SHA-256 manifest (sensors-helper/build.sh).
    // The elevated setup only copies/runs files that match it.
    let manifest = Path::new("../sensors-helper/dist/manifest.sha256");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = std::fs::read_to_string(manifest).unwrap_or_default();
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::write(Path::new(&out).join("sensors-manifest.sha256"), text)
        .expect("write sensors manifest");
    tauri_build::build()
}
