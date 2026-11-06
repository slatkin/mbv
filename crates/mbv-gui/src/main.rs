//! The `mbv-gui` program: a native window Client of the Owner process. The
//! core thread (design D3 of `add-tauri-gui-client`) attaches to the Owner
//! and runs behind the window; the window content is a placeholder until the
//! commands and the frame event land (row 2.5).

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> tauri::Result<()> {
    mbv_gui::core_thread::spawn();
    tauri::Builder::default().run(tauri::generate_context!())
}
