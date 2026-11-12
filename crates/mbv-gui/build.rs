//! Build script: runs `tauri_build`, which generates the Tauri context inputs
//! and the capability JSON schemas under `gen/`. The app manifest declares
//! the bridge commands so their `allow-$command` permissions exist for the
//! capability file to grant (design D11, add-gui-library-panel design D2,
//! add-gui-settings-screen design D11). A command missing here is one the
//! webview's `invoke` cannot call: the ACL denies it and the front end's
//! `void`-ed promise swallows the denial, so the feature dies silently — the
//! live finding the `library` command's absence caused, and the same one the
//! `settings` command's absence caused for the Settings screen (regression
//! test `crates/mbv-gui/tests/capabilities.rs`).

fn main() {
    // The failure path mirrors `tauri_build::build`'s own: print the error
    // and end the build script with a nonzero exit, no panic.
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "key",
            "pointer",
            "report_layout",
            "library",
            "settings",
        ]));
    if let Err(error) = tauri_build::try_build(attributes) {
        println!("failed to run tauri-build: {error:#}");
        std::process::exit(1);
    }
}
