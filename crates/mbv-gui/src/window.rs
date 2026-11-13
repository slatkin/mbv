//! The Blitz window half (D3 of `replace-tauri-with-blitz`): the window
//! channel (D4), the launch copy (D5), and the window's own modules as the
//! port lands them. One Rust module per Svelte component or TypeScript
//! module under `crates/mbv-gui/ui/src/` (D15).

pub mod cells;
pub mod channel;
pub mod clock;
pub mod icons;
pub mod input;
pub mod launch;
pub mod net;
pub mod playhead;
pub mod sections;
pub mod status;
pub mod theme;
pub mod time;
pub mod truncate;
