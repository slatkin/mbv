//! Live-tick coverage for LibraryPanel's resolved scroll request.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::render::make_movie_app;
use crate::app::tests::tick_integration::harness::{StepOutcome, TickHarness};
use crate::app::{PanelFocus, PanelMode};

fn apply(harness: &mut TickHarness, outcome: StepOutcome) {
    let (mut music, mut tv) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music, &mut tv);
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
    assert_eq!(
        resting.cursor(),
        1,
        "keyboard position must win over stale wheel state"
    );
}

#[test]
fn library_panel_wheel_at_loaded_edge_fetches_next_page() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.libs[0].nav_stack[0].total_count = 20;
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
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: list.x + 1,
        row: list.y,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(
            message,
            Msg::Shell(ShellRequest::EmbyLibraryCursorIndex { .. })
        )
    }));
    apply(&mut harness, outcome);
    assert!(
        harness.model().app.libs[0].nav_stack[0].loading,
        "wheel navigation at the loaded edge must start the next-page fetch"
    );
}
