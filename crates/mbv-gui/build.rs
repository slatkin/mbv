//! Build script: runs `tauri_build`, which generates the Tauri context inputs
//! and the capability JSON schemas under `gen/`.

fn main() {
    tauri_build::build();
}
