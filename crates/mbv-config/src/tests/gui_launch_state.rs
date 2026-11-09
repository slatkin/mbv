use crate::gui_launch_state::{load_gui_launch_state_at, save_gui_launch_state_at};
use crate::{
    GUI_LAUNCH_STATE_VERSION, GuiDestinationCollections, GuiLaunchState, LaunchPanelFocus,
    TabIdentity, TestTempDir,
};
use mbv_queue::ServiceKind;

// Focused filesystem tests for the GUI launch location
// (`gui_launch_state.rs`). Hermetic by construction: every test writes through
// the explicit-path `_at` variants into fresh `TestTempDir` scratch
// directories (uuid-qualified, removed on drop), so no test touches the real
// state dir or a sibling test's files.

/// The identity of the Music destination the location names.
fn music_destination() -> TabIdentity {
    TabIdentity::ServiceLibrary {
        kind: ServiceKind::Emby,
        library_id: "lib-music".to_string(),
    }
}

/// The GUI launch location a Music session with two selected groups leaves.
fn two_group_location() -> GuiLaunchState {
    GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: music_destination(),
        panel_focus: LaunchPanelFocus::Library,
        collections: vec![GuiDestinationCollections {
            destination: music_destination(),
            group_ids: vec!["grp-hiphop".to_string(), "grp-soul".to_string()],
            picks: Vec::new(),
        }],
    }
}

#[test]
fn gui_launch_state_round_trips_a_two_group_location() {
    let scratch = TestTempDir::new();
    let path = scratch.join("gui_launch_state.json");
    let state = two_group_location();

    save_gui_launch_state_at(&path, &state).unwrap();

    assert_eq!(load_gui_launch_state_at(&path), Some(state));
}

/// A file from a newer format never mis-resolves into stale identities: the
/// loader rejects it, so the start falls back to the Client's first choices.
#[test]
fn gui_launch_state_wrong_version_loads_none() {
    let scratch = TestTempDir::new();
    let path = scratch.join("gui_launch_state.json");
    std::fs::write(
        &path,
        r#"{"version":999,"destination":{"type":"home"},"panel_focus":"library"}"#,
    )
    .unwrap();

    assert_eq!(load_gui_launch_state_at(&path), None);
}

/// A file written before the group picks existed still loads: the selection
/// survives the upgrade, and the picks start empty (design D13).
#[test]
fn gui_launch_state_without_picks_loads_with_none() {
    let scratch = TestTempDir::new();
    let path = scratch.join("gui_launch_state.json");
    let mut value = serde_json::to_value(two_group_location()).unwrap();
    value["collections"][0]
        .as_object_mut()
        .unwrap()
        .remove("picks");
    std::fs::write(&path, value.to_string()).unwrap();

    let loaded = load_gui_launch_state_at(&path).expect("the old file loads");

    assert_eq!(loaded.collections[0].picks, Vec::new());
    assert_eq!(
        loaded.collections[0].group_ids,
        two_group_location().collections[0].group_ids
    );
}
