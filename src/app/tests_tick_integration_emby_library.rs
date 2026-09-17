use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::emby_library_content::EmbyLibraryContent as BrowserOwner;
use crate::app::components::inline_search::InlineSearchHost;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::render::make_movie_app;

use crate::app::tests::{install_test_emby, make_item, make_session};
use std::time::{Duration, Instant};
use crate::app::tests_tick_harness::TickHarness;

/// The migrated Movies/HomeVideos/Generic owner inside the mounted
/// `LibraryPanel` (task 6.1): the panel is the library area's one event
/// boundary, and the browse state is read through the owner the panel hosts
/// — never a destination component (`emby_browser_id` stays `None` for
/// these kinds).
fn browser_owner(harness: &TickHarness) -> &BrowserOwner {
    let (_, key, _) = harness
        .model().active_emby_library_owner()
        .expect("the active library's owner has migrated");
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
        .owner(&key)
        .and_then(|owner| owner.as_any().downcast_ref::<BrowserOwner>())
        .expect("browser owner installed")
}

fn draw(harness: &mut TickHarness, width: u16, height: u16) -> Terminal<TestBackend> {
    harness.model_mut().app.terminal_width = width;
    harness.model_mut().app.terminal_height = height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    terminal
}

#[test]
fn browser_wide_tick_moves_control_without_recomputing_app_cursor() {
    let mut app = make_movie_app();
    app.tab = crate::app::TabSelection::EmbyLibrary(0);
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let _terminal = draw(&mut harness, 100, 30);

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::Shell(ShellRequest::EmbyLibraryCursorIndex { index: 1 }))
    }));
    assert_eq!(browser_owner(&harness).cursor(), 1);
    assert_eq!(harness.model().app.libs[0].nav_stack[0].resting().cursor(), 0);

    let _ = draw(&mut harness, 100, 30);
}

#[test]
fn inline_search_on_movies_library_receives_the_shell_pool_push() {
    let mut app = make_movie_app();
    app.tab = crate::app::TabSelection::EmbyLibrary(0);
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _terminal = draw(&mut harness, 100, 30);

    // `/` opens the embedded Inline Search through the shell, and the
    // shell's `OpenInlineSearch` host path resolves the Movies owner: the
    // nav-stack items are pushed as the search pool. (Regression: the
    // #695-era conversion collapsed the host path to the Music owner only,
    // so every non-Music library scored typed queries against an empty pool
    // and populated no results.)
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('/'),
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome
            .messages
            .iter()
            .any(|message| matches!(message, Msg::Shell(ShellRequest::OpenInlineSearch))),
        "\"/\" emits the shell open request: {:?}",
        outcome.messages
    );
    // Drain the step's shell requests like the run loop: the open request
    // loads/pushes the pool into the owner's session. The fetch is NOT one
    // of them: an open box with an empty query loads nothing and shows no
    // results (search starts with the first typed character).
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().active_inline_search_is_open());
    assert_eq!(
        browser_owner(&harness).inline_search().results_len(),
        0,
        "an open box with an empty query shows no results"
    );

    // A typed query arms the debounce; the first keystroke also asks the
    // shell to start the corpus load (deferred from open).
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('o'),
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome
            .messages
            .iter()
            .any(|message| matches!(
                message,
                Msg::Shell(ShellRequest::InlineSearchQueryStarted)
            )),
        "the first keystroke emits the corpus-load request: {:?}",
        outcome.messages
    );
    // Fire the debounce with a clock tick past the deadline: "o" matches
    // both rows of the two-item nav-stack level.
    harness
        .model_mut()
        .tick_inline_search_clock(Instant::now() + Duration::from_millis(301));
    let owner = browser_owner(&harness);
    assert_eq!(owner.inline_search().query(), "o");
    assert_eq!(
        owner.inline_search().results_len(),
        2,
        "typed query resolves rows from the shell-pushed pool"
    );

    // A flat browse completion under an open session (Enter on a folder
    // result drills in: select_item pushes a loading placeholder level and
    // the fetch completes asynchronously) re-pushes the pool at the
    // lib-event boundary: without the `Loaded` re-push the search kept a
    // stale empty pool and the list/hero painted blank. The query is
    // re-scored against the new level's rows.
    harness.model_mut().app.libs[0].nav_stack[0].loading = true;
    harness
        .model_mut()
        .handle_inline_search_lib_event(crate::app::LibEvent::Loaded {
            lib_idx: 0,
            parent_id: "lib-movies".into(),
            level: Box::new(crate::app::BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![
                    crate::app::tests::make_item("Anchor", "Movie"),
                    crate::app::tests::make_item("Another One", "Movie"),
                    crate::app::tests::make_item("Another Two", "Movie"),
                ],
                total_count: 3,
                resting: crate::app::types_browse::BrowseResting::new(0, 0),
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
                music_grouping: None,
            }),
        });
    let owner = browser_owner(&harness);
    assert_eq!(
        owner.inline_search().results_len(),
        3,
        "the browse completion re-pushes the pool; the query scores its rows"
    );
}

#[test]
fn browser_narrow_tick_click_uses_retained_geometry() {
    let mut app = make_movie_app();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    // At 60 columns the stored panel mode is ignored and the mini view
    // derives the visible panel from `mini_view_focus`; a narrow *library*
    // view must select Library here.
    app.mini_view_focus = crate::app::PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _terminal = draw(&mut harness, 60, 30);

    // The panel's own retained Narrow geometry is the painted truth: the
    // fixed-row selected rectangle resolves to the stable target through the
    // owner's carrier — never a stale-geometry row re-resolution.
    let geometry = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
        .test_narrow_geometry()
        .expect("the panel painted a Narrow skeleton");
    let area = geometry.selected.expect("selected row retained geometry");
    assert!(!area.is_empty(), "selected row retained geometry");
    let position = Position::new(area.x, area.y);
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: position.x,
        row: position.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ShellRequest::EmbyLibraryRowClick { target: Some(target) })
                if target == "movie-focused"
        )),
        "the painted fixed-row browser resolves to the selected row: {:?}", outcome.raw_messages
    );
    assert_eq!(browser_owner(&harness).cursor(), 0);
    assert_eq!(harness.model().app.libs[0].nav_stack[0].resting().cursor(), 0);
    let _ = draw(&mut harness, 60, 30);
}

/// The generic catalog's narrow surface routes through the shared owner. (The
/// test previously pinned the Grid presentation for this surface; Grid was
/// deleted as unreachable by design D13 — no library in use lacks a hero.)
#[test]
fn browser_generic_narrow_tick_isolated_from_canonical_controls() {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "other".into();
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw(&mut harness, 100, 30);
    assert_eq!(browser_owner(&harness).cursor(), 0);

    // Tick navigation moves the shared owner and echoes the resolved index.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::Shell(ShellRequest::EmbyLibraryCursorIndex { index: 1 }))
    }));
    assert_eq!(browser_owner(&harness).cursor(), 1);

    // The next frame retains the selection in the shared owner.
    let _ = draw(&mut harness, 100, 30);
    assert_eq!(browser_owner(&harness).cursor(), 1);
}

#[test]
fn tick_play_prompt_mounts_and_accepts_local_fall_through() {
    let mut app = make_movie_app();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    let http = mbv_core::mock_http::MockHttp::new();
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    http.respond(404, "");
    let (remote, player_rx, command_rx) =
        mbv_core::remote_player::RemotePlayer::stub_audio_only_with_command_rx(Vec::new(), 0);
    let session = make_session("audio-owner", "mbv");
    let endpoint = mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap());
    app.switch_to_direct_remote(&session, remote, player_rx, &endpoint);
    while command_rx.try_recv().is_ok() {}
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw(&mut harness, 100, 30);

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('p'),
        modifiers: KeyModifiers::CONTROL,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(
            message,
            Msg::Shell(ShellRequest::EmbyLibraryPlay { item })
                if item.id == "movie-focused"
        )
    }));
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages.iter().cloned() {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    let confirm_id = ComponentId::Modal(crate::app::components::ModalId::Confirm);
    assert!(harness.model().application.get_component(&confirm_id).is_some());
    assert_eq!(harness.model().application.focus(), Some(&confirm_id));
    assert!(command_rx.try_recv().is_err(), "play stayed unsubmitted");

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('y'),
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness.model().application.get_component(&confirm_id).is_none());
    assert!(!harness.model().app.player.is_remote());
    assert!(harness.model().app.player.status.lock().unwrap().active);
    assert!(!harness.model().app.direct_remote_connected);
    assert!(harness.model().app.remote_player_tab.is_none());
    assert_eq!(harness.model().app.queue_scope, crate::app::QueueScope::Local);
    let local_items = harness.model().app.player_tab.emby_items();
    assert_eq!(
        local_items.first().expect("local playback queue is non-empty").id,
        "movie-focused"
    );
    assert!(command_rx.try_iter().any(|command| {
        matches!(
            command,
            mbv_core::ctrl::CtrlCmd::PlaybackIntent(intent)
                if intent.action == mbv_core::ctrl::PlaybackIntentAction::Stop
        )
    }));
}


/// The viewport chords (task 6.1, design D6/D7): `Ctrl+e`/`Ctrl+y` step the
/// window one display row and `PgDn` pages it, at Wide and Narrow heights —
/// the height enters from the retained painted frame (design D2). A
/// window-only step emits no cursor echo (design D8): the chord is claimed
/// framework-locally and the reached position reports through the panel's
/// deferred resting-scroll update.
#[test]
fn browser_viewport_chords_step_the_window_at_wide_and_narrow_heights() {
    let mut app = make_movie_app();
    let mut items = app.libs[0].nav_stack[0].items.clone();
    for i in 2..30 {
        let mut item = make_item(&format!("Movie {i}"), "Movie");
        item.id = format!("movie-{i}");
        items.push(item);
    }
    app.libs[0].nav_stack[0].items = items;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.panel_focus = crate::app::PanelFocus::Library;

    for width in [100, 70] {
        let mut harness = {
            let mut app = make_movie_app();
            let mut items = app.libs[0].nav_stack[0].items.clone();
            for i in 2..30 {
                let mut item = make_item(&format!("Movie {i}"), "Movie");
                item.id = format!("movie-{i}");
                items.push(item);
            }
            app.libs[0].nav_stack[0].items = items;
            app.panel_mode = crate::app::PanelMode::LibraryOnly;
            app.panel_focus = crate::app::PanelFocus::Library;
            app.mini_view_focus = crate::app::PanelFocus::Library;
            TickHarness::new(app)
        };
        harness.model_mut().sync_mounted_surfaces();
        let _terminal = draw(&mut harness, width, 30);

        // Seed the selection mid-window so no chord below drags it.
        {
            let (_, key, _) = harness
                .model()
                .active_emby_library_owner()
                .expect("the active library's owner has migrated");
            harness
                .model_mut()
                .application
                .get_component_mut(&ComponentId::Library)
                .expect("Library panel mounted")
                .as_any_mut()
                .downcast_mut::<LibraryPanel>()
                .expect("Library panel type")
                .owner_mut(&key)
                .and_then(|owner| owner.as_any_mut().downcast_mut::<BrowserOwner>())
                .expect("browser owner installed")
                .set_cursor_for_test(10);
        }
        let _ = draw(&mut harness, width, 30);

        let before = browser_scroll(&harness);
        let before_cursor = browser_cursor(&harness);

        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Char('y'),
            modifiers: KeyModifiers::CONTROL,
        }));
        let outcome = harness.step();
        assert_eq!(
            browser_scroll(&harness),
            before + 1,
            "Ctrl+y steps the window one row at width {width}"
        );
        assert_eq!(
            browser_cursor(&harness),
            before_cursor,
            "the selection rides nowhere"
        );
        assert!(
            outcome
                .raw_messages
                .iter()
                .all(|message| !matches!(
                    message,
                    Msg::Shell(ShellRequest::EmbyLibraryCursorIndex { .. })
                )),
            "a window-only chord step emits no cursor echo"
        );

        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Char('e'),
            modifiers: KeyModifiers::CONTROL,
        }));
        let _ = harness.step();
        assert_eq!(browser_scroll(&harness), before, "Ctrl+e steps the window back");

        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::PageDown,
            modifiers: KeyModifiers::NONE,
        }));
        let _ = harness.step();
        let paged_scroll = browser_scroll(&harness);
        assert!(
            paged_scroll > before + 1,
            "PgDn pages the window at width {width}"
        );
        // The page may drag a left-behind selection; Invariant 6 lands it on
        // the leading edge of the page — the last selectable row the paged
        // window shows — and never leaves it outside. The height is the
        // painted list slot's, the same rectangle the step resolves from.
        let list_height = harness
            .model()
            .application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .and_then(|panel| {
                panel
                    .test_wide_geometry()
                    .or_else(|| panel.test_narrow_geometry())
            })
            .expect("the panel painted a list slot")
            .list_area
            .height as usize;
        let after_cursor = browser_cursor(&harness);
        assert!(
            after_cursor == before_cursor
                || after_cursor == (paged_scroll + list_height - 1).min(29),
            "the page lands a dragged selection on the paged window's last selectable row (29, the fixture's last) at width {width}"
        );
        assert!(
            after_cursor == before_cursor
                || (after_cursor >= paged_scroll && after_cursor < paged_scroll + list_height),
            "the page leaves the selection inside the paged window at width {width}"
        );
        let _ = draw(&mut harness, width, 30);
    }
}

fn browser_scroll(harness: &TickHarness) -> usize {
    let (_, key, _) = harness.model().active_emby_library_owner().expect("owner");
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .and_then(|panel| panel.owner(&key))
        .and_then(|owner| owner.as_any().downcast_ref::<BrowserOwner>())
        .expect("browser owner installed")
        .scroll()
}

fn browser_cursor(harness: &TickHarness) -> usize {
    let (_, key, _) = harness.model().active_emby_library_owner().expect("owner");
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .and_then(|panel| panel.owner(&key))
        .and_then(|owner| owner.as_any().downcast_ref::<BrowserOwner>())
        .expect("browser owner installed")
        .cursor()
}
