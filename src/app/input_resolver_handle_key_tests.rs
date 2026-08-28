use crate::app::components::{ComponentId, ContextMenuComponent, OverlayId};
use crate::app::tests::make_app_stub;
use crate::app::types_context_menu::{
    ContextAction, ContextMenu, ContextMenuAnchor, ContextMenuEntry,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mbv_core::player::PlayerCommand;

fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn open_context_menu_on(model: &mut crate::app::Model, menu: ContextMenu) {
    model.app.pending_overlay = Some(crate::app::types_overlay::OverlayRequest::ContextMenu(menu));
    model.sync_modal_requests();
}

fn context_menu_component(model: &crate::app::Model) -> &ContextMenuComponent {
    model
        .application
        .get_component(&ComponentId::Overlay(OverlayId::ContextMenu))
        .and_then(|component| component.as_any().downcast_ref::<ContextMenuComponent>())
        .expect("context menu mounted")
}

fn context_menu_mounted(model: &crate::app::Model) -> bool {
    model
        .application
        .mounted(&ComponentId::Overlay(OverlayId::ContextMenu))
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
fn f2_requests_settings_sidebar_mount() {
    let mut app = make_app_stub();
    app.handle_key(ev(KeyCode::F(2), KeyModifiers::NONE));
    assert!(matches!(
        app.pending_overlay,
        Some(crate::app::types_overlay::OverlayRequest::ToggleSidebar(
            crate::app::SidebarId::Settings
        ))
    ));

    let mut model = crate::app::Model::new(app);
    model.sync_modal_requests();
    assert!(model
        .application
        .mounted(&crate::app::components::ComponentId::Overlay(
            crate::app::components::OverlayId::Settings,
        )));
}

#[test]
fn f3_requests_sessions_sidebar_mount() {
    let mut app = make_app_stub();
    app.handle_key(ev(KeyCode::F(3), KeyModifiers::NONE));
    assert!(matches!(
        app.pending_overlay,
        Some(crate::app::types_overlay::OverlayRequest::OpenSidebar(
            crate::app::SidebarId::Sessions
        ))
    ));
}

#[test]
fn f4_requests_playlists_sidebar_mount() {
    let mut app = make_app_stub();
    app.handle_key(ev(KeyCode::F(4), KeyModifiers::NONE));
    assert!(matches!(
        app.pending_overlay,
        Some(crate::app::types_overlay::OverlayRequest::OpenSidebar(
            crate::app::SidebarId::Playlists
        ))
    ));
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

fn test_empty_context_menu() -> ContextMenu {
    ContextMenu {
        anchor: ContextMenuAnchor::Pointer { x: 0, y: 0 },
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
    let app = make_app_stub();
    let mut model = crate::app::Model::new(app);
    open_context_menu_on(&mut model, test_empty_context_menu());
    let before = model.app.panel_mode;
    // The Context menu is now a TuiRealm component (task 2.5); its key
    // dispatch goes through the shell's `handle_context_menu_key`, not
    // through CONTEXT_STACK. 'x' is swallowed by the menu.
    model.handle_context_menu_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(
        model.app.panel_mode, before,
        "Panel mode must not cycle while a context menu is open"
    );
}

#[test]
fn context_menu_owns_keyboard_navigation_and_dismissal() {
    let app = make_app_stub();
    let mut model = crate::app::Model::new(app);
    open_context_menu_on(
        &mut model,
        ContextMenu {
            anchor: ContextMenuAnchor::Pointer { x: 0, y: 0 },
            entries: vec![
                ContextMenuEntry {
                    label: "first",
                    action: Some(ContextAction::Play),
                },
                ContextMenuEntry {
                    label: "separator",
                    action: None,
                },
                ContextMenuEntry {
                    label: "last",
                    action: Some(ContextAction::Play),
                },
            ],
            cursor: 0,
        },
    );

    // The Context menu is now a TuiRealm component (task 2.5); its key
    // dispatch goes through the shell's `handle_context_menu_key`.
    model.handle_context_menu_key(ev(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(context_menu_component(&model).cursor(), 2);
    model.handle_context_menu_key(ev(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(context_menu_component(&model).cursor(), 0);
    model.handle_context_menu_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(context_menu_mounted(&model), "unrelated keys are swallowed");
    model.handle_context_menu_key(ev(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!context_menu_mounted(&model));
}

#[test]
fn context_menu_mount_dismisses_sidebar_surface() {
    let mut app = make_app_stub();
    app.pending_overlay = Some(crate::app::types_overlay::OverlayRequest::OpenSidebar(
        crate::app::SidebarId::Sessions,
    ));
    let mut model = crate::app::Model::new(app);
    model.sync_modal_requests();
    open_context_menu_on(&mut model, test_empty_context_menu());
    assert!(!model
        .application
        .mounted(&crate::app::components::ComponentId::Overlay(
            crate::app::components::OverlayId::Sessions,
        )));
    assert!(context_menu_mounted(&model));
}

#[test]
fn context_menu_swallow_regression_shortcuts() {
    let mut app = make_app_stub();
    let mut model = crate::app::Model::new(app);
    open_context_menu_on(&mut model, test_empty_context_menu());
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
    // dispatch goes through the shell's `handle_context_menu_key`, not
    // through CONTEXT_STACK.
    for key in keys {
        model.handle_context_menu_key(key);
        assert!(context_menu_mounted(&model), "{key:?} must be swallowed");
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
    let mut model = crate::app::Model::new(app);
    open_context_menu_on(&mut model, test_empty_context_menu());
    model.handle_context_menu_key(ev(KeyCode::Char('c'), KeyModifiers::NONE));
    assert!(
        !matches!(
            model.app.pending_overlay.as_ref(),
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
