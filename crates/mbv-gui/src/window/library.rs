//! The Library panel's window-content logic (`replace-tauri-with-blitz`, the
//! port of `lib/library/`): the row model, the Hero's facts, the interaction
//! state, the scroll stores, and the key routing. One Rust module per
//! TypeScript module (D15).

pub mod audiobooks;
pub mod audiobooks_keys;
pub mod audiobooks_list;
pub mod audiobooks_workspace;
pub mod emby;
pub mod emby_keys;
pub mod emby_list;
pub mod hero;
pub mod home;
pub mod home_list;
pub mod keys;
pub mod keys_shared;
pub mod list;
pub mod parts;
pub mod pill_list;
pub mod reset;
pub mod row_memo;
pub mod scroll;
pub mod shows;
pub mod shows_keys;
pub mod shows_list;
pub mod shows_workspace;
pub mod state;
pub mod status;
