use super::super::test_helpers::{
    assert_surface_pills, buffer_to_string, render_library_to_string_sized,
    render_library_to_terminal_focused,
};
use super::make_audiobookshelf_book_app;
use crate::app::layout::{LayoutMain, LibraryRowTarget};
use ratatui::layout::Rect;

/// Narrow inline Books reserve only the hero and leave chapters to the modal;
/// the bucket-filtered browser remains available through the shared target.
#[test]
fn narrow_layout_renders_hero_only_and_browser_together() {
    let mut app = make_audiobookshelf_book_app();
    let mut layout = LayoutMain::default();
    let terminal = render_library_to_terminal_focused(&mut app, &mut layout, true);
    let out = buffer_to_string(&terminal); // 60x20, below TWO_COLUMN_THRESHOLD

    assert!(
        out.contains("Alpha Tales"),
        "narrow hero must still show the cursor's book:\n{out}"
    );
    assert!(
        out.contains("A–C"),
        "narrow layout must still render the bucket-pill row:\n{out}"
    );
    assert!(
        !out.contains("Chapter One"),
        "narrow inline hero must not render chapter rows:\n{out}"
    );
    assert!(
        layout.audiobookshelf_book_chapter_rows.is_empty(),
        "narrow inline hero must not publish chapter targets"
    );
    assert!(
        layout.audiobookshelf_book_right_area.height > 0,
        "narrow layout must still populate the browser area, not just the hero"
    );

    app.layout.main = layout;
    app.layout.main.browse_destination = Some(app.tab);
    app.handle_key_view_dispatch(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Left,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert_eq!(
        app.audiobookshelf_book_browse[0].chapter_selection, None,
        "narrow inline mode must not focus chapters"
    );

    let browser = app.layout.main.audiobookshelf_book_right_area;
    let buffer = terminal.backend().buffer();
    let selected_row = browser.y
        + app
            .layout
            .main
            .left_row_targets
            .iter()
            .position(|target| *target == Some(LibraryRowTarget::Book(0)))
            .expect("narrow selected book row should be mapped") as u16;
    assert_eq!(
        app.layout.main.selected_item_rect,
        Some(app.layout.main.hero_area),
        "inline hero must own the selected book geometry"
    );
    let row_text = (browser.x..browser.right())
        .map(|x| buffer[(x, selected_row)].symbol())
        .collect::<String>();
    assert!(
        !row_text.contains("Alpha Tales"),
        "inline selected book row must not repeat the hero title: {row_text:?}"
    );
}

#[test]
fn narrow_books_have_one_pill_bar_and_valid_targets() {
    let mut app = make_audiobookshelf_book_app();
    let mut layout = LayoutMain::default();
    let terminal = render_library_to_terminal_focused(&mut app, &mut layout, true);
    let buffer = terminal.backend().buffer();

    let pill_rows = (0..buffer.area().height)
        .filter(|y| {
            let row = (0..buffer.area().width)
                .map(|x| buffer[(x, *y)].symbol())
                .collect::<String>();
            row.contains("A–C") && row.contains('⌘')
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pill_rows,
        vec![0],
        "the narrow Books buffer paints one surname bar"
    );
    assert_surface_pills(
        &terminal,
        &layout,
        Rect::new(0, 0, 60, 20),
        1,
        ratatui::style::Color::Reset,
        &[0, 1, 2],
        &["⌘", "A–C", "M–O", "V–Z"],
        0,
    );
    assert_eq!(layout.selector_tabs[0].0.y, 0);
    assert_eq!(
        buffer[(layout.selector_tabs[0].0.x, 1)].style().bg,
        Some(ratatui::style::Color::Reset),
        "the visible bar's spacer keeps the reset background"
    );

    app.layout.main = layout;
    app.layout.main.browse_destination = Some(app.tab);
    let first_bar_x = app.layout.main.selector_tabs[0].0.x + 1;
    assert!(app.click_set_cursor(first_bar_x, 0));
    assert_eq!(app.audiobookshelf_book_browse[0].selected_bucket, 0);
}

#[test]
fn narrow_book_has_no_chapter_targets() {
    let mut app = make_audiobookshelf_book_app();
    app.audiobookshelf_book_browse[0].chapter_selection = None;
    let mut layout = LayoutMain::default();
    let _ = render_library_to_string_sized(&mut app, &mut layout, 60, 30);
    assert!(layout.audiobookshelf_book_chapter_rows.is_empty());
    app.layout.main = layout;
    app.layout.main.browse_destination = Some(app.tab);
    assert_eq!(app.audiobookshelf_book_browse[0].chapter_selection, None);
}

#[test]
fn narrow_book_detail_is_suppressed_in_a_short_viewport() {
    let mut app = make_audiobookshelf_book_app();
    let mut layout = LayoutMain::default();
    let out = render_library_to_string_sized(&mut app, &mut layout, 60, 4);

    assert_eq!(layout.hero_area.height, 0);
    assert_eq!(layout.inline_hero_area.height, 0);
    assert!(out.contains("Alpha Tales"));
    assert!(layout
        .left_row_targets
        .contains(&Some(LibraryRowTarget::Book(0))));

    app.layout.main = layout;
    app.layout.main.browse_destination = Some(app.tab);
    app.refocus_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
    let row = app
        .layout
        .main
        .left_row_targets
        .iter()
        .position(|target| *target == Some(LibraryRowTarget::Book(0)))
        .expect("ordinary fallback target") as u16;
    let click = crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: app.layout.main.left_area.x + 1,
        row: app.layout.main.left_area.y + row,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };
    app.handle_mouse(click);
    app.handle_mouse(click);
    assert!(app.selection_modal.is_none());
    assert_eq!(app.status, "Audiobookshelf playback owner is unavailable");
}

#[test]
fn narrow_book_enter_uses_the_completed_60x20_layout_for_modal_activation() {
    let mut app = make_audiobookshelf_book_app();
    let mut layout = LayoutMain::default();
    let _ = render_library_to_string_sized(&mut app, &mut layout, 60, 20);

    assert!(layout.inline_hero_area.height > 0);
    app.layout.main = layout;
    app.layout.main.browse_destination = Some(app.tab);
    assert_eq!(
        app.handle_key_view_dispatch(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )),
        Some(false)
    );
    let modal = app
        .selection_modal
        .as_ref()
        .expect("rendered narrow hero Enter must open the chapter modal");
    assert!(matches!(
        modal.source,
        crate::app::SelectionModalSource::Book { .. }
    ));
    assert!(modal.state.rows().iter().any(|row| {
        matches!(
            row,
            crate::app::SelectionModalRow::Item(item) if item.name == "Chapter One"
        )
    }));
}

#[test]
fn cannot_fit_book_enter_uses_the_completed_60x4_layout_for_ordinary_activation() {
    let mut app = make_audiobookshelf_book_app();
    let mut layout = LayoutMain::default();
    let _ = render_library_to_string_sized(&mut app, &mut layout, 60, 4);

    assert_eq!(layout.inline_hero_area.height, 0);
    assert!(layout
        .left_row_targets
        .contains(&Some(LibraryRowTarget::Book(0))));
    app.layout.main = layout;
    app.layout.main.browse_destination = Some(app.tab);
    assert_eq!(
        app.handle_key_view_dispatch(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )),
        Some(false)
    );
    assert!(app.selection_modal.is_none());
    assert_eq!(app.status, "Audiobookshelf playback owner is unavailable");
}

#[test]
fn wide_book_enter_uses_the_completed_100x20_layout_for_chapter_workspace() {
    let mut app = make_audiobookshelf_book_app();
    let mut layout = LayoutMain::default();
    let _ = render_library_to_string_sized(&mut app, &mut layout, 100, 20);

    assert!(layout.audiobookshelf_book_wide_right_area.width > 0);
    assert!(!layout.audiobookshelf_book_chapter_rows.is_empty());
    app.layout.main = layout;
    app.layout.main.browse_destination = Some(app.tab);
    assert_eq!(
        app.handle_key_view_dispatch(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Left,
            crossterm::event::KeyModifiers::NONE,
        )),
        Some(false)
    );
    assert_eq!(app.audiobookshelf_book_browse[0].chapter_selection, Some(0));
    assert_eq!(
        app.handle_key_view_dispatch(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )),
        Some(false)
    );
    assert!(app.selection_modal.is_none());
    assert_eq!(app.audiobookshelf_book_browse[0].chapter_selection, Some(0));
    assert_ne!(app.status, "Audiobookshelf playback owner is unavailable");
}

#[test]
fn wide_book_chapter_target_keeps_mouse_activation_path() {
    let mut app = make_audiobookshelf_book_app();
    let mut layout = LayoutMain::default();
    let _ = render_library_to_string_sized(&mut app, &mut layout, 100, 20);
    let (rect, chapter) = layout
        .audiobookshelf_book_chapter_rows
        .first()
        .copied()
        .expect("wide Books must publish a chapter target");

    app.layout.main = layout;
    app.layout.main.browse_destination = Some(app.tab);
    app.refocus_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
    let click = crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: rect.x + 1,
        row: rect.y,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };
    app.handle_mouse(click);
    assert_eq!(
        app.audiobookshelf_book_browse[0].chapter_selection,
        Some(chapter)
    );
    app.handle_mouse(click);
    assert!(app.selection_modal.is_none());
}
