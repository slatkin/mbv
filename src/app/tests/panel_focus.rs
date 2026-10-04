use super::*;
use crate::app::tests::tick_integration::harness::TickHarness;
use mbv_ui_msg::{Msg, TerminalObserverEvent};
use tuirealm::event::Event;

/// The appearance-focus contract: below `MINI_VIEW_THRESHOLD` the single
/// displayed panel paints the default (unfocused) palette no matter which
/// panel holds focus, so `panel_appearance_focus` reports false for both
/// panels; at 80+ columns it follows the effective focus per panel.
#[test]
fn mini_view_reports_no_panel_appearance_focus_while_wide_follows_focus() {
    let mut mini = make_app_stub();
    mini.terminal_width = 60;
    mini.mini_view_focus = PanelFocus::Library;
    assert!(!mini.panel_appearance_focus(PanelFocus::Library));
    assert!(!mini.panel_appearance_focus(PanelFocus::Queue));

    mini.mini_view_focus = PanelFocus::Queue;
    assert!(!mini.panel_appearance_focus(PanelFocus::Library));
    assert!(!mini.panel_appearance_focus(PanelFocus::Queue));

    let mut wide = make_app_stub();
    wide.terminal_width = 100;
    wide.panel_focus = PanelFocus::Library;
    assert!(wide.panel_appearance_focus(PanelFocus::Library));
    assert!(!wide.panel_appearance_focus(PanelFocus::Queue));

    wide.panel_focus = PanelFocus::Queue;
    assert!(!wide.panel_appearance_focus(PanelFocus::Library));
    assert!(wide.panel_appearance_focus(PanelFocus::Queue));
}

/// The library column's named body-fill authority follows the appearance
/// bit: in mini view the column rests at the unfocused `LibraryColumn` fill
/// even while the library holds focus; wide keeps the focused fill.
#[test]
fn mini_view_library_body_fill_rests_while_wide_keeps_the_focused_fill() {
    let mut mini = make_app_stub();
    mini.terminal_width = 60;
    mini.mini_view_focus = PanelFocus::Library;
    let harness = TickHarness::new(mini);
    assert_eq!(
        harness.model().library_body_fill(),
        mbv_theme::surface_colors(mbv_theme::Surface::LibraryColumn, false).fill,
    );

    let mut wide = make_app_stub();
    wide.terminal_width = 100;
    wide.panel_focus = PanelFocus::Library;
    let wide_harness = TickHarness::new(wide);
    assert_eq!(
        wide_harness.model().library_body_fill(),
        mbv_theme::surface_colors(mbv_theme::Surface::LibraryColumn, true).fill,
    );
}

/// The painted proof: in mini view with Queue focused, the queue panel's
/// frame fill and the queue rows' zebra rest at the unfocused tones even
/// though the component holds interaction focus -- one palette, the default
/// (unfocused) one, because there is no sibling panel to contrast against.
/// The selected row is the exception: the focused panel keeps its cursor bar
/// on the resting stripe.
#[test]
fn mini_view_paints_the_focused_queue_panel_with_the_unfocused_palette() {
    let mut app = make_app_stub();
    app.terminal_width = 60;
    app.mini_view_focus = PanelFocus::Queue;
    app.local_view.adopt_items(make_items(3), 0);
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();

    let resting_zebra = mbv_theme::surface_colors(mbv_theme::Surface::QueueColumn, false).fill;
    let focused_zebra = mbv_theme::surface_colors(mbv_theme::Surface::QueueColumn, true).fill;
    assert_ne!(
        resting_zebra, focused_zebra,
        "the QueueColumn focus pair must differ for this proof to bind"
    );
    let panel = harness.model().app.queue_panel_placement().panel_area;
    assert!(panel.width > 2 && panel.height > 0, "queue panel placed");
    let buf = terminal.backend().buffer();
    // The placement's outer frame columns stay outside the inset list box,
    // so their fill is the frame's own QueueColumn tone.
    assert_eq!(
        buf[(panel.x, panel.y)].style().bg,
        Some(resting_zebra),
        "mini view frame fill rests despite Queue holding focus"
    );
    // The zebra stripes across the rows follow the same suppression: no row
    // paints the focused tone, and the resting stripe did paint. The
    // selected row is the exception: it keeps the canonical cursor bar.
    let box_area = mbv_render::arrangements::queue::queue_list_box(panel);
    assert_eq!(
        buf[(box_area.x, box_area.y)].style().bg,
        Some(mbv_theme::SELECTED_ROW_BG),
        "mini view keeps the cursor bar on the first row"
    );
    let mut striped = 0;
    for y in box_area.y..box_area.bottom() {
        for x in box_area.x..box_area.right() {
            let bg = buf[(x, y)].style().bg;
            if bg == Some(mbv_theme::SELECTED_ROW_BG) {
                continue;
            }
            assert_ne!(
                bg,
                Some(focused_zebra),
                "focused zebra tone at ({x}, {y}) in mini view"
            );
            striped += u32::from(bg == Some(resting_zebra));
        }
    }
    assert!(striped > 0, "the resting zebra painted");
}

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

    // Production order (task 1.2): the resize observer message is dispatched
    // and the sync pass runs before the next draw. The resize side effects --
    // including the mini-view focus hand-off -- belong to the sync pass now;
    // the draw path only reads geometry.
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

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

/// Task 1.2: the mini-view threshold crossing is a sync-pass side effect now.
/// After one `tick()` + sync pass and without drawing, the ephemeral mini-view
/// focus has moved to Queue while the stored wide focus is untouched.
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
    assert_eq!(
        app.local_view.emby_items(),
        [] as [mbv_emby_model::EmbyItem; 0]
    );
}

#[test]
fn panel_focus_changes_do_not_write_legacy_launch_prefs() {
    let _guard = crate::config::TestStateDirGuard::new();
    std::fs::write(
        crate::config::prefs_path(),
        serde_json::json!({ "panel_focus": "queue_side", "library_tab": 3 }).to_string(),
    )
    .expect("write legacy prefs");
    let mut app = make_app_stub();

    app.set_panel_focus(PanelFocus::Library);

    let prefs: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::config::prefs_path()).expect("prefs retained"),
    )
    .expect("prefs json");
    assert_eq!(prefs["panel_focus"].as_str(), Some("queue_side"));
    assert_eq!(prefs["library_tab"].as_u64(), Some(3));
}

#[test]
fn entering_queue_focus_selects_now_playing_item() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.local_view.adopt_items(make_items(3), 0);
    app.local_view.set_cursor(2);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 1;
    };

    app.set_panel_focus(PanelFocus::Queue);

    assert_eq!(app.local_view.cursor(), 1);
}

#[test]
fn entering_queue_focus_defaults_invalid_queue_cursor_to_first_item() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.local_view.adopt_items(make_items(3), 0);
    app.local_view.set_cursor(99);

    app.set_panel_focus(PanelFocus::Queue);

    assert_eq!(app.local_view.cursor(), 0);
}
