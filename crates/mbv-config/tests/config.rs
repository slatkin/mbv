//! Public-API contract tests for `mbv-config` (`parse_config`, `Config`,
//! path resolution, saved-state helpers). Files here compile against the
//! crate's public API only; tests that need `cfg(test)`-only support
//! (`SYS_ENV_LOCK`, `TEST_DEFAULT_STATE_DIR`, the `test_support` guards)
//! stay in `src/tests/` (design Decision 5 of the
//! `crate-integration-test-targets` change).
//!
//! The inline `mod config` makes the child files resolve against
//! `tests/config/` (one binary per crate; no `mod.rs`).

mod config {
    mod emby_admin;
    mod keybinds;
    mod launch_state;
    mod library;
    mod paths_env;
    mod script_source;
    mod settings;
}
