use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

use crate::app::components::TvWorkspaceComponent;
use crate::app::render::make_movie_app;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::{PanelFocus, PanelMode, TabSelection};

fn tv_harness() -> TickHarness {
    let mut app = make_movie_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    app.terminal_width = 160;
    app.terminal_height = 30;
    app.libs[0].library.collection_type = "tvshows".into();
    for (index, item) in app.libs[0].nav_stack[0].items.iter_mut().enumerate() {
        item.item_type = "Series".into();
        item.id = format!("series-{index}");
    }
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_tv_workspace();
    harness.model_mut().sync_active_destination();
    harness
}

fn tv(harness: &TickHarness) -> &TvWorkspaceComponent {
    harness
        .model()
        .application
        .get_component(harness.model().tv_workspace_id.as_ref().expect("TV workspace id"))
        .expect("TV workspace mounted")
        .as_any()
        .downcast_ref::<TvWorkspaceComponent>()
        .expect("TV workspace component")
}

fn draw(harness: &mut TickHarness) {
    let mut terminal = Terminal::new(TestBackend::new(160, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
}

#[test]
fn tv_wide_tick_navigation_updates_the_painted_control() {
    let mut harness = tv_harness();
    draw(&mut harness);
    assert_eq!(tv(&harness).selected_item_id(), Some("series-0".into()));

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    harness.step();
    draw(&mut harness);

    assert_eq!(tv(&harness).selected_item_id(), Some("series-1".into()));
}
