//! Live-tick coverage for LibraryPanel's resolved scroll request.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, MouseEvent, MouseEventKind};

use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::render::make_movie_app;
use crate::app::tests_tick_harness::{StepOutcome, TickHarness};
use crate::app::{PanelFocus, PanelMode};

fn apply(harness: &mut TickHarness, outcome: StepOutcome) {
    let (mut music, mut tv) = (false, false);
    for message in outcome.messages {
        harness.model_mut().handle_terminal_message(message, &mut music, &mut tv);
    }
    harness.model_mut().sync_mounted_surfaces();
}

#[test]
fn library_panel_wheel_persists_owner_resolved_scroll() {
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
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: list.x + 1,
        row: list.y,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    let (key, index, scroll) = outcome
        .raw_messages
        .iter()
        .find_map(|message| match message {
            Msg::Shell(ShellRequest::LibraryScroll { key, index, scroll }) => {
                Some((key.clone(), *index, *scroll))
            }
            _ => None,
        })
        .expect("the panel emits a resolved LibraryScroll request");
    assert!(matches!(
        harness.model().active_migrated_browser_owner().unwrap().1,
        crate::app::components::library_panel::LibraryKey::Service(ref active) if active == &key
    ));
    apply(&mut harness, outcome);
    let library = harness.model().app.tab.emby_library_index().unwrap();
    let resting = harness.model().app.libs[library].nav_stack.last().unwrap().resting();
    assert_eq!(resting.cursor(), index);
    assert_eq!(resting.scroll(), scroll);
}
