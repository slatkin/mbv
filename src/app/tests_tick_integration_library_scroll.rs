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

// ── Task 8.1: live-tick evidence for the viewport step ─────────────────────
//
// The Emby browser's letter-grouped flow is the grouped surface: with a
// total at or above 50 the owner projects `Heading`/`Spacer`/`Item` rows, so
// the first display row is the first group's `Heading` and every step composes
// against non-selectable rows. Both inputs — the wheel and the one-row
// viewport chord — are driven through the real `Application::tick()` at the
// Wide and the non-Wide (Narrow) breakpoint.

const GROUPS: usize = 8;
const GROUP_ITEMS: usize = 10;
/// Group `g` paints heading + items + spacer (except the last group: no
/// trailing spacer), so item `i` sits at display row `12*(i/10) + 1 + i%10`.
const FIXTURE_ITEMS: usize = GROUPS * GROUP_ITEMS;
const FIRST_HEADING: &str = "A\u{2013}C";
const PREFIXES: [&str; GROUPS] = [
    "Apple", "Delta", "Golf", "Juliet", "Mike", "Papa", "Sierra", "Victor",
];

fn display_row(i: usize) -> usize {
    12 * (i / GROUP_ITEMS) + 1 + (i % GROUP_ITEMS)
}

fn grouped_app() -> crate::app::App {
    let mut app = make_movie_app();
    let items = (0..FIXTURE_ITEMS)
        .map(|i| {
            let mut item = make_item(&format!("{} {i:02}", PREFIXES[i / GROUP_ITEMS]), "Movie");
            item.id = format!("item-{i}");
            item
        })
        .collect();
    app.libs[0].nav_stack[0].items = items;
    // Fully loaded: pagination has no work to do, so every position report
    // below stays a pure persistence write.
    app.libs[0].nav_stack[0].total_count = FIXTURE_ITEMS;
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app
}

fn seeded_harness(width: u16, height: u16) -> (TickHarness, Terminal<TestBackend>, ratatui::layout::Rect) {
    let mut harness = TickHarness::new(grouped_app());
    harness.model_mut().app.mini_view_focus = PanelFocus::Library;
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().app.terminal_width = width;
    harness.model_mut().app.terminal_height = height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();

    // Seed the selection deep in the flow so the window is parked below and
    // every upward step has work to do.
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
        .set_cursor_for_test(50);
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();

    let list = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .and_then(|panel| {
            panel
                .test_wide_geometry()
                .or_else(|| panel.test_narrow_geometry())
        })
        .map(|geometry| geometry.list_area)
        .expect("the panel painted a list skeleton");
    (harness, terminal, list)
}

fn owner(harness: &TickHarness) -> &crate::app::components::emby_library_content::EmbyLibraryContent {
    let (_, key, _) = harness
        .model()
        .active_emby_library_owner()
        .expect("the Movies owner has migrated");
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .and_then(|panel| panel.owner(&key))
        .and_then(|owner| {
            owner
                .as_any()
                .downcast_ref::<crate::app::components::emby_library_content::EmbyLibraryContent>()
        })
        .expect("browser owner installed")
}

fn wheel_event(
    kind: MouseEventKind,
    list: ratatui::layout::Rect,
) -> Event<crate::app::components::UserEvent> {
    Event::Mouse(MouseEvent {
        kind,
        column: list.x + 2,
        row: list.y + 2,
        modifiers: KeyModifiers::NONE,
    })
}

fn step_input(harness: &mut TickHarness, wheel: bool, up: bool, list: ratatui::layout::Rect) {
    if wheel {
        // The 30 ms burst throttle collapses a test's back-to-back wheels;
        // the reset seam stands in for the wall-clock gap without sleeping.
        harness
            .model_mut()
            .application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
            .expect("Library panel mounted")
            .test_reset_wheel_throttle();
        let kind = if up { MouseEventKind::ScrollUp } else { MouseEventKind::ScrollDown };
        harness.inject(wheel_event(kind, list));
    } else {
        harness.inject(Event::Keyboard(KeyEvent {
            // The owner's chord arms (U6): `Ctrl+e` steps the window toward
            // the preceding row, `Ctrl+y` toward the following one.
            code: if up { Key::Char('e') } else { Key::Char('y') },
            modifiers: KeyModifiers::CONTROL,
        }));
    }
    let outcome = harness.step();
    apply(harness, outcome);
}

/// Task 8.1 (design D1/D2/D3): through the real tick, a wheel step and the
/// one-row viewport chord each move the letter-grouped window one display
/// row, drag the selection only on the step that would leave it outside, and
/// reach display row 0 — where the first group's `Heading` paints. Verified
/// at the Wide and the non-Wide breakpoint; the height enters from the
/// retained painted frame in both.
#[test]
fn viewport_step_inputs_walk_a_grouped_list_to_display_row_0_wide_and_narrow() {
    for (width, wheel) in [(120u16, true), (120, false), (70, true), (70, false)] {
        let (mut harness, mut terminal, list) = seeded_harness(width, 30);
        let height = list.height as usize;
        let cursor_of = |harness: &TickHarness| owner(harness).cursor();
        let scroll_of = |harness: &TickHarness| owner(harness).scroll();

        // A window-only step down: the window moves one display row, the
        // selection rides nowhere, and no cursor echo is emitted (design D8).
        let before_cursor = cursor_of(&harness);
        assert_eq!(scroll_of(&harness), 0, "test setup at width {width}");
        step_input(&mut harness, wheel, false, list);
        assert_eq!(
            scroll_of(&harness),
            display_row(50) + 2 - height,
            "the step moved the window one row from the displayed base at width {width}"
        );
        assert_eq!(cursor_of(&harness), before_cursor, "a window-only step drags nothing");

        // Step up to the top: the window decrements one row per step and the
        // selection is dragged exactly on the step that would leave it below
        // the window — never before.
        loop {
            let row = display_row(cursor_of(&harness));
            let before_cursor = cursor_of(&harness);
            let before_scroll = scroll_of(&harness);
            if before_scroll == 0 {
                break;
            }
            step_input(&mut harness, wheel, true, list);
            let after_scroll = scroll_of(&harness);
            assert_eq!(after_scroll, before_scroll - 1, "one row per step at width {width}");
            if row >= after_scroll && row < after_scroll + height {
                assert_eq!(
                    cursor_of(&harness),
                    before_cursor,
                    "the selection rides nowhere while it stays visible at width {width}"
                );
            } else {
                assert_ne!(
                    cursor_of(&harness),
                    before_cursor,
                    "the edge step drags the selection at width {width}"
                );
                let dragged = display_row(cursor_of(&harness));
                assert!(
                    dragged >= after_scroll && dragged < after_scroll + height,
                    "the dragged selection stays inside the window at width {width}"
                );
            }
        }
        assert_eq!(scroll_of(&harness), 0, "the window reached display row 0");

        // A step at the content end is a clamp: nothing moves.
        let before_cursor = cursor_of(&harness);
        step_input(&mut harness, wheel, true, list);
        assert_eq!(scroll_of(&harness), 0);
        assert_eq!(cursor_of(&harness), before_cursor);

        // At row 0 the first group's `Heading` is the list's first painted
        // row, and the dragged selection is still visible inside the window.
        terminal
            .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
            .unwrap();
        let buf = terminal.backend().buffer();
        let top: String = (list.x..list.x + list.width)
            .map(|x| buf[(x, list.y)].symbol().to_string())
            .collect();
        assert!(
            top.contains(FIRST_HEADING),
            "the first group's Heading paints at display row 0 at width {width}: {top:?}"
        );
        let selected_name = harness.model().app.libs[0].nav_stack[0].items
            [owner(&harness).cursor()]
        .name
        .clone();
        let painted: Vec<String> = (list.y..list.y + list.height)
            .map(|y| {
                (list.x..list.x + list.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect()
            })
            .collect();
        assert!(
            painted.iter().any(|row| row.contains(&selected_name)),
            "the selection stays visible inside the reached window at width {width}"
        );
    }
}
