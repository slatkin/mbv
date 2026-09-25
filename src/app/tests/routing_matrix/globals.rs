//! Routing matrix: globals precedence and policy rows.

use super::support::*;
use crate::app::components::msg::{ConfirmIntent, ContextMenuIntent};
use crate::app::components::{ComponentId, Msg, OverlayId, ShellRequest};
use crate::app::dispatch::action::Command;
use crate::app::input::router::{resolve_router_outcome_with_focused, RouterOutcome};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn clear_queue_c_does_not_fire_under_open_context_menu() {
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::ContextMenuIntent(
        ContextMenuIntent::Dismiss,
    ))));
    let out = fold_tick_with_outcome(
        leaf,
        key(KeyCode::Char('c')),
        Some(ComponentId::Overlay(OverlayId::ContextMenu)),
        RouterOutcome::Swallow,
    );
    assert!(
        out.is_empty(),
        "an open context menu must swallow 'c' (no clear-queue confirmation)"
    );
}
#[test]
fn ctrl_slash_both_terminal_encodings_route_identically() {
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::Quit)));
    let slash_out = fold_tick(
        leaf,
        KeyEvent::new(KeyCode::Char('/'), KeyModifiers::CONTROL),
        Some(ComponentId::UiRoot),
        idle_snapshot(),
    );
    let leaf = Some(Msg::Shell(Box::new(ShellRequest::Quit)));
    let underscore_out = fold_tick(
        leaf,
        KeyEvent::new(KeyCode::Char('_'), KeyModifiers::CONTROL),
        Some(ComponentId::UiRoot),
        idle_snapshot(),
    );
    assert_eq!(
        slash_out.len(),
        underscore_out.len(),
        "both Ctrl+/ encodings must route identically"
    );
}
#[test]
fn destination_independent_globals_resolve_to_router_commands() {
    let mut snapshot = idle_snapshot();
    let globals = [
        (key(KeyCode::Char('q')), Command::Quit),
        (key(KeyCode::Tab), Command::NextLibraryTab),
        (key(KeyCode::BackTab), Command::PreviousLibraryTab),
        (key(KeyCode::Char('1')), Command::SetLibraryTab(0)),
        (
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
            Command::ForceClear,
        ),
        (key(KeyCode::F(5)), Command::RefreshCurrentView),
        (key(KeyCode::Char('x')), Command::CyclePanelMode),
        (key(KeyCode::F(2)), Command::ToggleSettings),
        (key(KeyCode::F(3)), Command::OpenSessions),
        (key(KeyCode::F(4)), Command::OpenPlaylists),
        (
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::CONTROL),
            Command::OpenSearch,
        ),
    ];
    for (key, command) in globals {
        assert_eq!(
            resolve_router_outcome_with_focused(key, &snapshot, None, &default_keybinds()),
            RouterOutcome::Command(command),
            "global {key:?} must be claimed by the router"
        );
    }

    snapshot.help_overlay_open = false;
    assert_eq!(
        resolve_router_outcome_with_focused(
            key(KeyCode::F(1)),
            &snapshot,
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(Command::OpenHelp)
    );
}
#[test]
fn shift_tab_backtab_shift_encoding_fires_previous_library_tab() {
    // Crossterm delivers Shift+Tab as BackTab+SHIFT; the default binding is
    // the bare BackTab chord. The chord normalization must keep it firing
    // through the router (P1 regression).
    assert_eq!(
        resolve_router_outcome_with_focused(
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
            &idle_snapshot(),
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(Command::PreviousLibraryTab),
        "Shift+Tab must fire previous_library_tab under defaults"
    );
}
#[test]
fn exact_chord_narrowing_keeps_superset_chords_from_globals() {
    // Deliberate exact-chord semantics: chords the old permissive literals
    // happened to catch — Ctrl+C (clear queue) and the Ctrl+Shift encodings
    // of `/` (search) — must not fire their globals (P2 pin).
    let snapshot = idle_snapshot();
    assert_eq!(
        resolve_router_outcome_with_focused(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &snapshot,
            None,
            &default_keybinds()
        ),
        RouterOutcome::FallThrough,
        "Ctrl+C must not fire the clear-queue prompt"
    );
    for code in [KeyCode::Char('/'), KeyCode::Char('_')] {
        assert_eq!(
            resolve_router_outcome_with_focused(
                KeyEvent::new(code, KeyModifiers::CONTROL | KeyModifiers::SHIFT),
                &snapshot,
                None,
                &default_keybinds()
            ),
            RouterOutcome::FallThrough,
            "Ctrl+Shift+`/` must not open search"
        );
    }
}
#[test]
fn help_and_alt_router_guards_preserve_overlay_precedence() {
    let mut snapshot = idle_snapshot();
    snapshot.blocking_overlay_open = true;
    assert_eq!(
        resolve_router_outcome_with_focused(
            key(KeyCode::F(1)),
            &snapshot,
            None,
            &default_keybinds()
        ),
        RouterOutcome::Swallow,
        "F1 must not open Help over a blocking overlay"
    );

    snapshot.blocking_overlay_open = false;
    snapshot.help_overlay_open = true;
    assert_eq!(
        resolve_router_outcome_with_focused(
            key(KeyCode::F(1)),
            &snapshot,
            None,
            &default_keybinds()
        ),
        RouterOutcome::FallThrough,
        "Help keeps F1 for its dismiss request"
    );

    snapshot.help_overlay_open = false;
    snapshot.panel_focus = crate::app::PanelFocus::Queue;
    assert_eq!(
        resolve_router_outcome_with_focused(
            KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL),
            &snapshot,
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(Command::FocusPanel(crate::app::PanelFocus::Library))
    );
    assert_eq!(
        resolve_router_outcome_with_focused(
            KeyEvent::new(KeyCode::Down, KeyModifiers::ALT),
            &snapshot,
            None,
            &default_keybinds()
        ),
        RouterOutcome::Command(Command::NextLibraryTab)
    );
    assert_eq!(
        resolve_router_outcome_with_focused(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &snapshot,
            None,
            &default_keybinds()
        ),
        RouterOutcome::Swallow,
        "Alt+Right yielded its panel-switch duty to plain Right; alt_swallow claims it"
    );
    assert_eq!(
        resolve_router_outcome_with_focused(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT),
            &snapshot,
            None,
            &default_keybinds()
        ),
        RouterOutcome::Swallow,
        "unhandled Alt chords must not leak into destination handling"
    );
}
#[test]
fn plain_panel_arrows_yield_while_an_overlay_holds_focus() {
    // With the Playlists sidebar (or any overlay) holding focus, the plain
    // panel arrows must fall through to the leaf: the sidebar owns the
    // keyboard (its own Left collapses the open playlist), and panel focus
    // must not move behind it.
    let mut snapshot = idle_snapshot();
    snapshot.panel_focus = crate::app::PanelFocus::Library;
    snapshot.overlay_holds_focus = true;
    assert_eq!(
        resolve_router_outcome_with_focused(
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            &snapshot,
            None,
            &default_keybinds()
        ),
        RouterOutcome::FallThrough,
        "plain Left stays with the focused sidebar"
    );
    snapshot.panel_focus = crate::app::PanelFocus::Queue;
    assert_eq!(
        resolve_router_outcome_with_focused(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &snapshot,
            None,
            &default_keybinds()
        ),
        RouterOutcome::FallThrough,
        "plain Right stays with the focused sidebar"
    );
}
#[test]
fn quit_global_does_not_fire_in_search_sidebar() {
    let focused = ComponentId::Overlay(OverlayId::Search);
    assert_eq!(
        resolve_router_outcome_with_focused(
            key(KeyCode::Char('q')),
            &text_entry_snapshot(),
            Some(&focused),
            &default_keybinds()
        ),
        RouterOutcome::FallThrough,
        "`q` is character input in the search sidebar, not Quit"
    );
}
#[test]
fn panel_mode_cycle_global_does_not_fire_in_search_sidebar() {
    let focused = ComponentId::Overlay(OverlayId::Search);
    assert_eq!(
        resolve_router_outcome_with_focused(
            key(KeyCode::Char('x')),
            &text_entry_snapshot(),
            Some(&focused),
            &default_keybinds()
        ),
        RouterOutcome::FallThrough,
        "`x` is character input in the search sidebar, not panel-mode cycle"
    );
}
#[test]
fn library_tab_jump_does_not_fire_in_search_sidebar() {
    let focused = ComponentId::Overlay(OverlayId::Search);
    assert_eq!(
        resolve_router_outcome_with_focused(
            key(KeyCode::Char('1')),
            &text_entry_snapshot(),
            Some(&focused),
            &default_keybinds()
        ),
        RouterOutcome::FallThrough,
        "a digit is character input in the search sidebar, not a tab jump"
    );
}
#[test]
fn library_tab_jump_with_modifiers_is_swallowed() {
    let focused = ComponentId::Library;
    assert_eq!(
        resolve_router_outcome_with_focused(
            KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT),
            &idle_snapshot(),
            Some(&focused),
            &default_keybinds()
        ),
        RouterOutcome::Swallow,
        "`Alt+1` is claimed by the policy's alt_swallow entry, not a tab jump"
    );
}
#[test]
fn open_sessions_command_toggles_without_respawning_loads() {
    let _guard = crate::config::TestStateDirGuard::new();
    let app = crate::app::tests::make_app_stub();
    let mut model = crate::app::shell::Model::new(app);
    model.sync_mounted_surfaces();

    let sessions = ComponentId::Overlay(OverlayId::Sessions);

    model.app.dispatch(Command::OpenSessions);
    model.sync_modal_requests();
    assert!(
        model.application.mounted(&sessions),
        "first F3 opens Sessions"
    );

    // Clear the flag the open set so a spurious re-mount would be observable.
    model.app.sessions_loading = false;

    model.app.dispatch(Command::OpenSessions);
    model.sync_modal_requests();
    assert!(
        !model.application.mounted(&sessions),
        "second F3 toggles Sessions closed"
    );
    assert!(
        !model.app.sessions_loading,
        "toggle-close must not respawn the sessions load"
    );
}
#[test]
fn clear_queue_c_is_global_but_yields_to_text_entry() {
    let focused = ComponentId::Library;
    assert_eq!(
        resolve_router_outcome_with_focused(
            key(KeyCode::Char('c')),
            &idle_snapshot(),
            Some(&focused),
            &default_keybinds()
        ),
        RouterOutcome::Command(Command::RequestClearQueue),
        "`c` opens the clear-queue prompt with the browser focused"
    );

    let mut typing = idle_snapshot();
    typing.text_entry_focused = true;
    assert_eq!(
        resolve_router_outcome_with_focused(
            key(KeyCode::Char('c')),
            &typing,
            Some(&focused),
            &default_keybinds()
        ),
        RouterOutcome::FallThrough,
        "`c` is character input while a text entry owns focus"
    );
}

#[test]
fn rebound_global_respects_text_entry_and_keeps_its_default_inert() {
    // `quit` rebound from `q` to `w` (task 3.2): the configured chord claims
    // the router, the text-entry rule holds for it, and the declared default
    // is inert.
    let keybinds = keybinds_with_override("quit", "w");
    let rebound_key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE);
    let focused = ComponentId::Library;

    assert_eq!(
        resolve_router_outcome_with_focused(
            rebound_key,
            &idle_snapshot(),
            Some(&focused),
            &keybinds
        ),
        RouterOutcome::Command(Command::Quit),
        "the configured chord fires the rebound action"
    );
    assert_eq!(
        resolve_router_outcome_with_focused(
            rebound_key,
            &text_entry_snapshot(),
            Some(&focused),
            &keybinds
        ),
        RouterOutcome::FallThrough,
        "the rebound chord is character input while a text entry owns focus"
    );
    assert_eq!(
        resolve_router_outcome_with_focused(
            key(KeyCode::Char('q')),
            &idle_snapshot(),
            Some(&focused),
            &keybinds
        ),
        RouterOutcome::FallThrough,
        "the declared default of a rebound action is inert"
    );
}
#[test]
fn quit_and_visualizer_yield_to_text_entry() {
    let focused = ComponentId::Library;
    let mut typing = idle_snapshot();
    typing.text_entry_focused = true;
    for key in [key(KeyCode::Char('q')), key(KeyCode::Char('v'))] {
        assert_eq!(
            resolve_router_outcome_with_focused(key, &typing, Some(&focused), &default_keybinds()),
            RouterOutcome::FallThrough,
            "inline search must keep typed characters instead of quitting/toggling"
        );
    }
}
#[test]
fn confirm_accept_re_encodes_to_y_chord() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.set_items(
        crate::app::tests::make_items(3),
        app.player_tab.queue_cursor,
    );
    app.player_tab.queue_cursor = 1;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 1;
    }
    app.remove_from_queue(1);
    assert!(
        matches!(
            app.pending_overlay,
            Some(crate::app::state::types::overlay::OverlayRequest::Confirm(
                _
            ))
        ),
        "removing the active queue item asks for confirmation"
    );

    let mut model = crate::app::shell::Model::new(app);
    model.sync_mounted_surfaces();
    model.handle_confirm_intent(ConfirmIntent::Accept);

    assert_eq!(
        model.app.player_tab.emby_items().len(),
        2,
        "Accept re-encodes to Char('y') and reaches the RemoveActiveQueueItem accept arm"
    );
}
