//! Public-API tests of the bridge translations (`mbv_gui::bridge`).
//!
//! The inline `mod bridge` makes the child files resolve against
//! `tests/bridge/` (one binary per crate; no `mod.rs`).

mod bridge {
    mod keys;
}
