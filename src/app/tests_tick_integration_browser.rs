use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::{BrowserComponent, Msg, ShellRequest};
use crate::app::components::browser_narrow::NarrowBrowseExtras;
use crate::app::render::{
    browser_grid_media_list_paints, browser_inline_media_browser_paints,
    browser_legacy_plain_rows_paints, browser_wide_media_list_paints, make_movie_app,
    reset_browser_media_list_paints,
};
use crate::app::tests_tick_harness::TickHarness;
use crate::app::{PanelFocus, PanelMode, TabSelection};

fn browser(harness: &TickHarness) -> &BrowserComponent {
    harness
        .model()
        .application
        .get_component(harness.model().emby_browser_id.as_ref().expect("browser id"))
        .expect("browser mounted")
        .as_any()
        .downcast_ref::<BrowserComponent>()
        .expect("browser component")
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
fn browser_wide_tick_moves_control_without_recomputing_app_cursor_and_paints_once() {
    let mut app = make_movie_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    reset_browser_media_list_paints();
    let terminal = draw(&mut harness, 100, 30);
    assert_eq!(browser_wide_media_list_paints(), 1);
    assert_eq!(browser_inline_media_browser_paints(), 0);
    assert_eq!(browser_legacy_plain_rows_paints(), 0);
    assert!(terminal.backend().buffer().content().iter().any(|cell| cell.symbol() == "F"));

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::Shell(ShellRequest::BrowserCursorIndex { index: 1 }))
    }));
    assert_eq!(browser(&harness).cursor(), 1);
    assert_eq!(harness.model().app.libs[0].nav_stack[0].resting().cursor(), 0);

    reset_browser_media_list_paints();
    let _ = draw(&mut harness, 100, 30);
    assert_eq!(browser_wide_media_list_paints(), 1);
    assert_eq!(browser_legacy_plain_rows_paints(), 0);
}

#[test]
fn browser_narrow_tick_click_uses_retained_geometry_and_the_inline_painter_once() {
    let mut app = make_movie_app();
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let browser_id = harness.model().emby_browser_id.clone().expect("browser id");
    harness
        .model_mut()
        .application
        .get_component_mut(&browser_id)
        .expect("browser mounted")
        .as_any_mut()
        .downcast_mut::<BrowserComponent>()
        .expect("browser component")
        .set_narrow_extras(NarrowBrowseExtras {
            hero_placeholder: true,
            ..NarrowBrowseExtras::default()
        });
    reset_browser_media_list_paints();
    let _ = draw(&mut harness, 60, 30);
    assert_eq!(browser_inline_media_browser_paints(), 1);
    assert_eq!(browser_wide_media_list_paints(), 0);
    assert_eq!(browser_legacy_plain_rows_paints(), 0);

    let area = browser(&harness).test_layout().inline_hero_area;
    assert!(!area.is_empty(), "selected detail retained geometry");
    let position = Position::new(area.x, area.y);
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: position.x,
        row: position.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().all(|message| {
        !matches!(message, Msg::Shell(ShellRequest::BrowserRowClick { .. }))
    }), "detail/header geometry must be a no-op");
    assert_eq!(browser(&harness).cursor(), 0);
    assert_eq!(harness.model().app.libs[0].nav_stack[0].resting().cursor(), 0);

    reset_browser_media_list_paints();
    let _ = draw(&mut harness, 60, 30);
    assert_eq!(browser_inline_media_browser_paints(), 1);
    assert_eq!(browser_legacy_plain_rows_paints(), 0);
}

/// Task 4.1: the generic non-hero two-column catalog paints through the Grid
/// presentation over the shared owner — exactly one Grid painter runs, and no
/// legacy plain-row painter, Wide, or Inline presentation paints beside it.
#[test]
fn browser_generic_two_column_tick_isolated_from_canonical_controls() {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "other".into();
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    reset_browser_media_list_paints();
    let _ = draw(&mut harness, 100, 30);
    assert_eq!(browser_grid_media_list_paints(), 1);
    assert_eq!(browser_wide_media_list_paints(), 0);
    assert_eq!(browser_inline_media_browser_paints(), 0);
    assert_eq!(browser_legacy_plain_rows_paints(), 0);
    assert_eq!(browser(&harness).cursor(), 0);

    // Tick navigation moves the shared owner and echoes the resolved index.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::Shell(ShellRequest::BrowserCursorIndex { index: 1 }))
    }));
    assert_eq!(browser(&harness).cursor(), 1);

    // The next frame repaints through the Grid presentation only, with the
    // selection retained by the shared owner.
    reset_browser_media_list_paints();
    let _ = draw(&mut harness, 100, 30);
    assert_eq!(browser_grid_media_list_paints(), 1);
    assert_eq!(browser_legacy_plain_rows_paints(), 0);
    assert_eq!(browser(&harness).cursor(), 1);
}
