//! The Blitz window half (D3 of `replace-tauri-with-blitz`): the window
//! channel (D4), the launch copy (D5), and the window's own modules as the
//! port lands them. One Rust module per Svelte component or TypeScript
//! module under `crates/mbv-gui/ui/src/` (D15).

pub mod channel;
pub mod clock;
pub mod launch;
pub mod net;
