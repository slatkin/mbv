use crate::app::tests::make_app_stub;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::player::PlayerCommand;

fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

#[test]
fn help_f1_closes_help_via_handle_key() {
    let mut app = make_app_stub();
    app.show_help = true;
    let quit = app.handle_key(ev(KeyCode::F(1), KeyModifiers::NONE));
    assert!(!quit);
    assert!(!app.show_help, "F1 closes the help overlay");
}

#[test]
fn daemon_lost_q_runs_normal_quit_cleanup() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests::make_local_daemon_app_stub(Vec::new());
    app.daemon_lost_modal = Some(crate::app::DaemonLostModal {
        last_playing_title: None,
        daemon_log_path: "daemon.log".into(),
        restart_error: None,
    });
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-id".into()),
        name: "Saved".into(),
    };
    app.queue_dirty = true;
    app.client.lock().unwrap().config.save_playlist_on_quit = false;
    crate::app::QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);

    assert!(app.handle_key(ev(KeyCode::Char('q'), KeyModifiers::NONE)));
    assert!(app.daemon_lost_modal.is_none());
    assert!(
        !app.queue_dirty,
        "normal quit cleanup must discard dirty state"
    );

    crate::app::QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
}

#[test]
fn help_swallows_unbound_key_via_handle_key() {
    let mut app = make_app_stub();
    app.show_help = true;
    let quit = app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(!quit);
    assert!(
        app.show_help,
        "an unbound key is swallowed; help stays open"
    );
}

#[test]
fn space_toggles_pause_on_first_press_when_active_via_handle_key() {
    let mut app = make_app_stub();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
    }
    let rx = app.player.spy_on_commands();
    // Double-tap required: first press arms, second within 300ms dispatches.
    app.handle_key(ev(KeyCode::Char(' '), KeyModifiers::NONE));
    assert!(!matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)));
    app.handle_key(ev(KeyCode::Char(' '), KeyModifiers::NONE));
    assert!(matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)));
}

#[test]
fn space_does_not_toggle_pause_when_idle_via_handle_key() {
    let mut app = make_app_stub();
    let rx = app.player.spy_on_commands();
    // Idle home tab: Space must not emit a transport command (it falls
    // through to the view handler, which ignores it).
    app.handle_key(ev(KeyCode::Char(' '), KeyModifiers::NONE));
    assert!(
        !matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)),
        "Space is inert while nothing plays"
    );
}

#[test]
fn repeated_space_dispatches_each_available_toggle() {
    let mut app = make_app_stub();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
    }
    let rx = app.player.spy_on_commands();
    // Double-tap required: each dispatch needs a pair of presses.
    app.handle_key(ev(KeyCode::Char(' '), KeyModifiers::NONE));
    app.handle_key(ev(KeyCode::Char(' '), KeyModifiers::NONE));
    assert!(matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)));
    app.handle_key(ev(KeyCode::Char(' '), KeyModifiers::NONE));
    app.handle_key(ev(KeyCode::Char(' '), KeyModifiers::NONE));
    assert!(matches!(rx.try_recv(), Ok(PlayerCommand::TogglePause)));
}

#[test]
fn f1_opens_help_via_handle_key() {
    let mut app = make_app_stub();
    app.handle_key(ev(KeyCode::F(1), KeyModifiers::NONE));
    assert!(app.show_help);
}

#[test]
fn f2_opens_settings_via_handle_key() {
    let mut app = make_app_stub();
    assert!(!app.show_settings);
    app.handle_key(ev(KeyCode::F(2), KeyModifiers::NONE));
    assert!(app.show_settings);
    // PRESERVED QUIRK: a second F2 press does not close settings. Once
    // `show_settings` is true, `handle_key_settings` (ordered ahead of
    // `global_overlay_open`/`queue_column_width` in CONTEXT_STACK, matching the
    // pre-phase-2 branch order) claims F2 first and its match has no
    // `F(2)` arm, so it falls to `_ => {}` and swallows the key. This
    // predates phase 2 (verified against commit 2147343) — not a
    // regression introduced by this extraction.
    app.handle_key(ev(KeyCode::F(2), KeyModifiers::NONE));
    assert!(
        app.show_settings,
        "F2 does not toggle settings closed once open; only Esc/F1/F3/F4/q do"
    );
}

#[test]
fn f3_opens_sessions_via_handle_key() {
    let mut app = make_app_stub();
    assert!(!app.show_sessions);
    app.handle_key(ev(KeyCode::F(3), KeyModifiers::NONE));
    assert!(app.show_sessions);
}

#[test]
fn f4_opens_playlists_via_handle_key() {
    let mut app = make_app_stub();
    assert!(!app.show_playlists);
    app.handle_key(ev(KeyCode::F(4), KeyModifiers::NONE));
    assert!(app.show_playlists);
}

#[test]
fn confirm_clear_queue_yes_dispatches_clear_via_handle_key() {
    let mut app = make_app_stub();
    app.confirm_modal = Some(crate::app::ConfirmModal {
        title: " Clear Queue ".into(),
        message: "Clear the queue?".into(),
        hint: "[y] Confirm    [Esc] Cancel".into(),
        on_confirm: crate::app::ConfirmAction::ClearQueue,
    });
    app.handle_key(ev(KeyCode::Char('y'), KeyModifiers::NONE));
    assert!(
        app.confirm_modal.is_none(),
        "confirm modal clears regardless of answer"
    );
}

#[test]
fn confirm_rescan_no_clears_flag_without_rescan_via_handle_key() {
    let mut app = make_app_stub();
    app.confirm_modal = Some(crate::app::ConfirmModal {
        title: " Rescan Library ".into(),
        message: "Rescan 'Movies'?".into(),
        hint: "[y] Confirm    [Esc] Cancel".into(),
        on_confirm: crate::app::ConfirmAction::RescanLibrary(0),
    });
    app.handle_key(ev(KeyCode::Char('n'), KeyModifiers::NONE));
    assert!(app.confirm_modal.is_none());
}

#[test]
fn skip_intro_confirm_no_dismisses_via_handle_key() {
    let mut app = make_app_stub();
    app.skip_intro_end_ticks = Some(1000);
    app.handle_key(ev(KeyCode::Char('n'), KeyModifiers::NONE));
    assert!(app.skip_intro_end_ticks.is_none());
}

#[test]
fn next_up_confirm_no_dismisses_via_handle_key() {
    let mut app = make_app_stub();
    app.next_up_item = Some(crate::app::tests::make_item("item", "Movie"));
    app.handle_key(ev(KeyCode::Char('n'), KeyModifiers::NONE));
    assert!(app.next_up_item.is_none());
}

fn test_empty_context_menu() -> crate::app::ContextMenu {
    crate::app::ContextMenu {
        x: 0,
        y: 0,
        entries: Vec::new(),
        cursor: 0,
    }
}

#[test]
fn x_toggles_sidebar_via_handle_key() {
    let mut app = make_app_stub();
    let before = app.queue_column_collapsed;
    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_ne!(app.queue_column_collapsed, before);
}

#[test]
fn x_does_not_toggle_power_sidebar_while_context_menu_is_open_via_handle_key() {
    let mut app = make_app_stub();
    app.context_menu = Some(test_empty_context_menu());
    let before = app.queue_column_collapsed;
    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(
        app.queue_column_collapsed, before,
        "Sidebar must not toggle while a context menu is open"
    );
}

#[test]
fn x_moves_queue_focus_to_library_when_collapsing_power_sidebar() {
    let mut app = make_app_stub();
    app.panel_focus = crate::app::PanelFocus::Queue;

    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));

    assert!(app.queue_column_collapsed);
    assert_eq!(app.panel_focus, crate::app::PanelFocus::Library);

    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));

    assert!(!app.queue_column_collapsed);
    assert_eq!(app.panel_focus, crate::app::PanelFocus::Library);
}

/// Regression guard in the other direction: the sidebar toggle key used to
/// be 'h'; the toggle moved to 'x' so 'h' could be repurposed for vim-style
/// horizontal navigation in 2-col library lists. Make sure 'h' no longer
/// collapses the sidebar on its own.
#[test]
fn h_no_longer_toggles_sidebar_via_handle_key() {
    let mut app = make_app_stub();
    let before = app.queue_column_collapsed;
    app.handle_key(ev(KeyCode::Char('h'), KeyModifiers::NONE));
    assert_eq!(
        app.queue_column_collapsed, before,
        "Sidebar toggle moved from 'h' to 'x'; 'h' must not toggle the sidebar"
    );
}

#[test]
fn c_prompts_clear_queue_confirmation_via_handle_key() {
    let mut app = make_app_stub();
    app.player_tab
        .items
        .push(crate::app::tests::make_item("1", "Track"));
    app.handle_key(ev(KeyCode::Char('c'), KeyModifiers::NONE));
    assert!(matches!(
        app.confirm_modal.as_ref().map(|m| &m.on_confirm),
        Some(crate::app::ConfirmAction::ClearQueue)
    ));
}

#[test]
fn c_does_not_prompt_clear_queue_while_context_menu_is_open_via_handle_key() {
    // Behavior change (phase 6, #135): before this fix,
    // `clear_queue_prompt_c` had no `context_menu` guard and sat above
    // `context_menu` in CONTEXT_STACK, so 'c' bled through an open
    // context menu and silently opened the clear-queue confirmation. It
    // must now fall through to (and be swallowed by) the context-menu
    // layer instead.
    let mut app = make_app_stub();
    app.player_tab
        .items
        .push(crate::app::tests::make_item("1", "Track"));
    app.context_menu = Some(test_empty_context_menu());
    app.handle_key(ev(KeyCode::Char('c'), KeyModifiers::NONE));
    assert!(
        app.confirm_modal.is_none(),
        "clear-queue confirmation must not open while a context menu is open"
    );
}

#[test]
fn enter_on_queue_tab_dispatches_queue_play_cursor_via_handle_key() {
    // Issue #134: the queue tab's `Enter` key and a double-click on a
    // queue row both go through `Command::QueuePlayCursor` now. This
    // pins the keyboard side of that shared seam end-to-end through
    // `handle_key`.
    let mut app = make_app_stub();
    // `handle_queue_key` branches on `panel_focus`; queue-cursor Enter
    // only fires when the queue side is focused (equivalent of the old
    // default "Queue tab").
    app.panel_focus = crate::app::PanelFocus::Queue;
    app.player_tab.set_items(
        vec![
            crate::app::tests::make_item("1", "Audio"),
            crate::app::tests::make_item("2", "Audio"),
        ],
        1,
    );
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    let rx = app.player.spy_on_commands();

    app.handle_key(ev(KeyCode::Enter, KeyModifiers::NONE));

    assert!(matches!(
        rx.try_recv(),
        Ok(mbv_core::player::PlayerCommand::JumpTo(1))
    ));
}

#[test]
fn context_stack_order_is_pinned() {
    // Updated for the unified search modal: the per-view inline search
    // entries (`home_search`, `lib_search`) and the per-menu
    // `context_menu` entry were retired along with the inline search
    // path. The new `search_modal` entry handles all search keys
    // (any home/library tab) and the context menu's keys (`Esc` and
    // friends) are caught by the existing modal layers above it.
    let names: Vec<&str> = super::CONTEXT_STACK.iter().map(|e| e.name).collect();
    assert_eq!(
        names,
        vec![
            "daemon_lost_modal",
            "confirm_modal",
            "remote_reanchor",
            "save_playlist",
            "settings",
            "help",
            "sessions",
            "playlists",
            "global_overlay_open",
            "queue_column_width",
            "search_modal",
            "sidebar_toggle_x",
            "confirm_skip_intro",
            "confirm_next_up",
            "clear_queue_prompt_c",
            "visualizer",
            "playback",
            "ctrl_l_force_clear",
            "f5_refresh",
            "album_track_mode",
            "view_dispatch",
        ],
        "precedence order must match handle_key's pre-phase-2 branch order; \
         if this intentionally changes, update docs/adr/0002-centralized-input-handling.md too"
    );
}
