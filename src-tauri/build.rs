fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_E2E");
    println!("cargo:rerun-if-changed=capabilities");
    println!("cargo:rerun-if-changed=capabilities-e2e");

    let capabilities = if std::env::var_os("CARGO_FEATURE_E2E").is_some() {
        "./capabilities-e2e/**/*"
    } else {
        "./capabilities/**/*"
    };

    if let Err(error) = tauri_build::try_build(
        tauri_build::Attributes::new().capabilities_path_pattern(capabilities),
    ) {
        eprintln!("failed to build Tauri application: {error:#}");
        std::process::exit(1);
    }
}
