// Selective saved-presentation clearing (#745): the persistence boundary
// must forget launch location, per-library positions and presentation keys
// without touching volume/mute, unknown preference entries or unrelated
// state files. Every test uses the thread-local `TestStateDirGuard` so all
// resolved paths land in a uuid-qualified scratch directory; no test reads
// or writes a real state directory.

use crate::{
    TestStateDirGuard, clear_saved_ui_presentation_state, load_library_position_state,
    load_tui_launch_state, state_dir,
};

/// The presentation keys the reset contract must remove; kept as a literal
/// here so the test fails if the production list drifts (design decision 4).
const REMOVED_PREF_KEYS: &[&str] = &[
    "panel_focus",
    "queue_column_width",
    "list_pane_width",
    "visual_slot_hidden",
    "power_focus",
    "power_left_width",
    "power_left_tab",
    "library_tab",
];

fn state_file(name: &str) -> std::path::PathBuf {
    state_dir().join(name)
}

fn write_presentation_prefs() {
    let prefs = serde_json::json!({
        "ui_volume": 55,
        "mute_on": true,
        "pre_mute_volume": 40,
        "unrelated_future_key": "keep",
        "panel_focus": "queue_side",
        "queue_column_width": 40,
        "list_pane_width": 64,
        "visual_slot_hidden": true,
        "power_focus": "queue_side",
        "power_left_width": 40,
        "power_left_tab": 2,
        "library_tab": 3,
    });
    std::fs::write(state_file("prefs.json"), prefs.to_string()).unwrap();
}

fn seeded_state_dir() -> TestStateDirGuard {
    let guard = TestStateDirGuard::new();
    std::fs::write(
        state_file("tui_launch_state.json"),
        r#"{"version":1,"tab":{"type":"home"}}"#,
    )
    .unwrap();
    std::fs::write(
        state_file("library_position_state.json"),
        r#"{"libraries":{"movies":{"levels":[]}}}"#,
    )
    .unwrap();
    write_presentation_prefs();
    std::fs::write(state_file("queue_state.json"), "{}").unwrap();
    guard
}

fn read_prefs() -> serde_json::Value {
    let text = std::fs::read_to_string(state_file("prefs.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn assert_presentation_keys_absent(prefs: &serde_json::Value) {
    for key in REMOVED_PREF_KEYS {
        assert!(prefs.get(*key).is_none(), "{key} must be cleared");
    }
}

#[test]
fn ui_state_reset_clears_saved_presentation_and_preserves_unrelated_values() {
    let _guard = seeded_state_dir();

    clear_saved_ui_presentation_state().unwrap();

    assert_eq!(load_tui_launch_state(), None, "launch snapshot removed");
    assert!(
        load_library_position_state().libraries.is_empty(),
        "per-library positions cleared"
    );

    let prefs = read_prefs();
    assert_presentation_keys_absent(&prefs);
    assert_eq!(prefs["ui_volume"], serde_json::json!(55));
    assert_eq!(prefs["mute_on"], serde_json::json!(true));
    assert_eq!(prefs["pre_mute_volume"], serde_json::json!(40));
    assert_eq!(prefs["unrelated_future_key"], serde_json::json!("keep"));
    assert!(
        state_file("queue_state.json").exists(),
        "other state retained"
    );
}

#[test]
fn ui_state_reset_reports_partial_prefs_failure_after_clearing_other_state() {
    let _guard = TestStateDirGuard::new();
    std::fs::write(
        state_file("tui_launch_state.json"),
        r#"{"version":1,"tab":{"type":"home"}}"#,
    )
    .unwrap();
    std::fs::write(
        state_file("library_position_state.json"),
        r#"{"libraries":{"movies":{"levels":[]}}}"#,
    )
    .unwrap();
    // A directory at the prefs path makes the selective patch fail while the
    // other two independent clears still succeed.
    std::fs::create_dir(state_file("prefs.json")).unwrap();

    let error = clear_saved_ui_presentation_state().unwrap_err();

    assert!(error.is_state(), "expected a state error, got: {error:?}");
    assert_eq!(load_tui_launch_state(), None, "launch snapshot removed");
    assert!(
        load_library_position_state().libraries.is_empty(),
        "per-library positions cleared despite the prefs failure"
    );
    assert!(state_file("prefs.json").is_dir(), "failed path untouched");
}
