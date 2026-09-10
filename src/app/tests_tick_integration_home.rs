use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::{ComponentId, HomeComponent, Msg, ShellRequest};
use crate::app::render::{
    home_inline_media_browser_paints, home_wide_media_list_paints, reset_home_media_list_paints,
};
use crate::app::tests::{make_app_stub, make_item};
use crate::app::tests_tick_harness::TickHarness;
use crate::app::{PanelFocus, TabSelection};

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

fn home(harness: &TickHarness) -> &HomeComponent {
    harness
        .model()
        .application
        .get_component(&ComponentId::Home)
        .expect("Home mounted")
        .as_any()
        .downcast_ref::<HomeComponent>()
        .expect("Home component")
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

fn key(code: Key) -> Event<crate::app::components::UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
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
fn home_wide_tick_navigation_paints_the_selected_row_once() {
    let mut harness = home_harness(160, 30, 8);
    let _ = draw(&mut harness, 160, 30);
    harness.inject(key(Key::Down));
    let outcome = harness.step();
    assert!(outcome.messages.is_empty(), "local navigation emits no shell request");

    reset_home_media_list_paints();
    let terminal = draw(&mut harness, 160, 30);
    let selected = home(&harness).menu_placement_geometry().1.expect("selected row");
    assert_eq!(home(&harness).cursor(), 1);
    assert_eq!(home_wide_media_list_paints(), 1);
    let row: String = (selected.x..selected.right())
        .map(|x| terminal.backend().buffer()[(x, selected.y)].symbol())
        .collect();
    assert!(row.contains("Home Item 1"), "painted row must match selection: {row:?}");
}

#[test]
fn home_narrow_tick_wheel_and_click_use_current_inline_geometry() {
    let mut harness = home_harness(60, 20, 8);
    let _ = draw(&mut harness, 60, 20);
    let (selected, _) = home(&harness).test_hitmap()[0];
    harness.inject(wheel(selected.x, selected.y));
    let _outcome = harness.step();
    assert_eq!(home(&harness).cursor(), 1);
    let _ = draw(&mut harness, 60, 20);

    let (target, _) = home(&harness).test_hitmap()[1];
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: target.x,
        row: target.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome
        .messages
        .iter()
        .any(|message| matches!(message, Msg::Shell(ShellRequest::HomeRowClick { target: _ }))));
    assert_eq!(home(&harness).cursor(), 2);
    reset_home_media_list_paints();
    let terminal = draw(&mut harness, 60, 20);
    assert_eq!(home_inline_media_browser_paints(), 1);
    let painted: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(
        painted.contains("Home Item 2"),
        "painted frame must show selected row: {painted:?}"
    );
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
    assert_eq!(home(&harness).cursor(), 15);
    harness.model_mut().home_content.continue_items.swap(0, 15);
    harness.model_mut().push_home_content();
    assert_eq!(home(&harness).cursor(), 0, "flat cursor follows stable id after reorder");

    // Move to a distant row, then flip presentation. The component's single
    // retained anchor transfers target and screen-row offset.
    harness.inject(key(Key::End));
    harness.step();
    let _ = draw(&mut harness, 160, 30);
    let _ = draw(&mut harness, 60, 12);
    assert_eq!(home(&harness).cursor(), 39);
    assert!(home(&harness).test_active_scroll() > 0);
}
