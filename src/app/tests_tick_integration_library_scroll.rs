//! Live-tick coverage for LibraryPanel's resolved scroll request.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::render::make_movie_app;
use crate::app::tests::make_item;
use crate::app::tests_tick_harness::{StepOutcome, TickHarness};
use crate::app::{PanelFocus, PanelMode};

fn apply(harness: &mut TickHarness, outcome: StepOutcome) {
    let (mut music, mut tv) = (false, false);
    for message in outcome.messages {
        harness.model_mut().handle_terminal_message(message, &mut music, &mut tv);
    }
    harness
        .model_mut()
        .drain_deferred_library_message(&mut music, &mut tv);
    harness.model_mut().sync_mounted_surfaces();
}

#[test]
fn deferred_wheel_is_drained_before_following_keyboard_cursor_move() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();

    let list = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .and_then(|panel| panel.test_wide_geometry())
        .expect("the panel paints a Wide list")
        .list_area;
    // Simulate a claimed wheel whose owner returned no message: the deferred
    // scroll still must be consumed before the next primary dispatch.
    let _ = harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .expect("Library panel mounted")
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: list.x + 1,
            row: list.y,
            modifiers: KeyModifiers::NONE,
        }));
    let (mut music, mut tv) = (false, false);
    harness
        .model_mut()
        .drain_deferred_library_message(&mut music, &mut tv);
    let end = harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .expect("Library panel mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::End,
            modifiers: KeyModifiers::NONE,
        }))
        .expect("End emits the cursor echo");
    harness
        .model_mut()
        .handle_terminal_message(end, &mut music, &mut tv);
    let resting = harness.model().app.libs[0].nav_stack[0].resting();
    assert_eq!(resting.cursor(), 1, "keyboard position must win over stale wheel state");
}

#[test]
fn library_panel_viewport_wheel_reports_position_without_a_cursor_echo() {
    // Task 5.2 (design D8): the wheel is the viewport step. A window-only
    // step emits no `EmbyLibraryCursorIndex` echo, the reached position still
    // persists through the panel's deferred `LibraryScroll`, and pagination
    // fires from the position the window reached. The pending-fetch guard
    // (`BrowseLevel::loading`) is what keeps repeated reports at the loaded
    // end to one in-flight fetch.
    let mut app = make_movie_app();
    let mut items = app.libs[0].nav_stack[0].items.clone();
    for i in 2..30 {
        let mut item = make_item(&format!("Movie {i}"), "Movie");
        item.id = format!("movie-{i}");
        items.push(item);
    }
    app.libs[0].nav_stack[0].items = items;
    // Not fully loaded: pagination must still have work to do at the reach.
    app.libs[0].nav_stack[0].total_count = 90;
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();

    // Seed the selection mid-window so the wheel below cannot drag it.
    let owner_key = harness
        .model()
        .active_emby_library_owner()
        .map(|(_, key, _)| key)
        .expect("the Movies owner has migrated");
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        .and_then(|panel| panel.owner_mut(&owner_key))
        .and_then(|owner| owner.as_any_mut().downcast_mut::<crate::app::components::emby_library_content::EmbyLibraryContent>())
        .expect("browser owner installed")
        .set_cursor_for_test(10);

    let list = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .and_then(|panel| panel.test_wide_geometry())
        .expect("the panel paints a Wide list")
        .list_area;
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: list.x + 1,
        row: list.y,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome
            .raw_messages
            .iter()
            .all(|message| !matches!(
                message,
                Msg::Shell(ShellRequest::EmbyLibraryCursorIndex { .. })
            )),
        "a window-only wheel step emits no cursor echo"
    );
    let reached = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .and_then(|panel| {
            panel
                .owner(&owner_key)
                .and_then(|owner| owner.as_any().downcast_ref::<crate::app::components::emby_library_content::EmbyLibraryContent>())
        })
        .map(|owner| owner.scroll())
        .expect("browser owner installed");
    assert_eq!(
        reached, 1,
        "the wheel stepped the window one display row from the displayed top"
    );
    apply(&mut harness, outcome);
    let level = &harness.model().app.libs[0].nav_stack[0];
    assert!(
        level.loading,
        "the position report still feeds pagination at the loaded edge"
    );
    assert_eq!(level.items.len(), 30);
    assert!(!level.is_fully_loaded());

    // A further cursor report at the loaded end cannot start a second fetch:
    // the pending-fetch guard holds the line until the first one resolves.
    let down = harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        .and_then(|panel| panel.owner_mut(&owner_key))
        .and_then(|owner| owner.on_key(&tuirealm::event::KeyEvent {
            code: tuirealm::event::Key::Down,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        }));
    harness
        .model_mut()
        .handle_terminal_message(
            down.expect("the keyboard move echoes its resolved index"),
            &mut false,
            &mut false,
        );
    let level = &harness.model().app.libs[0].nav_stack[0];
    assert!(level.loading, "the pending fetch is still the one fetch");
    assert_eq!(level.items.len(), 30);
}

/// Task 6.1 (design D8): a keyboard viewport chord behaves like the wheel —
/// a window-only `Ctrl+y` step emits no `EmbyLibraryCursorIndex` echo, the
/// reached position still persists through the panel's deferred
/// `LibraryScroll`, and pagination fires from the position the window
/// reached.
#[test]
fn library_panel_viewport_chord_reports_position_without_a_cursor_echo() {
    let mut app = make_movie_app();
    let mut items = app.libs[0].nav_stack[0].items.clone();
    for i in 2..30 {
        let mut item = make_item(&format!("Movie {i}"), "Movie");
        item.id = format!("movie-{i}");
        items.push(item);
    }
    app.libs[0].nav_stack[0].items = items;
    // Not fully loaded: pagination must still have work to do at the reach.
    app.libs[0].nav_stack[0].total_count = 90;
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();

    // Seed the selection mid-window so the chord below cannot drag it.
    let owner_key = harness
        .model()
        .active_emby_library_owner()
        .map(|(_, key, _)| key)
        .expect("the Movies owner has migrated");
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        .and_then(|panel| panel.owner_mut(&owner_key))
        .and_then(|owner| {
            owner
                .as_any_mut()
                .downcast_mut::<crate::app::components::emby_library_content::EmbyLibraryContent>()
        })
        .expect("browser owner installed")
        .set_cursor_for_test(5);
    let _ = terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('y'),
        modifiers: KeyModifiers::CONTROL,
    }));
    let outcome = harness.step();
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
    apply(&mut harness, outcome);
    let level = &harness.model().app.libs[0].nav_stack[0];
    assert!(
        level.loading,
        "the chord's position report still feeds pagination at the loaded edge"
    );
    assert_eq!(harness.model().app.libs[0].nav_stack[0].items.len(), 30);
}
