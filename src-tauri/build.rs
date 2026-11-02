use std::path::Path;

fn main() {
    // winperf: embed the sensor helper's SHA-256 manifest (sensors-helper/build.sh).
    // The elevated setup only copies/runs files that match it.
    let manifest = Path::new("../sensors-helper/dist/manifest.sha256");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = std::fs::read_to_string(manifest).unwrap_or_default();
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::write(Path::new(&out).join("sensors-manifest.sha256"), text)
        .expect("write sensors manifest");
    // asInvoker: Fob itself never asks for admin; only the explicit
    // "enable sensors" step relaunches it elevated (one UAC prompt).
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("fob.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("tauri build");
    require_windows_resources(Path::new(&out));
}

/// tauri-build writes `resource.rc` (manifest with Common-Controls 6, Fob icon,
/// VERSIONINFO) and hands it to embed-resource, which silently skips it when no
/// resource compiler is found (cross builds need `llvm-rc` on PATH or `RC` set).
/// The exe then links without any resources and Windows refuses to start it:
/// "TaskDialogIndirect not found" (comctl32 v5). Fail the build instead.
fn require_windows_resources(out: &Path) {
    let target = std::env::var("TARGET").unwrap_or_default();
    let host = std::env::var("HOST").unwrap_or_default();
    if !target.contains("windows") || host.contains("windows") {
        return;
    }
    let modified = |name: &str| {
        std::fs::metadata(out.join(name))
            .and_then(|m| m.modified())
            .ok()
    };
    match (modified("resource.rc"), modified("resource.lib")) {
        (Some(rc), Some(lib)) if lib >= rc => {}
        _ => panic!(
            "resource.rc was not compiled into resource.lib, so fob.exe would have no \
             manifest, icon or VERSIONINFO. Put llvm-rc on PATH (llvm-mingw bin) or set RC."
        ),
    }
}
