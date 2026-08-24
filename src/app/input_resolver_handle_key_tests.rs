use crate::app::tests::make_app_stub;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::player::PlayerCommand;

fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

#[test]
fn input_snapshot_has_remote_session_true_while_cast_attached() {
    let mut app = make_app_stub();
    assert!(!app.input_snapshot().has_remote_session);
    app.attach_cast("device-1".to_string());
    assert!(app.input_snapshot().has_remote_session);
    app.detach_cast();
    assert!(!app.input_snapshot().has_remote_session);
}

#[test]
fn daemon_lost_q_runs_normal_quit_cleanup() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests::make_local_daemon_app_stub(Vec::new());
    app.pending_overlay = Some(crate::app::types_overlay::OverlayRequest::DaemonLost(
        crate::app::DaemonLostModal {
            last_playing_title: None,
            daemon_log_path: "daemon.log".into(),
            restart_error: None,
        },
    ));
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-id".into()),
        name: "Saved".into(),
    };
    app.queue_dirty = true;
    app.config.lock().unwrap().save_playlist_on_quit = false;
    crate::app::QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);

    let mut model = crate::app::Model::new(app);
    model.sync_modal_requests();
    let quit = model.handle_daemon_lost_key(ev(KeyCode::Char('q'), KeyModifiers::NONE));
    assert!(quit);
    assert!(!model
        .application
        .mounted(&crate::app::components::ComponentId::Modal(
            crate::app::components::ModalId::DaemonLost
        )));
    assert!(
        !model.app.queue_dirty,
        "normal quit cleanup must discard dirty state"
    );

    crate::app::QUIT_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
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
    app.ask_confirm(crate::app::ConfirmModal {
        title: " Clear Queue ".into(),
        message: "Clear the queue?".into(),
        hint: "[y] Confirm    [Esc] Cancel".into(),
        on_confirm: crate::app::ConfirmAction::ClearQueue,
    });
    let mut model = crate::app::Model::new(app);
    model.sync_modal_requests();
    model.handle_confirm_key(ev(KeyCode::Char('y'), KeyModifiers::NONE));
    assert!(
        !model
            .application
            .mounted(&crate::app::components::ComponentId::Modal(
                crate::app::components::ModalId::Confirm
            )),
        "confirm modal clears regardless of answer"
    );
}

#[test]
fn confirm_rescan_no_clears_flag_without_rescan_via_handle_key() {
    let mut app = make_app_stub();
    app.ask_confirm(crate::app::ConfirmModal {
        title: " Rescan Library ".into(),
        message: "Rescan 'Movies'?".into(),
        hint: "[y] Confirm    [Esc] Cancel".into(),
        on_confirm: crate::app::ConfirmAction::RescanLibrary(0),
    });
    let mut model = crate::app::Model::new(app);
    model.sync_modal_requests();
    model.handle_confirm_key(ev(KeyCode::Char('n'), KeyModifiers::NONE));
    assert!(!model
        .application
        .mounted(&crate::app::components::ComponentId::Modal(
            crate::app::components::ModalId::Confirm
        )));
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
        anchor: crate::app::ContextMenuAnchor::Pointer { x: 0, y: 0 },
        entries: Vec::new(),
        cursor: 0,
    }
}

#[test]
fn x_cycles_panel_mode_via_handle_key() {
    let mut app = make_app_stub();
    assert_eq!(app.panel_mode, crate::app::PanelMode::Both);
    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(app.panel_mode, crate::app::PanelMode::QueueOnly);
    assert_eq!(app.panel_focus, crate::app::PanelFocus::Queue);
    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(app.panel_mode, crate::app::PanelMode::LibraryOnly);
    assert_eq!(app.panel_focus, crate::app::PanelFocus::Library);
    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(app.panel_mode, crate::app::PanelMode::Both);
    assert_eq!(app.panel_focus, crate::app::PanelFocus::Library);
}

#[test]
fn x_does_not_cycle_panel_mode_while_context_menu_is_open_via_handle_key() {
    let mut app = make_app_stub();
    app.context_menu = Some(test_empty_context_menu());
    let before = app.panel_mode;
    // The Context menu is now a TuiRealm component (task 2.5); its key
    // dispatch goes through `handle_key_context_menu` directly, not through
    // CONTEXT_STACK. 'x' is swallowed by the menu.
    app.handle_key_context_menu(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(
        app.panel_mode, before,
        "Panel mode must not cycle while a context menu is open"
    );
}

#[test]
fn context_menu_owns_keyboard_navigation_and_dismissal() {
    let mut app = make_app_stub();
    app.context_menu = Some(crate::app::ContextMenu {
        anchor: crate::app::ContextMenuAnchor::Pointer { x: 0, y: 0 },
        entries: vec![
            crate::app::ContextMenuEntry {
                label: "first",
                action: Some(crate::app::ContextAction::Play),
            },
            crate::app::ContextMenuEntry {
                label: "separator",
                action: None,
            },
            crate::app::ContextMenuEntry {
                label: "last",
                action: Some(crate::app::ContextAction::Play),
            },
        ],
        cursor: 0,
    });

    // The Context menu is now a TuiRealm component (task 2.5); its key
    // dispatch goes through `handle_key_context_menu` directly.
    app.handle_key_context_menu(ev(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.context_menu.as_ref().unwrap().cursor, 2);
    app.handle_key_context_menu(ev(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.context_menu.as_ref().unwrap().cursor, 0);
    app.handle_key_context_menu(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(app.context_menu.is_some(), "unrelated keys are swallowed");
    app.handle_key_context_menu(ev(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.context_menu.is_none());
}

#[test]
fn context_menu_open_is_refused_over_sidebar_surface() {
    let mut app = make_app_stub();
    app.show_sessions = true;
    app.open_context_menu();
    assert!(app.context_menu.is_none());
}

#[test]
fn context_menu_swallow_regression_shortcuts() {
    let mut app = make_app_stub();
    app.context_menu = Some(test_empty_context_menu());
    let keys = [
        ev(KeyCode::F(1), KeyModifiers::NONE),
        ev(KeyCode::F(2), KeyModifiers::NONE),
        ev(KeyCode::F(3), KeyModifiers::NONE),
        ev(KeyCode::F(4), KeyModifiers::NONE),
        ev(KeyCode::Char('/'), KeyModifiers::CONTROL),
        ev(KeyCode::Tab, KeyModifiers::NONE),
        ev(KeyCode::BackTab, KeyModifiers::SHIFT),
        ev(KeyCode::Char('1'), KeyModifiers::NONE),
        ev(KeyCode::Char('9'), KeyModifiers::NONE),
        ev(KeyCode::F(5), KeyModifiers::NONE),
        ev(KeyCode::Char('c'), KeyModifiers::NONE),
        ev(KeyCode::Char('x'), KeyModifiers::NONE),
    ];
    // The Context menu is now a TuiRealm component (task 2.5); its key
    // dispatch goes through `handle_key_context_menu` directly, not through
    // CONTEXT_STACK.
    for key in keys {
        app.handle_key_context_menu(key);
        assert!(app.context_menu.is_some(), "{key:?} must be swallowed");
        assert!(!app.show_settings && !app.show_sessions);
    }
}

#[test]
fn x_moves_queue_focus_to_library_when_entering_library_only() {
    let mut app = make_app_stub();
    app.panel_focus = crate::app::PanelFocus::Queue;

    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));

    assert_eq!(
        app.panel_mode,
        crate::app::PanelMode::QueueOnly,
        "first x from Both enters QueueOnly"
    );
    assert_eq!(app.panel_focus, crate::app::PanelFocus::Queue);

    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));

    assert_eq!(app.panel_mode, crate::app::PanelMode::LibraryOnly);
    assert_eq!(app.panel_focus, crate::app::PanelFocus::Library);
}

#[test]
fn x_entering_both_leaves_focus_alone() {
    let mut app = make_app_stub();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;

    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));

    assert_eq!(app.panel_mode, crate::app::PanelMode::Both);
    assert_eq!(
        app.panel_focus,
        crate::app::PanelFocus::Library,
        "returning to Both must not move focus"
    );
}

// Mini view: below MINI_VIEW_THRESHOLD columns, `x` toggles exactly two
// states (library-only ⇄ queue-only), never Both, and carries panel focus with
// it. Widening back to 80+ restores the prior wide-mode panel_mode/panel_focus
// untouched (mini view is derived, never written into stored state).

#[test]
fn mini_view_x_toggles_library_only_and_queue_only_only() {
    let mut app = make_app_stub();
    app.terminal_width = crate::app::MINI_VIEW_THRESHOLD - 1;
    app.panel_mode = crate::app::PanelMode::Both;
    app.panel_focus = crate::app::PanelFocus::Library;
    app.mini_view_focus = crate::app::PanelFocus::Library;

    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(
        app.effective_panel_mode(),
        crate::app::PanelMode::QueueOnly,
        "first x in mini view goes to queue-only"
    );
    assert_eq!(
        app.effective_panel_focus(),
        crate::app::PanelFocus::Queue,
        "toggling to queue moves focus with the panel"
    );

    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(
        app.effective_panel_mode(),
        crate::app::PanelMode::LibraryOnly,
        "second x in mini view goes to library-only"
    );
    assert_eq!(
        app.effective_panel_focus(),
        crate::app::PanelFocus::Library,
        "toggling back to library moves focus with the panel"
    );

    // Still narrow: any number of presses never shows both panels, and the
    // stored wide-mode state was never touched.
    for _ in 0..3 {
        app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_ne!(
            app.effective_panel_mode(),
            crate::app::PanelMode::Both,
            "Both must be unreachable in mini view"
        );
    }
    assert_eq!(
        app.panel_mode,
        crate::app::PanelMode::Both,
        "stored wide panel_mode must stay untouched while narrow"
    );
    assert_eq!(
        app.panel_focus,
        crate::app::PanelFocus::Library,
        "stored wide panel_focus must stay untouched while narrow"
    );
}

#[test]
fn mini_view_widening_restores_prior_wide_mode_state() {
    let mut app = make_app_stub();
    app.terminal_width = crate::app::MINI_VIEW_THRESHOLD - 1;
    // Simulate a queue-only-with-queue-focus wide state that was current
    // before narrowing.
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.panel_focus = crate::app::PanelFocus::Queue;

    // Narrow: toggle mini view to library-only and back a couple times.
    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));

    // Widen back to 80+.
    app.terminal_width = crate::app::MINI_VIEW_THRESHOLD;
    // The stored wide state is unchanged by any narrow toggling.
    assert_eq!(app.panel_mode, crate::app::PanelMode::QueueOnly);
    assert_eq!(app.panel_focus, crate::app::PanelFocus::Queue);
    // And the effective view now reflects that restored wide state.
    assert_eq!(app.effective_panel_mode(), crate::app::PanelMode::QueueOnly);
    assert_eq!(app.effective_panel_focus(), crate::app::PanelFocus::Queue);
}

/// Regression guard in the other direction: the panel-mode cycle key used to
/// be 'h'; the binding moved to 'x' so 'h' could be repurposed for vim-style
/// horizontal navigation in 2-col library lists. Make sure 'h' no longer
/// cycles the panel mode on its own.
#[test]
fn h_no_longer_cycles_panel_mode_via_handle_key() {
    let mut app = make_app_stub();
    let before = app.panel_mode;
    app.handle_key(ev(KeyCode::Char('h'), KeyModifiers::NONE));
    assert_eq!(
        app.panel_mode, before,
        "Panel-mode cycle moved from 'h' to 'x'; 'h' must not change the mode"
    );
}

#[test]
fn c_prompts_clear_queue_confirmation_via_handle_key() {
    let mut app = make_app_stub();
    app.player_tab
        .append_item(crate::app::tests::make_item("1", "Track"));
    app.handle_key(ev(KeyCode::Char('c'), KeyModifiers::NONE));
    assert!(matches!(
        app.pending_overlay.as_ref(),
        Some(crate::app::types_overlay::OverlayRequest::Confirm(modal))
            if matches!(&modal.on_confirm, crate::app::ConfirmAction::ClearQueue)
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
    //
    // The Context menu is now a TuiRealm component (task 2.5); its key
    // dispatch goes through `handle_key_context_menu` directly, not through
    // CONTEXT_STACK. 'c' is swallowed by the menu.
    let mut app = make_app_stub();
    app.player_tab
        .append_item(crate::app::tests::make_item("1", "Track"));
    app.context_menu = Some(test_empty_context_menu());
    app.handle_key_context_menu(ev(KeyCode::Char('c'), KeyModifiers::NONE));
    assert!(
        !matches!(
            app.pending_overlay.as_ref(),
            Some(crate::app::types_overlay::OverlayRequest::Confirm(_))
        ),
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
    // The unified search modal was retired in favor of a global `Ctrl+/`
    // panel (now the
    // `SearchSidebarComponent`, task 3.2 — its `CONTEXT_STACK` entry was
    // removed when the sidebar became a TuiRealm component).
    // The context menu owns every key while open and therefore precedes all
    // other modal and view contexts.
    let names: Vec<&str> = super::CONTEXT_STACK.iter().map(|e| e.name).collect();
    assert_eq!(
        names,
        vec![
            "selection_modal",
            "settings",
            "playlists",
            "global_overlay_open",
            "queue_column_width",
            "panel_mode_cycle_x",
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
