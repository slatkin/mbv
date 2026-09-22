use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::home_content::HomeContent;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::media_list::{LibrarySelectionOrigin, SelectionOrigin};
use crate::app::components::msg::HomeRowTarget;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::palette;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::tests_tick_harness::TickHarness;
use crate::app::home_latest::HomeLatestLaunchWindow;
use crate::app::types_playback::{HomeContent as ModelHomeContent, HomeLatestSection, HomeLatestSource};
use crate::app::{PanelFocus, PanelMode, TabSelection};
use mbv_core::playback_queue::QueueItem;

fn home_harness(width: u16, height: u16, count: usize) -> TickHarness {
    let mut app = make_app_stub();
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    app.terminal_width = width;
    app.terminal_height = height;
    let mut harness = TickHarness::new(app);
    harness.model_mut().home_content.continue_items = (0..count)
        .map(|i| {
            let mut item = make_item(&format!("Home Item {i}"), "Movie");
            item.id = format!("home-{i}");
            item
        })
        .collect();
    harness.model_mut().home_content.loading = false;
    harness.model_mut().push_home_content();
    harness.model_mut().sync_mounted_surfaces();
    harness
}

/// The Home content owner inside the mounted `LibraryPanel` (task 5.11):
/// the panel is the library area's one event boundary, and Home's state is
/// read through the owner the panel hosts — never a destination component.
fn home_owner(harness: &TickHarness) -> &HomeContent {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
        .owner(&crate::app::components::library_panel::LibraryKey::Home)
        .and_then(|owner| owner.as_any().downcast_ref::<HomeContent>())
        .expect("Home owner installed")
}

/// The mounted panel's retained region for one selector pill.
fn pill_region(harness: &TickHarness, id: usize) -> Rect {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
        .test_selector_hits()
        .regions()
        .iter()
        .find(|(_, target)| *target == id)
        .map(|(rect, _)| *rect)
        .unwrap_or_else(|| panic!("selector pill {id} painted"))
}

/// The mounted panel's selector state from its last content projection.
fn selector_markers(harness: &TickHarness) -> &[bool] {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
        .test_selector_markers()
}

/// The mounted Queue component, for seeding/reading its local selection.
fn queue_owner(harness: &TickHarness) -> Option<&crate::app::components::QueueComponent> {
    harness
        .model()
        .application
        .get_component(&ComponentId::Queue)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::QueueComponent>()
        })
}

/// The painted row cell for one row title (the panel's own paint is the
/// authoritative hit geometry; the deleted `HomeComponent::test_hitmap` is
/// replaced by buffer-text lookup on the panel output).
fn row_cell(terminal: &Terminal<TestBackend>, title: &str) -> (u16, u16) {
    let buf = terminal.backend().buffer();
    for y in 0..buf.area().height {
        for x in 0..buf.area().width {
            if buf[(x, y)].symbol() == &title[..1] {
                let row: String = (x.saturating_sub(2)..buf.area().width)
                    .map(|cx| buf[(cx, y)].symbol())
                    .collect();
                if row.contains(title) {
                    return (x, y);
                }
            }
        }
    }
    panic!("row text {title:?} not painted");
}

fn draw(harness: &mut TickHarness, width: u16, height: u16) -> Terminal<TestBackend> {
    harness.model_mut().app.terminal_width = width;
    harness.model_mut().app.terminal_height = height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            harness.model_mut().draw_frame(frame, false, false);
        })
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    terminal
}

fn assert_tick_frame_nonempty(mode: PanelMode, width: u16) {
    let mut harness = home_harness(width, 24, 1);
    harness.model_mut().app.panel_mode = mode;
    harness.inject(key(Key::Char('x')));
    harness.step();
    let terminal = draw(&mut harness, width, 24);
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.symbol() != " "),
        "panel mode {mode:?} produced an empty frame"
    );
}

#[test]
fn tick_frame_is_nonempty_in_both_mode() {
    assert_tick_frame_nonempty(PanelMode::Both, 140);
}

#[test]
fn tick_frame_is_nonempty_in_queue_only_mode() {
    assert_tick_frame_nonempty(PanelMode::QueueOnly, 140);
}

#[test]
fn tick_frame_is_nonempty_in_library_only_mode() {
    assert_tick_frame_nonempty(PanelMode::LibraryOnly, 140);
}

#[test]
fn tick_frame_is_nonempty_in_mini_view() {
    assert_tick_frame_nonempty(PanelMode::QueueOnly, 60);
}

/// Section content can arrive after the Home owner is mounted. The marker is
/// projected through the mounted Library panel, and visiting its pill records
/// a shell-owned acknowledgement when the shell pushes the same snapshot
/// again (the shell snapshot deliberately stays marked).
#[test]
fn home_latest_marker_arrival_and_pill_acknowledgement_flow_through_tick() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut harness = home_harness(160, 30, 0);
    let _ = draw(&mut harness, 160, 30);

    let marked_source = HomeLatestSource::Emby("marked-library".into());
    let mut item = make_item("New movie", "Movie");
    item.id = "new-movie".into();
    harness.model_mut().home_content.latest = vec![HomeLatestSection {
        title: "Latest Movies".into(),
        source: marked_source.clone(),
        items: vec![QueueItem::Emby(Box::new(item))],
        has_new_content: true,
    }];
    // This is the shell's content-push seam used by asynchronous provider
    // delivery; the following Application::tick drives the mounted tree.
    harness.model_mut().push_home_content();
    harness.inject(key(Key::Char('x')));
    let outcome = harness.step();
    handle_tick_messages(&mut harness, outcome.messages);

    let _ = draw(&mut harness, 160, 30);
    let marked_region = pill_region(&harness, 1);
    assert_eq!(selector_markers(&harness), &[false, true]);
    assert!(
        harness.model().home_content.latest[0].has_new_content,
        "the shell snapshot remains marked; acknowledgement is not mirrored"
    );

    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: marked_region.x,
        row: marked_region.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.messages.contains(&Msg::Shell(ShellRequest::HomePillClick {
        target: 1,
    })));
    handle_tick_messages(&mut harness, outcome.messages);
    let _ = draw(&mut harness, 160, 30);
    assert_eq!(selector_markers(&harness), &[false, false]);
    assert!(harness
        .model()
        .acknowledged_home_latest_sources
        .contains(&marked_source));

    let other_source = HomeLatestSource::Audiobookshelf("other-library".into());
    harness.model_mut().home_content.latest = vec![
        HomeLatestSection {
            title: "Other Latest".into(),
            source: other_source,
            items: Vec::new(),
            has_new_content: true,
        },
        HomeLatestSection {
            title: "Latest Movies".into(),
            source: marked_source,
            items: Vec::new(),
            has_new_content: true,
        },
    ];
    harness.model_mut().push_home_content();
    harness.inject(key(Key::Char('x')));
    let outcome = harness.step();
    handle_tick_messages(&mut harness, outcome.messages);
    let _ = draw(&mut harness, 160, 30);

    // Reorder the asynchronously refreshed sections. The selected source is
    // re-anchored by identity, and its marker remains absent in the mounted
    // projection; the other source is still marked before it is selected.
    assert_eq!(selector_markers(&harness), &[false, true, false]);

    // Select Other so the acknowledged source is unselected. Its missing marker
    // then proves source-based acknowledgement rather than selected-pill
    // suppression.
    let other_region = pill_region(&harness, 1);
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: other_region.x,
        row: other_region.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.messages.contains(&Msg::Shell(ShellRequest::HomePillClick {
        target: 1,
    })));
    handle_tick_messages(&mut harness, outcome.messages);
    let _ = draw(&mut harness, 160, 30);
    assert_eq!(selector_markers(&harness), &[false, false, false]);
    assert_eq!(home_owner(&harness).section(), 1);
    assert!(harness.model().home_content.latest[1].has_new_content);
}

#[test]
fn home_latest_fetch_after_launch_does_not_add_a_marker() {
    let mut app = make_app_stub();
    app.home_latest_launch_window = HomeLatestLaunchWindow {
        previous: Some(100),
        current: 200,
    };
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    let mut item = make_item("Fetched after launch", "Movie");
    item.date_added = "1970-01-01T00:05:00Z".into();
    harness.model_mut().assign_home_content(ModelHomeContent {
        continue_items: Vec::new(),
        latest: vec![HomeLatestSection {
            title: "Latest".into(),
            source: HomeLatestSource::Emby("library".into()),
            items: vec![QueueItem::Emby(Box::new(item))],
            has_new_content: true,
        }],
        loading: false,
        feed_names: std::collections::HashMap::new(),
    });

    assert!(!harness.model().home_content.latest[0].has_new_content);
}

fn key(code: Key) -> Event<crate::app::components::UserEvent> {
    key_with_modifiers(code, KeyModifiers::NONE)
}

fn key_with_modifiers(
    code: Key,
    modifiers: KeyModifiers,
) -> Event<crate::app::components::UserEvent> {
    Event::Keyboard(KeyEvent { code, modifiers })
}

fn handle_tick_messages(harness: &mut TickHarness, messages: Vec<Msg>) {
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
}

fn wheel(column: u16, row: u16) -> Event<crate::app::components::UserEvent> {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn visual_mode_space_toggles_selection_without_playback() {
    let mut harness = home_harness(160, 30, 2);
    let _ = draw(&mut harness, 160, 30);

    harness.inject(key_with_modifiers(Key::Char('v'), KeyModifiers::SHIFT));
    let outcome = harness.step();
    assert!(outcome.messages.iter().all(|message| !matches!(
        message,
        Msg::Playback(crate::app::components::PlaybackRequest::TogglePlayPause)
    )));
    handle_tick_messages(&mut harness, outcome.messages);
    assert_eq!(home_owner(&harness).test_multi_selection_len(), 1);

    harness.inject(key(Key::Char(' ')));
    let outcome = harness.step();
    assert!(outcome.messages.iter().all(|message| !matches!(
        message,
        Msg::Playback(crate::app::components::PlaybackRequest::TogglePlayPause)
    )));
    // The consumed chord also cancels the shell's deferred candidate: the
    // playback action must not fire behind the row-local toggle.
    assert!(!outcome.deferred_fired);
    handle_tick_messages(&mut harness, outcome.messages);
    assert_eq!(home_owner(&harness).test_multi_selection_len(), 0);
}

#[test]
fn visual_mode_status_bar_click_clears_selection_through_tick() {
    let mut harness = home_harness(160, 30, 2);
    let _ = draw(&mut harness, 160, 30);

    harness.inject(key_with_modifiers(Key::Char('v'), KeyModifiers::SHIFT));
    let outcome = harness.step();
    handle_tick_messages(&mut harness, outcome.messages);
    assert!(harness.model().visual_selection.is_some());
    assert_eq!(home_owner(&harness).test_multi_selection_len(), 1);

    harness.inject(key(Key::Down));
    let outcome = harness.step();
    handle_tick_messages(&mut harness, outcome.messages);
    assert_eq!(home_owner(&harness).test_multi_selection_len(), 2);

    let _ = draw(&mut harness, 160, 30);
    let clear = harness
        .model()
        .application
        .get_component(&ComponentId::StatusBarPanel)
        .expect("status bar mounted")
        .as_any()
        .downcast_ref::<crate::app::components::StatusBarPanel>()
        .expect("status bar component")
        .regions()
        .visual_clear
        .expect("visual clear region painted");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: clear.x,
        row: clear.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.messages.contains(&Msg::Shell(ShellRequest::ClearMultiSelection(
            SelectionOrigin::Library(LibrarySelectionOrigin::Home),
        ))),
        "clear request must reach the shell: {:?}",
        outcome.messages
    );
    handle_tick_messages(&mut harness, outcome.messages);
    assert!(harness.model().visual_selection.is_none());
    assert_eq!(home_owner(&harness).test_multi_selection_len(), 0);
}

/// P1 regression guard: the Home route never reaches the shell's generic
/// `RowContextMenu` arm, so its bulk menu must capture the Home origin itself
/// or the bulk action leaves the multi-selection (and Visual mode) armed.
#[test]
fn home_bulk_context_action_clears_home_multi_selection() {
    let mut harness = home_harness(160, 30, 3);
    let _ = draw(&mut harness, 160, 30);

    harness.inject(key_with_modifiers(Key::Char('v'), KeyModifiers::SHIFT));
    let outcome = harness.step();
    handle_tick_messages(&mut harness, outcome.messages);
    harness.inject(key(Key::Down));
    let outcome = harness.step();
    handle_tick_messages(&mut harness, outcome.messages);
    assert_eq!(home_owner(&harness).test_multi_selection_len(), 2);

    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Home(vec![
                HomeRowTarget {
                    item_id: Some("home-0".into()),
                    source: None,
                    from_continue_watching: true,
                },
                HomeRowTarget {
                    item_id: Some("home-1".into()),
                    source: None,
                    from_continue_watching: true,
                },
            ]),
            None,
        )),
        &mut false,
        &mut false,
    );
    let idx = {
        let Some(crate::app::types_overlay::OverlayRequest::ContextMenu(ref menu)) =
            harness.model().app.pending_overlay
        else {
            panic!("Home bulk selection must open a context menu");
        };
        menu.entries
            .iter()
            .position(|entry| {
                matches!(
                    entry.action,
                    Some(crate::app::ContextAction::EnqueueSelection(_))
                )
            })
            .expect("bulk enqueue entry")
    };
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().handle_context_menu_select(idx);

    assert_eq!(
        home_owner(&harness).test_multi_selection_len(),
        0,
        "a Home bulk context action must clear the Home multi-selection"
    );
}

/// The clear intent routes by the origin it carries, never by the panel that
/// happens to hold focus when it is dispatched (design D6 contract 4).
#[test]
fn clear_multi_selection_routes_by_origin_not_dispatch_focus() {
    let mut harness = home_harness(160, 30, 2);
    let _ = draw(&mut harness, 160, 30);

    harness.inject(key_with_modifiers(Key::Char('v'), KeyModifiers::SHIFT));
    let outcome = harness.step();
    handle_tick_messages(&mut harness, outcome.messages);
    harness.inject(key(Key::Down));
    let outcome = harness.step();
    handle_tick_messages(&mut harness, outcome.messages);
    assert_eq!(home_owner(&harness).test_multi_selection_len(), 2);

    // Give the Queue its own selection, then focus it: dispatch-time focus now
    // names the Queue while the captured origin still names the Library.
    harness.model_mut().app.player_tab.set_queue_items(
        vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item(
            "Queued",
            "Movie",
        )))],
        0,
    );
    harness.model_mut().sync_mounted_surfaces();
    let slot0 = queue_owner(&harness)
        .and_then(|queue| queue.test_selected_target())
        .expect("queue row selected");
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Queue)
        .and_then(|component| {
            component
                .as_any_mut()
                .downcast_mut::<crate::app::components::QueueComponent>()
        })
        .expect("queue mounted")
        .test_toggle_selection(slot0);
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.model_mut().app.mini_view_focus = PanelFocus::Queue;
    assert_eq!(
        queue_owner(&harness).map(|queue| queue.test_multi_selection().len()),
        Some(1),
        "the Queue must hold dispatch-time focus with its own selection"
    );

    // The captured origin still names the Library, so the clear must route
    // there rather than to the focused Queue.
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::ClearMultiSelection(
            SelectionOrigin::Library(LibrarySelectionOrigin::Home),
        )),
        &mut false,
        &mut false,
    );
    assert_eq!(
        home_owner(&harness).test_multi_selection_len(),
        0,
        "the named origin's list must clear even while another panel is focused"
    );
    assert_eq!(
        queue_owner(&harness).map(|queue| queue.test_multi_selection().len()),
        Some(1),
        "the dispatch-focused list must be left untouched"
    );
}

/// The Queue-focused half of the same contract: the pill projected from the
/// Queue summary clears the Queue.
#[test]
fn status_bar_clear_with_queue_origin_clears_queue() {
    let mut harness = home_harness(160, 30, 1);
    let _ = draw(&mut harness, 160, 30);

    harness.model_mut().app.player_tab.set_queue_items(
        vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item(
            "Queued",
            "Movie",
        )))],
        0,
    );
    harness.model_mut().sync_mounted_surfaces();
    let slot0 = queue_owner(&harness)
        .and_then(|queue| queue.test_selected_target())
        .expect("queue row selected");
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Queue)
        .and_then(|component| {
            component
                .as_any_mut()
                .downcast_mut::<crate::app::components::QueueComponent>()
        })
        .expect("queue mounted")
        .test_toggle_selection(slot0);
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.model_mut().app.mini_view_focus = PanelFocus::Queue;
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw(&mut harness, 160, 30);
    assert!(
        harness
            .model()
            .visual_selection
            .is_some_and(|(focus, count)| focus == PanelFocus::Queue && count == 1),
        "the Queue summary must project onto the pill"
    );

    let clear = harness
        .model()
        .application
        .get_component(&ComponentId::StatusBarPanel)
        .expect("status bar mounted")
        .as_any()
        .downcast_ref::<crate::app::components::StatusBarPanel>()
        .expect("status bar component")
        .regions()
        .visual_clear
        .expect("visual clear region painted");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: clear.x,
        row: clear.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.messages.contains(&Msg::Shell(ShellRequest::ClearMultiSelection(
            SelectionOrigin::Queue,
        ))),
        "clear request must carry the projected Queue origin: {:?}",
        outcome.messages
    );
    handle_tick_messages(&mut harness, outcome.messages);
    assert_eq!(
        queue_owner(&harness).map(|queue| queue.test_multi_selection().len()),
        Some(0),
        "the Queue summary's clear must clear the Queue"
    );
}

#[test]
fn visual_mode_escape_clears_without_firing_playback_stop() {
    let mut harness = home_harness(160, 30, 2);
    let _ = draw(&mut harness, 160, 30);

    harness.inject(key_with_modifiers(Key::Char('v'), KeyModifiers::SHIFT));
    let outcome = harness.step();
    handle_tick_messages(&mut harness, outcome.messages);
    assert!(!outcome.deferred_fired);

    harness.inject(key(Key::Esc));
    let outcome = harness.step();
    assert!(outcome.messages.iter().all(|message| !matches!(
        message,
        Msg::Playback(crate::app::components::PlaybackRequest::Stop)
    )));
    handle_tick_messages(&mut harness, outcome.messages);
    assert_eq!(home_owner(&harness).test_multi_selection_len(), 0);
    // The consumed press (Visual clear) cancels the stop candidate without
    // recording any timing state.
    assert!(!outcome.deferred_fired);

    // Once Visual mode has exited, the next Esc is consumed by the library
    // back/dismiss leaf again: the stop candidate stays cancelled.
    harness.inject(key(Key::Esc));
    let outcome = harness.step();
    assert!(outcome.messages.iter().all(|message| !matches!(
        message,
        Msg::Playback(crate::app::components::PlaybackRequest::Stop)
    )));
    assert!(!outcome.deferred_fired);
}

#[test]
fn home_wide_tick_navigation_keeps_the_selected_owner_row() {
    let mut harness = home_harness(160, 30, 8);
    let _ = draw(&mut harness, 160, 30);
    harness.inject(key(Key::Down));
    let outcome = harness.step();
    assert!(outcome.messages.iter().all(|message| !matches!(
        message,
        Msg::TerminalEvent(crate::app::components::TerminalObserverEvent::KeyClaimed)
    )), "the fold consumes the local claim marker");

    let _ = draw(&mut harness, 160, 30);
    assert_eq!(home_owner(&harness).cursor(), 1);
}

#[test]
fn home_narrow_tick_wheel_and_click_use_current_inline_geometry() {
    let mut harness = home_harness(60, 20, 8);
    let _ = draw(&mut harness, 60, 20);
    let (x, y) = row_cell(&draw(&mut harness, 60, 20), "Home Item 0");
    harness.inject(wheel(x, y));
    let _outcome = harness.step();
    assert_eq!(home_owner(&harness).cursor(), 1);
    let _ = draw(&mut harness, 60, 20);

    let (target_x, target_y) = row_cell(&draw(&mut harness, 60, 20), "Home Item 2");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: target_x,
        row: target_y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome
        .messages
        .iter()
        .any(|message| matches!(message, Msg::Shell(ShellRequest::HomeRowClick { target: _ }))));
    assert_eq!(home_owner(&harness).cursor(), 2);
    let _ = draw(&mut harness, 60, 20);
    assert_eq!(home_owner(&harness).cursor(), 2);
}

#[test]
fn home_tick_refresh_preserves_target_and_breakpoint_handoff_preserves_offset() {
    let mut harness = home_harness(160, 30, 40);
    let _ = draw(&mut harness, 160, 30);
    for _ in 0..15 {
        harness.inject(key(Key::Down));
        harness.step();
    }
    let _ = draw(&mut harness, 160, 30);
    assert_eq!(home_owner(&harness).cursor(), 15);
    harness.model_mut().home_content.continue_items.swap(0, 15);
    harness.model_mut().push_home_content();
    assert_eq!(
        home_owner(&harness).cursor(),
        0,
        "flat cursor follows stable id after reorder"
    );

    // Move to a distant row, then flip presentation. The owner's single
    // retained anchor transfers target and screen-row offset (the panel
    // drives the presentation through `set_presentation`).
    harness.inject(key(Key::End));
    harness.step();
    let _ = draw(&mut harness, 160, 30);
    let _ = draw(&mut harness, 60, 12);
    assert_eq!(home_owner(&harness).cursor(), 39);
    assert!(home_owner(&harness).test_active_scroll() > 0);
}

/// Task 5.11: local navigation through the panel's keyboard forwarding keeps
/// the owner's cursor local — a content push preserves it (no App mirror).
#[test]
fn home_owner_cursor_survives_a_content_push() {
    let mut harness = home_harness(160, 30, 2);
    let _ = draw(&mut harness, 160, 30);
    harness.inject(key(Key::Down));
    harness.step();
    assert_eq!(home_owner(&harness).cursor(), 1);
    harness.model_mut().push_home_content();
    assert_eq!(
        home_owner(&harness).cursor(),
        1,
        "the owner's cursor survives the content projection"
    );
}

/// The list-row cells for Home's two fixture rows: the split row's
/// `(context_x, title_x, y)` and the single-part row's `(x, y)` on the line
/// directly below it (the list's fixed-row flow, no `Heading`/`Spacer`
/// between). The cursor-following Hero can paint either title on its own
/// line, so only the adjacent pair identifies the list rows.
fn home_list_row_cells(
    terminal: &Terminal<TestBackend>,
    context: &str,
    split_title: &str,
    single_title: &str,
) -> ((u16, u16, u16), (u16, u16)) {
    let buf = terminal.backend().buffer();
    let whole = format!("{context} {split_title}");
    let row_text = |y: u16| -> String {
        (0..buf.area().width).map(|x| buf[(x, y)].symbol()).collect()
    };
    for y in 0..buf.area().height.saturating_sub(1) {
        let row = row_text(y);
        let Some(at) = row.find(&whole) else {
            continue;
        };
        if let Some(single_at) = row_text(y + 1).find(single_title) {
            return (
                (at as u16, (at + context.len() + 1) as u16, y),
                (single_at as u16, y + 1),
            );
        }
    }
    panic!(
        "adjacent list rows for split {whole:?} and single-part {single_title:?} not painted"
    );
}

fn assert_home_palette_painted(
    terminal: &Terminal<TestBackend>,
    label: &str,
    single_fg: ratatui::style::Color,
) {
    use crate::app::palette;

    let buf = terminal.backend().buffer();
    // A split row (episode → series context + item title): the context name
    // paints the split-row context role and the item title the split-row
    // title role.
    let ((ctx_x, title_x, row_y), (single_x, single_y)) =
        home_list_row_cells(terminal, "Severance", "Broken Bird", "The Long Goodbye");
    assert_eq!(
        buf[(ctx_x, row_y)].fg,
        palette::SPLIT_ROW_CONTEXT_FG,
        "{label}: split-row context must paint the split-row context role"
    );
    assert_eq!(
        buf[(title_x, row_y)].fg,
        palette::SPLIT_ROW_TITLE_FG,
        "{label}: split-row item title must paint the split-row title role"
    );
    // A single-part row (movie, no container) uses its ordinary role when
    // unselected and the Ink role when it owns the selected bar.
    assert_eq!(
        buf[(single_x, single_y)].fg,
        single_fg,
        "{label}: single-part row foreground must match its selection state"
    );
}

fn assert_home_single_part_role(
    terminal: &Terminal<TestBackend>,
    label: &str,
    expected: ratatui::style::Color,
) {
    let (_, (single_x, single_y)) =
        home_list_row_cells(terminal, "Severance", "Broken Bird", "The Long Goodbye");
    assert_eq!(
        terminal.backend().buffer()[(single_x, single_y)].fg,
        expected,
        "{label}: unselected single-part row must keep its ordinary title role"
    );
}

/// Task 4.2: a Home section render through the shell sync pass (a real tick
/// followed by the panel's own `draw_frame` placement) paints the now-playing
/// two-tone palette on split rows and the ordinary role on single-part rows,
/// in both the Wide and Narrow presentations.
#[test]
fn home_tick_render_paints_split_row_palette_and_ordinary_single_part_role() {
    let mut harness = home_harness(160, 30, 0);
    let mut episode = make_item("Broken Bird", "Episode");
    episode.id = "home-ep".into();
    episode.series_name = "Severance".into();
    let mut movie = make_item("The Long Goodbye", "Movie");
    movie.id = "home-movie".into();
    harness.model_mut().home_content.continue_items = vec![episode, movie];
    harness.model_mut().home_content.loading = false;
    harness.model_mut().push_home_content();
    harness.model_mut().sync_mounted_surfaces();

    // A real tick through the shell sync pass moves the cursor onto the
    // single-part row before the frame is drawn.
    harness.inject(key(Key::Down));
    let outcome = harness.step();
    handle_tick_messages(&mut harness, outcome.messages);

    let wide = draw(&mut harness, 160, 30);
    assert_home_palette_painted(&wide, "Wide", palette::SELECTED_ROW_FG);

    // The Narrow presentation reuses the same owner through the panel; the
    // selected row keeps the Ink foreground across the geometry transition.
    let narrow = draw(&mut harness, 60, 20);
    assert_home_palette_painted(&narrow, "Narrow", palette::SELECTED_ROW_FG);

    // Move the bar back to the split row and pin the adjacent ordinary row's
    // unselected role: it must never inherit the selected-row foreground.
    harness.inject(key(Key::Up));
    let outcome = harness.step();
    handle_tick_messages(&mut harness, outcome.messages);
    let unselected = draw(&mut harness, 160, 30);
    assert_home_single_part_role(&unselected, "Wide", palette::TEXT_EMPHASIS);
}

