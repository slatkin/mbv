use super::*;
use crate::app::components::{Msg, TerminalObserverEvent};
use crate::app::tests::*;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::types_playback::HomeLatestSource;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::Event;

#[test]
fn resize_tick_selects_mini_queue_without_changing_wide_focus() {
    let mut app = make_app_stub();
    app.terminal_width = 100;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);

    let mut wide_terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    wide_terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();

    harness.inject(Event::WindowResize(60, 24));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::TerminalEvent(TerminalObserverEvent::Resize {
            width: 60,
            height: 24
        })
    )));

    let mut narrow_terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    narrow_terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    assert_eq!(
        harness.model().app.effective_panel_focus(),
        PanelFocus::Queue,
        "crossing into mini view selects Queue"
    );
    assert_eq!(
        harness.model().app.panel_focus,
        PanelFocus::Library,
        "mini-view focus must not replace the stored wide focus"
    );

    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    let mut widened_terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    widened_terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    assert_eq!(
        harness.model().app.effective_panel_focus(),
        PanelFocus::Library,
        "widening restores the prior wide focus"
    );
    assert_eq!(harness.model().app.panel_focus, PanelFocus::Library);
}

#[test]
fn build_restores_panel_focus_from_prefs_for_both_values() {
    let _guard = crate::config::TestStateDirGuard::new();
    for (pref, expected) in [
        ("queue_side", PanelFocus::Queue),
        ("library_side", PanelFocus::Library),
    ] {
        std::fs::write(
            crate::config::prefs_path(),
            serde_json::json!({ "panel_focus": pref }).to_string(),
        )
        .expect("write prefs");

        let app = make_built_app();

        assert_eq!(app.panel_focus, expected);
    }
}

#[test]
fn build_always_starts_on_home_without_affecting_saved_queue_state() {
    let _guard = crate::config::TestStateDirGuard::new();
    std::fs::write(
        crate::config::prefs_path(),
        serde_json::json!({ "library_tab": 3 }).to_string(),
    )
    .expect("write prefs");

    let app = make_built_app();

    assert!(app.tab.is_home());
    assert_eq!(app.library_tab_pending, 0);
    assert!(app.player_tab.emby_items().is_empty());
}

#[test]
fn build_restores_home_section_pending_from_prefs() {
    let _guard = crate::config::TestStateDirGuard::new();
    std::fs::write(
        crate::config::prefs_path(),
        serde_json::json!({ "home_section": "abs:lib-1" }).to_string(),
    )
    .expect("write prefs");

    let model = Model::new(make_built_app());

    assert_eq!(
        model.home_section_pending,
        Some(HomeLatestSource::Audiobookshelf("lib-1".into())),
        "the saved pill identity is loaded, to be applied once the section exists"
    );
}

#[test]
fn save_prefs_persists_panel_focus_for_both_values() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();

    app.set_panel_focus(PanelFocus::Queue);
    let queue_prefs: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
    )
    .expect("prefs json");
    assert_eq!(queue_prefs["panel_focus"].as_str(), Some("queue_side"));

    app.set_panel_focus(PanelFocus::Library);
    let library_prefs: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs written"),
    )
    .expect("prefs json");
    assert_eq!(library_prefs["panel_focus"].as_str(), Some("library_side"));
}

#[test]
fn entering_queue_focus_selects_now_playing_item() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.player_tab.set_items(make_items(3), 0);
    app.player_tab.queue_cursor = 2;
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 1;
    }

    app.set_panel_focus(PanelFocus::Queue);

    assert_eq!(app.player_tab.queue_cursor, 1);
}

#[test]
fn entering_queue_focus_preserves_valid_queue_cursor_without_now_playing() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.player_tab.set_items(make_items(3), 0);
    app.player_tab.queue_cursor = 2;

    app.set_panel_focus(PanelFocus::Queue);

    assert_eq!(app.player_tab.queue_cursor, 2);
}

#[test]
fn entering_queue_focus_defaults_invalid_queue_cursor_to_first_item() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.player_tab.set_items(make_items(3), 0);
    app.player_tab.queue_cursor = 99;

    app.set_panel_focus(PanelFocus::Queue);

    assert_eq!(app.player_tab.queue_cursor, 0);
}

#[test]
fn building_from_panel_focus_prefs_does_not_mutate_saved_library_positions() {
    let _guard = crate::config::TestStateDirGuard::new();
    let state = crate::config::LibraryPositionState {
        libraries: std::iter::once((
            "lib-movies".into(),
            crate::config::LibraryPosition {
                levels: vec![crate::config::LibraryPositionLevel {
                    parent_id: "lib-movies".into(),
                    title: "Movies".into(),
                    focused_item_id: Some("id1".into()),
                    cursor_index: 1,
                    item_types: Some("Movie".into()),
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    letter_filter_index: None,
                    library_total: None,
                }],
                ..Default::default()
            },
        ))
        .collect(),
    };
    crate::config::save_library_position_state(&state);
    std::fs::write(
        crate::config::prefs_path(),
        serde_json::json!({ "panel_focus": "queue_side" }).to_string(),
    )
    .expect("write prefs");

    let _app = make_built_app();

    assert_eq!(crate::config::load_library_position_state(), state);
}
