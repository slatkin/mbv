//! The Library panel's window-content logic (`replace-tauri-with-blitz`, the
//! port of `lib/library/`): the row model, the Hero's facts, the interaction
//! state, the scroll stores, and the key routing. One Rust module per
//! TypeScript module (D15).

pub mod hero;
pub mod keys;
pub mod keys_shared;
pub mod list;
pub mod parts;
pub mod pill_list;
pub mod reset;
pub mod row_memo;
pub mod scroll;
pub mod state;
pub mod status;
