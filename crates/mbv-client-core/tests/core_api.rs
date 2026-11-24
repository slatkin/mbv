//! The core's public-API tests (`2-split-tui-gui-clients` row 3.5).
//!
//! Each test here names only the core's public surface: `mbv_client_core::`
//! items, the `test`-feature fixtures and seams a Client's own tests name, and
//! the crate's dev-dependencies. A test that names a `pub(in crate::app)` item
//! or an `App` field stays in `src/`, because that contract is private to the
//! core and moving it would need a visibility widening.
//!
//! The inline `mod core_api` makes the child files resolve against
//! `tests/core_api/` (one binary per crate; no `mod.rs`).

mod core_api {
    mod client_capabilities;
    mod feeds_manage;
    mod library_route;
    mod owner_restart;
    mod queue_view;
}
