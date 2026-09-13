use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::browser_content::BrowserContent as BrowserOwner;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::render::{
    browser_inline_media_browser_paints, browser_legacy_plain_rows_paints,
    browser_wide_media_list_paints, make_movie_app, reset_browser_media_list_paints,
};

use crate::app::tests_tick_harness::TickHarness;

/// The migrated Movies/HomeVideos/Generic owner inside the mounted
/// `LibraryPanel` (task 6.1): the panel is the library area's one event
/// boundary, and the browse state is read through the owner the panel hosts
/// — never a destination component (`emby_browser_id` stays `None` for
/// these kinds).
fn browser_owner(harness: &TickHarness) -> &BrowserOwner {
    let (_, key, _) = harness
        .model()
        .active_migrated_browser_owner()
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
fn browser_wide_tick_moves_control_without_recomputing_app_cursor_and_paints_once() {
    let mut app = make_movie_app();
    app.tab = crate::app::TabSelection::EmbyLibrary(0);
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
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
    assert_eq!(browser_owner(&harness).cursor(), 1);
    assert_eq!(harness.model().app.libs[0].nav_stack[0].resting().cursor(), 0);

    reset_browser_media_list_paints();
    let _ = draw(&mut harness, 100, 30);
    assert_eq!(browser_wide_media_list_paints(), 1);
    assert_eq!(browser_legacy_plain_rows_paints(), 0);
}

#[test]
fn browser_narrow_tick_click_uses_retained_geometry_and_the_inline_painter_once() {
    let mut app = make_movie_app();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    // At 60 columns the stored panel mode is ignored and the mini view
    // derives the visible panel from `mini_view_focus`; a narrow *library*
    // view must select Library here.
    app.mini_view_focus = crate::app::PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    reset_browser_media_list_paints();
    let terminal = draw(&mut harness, 60, 30);
    assert_eq!(browser_inline_media_browser_paints(), 1);
    assert_eq!(browser_wide_media_list_paints(), 0);
    assert_eq!(browser_legacy_plain_rows_paints(), 0);

    // The panel's own retained Narrow geometry is the painted truth: the
    // admitted inline hero block is the Inline presentation's detail rect,
    // so a click on it resolves to the retained selected target through the
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
    let area = geometry
        .inline_hero
        .expect("selected detail retained geometry");
    assert!(!area.is_empty(), "selected detail retained geometry");
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
            Msg::Shell(ShellRequest::BrowserRowClick { target: Some(target) })
                if target == "movie-focused"
        )),
        "the painted inline hero block resolves to the selected row: {:?}", outcome.raw_messages
    );
    assert_eq!(browser_owner(&harness).cursor(), 0);
    assert_eq!(harness.model().app.libs[0].nav_stack[0].resting().cursor(), 0);
    let _ = terminal;

    reset_browser_media_list_paints();
    let _ = draw(&mut harness, 60, 30);
    assert_eq!(browser_inline_media_browser_paints(), 1);
    assert_eq!(browser_legacy_plain_rows_paints(), 0);
}

/// Task 6.1: the generic catalog's narrow surface paints through the Inline
/// presentation over the shared owner — exactly one Inline painter runs, and
/// no legacy plain-row painter or Wide presentation paints beside it. (The
/// test previously pinned the Grid presentation for this surface; Grid was
/// deleted as unreachable by design D13 — no library in use lacks a hero.)
#[test]
fn browser_generic_narrow_tick_isolated_from_canonical_controls() {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "other".into();
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    reset_browser_media_list_paints();
    let _ = draw(&mut harness, 100, 30);
    assert_eq!(browser_inline_media_browser_paints(), 1);
    assert_eq!(browser_wide_media_list_paints(), 0);
    assert_eq!(browser_legacy_plain_rows_paints(), 0);
    assert_eq!(browser_owner(&harness).cursor(), 0);

    // Tick navigation moves the shared owner and echoes the resolved index.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::Shell(ShellRequest::BrowserCursorIndex { index: 1 }))
    }));
    assert_eq!(browser_owner(&harness).cursor(), 1);

    // The next frame repaints through the Inline presentation only, with the
    // selection retained by the shared owner.
    reset_browser_media_list_paints();
    let _ = draw(&mut harness, 100, 30);
    assert_eq!(browser_inline_media_browser_paints(), 1);
    assert_eq!(browser_legacy_plain_rows_paints(), 0);
    assert_eq!(browser_owner(&harness).cursor(), 1);
}