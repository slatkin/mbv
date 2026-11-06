//! Build script: runs `tauri_build`, which generates the Tauri context inputs
//! and the capability JSON schemas under `gen/`. The app manifest declares the
//! three bridge commands so their `allow-$command` permissions exist for the
//! capability file to grant (design D11).

fn main() {
    // The failure path mirrors `tauri_build::build`'s own: print the error
    // and end the build script with a nonzero exit, no panic.
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "key",
            "pointer",
            "report_layout",
        ]));
    if let Err(error) = tauri_build::try_build(attributes) {
        println!("failed to run tauri-build: {error:#}");
        std::process::exit(1);
    }
}
