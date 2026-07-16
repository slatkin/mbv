//! Central input resolution: the single, testable seam that turns a key press
//! (in a given UI context) into a semantic `Command`, a `Swallow`, or a
//! `FallThrough`. See `docs/adr/0002-centralized-input-handling.md`.
//!
//! Phase 1 (#130) covers only the Playback and Help contexts. The full
//! context-priority stack that *selects* the context arrives in phase 2 (#131).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A normalized key press: physical key code plus active modifiers, with the
/// terminal-specific `kind`/`state` fields of `KeyEvent` dropped. This is the
/// unit the resolver matches bindings against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct KeyChord {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

impl KeyChord {
    // Test-only constructor: production code always builds a `KeyChord` from
    // a real `KeyEvent` via `from_key`. `#[cfg(test)]` keeps it out of the
    // non-test build, where it would otherwise be unreachable dead code (see
    // `cargo clippy --all-targets -D warnings` in `docs/CHECKIN.md`).
    #[cfg(test)]
    pub(super) fn new(code: KeyCode, mods: KeyModifiers) -> Self {
        Self { code, mods }
    }

    pub(super) fn from_key(key: KeyEvent) -> Self {
        Self {
            code: key.code,
            mods: key.modifiers,
        }
    }
}

use super::action::Command;
use super::App;

/// A UI context that can bind keys. Phase 1 has only the two contexts that
/// already had a pure translation seam; phase 2 (#131) adds the rest and the
/// priority stack that selects among them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputContext {
    Playback,
}

/// The outcome of resolving a chord in a context.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum KeyResolution {
    /// Dispatch this semantic command.
    Command(Command),
    /// Consume the key with no action (e.g. an overlay eating unknown keys).
    Swallow,
    /// Decline the key; a lower-priority context (or the view handler) handles it.
    FallThrough,
}

/// The plain-data view of app state the resolver reads, so resolution stays a
/// pure function testable without constructing an `App`. Phase 1 carries only
/// the fields the Playback gate needs; phase 2 grows this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct InputSnapshot {
    pub player_active: bool,
    pub has_remote_session: bool,
}

/// Resolve a chord within a single context. Pure: no `App`/`Player` access.
pub(super) fn help_resolve(chord: KeyChord) -> KeyResolution {
    match super::action::help_command_for_key(chord) {
        Some(cmd) => KeyResolution::Command(cmd),
        None => KeyResolution::Swallow,
    }
}

/// Resolve a chord within a single context. Pure: no `App`/`Player` access.
pub(super) fn resolve_key(
    context: InputContext,
    snapshot: &InputSnapshot,
    chord: KeyChord,
) -> KeyResolution {
    match context {
        // Playback keys are gated; an unbound or gate-closed key falls through
        // to the handlers below it in `handle_key`.
        InputContext::Playback => {
            match super::action::playback_command_for_key(
                chord,
                snapshot.player_active,
                snapshot.has_remote_session,
            ) {
                Some(cmd) => KeyResolution::Command(cmd),
                None => KeyResolution::FallThrough,
            }
        }
    }
}

impl App {
    /// Build the input snapshot from current app state. Single build-site so
    /// "what does input depend on?" has one auditable answer.
    pub(super) fn input_snapshot(&self) -> InputSnapshot {
        InputSnapshot {
            player_active: self.player.status.lock().unwrap().active,
            has_remote_session: self.connected_session_id.is_some(),
        }
    }
}

/// One layer of the keyboard precedence stack: a name for assertions/debugging
/// and a handler that returns `Some(quit)` if this context claimed the key, or
/// `None` to fall through to the next-lower-priority context. Phase 2 (#131)
/// makes `handle_key`'s branch order into this explicit, ordered, testable
/// list instead of implicit control flow.
///
/// A stack-entry handler is only meant to be invoked through `CONTEXT_STACK`
/// via `handle_key`'s loop, never called directly — direct calls would bypass
/// the explicit precedence order this stack exists to make assertable. The
/// `pub(super)` visibility on these handlers is required for the fn-pointer
/// table below, not an invitation to call them from elsewhere in `app`.
#[derive(Clone, Copy)]
pub(super) struct ContextEntry {
    // Only read by the `context_stack_order_is_pinned` characterization test
    // today; kept outside `#[cfg(test)]` since it's part of the type's
    // intended (debugging/assertion) purpose, not test-only scaffolding.
    #[allow(dead_code)]
    pub name: &'static str,
    pub handler: fn(&mut App, KeyEvent) -> Option<bool>,
}

/// The full keyboard context-priority stack, first-match-wins, in the exact
/// order `handle_key` checked them before phase 2. See
/// `docs/adr/0002-centralized-input-handling.md`.
pub(super) const CONTEXT_STACK: &[ContextEntry] = &[
    ContextEntry {
        name: "save_modal",
        handler: App::handle_key_save_modal,
    },
    ContextEntry {
        name: "save_playlist",
        handler: App::handle_key_save_playlist_entry,
    },
    ContextEntry {
        name: "settings",
        handler: App::handle_key_settings,
    },
    ContextEntry {
        name: "help",
        handler: App::handle_key_help,
    },
    ContextEntry {
        name: "sessions",
        handler: App::handle_key_sessions,
    },
    ContextEntry {
        name: "playlists",
        handler: App::handle_key_playlists,
    },
    ContextEntry {
        name: "global_overlay_open",
        handler: App::handle_key_global_overlay_open,
    },
    ContextEntry {
        name: "power_left_width",
        handler: App::handle_key_power_left_width,
    },
    ContextEntry {
        name: "home_search",
        handler: App::handle_key_home_search,
    },
    ContextEntry {
        name: "power_lib_search",
        handler: App::handle_key_power_lib_search,
    },
    ContextEntry {
        name: "lib_search",
        handler: App::handle_key_lib_search,
    },
    ContextEntry {
        name: "panel_toggle_h",
        handler: App::handle_key_panel_toggle,
    },
    ContextEntry {
        name: "confirm_clear_queue",
        handler: App::handle_key_confirm_clear_queue,
    },
    ContextEntry {
        name: "confirm_rescan",
        handler: App::handle_key_confirm_rescan,
    },
    ContextEntry {
        name: "confirm_skip_intro",
        handler: App::handle_key_confirm_skip_intro,
    },
    ContextEntry {
        name: "confirm_next_up",
        handler: App::handle_key_confirm_next_up,
    },
    ContextEntry {
        name: "clear_queue_prompt_c",
        handler: App::handle_key_clear_queue_prompt,
    },
    ContextEntry {
        name: "context_menu",
        handler: App::handle_key_context_menu,
    },
    ContextEntry {
        name: "playback",
        handler: App::handle_playback_key,
    },
    ContextEntry {
        name: "ctrl_l_force_clear",
        handler: App::handle_key_ctrl_l,
    },
    ContextEntry {
        name: "f5_refresh",
        handler: App::handle_key_f5_refresh,
    },
    ContextEntry {
        name: "power_album_track_mode",
        handler: App::handle_key_power_album_track_mode,
    },
    ContextEntry {
        name: "view_dispatch",
        handler: App::handle_key_view_dispatch,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::action::Command;

    fn snap(active: bool, remote: bool) -> InputSnapshot {
        InputSnapshot {
            player_active: active,
            has_remote_session: remote,
        }
    }

    #[test]
    fn help_context_maps_bound_key_to_command() {
        let r = help_resolve(KeyChord::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(r, KeyResolution::Command(Command::CloseHelp));
    }

    #[test]
    fn help_context_swallows_unbound_key() {
        // The help overlay consumes every key while open.
        let r = help_resolve(KeyChord::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(r, KeyResolution::Swallow);
    }

    #[test]
    fn help_context_resolution_ignores_snapshot_fields() {
        let a = InputSnapshot {
            player_active: true,
            has_remote_session: true,
        };
        let b = InputSnapshot {
            player_active: false,
            has_remote_session: false,
        };
        assert_ne!(a, b, "the snapshots must differ to prove Help ignores them");
        let chord = KeyChord::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(
            help_resolve(chord),
            KeyResolution::Command(Command::CloseHelp)
        );
    }

    #[test]
    fn playback_context_maps_gated_key_to_command_when_active() {
        let r = resolve_key(
            InputContext::Playback,
            &snap(true, false),
            KeyChord::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::Command(Command::TogglePlayPause));
    }

    #[test]
    fn playback_context_falls_through_when_gate_closed() {
        // Space is a no-op that must reach the view handler when nothing plays.
        let r = resolve_key(
            InputContext::Playback,
            &snap(false, false),
            KeyChord::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::FallThrough);
    }

    #[test]
    fn playback_context_falls_through_on_unbound_key() {
        let r = resolve_key(
            InputContext::Playback,
            &snap(true, false),
            KeyChord::new(KeyCode::Char('x'), KeyModifiers::NONE),
        );
        assert_eq!(r, KeyResolution::FallThrough);
    }
}

#[cfg(test)]
mod app_level_tests {
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
    fn help_overlay_blocks_home_search_char_capture_via_handle_key() {
        let mut app = make_app_stub();
        app.show_help = true;
        app.search.set_state_for_test(Some(test_home_search()));
        if let Some(hs) = app.search.state_mut() {
            hs.input_focused = true;
        }
        app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(
            app.search.state().unwrap().query.is_empty(),
            "help sits above home_search in CONTEXT_STACK and must swallow 'x'"
        );
    }

    #[test]
    fn space_toggles_pause_when_active_via_handle_key() {
        let mut app = make_app_stub();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
        }
        let rx = app.player.spy_on_commands();
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
        // `global_overlay_open`/`power_left_width` in CONTEXT_STACK, matching the
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
    fn settings_overlay_blocks_home_search_char_capture_via_handle_key() {
        let mut app = make_app_stub();
        app.show_settings = true;
        app.search.set_state_for_test(Some(test_home_search()));
        if let Some(hs) = app.search.state_mut() {
            hs.input_focused = true;
        }
        app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(
            app.search.state().unwrap().query.is_empty(),
            "settings sits above home_search in CONTEXT_STACK and must swallow 'x'"
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
    fn sessions_overlay_blocks_home_search_char_capture_via_handle_key() {
        let mut app = make_app_stub();
        app.show_sessions = true;
        app.search.set_state_for_test(Some(test_home_search()));
        if let Some(hs) = app.search.state_mut() {
            hs.input_focused = true;
        }
        app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(
            app.search.state().unwrap().query.is_empty(),
            "sessions sits above home_search in CONTEXT_STACK and must swallow 'x'"
        );
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
        app.confirm_clear_queue = true;
        app.handle_key(ev(KeyCode::Char('y'), KeyModifiers::NONE));
        assert!(
            !app.confirm_clear_queue,
            "confirm flag clears regardless of answer"
        );
    }

    #[test]
    fn confirm_rescan_no_clears_flag_without_rescan_via_handle_key() {
        let mut app = make_app_stub();
        app.confirm_rescan = true;
        app.handle_key(ev(KeyCode::Char('n'), KeyModifiers::NONE));
        assert!(!app.confirm_rescan);
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

    fn test_home_search() -> crate::app::search::HomeSearch {
        crate::app::search::HomeSearch {
            query: String::new(),
            last_query: String::new(),
            results: Vec::new(),
            cursor: 0,
            loading: false,
            scroll: 0,
            type_filter: 0,
            input_focused: false,
        }
    }

    fn test_empty_context_menu() -> crate::app::ContextMenu {
        crate::app::ContextMenu {
            x: 0,
            y: 0,
            entries: Vec::new(),
            cursor: 0,
        }
    }

    fn test_lib_with_search() -> crate::app::LibraryTab {
        use crate::app::tests::make_item;
        use crate::app::{BrowseLevel, LibSearch, LibraryTab};
        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: Vec::new(),
                total_count: 0,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
            }],
            search: Some(LibSearch {
                query: String::new(),
                items: Vec::new(),
                results: Vec::new(),
                cursor: 0,
                scroll: 0,
                loading: false,
            }),
            feed_home_video: None,
            power_detail_scroll: 0,

            album_track_focus: None,
            artist_header_focus: None,
        }
    }

    #[test]
    fn home_search_captures_char_via_handle_key() {
        let mut app = make_app_stub();
        app.search.set_state_for_test(Some(test_home_search()));
        if let Some(hs) = app.search.state_mut() {
            hs.input_focused = true;
        }
        app.handle_key(ev(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(app.search.state().unwrap().query, "x");
    }

    #[test]
    fn home_search_esc_closes_via_handle_key() {
        let mut app = make_app_stub();
        app.search.set_state_for_test(Some(test_home_search()));
        app.handle_key(ev(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.search.is_open());
    }

    #[test]
    fn home_search_char_capture_wins_over_h_panel_toggle_via_handle_key() {
        // Regression guard: `panel_toggle_h` must stay ordered after
        // `home_search` in CONTEXT_STACK (matching the pre-phase-2 source,
        // where the h-toggle ran after all three search blocks). If it were
        // ever reordered ahead of home_search, pressing the literal 'h'
        // character while a search box is focused would toggle the panel
        // instead of typing 'h' into the query — a real behavior change.
        let mut app = make_app_stub();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true; // show_controls == true, so panel_toggle_h would fire if reached
        }
        app.search.set_state_for_test(Some(test_home_search()));
        if let Some(hs) = app.search.state_mut() {
            hs.input_focused = true;
        }
        let panel_mode_before = app.panel_mode;
        app.handle_key(ev(KeyCode::Char('h'), KeyModifiers::NONE));
        assert_eq!(
            app.search.state().unwrap().query,
            "h",
            "home search must capture the literal 'h' character"
        );
        assert_eq!(
            app.panel_mode, panel_mode_before,
            "panel mode must not toggle while home search captures 'h'"
        );
    }

    #[test]
    fn h_toggles_panel_mode_when_active_via_handle_key() {
        // Positive counterpart to the precedence test above: with no search
        // active, 'h' must actually reach `handle_key_panel_toggle` and
        // advance `panel_mode`, not just correctly decline to fire when a
        // higher-priority context claims the key.
        let mut app = make_app_stub();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
        }
        let before = app.panel_mode;
        app.handle_key(ev(KeyCode::Char('h'), KeyModifiers::NONE));
        assert_ne!(app.panel_mode, before);
    }

    #[test]
    fn h_does_not_toggle_panel_mode_while_context_menu_is_open_via_handle_key() {
        // Behavior change (phase 6, #135): before this fix, `panel_toggle_h`
        // had no `context_menu` guard and sat above `context_menu` in
        // CONTEXT_STACK, so 'h' bled through an open context menu and
        // silently toggled the panel. It must now fall through to (and be
        // swallowed by) the context-menu layer instead.
        let mut app = make_app_stub();
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true; // show_controls == true, so panel_toggle_h would fire if reached
        }
        app.context_menu = Some(test_empty_context_menu());
        let before = app.panel_mode;
        app.handle_key(ev(KeyCode::Char('h'), KeyModifiers::NONE));
        assert_eq!(
            app.panel_mode, before,
            "panel mode must not toggle while a context menu is open"
        );
    }

    #[test]
    fn power_lib_search_esc_closes_via_handle_key() {
        let mut app = make_app_stub();
        app.tab_idx = 1;
        app.queue_view = crate::app::QUEUE_VIEW_POWER;
        app.power_focus = crate::app::PowerFocus::Left;
        app.power_left_tab = 1;
        app.libs.push(test_lib_with_search());
        app.handle_key(ev(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.libs[0].search.is_none());
    }

    #[test]
    fn lib_search_esc_closes_via_handle_key() {
        let mut app = make_app_stub();
        app.tab_idx = 2;
        app.libs.push(test_lib_with_search());
        app.handle_key(ev(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.libs[0].search.is_none());
    }

    #[test]
    fn c_prompts_clear_queue_confirmation_via_handle_key() {
        let mut app = make_app_stub();
        app.player_tab
            .items
            .push(crate::app::tests::make_item("1", "Track"));
        app.handle_key(ev(KeyCode::Char('c'), KeyModifiers::NONE));
        assert!(app.confirm_clear_queue);
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
            !app.confirm_clear_queue,
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
        app.tab_idx = 1;
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
        let names: Vec<&str> = super::CONTEXT_STACK.iter().map(|e| e.name).collect();
        assert_eq!(
            names,
            vec![
                "save_modal",
                "save_playlist",
                "settings",
                "help",
                "sessions",
                "playlists",
                "global_overlay_open",
                "power_left_width",
                "home_search",
                "power_lib_search",
                "lib_search",
                "panel_toggle_h",
                "confirm_clear_queue",
                "confirm_rescan",
                "confirm_skip_intro",
                "confirm_next_up",
                "clear_queue_prompt_c",
                "context_menu",
                "playback",
                "ctrl_l_force_clear",
                "f5_refresh",
                "power_album_track_mode",
                "view_dispatch",
            ],
            "precedence order must match handle_key's pre-phase-2 branch order; \
             if this intentionally changes, update docs/adr/0002-centralized-input-handling.md too"
        );
    }
}
