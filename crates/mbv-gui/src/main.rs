//! The `mbv-gui` program: a native window Client of the Owner process. The
//! window content is a placeholder until the Client core and bridge land.

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> tauri::Result<()> {
    tauri::Builder::default().run(tauri::generate_context!())
}
