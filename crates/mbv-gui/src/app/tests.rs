//! The headless GUI tests' `App` isolation (U17 re-review).

/// U17 re-review, judgment 2: a test-built `App` isolates the state and
/// config directories. This test binary builds `mbv` with its `test` feature
/// and without `cfg(test)`, so `App::build` must install the test guard under
/// that feature. Without it, every headless GUI test reads and writes the
/// developer's real `prefs.json` and `config.toml`.
#[test]
fn stub_app_isolates_state_and_config_dirs_under_temp_dir() {
    let _app = mbv::stub_app_with_queue(0);
    let temp = std::env::temp_dir();
    assert!(mbv_config::state_dir().starts_with(&temp));
    assert!(mbv_config::config_path().starts_with(&temp));
}
