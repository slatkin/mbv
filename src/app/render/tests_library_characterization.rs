use super::test_helpers::{
    assert_surface_pills, buffer_to_string, make_large_movie_library_app, make_movie_app,
    render_library_to_string_sized,
};
use super::*;
use crate::app::layout::LayoutMain;
use crate::app::tests::make_item;
use crate::app::TabSelection;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_library(app: &mut App, width: u16, height: u16, focused: bool) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut layout = LayoutMain::default();
    terminal
        .draw(|f| {
            app.render_library(f, Rect::new(0, 0, width, height), focused, &mut layout);
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn library_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    // Note: width 120 triggers wide Movies layout, which is now handled by
    // BrowserComponent (5.3d.17a). Use narrow widths to test the legacy path.
    let states = [(60, 20, true, 0), (60, 20, false, 0), (60, 20, true, 1)];
    for (width, height, focused, cursor) in states {
        let mut app = make_movie_app();
        app.libs[0].nav_stack[0].cursor = cursor;
        let output = render_library(&mut app, width, height, focused);
        assert!(
            output.contains("Movie"),
            "library rows missing in {width}x{height}: {output:?}"
        );
    }
}

// Note: movies_pill_row_and_targets_are_characterized_end_to_end deleted.
// It tested the legacy wide Movies layout, which is now handled by
// BrowserComponent (5.3d.17a). Component rendering is tested separately.

#[test]
fn movies_plain_replacement_characterization_covers_bottom_scroll_fallback_and_targets() {
    let mut app = make_movie_app();
    app.libs[0].nav_stack[0].items[1].overview = "The selected movie overview.".into();
    app.libs[0].nav_stack[0].cursor = 1;
    app.libs[0].nav_stack[0].scroll = 1;
    let mut layout = LayoutMain::default();
    let output = render_library_to_string_sized(&mut app, &mut layout, 70, 30);

    assert!(
        output.contains("Second Movie"),
        "selected movie is missing:\n{output}"
    );
    assert!(
        layout.hero_area.height > 0,
        "complete selected replacement should fit: hero={:?} rows={:?}\n{output}",
        layout.hero_area,
        layout.left_item_rows
    );
    assert_eq!(layout.selected_item_rect, Some(layout.hero_area));
    let hero_lines = output
        .lines()
        .skip(layout.hero_area.y as usize)
        .take(layout.hero_area.height as usize)
        .collect::<String>();
    assert!(
        !hero_lines.contains('▎'),
        "ordinary selection marker leaked into the hero"
    );
    assert_eq!(
        layout
            .left_item_rows
            .iter()
            .filter(|row| row.as_slice() == [1])
            .count(),
        1,
        "replacement owns one parent row: {:?}",
        layout.left_item_rows
    );
    assert!(
        layout.left_item_rows.iter().any(|row| row.is_empty()),
        "continuation rows must not have ordinary item targets"
    );
    assert_eq!(
        app.libs[0].nav_stack[0].scroll, 1,
        "persisted scroll is retained"
    );

    let mut cannot_fit = make_movie_app();
    cannot_fit.libs[0].nav_stack[0].items[1].overview = "The selected movie overview.".into();
    cannot_fit.libs[0].nav_stack[0].cursor = 1;
    let mut fallback_layout = LayoutMain::default();
    let fallback = render_library_to_string_sized(&mut cannot_fit, &mut fallback_layout, 70, 4);
    assert!(
        fallback.contains("Second Movie"),
        "ordinary fallback loses the row:\n{fallback}"
    );
    assert_eq!(fallback_layout.hero_area.height, 0);
    assert!(
        fallback_layout
            .left_item_rows
            .iter()
            .any(|row| row.as_slice() == [1]),
        "ordinary fallback restores the selected row"
    );
}

#[test]
fn tv_letter_grouped_replacement_characterization_covers_header_fit_and_marker_suppression() {
    let mut app = make_movie_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.libs[0].library.collection_type = "tvshows".into();
    let items = (0..55)
        .map(|i| {
            let mut item = make_item(
                &format!("{} Series {i:02}", (b'A' + (i % 26) as u8) as char),
                "Series",
            );
            item.id = format!("series-{i}");
            item.is_folder = true;
            item.overview = "The selected series overview.".into();
            item
        })
        .collect();
    app.libs[0].nav_stack[0].items = items;
    app.libs[0].nav_stack[0].total_count = 55;
    app.libs[0].nav_stack[0].cursor = 54;
    app.libs[0].nav_stack[0].scroll = 12;
    app.libs[0].library_total = Some(55);

    let mut layout = LayoutMain::default();
    let output = render_library_to_string_sized(&mut app, &mut layout, 70, 20);

    assert!(
        output.contains("Series 54"),
        "selected series is missing:\n{output}"
    );
    assert!(
        layout.left_item_rows.iter().any(|row| row.is_empty()),
        "group headers and continuation rows remain targetless"
    );
    assert_eq!(
        layout
            .left_item_rows
            .iter()
            .filter(|row| row.as_slice() == [54])
            .count(),
        1,
        "grouped replacement owns one parent row"
    );
    assert!(
        layout.hero_area.height > 0,
        "grouped complete replacement should fit"
    );
    assert!(
        layout.left_row_map.iter().any(Option::is_none),
        "letter headers have no ordinary target"
    );
    let selected_display_row = layout
        .left_item_rows
        .iter()
        .position(|row| row.as_slice() == [54])
        .expect("selected grouped row should be present in the physical flow");
    let detail_screen_row = layout.hero_area.y.saturating_sub(layout.left_area.y) as usize;
    assert_eq!(
        app.libs[0].nav_stack[0].scroll,
        selected_display_row - detail_screen_row,
        "first render must persist the shared flow offset"
    );
    let hero_lines = output
        .lines()
        .skip(layout.hero_area.y as usize)
        .take(layout.hero_area.height as usize)
        .collect::<String>();
    assert!(
        !hero_lines.contains('▎'),
        "ordinary marker leaked into the grouped hero"
    );

    let mut boundary = app;
    boundary.libs[0].nav_stack[0].scroll = 1;
    let mut boundary_layout = LayoutMain::default();
    let boundary_output =
        render_library_to_string_sized(&mut boundary, &mut boundary_layout, 70, 8);
    assert!(
        boundary_output.contains("Series 54"),
        "header fit boundary hides selected row: hero={:?} map={:?}\n{boundary_output}",
        boundary_layout.hero_area,
        boundary_layout.left_row_map
    );
    assert_eq!(
        boundary_layout.hero_area.height, 0,
        "cannot-fit grouped detail restores ordinary rows"
    );
    assert!(boundary_layout.left_row_map.contains(&Some(54)));
}
