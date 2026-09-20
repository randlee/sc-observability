fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new()
                .commands(&["app_observability_level_change"]),
        ),
    )
    .expect("failed to build Tauri application metadata");
}
