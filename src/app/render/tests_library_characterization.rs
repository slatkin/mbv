use super::test_helpers::{
    draw_mounted_frame, make_movie_app, mounted_browser_layout, mounted_browser_scroll,
    mounted_model_at,
};
use super::*;
use crate::app::tests::make_item;
use crate::app::TabSelection;

#[test]
fn library_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    // Note: width 120 triggers wide Movies layout, which is now handled by
    // BrowserComponent (5.3d.17a). Narrow Movies is likewise painted by the
    // mounted `BrowserComponent` now (task 3.8), so route through the real
    // `Model::draw_frame` path.
    let states = [(60, 20, 0), (60, 20, 1)];
    for (width, height, cursor) in states {
        let mut app = make_movie_app();
        app.libs[0].nav_stack[0].set_resting_cursor(cursor);
        let mut model = mounted_model_at(app, width, height);
        let output = draw_mounted_frame(&mut model, width, height);
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
    app.libs[0].nav_stack[0].set_resting_cursor(1);
    app.libs[0].nav_stack[0].set_resting_scroll(1);
    let mut model = mounted_model_at(app, 70, 30);
    let output = draw_mounted_frame(&mut model, 70, 30);
    let layout = mounted_browser_layout(&model);

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
    let selected_rect = layout
        .selected_item_rect
        .expect("selected movie keeps a parent-owned row target");
    assert_eq!(selected_rect.x, layout.hero_area.x);
    assert_eq!(selected_rect.y, layout.hero_area.y);
    assert_eq!(selected_rect.width, layout.hero_area.width);
    assert!(selected_rect.height > 0);
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
        model.app.libs[0].nav_stack[0].resting().scroll(),
        1,
        "persisted scroll is retained"
    );

    let mut cannot_fit = make_movie_app();
    cannot_fit.libs[0].nav_stack[0].items[1].overview = "The selected movie overview.".into();
    cannot_fit.libs[0].nav_stack[0].set_resting_cursor(1);
    let mut fallback_model = mounted_model_at(cannot_fit, 70, 12);
    let fallback = draw_mounted_frame(&mut fallback_model, 70, 12);
    let fallback_layout = mounted_browser_layout(&fallback_model);
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

fn tv_letter_grouped_app(scroll: usize) -> App {
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
    app.libs[0].nav_stack[0].set_resting_cursor(54);
    app.libs[0].nav_stack[0].set_resting_scroll(scroll);
    app.libs[0].library_total = Some(55);
    app
}

#[test]
fn tv_letter_grouped_replacement_characterization_covers_header_fit_and_marker_suppression() {
    let mut model = mounted_model_at(tv_letter_grouped_app(12), 70, 20);
    let output = draw_mounted_frame(&mut model, 70, 20);
    let layout = mounted_browser_layout(&model);

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
        mounted_browser_scroll(&model),
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

    let mut boundary_model = mounted_model_at(tv_letter_grouped_app(1), 70, 14);
    let boundary_output = draw_mounted_frame(&mut boundary_model, 70, 14);
    let boundary_layout = mounted_browser_layout(&boundary_model);
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
