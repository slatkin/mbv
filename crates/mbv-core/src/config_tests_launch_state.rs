// Focused filesystem tests for the TUI launch-state snapshot
// (`config_launch_state.rs`). Hermetic by construction: every test writes
// through the explicit-path `_at` variants into fresh `TestTempDir`
// scratch directories (uuid-qualified, removed on drop), so no test
// touches env overrides, the real state dir, or a sibling test's files.

fn launch_state_sample() -> TuiLaunchState {
    TuiLaunchState {
        version: TUI_LAUNCH_STATE_VERSION,
        tab: TabIdentity::ServiceLibrary {
            kind: ServiceKind::Emby,
            library_id: "lib-movies".to_string(),
        },
        panel_focus: LaunchPanelFocus::Library,
        selector: Some(SelectorIdentity::Emby {
            key: "A-C".to_string(),
        }),
        item: Some(LibraryItemIdentity::Emby {
            id: "movie-2".to_string(),
        }),
    }
}

fn launch_state_queue_focus_sample() -> TuiLaunchState {
    TuiLaunchState {
        version: TUI_LAUNCH_STATE_VERSION,
        tab: TabIdentity::Home,
        panel_focus: LaunchPanelFocus::Queue,
        selector: Some(SelectorIdentity::Home {
            key: "continue".to_string(),
        }),
        item: None,
    }
}

#[test]
fn tui_launch_state_round_trips_through_two_distinct_paths() {
    let first = TestTempDir::new();
    let second = TestTempDir::new();
    let first_path = first.join("tui_launch_state.json");
    let second_path = second.join("tui_launch_state.json");

    let library_state = launch_state_sample();
    let home_state = launch_state_queue_focus_sample();

    save_tui_launch_state_at(&first_path, &library_state).unwrap();
    save_tui_launch_state_at(&second_path, &home_state).unwrap();

    assert_eq!(load_tui_launch_state_at(&first_path), Some(library_state));
    assert_eq!(load_tui_launch_state_at(&second_path), Some(home_state));
}

#[test]
fn tui_launch_state_missing_file_loads_none() {
    let scratch = TestTempDir::new();
    assert_eq!(
        load_tui_launch_state_at(&scratch.join("tui_launch_state.json")),
        None
    );
}

#[test]
fn tui_launch_state_malformed_file_loads_none() {
    let scratch = TestTempDir::new();
    let path = scratch.join("tui_launch_state.json");
    std::fs::write(&path, "{not json").unwrap();
    assert_eq!(load_tui_launch_state_at(&path), None);
}

#[test]
fn tui_launch_state_wrong_version_loads_none() {
    let scratch = TestTempDir::new();
    let path = scratch.join("tui_launch_state.json");
    std::fs::write(
        &path,
        r#"{"version":999,"tab":{"type":"home"},"panel_focus":"library"}"#,
    )
    .unwrap();
    assert_eq!(load_tui_launch_state_at(&path), None);
}

#[test]
fn tui_launch_state_unknown_tab_shape_loads_none() {
    let scratch = TestTempDir::new();
    let path = scratch.join("tui_launch_state.json");
    std::fs::write(
        &path,
        r#"{"version":1,"tab":{"type":"no_such_tab"},"panel_focus":"library"}"#,
    )
    .unwrap();
    assert_eq!(load_tui_launch_state_at(&path), None);
}

#[test]
fn tui_launch_state_partial_document_uses_defaults() {
    let scratch = TestTempDir::new();
    let path = scratch.join("tui_launch_state.json");
    std::fs::write(&path, r#"{"tab":{"type":"feeds"}}"#).unwrap();
    assert_eq!(
        load_tui_launch_state_at(&path),
        Some(TuiLaunchState {
            version: TUI_LAUNCH_STATE_VERSION,
            tab: TabIdentity::Feeds,
            panel_focus: LaunchPanelFocus::Library,
            selector: None,
            item: None,
        })
    );
}

#[test]
fn tui_launch_state_failed_replace_leaves_no_tmp_sibling() {
    let scratch = TestTempDir::new();
    let path = scratch.join("tui_launch_state.json");
    // A non-empty directory at the target path makes the atomic rename
    // deterministically fail, so the failed replace must not orphan its
    // process-unique temp file.
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("sentinel"), "x").unwrap();
    let err = save_tui_launch_state_at(&path, &launch_state_sample()).unwrap_err();
    assert!(
        matches!(err, TuiLaunchStateError::Replace(_)),
        "expected Replace error, got: {err:?}"
    );
    let debris: Vec<_> = std::fs::read_dir(scratch.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains("tui_launch_state.json.tmp-")
        })
        .collect();
    assert!(debris.is_empty(), "unexpected temp debris: {debris:?}");
}

#[test]
fn tui_launch_state_tmp_paths_are_distinct_within_the_state_dir() {
    let scratch = TestTempDir::new();
    let path = scratch.join("tui_launch_state.json");
    let first = tui_launch_state_tmp_path(&path);
    let second = tui_launch_state_tmp_path(&path);
    assert_ne!(first, second);
    assert_eq!(first.parent(), second.parent());
    assert_eq!(first.parent(), path.parent());
}

#[test]
fn tui_launch_state_interrupted_write_keeps_previous_snapshot() {
    let scratch = TestTempDir::new();
    let path = scratch.join("tui_launch_state.json");

    let first = launch_state_sample();
    save_tui_launch_state_at(&path, &first).unwrap();

    // A crashed writer leaves only its process-unique temp file behind; it
    // never renamed over the snapshot, so the previous launch location
    // must still load intact.
    let orphan = tui_launch_state_tmp_path(&path);
    std::fs::write(&orphan, "{partial").unwrap();
    assert_eq!(load_tui_launch_state_at(&path), Some(first));

    // The next orderly exit replaces the snapshot and leaves no temp
    // debris behind.
    let second = launch_state_queue_focus_sample();
    save_tui_launch_state_at(&path, &second).unwrap();
    let _ = std::fs::remove_file(&orphan);
    assert_eq!(load_tui_launch_state_at(&path), Some(second));
    let debris: Vec<_> = std::fs::read_dir(scratch.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains("tui_launch_state.json.tmp-")
        })
        .collect();
    assert!(debris.is_empty(), "unexpected temp debris: {debris:?}");
}
