fn main() {
    // Release on Windows: 用 tauri_build 的 WindowsAttributes 覆盖默认 manifest，
    // 把 UAC requireAdministrator 一起写进去，避免和 Tauri 默认 manifest 撞资源 ID
    // (CVT1100 duplicate resource)。
    // Dev 不嵌，避免 cargo tauri dev 因子进程提权脱离父进程导致 stdio/热重载异常。
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let profile = std::env::var("PROFILE").unwrap_or_default();
    if target_os == "windows" && profile == "release" {
        let manifest = include_str!("naxone.exe.manifest");
        let windows = tauri_build::WindowsAttributes::new().app_manifest(manifest);
        let attrs = tauri_build::Attributes::new().windows_attributes(windows);
        tauri_build::try_build(attrs).expect("tauri build (release) failed");
    } else {
        tauri_build::build();
    }
}
