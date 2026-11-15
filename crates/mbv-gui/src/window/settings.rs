//! The Settings screen's window-content logic (the port of `lib/settings/`,
//! `replace-tauri-with-blitz` rows 8.1): the interaction state, the content
//! entries, the pick menu, the form focus, and the key routing. One Rust
//! module per TypeScript module (D15). The module-level stores of the
//! TypeScript are one [`state::SettingsState`] the root provides as context
//! (D15).

pub mod entries;
pub mod focus;
pub mod keys;
pub mod pick_menu;
pub mod state;

#[cfg(test)]
mod tests;
