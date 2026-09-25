//! The shared marquee clock on the focused selected tree title: the hold
//! window, the scroll step, and the reset when the selection changes titles.

use super::test_helpers::{
    album, artist, artist_root, draw_mounted_frame, expand_root, make_music_tree_group_app,
    mounted_model_at, mounted_music_tree_browser, mounted_music_wide_geometry, music_tree_frame,
    music_tree_long_leaf, music_tree_row_text, reset_projection, row_y, select_target,
    tree_browser, MUSIC_TREE_LONG_TITLE, MUSIC_TREE_WIDE_HEIGHT, MUSIC_TREE_WIDE_WIDTH,
    RESET_TITLE_FIRST, RESET_TITLE_SECOND,
};
use ratatui::layout::Rect;

/// The focused selected row starts its title window at the beginning, while
/// an unfocused row truncates with an ellipsis instead.
#[test]
fn wide_music_tree_marquee_scrolls_the_focused_selected_title() {
    let mut model = mounted_model_at(
        make_music_tree_group_app(),
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let _ = draw_mounted_frame(&mut model, MUSIC_TREE_WIDE_WIDTH, MUSIC_TREE_WIDE_HEIGHT);
    let list_area = mounted_music_wide_geometry(&model).list_area;

    let mut browser = mounted_music_tree_browser(&model);
    let alpha_root = artist_root(&browser, "Alpha");
    expand_root(&mut browser, &alpha_root);
    let long_leaf = music_tree_long_leaf();
    select_target(&mut browser, &long_leaf);

    // The marquee window strips to the row's name slot (after the hierarchy
    // plain-space indentation the crate composes).
    fn name_slot(row: &str) -> &str {
        row.trim_start()
    }

    // The first painted frame rests on the title's beginning — no ellipsis.
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let resting_row = music_tree_row_text(
        &term,
        row_y(&browser, &long_leaf),
        list_area.x,
        list_area.right(),
    );
    let resting = name_slot(&resting_row);
    assert!(
        resting.starts_with(&MUSIC_TREE_LONG_TITLE[..12]),
        "hold shows the title start: {resting}"
    );
    assert!(
        !resting.contains('\u{2026}'),
        "the marquee window carries no ellipsis"
    );

    // Unfocused, the same row truncates with an ellipsis instead of marqueeing.
    browser.set_focused(false);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let unfocused_row = music_tree_row_text(
        &term,
        row_y(&browser, &long_leaf),
        list_area.x,
        list_area.right(),
    );
    let unfocused = name_slot(&unfocused_row);
    assert!(
        unfocused.contains('\u{2026}'),
        "unfocused title truncates: {unfocused}"
    );
}

/// previous row's scroll position. The clock is injected directly (no sleeps);
/// the hold window is what the reset produces.
#[test]
fn music_tree_title_clock_resets_when_the_selection_changes() {
    const WIDTH: u16 = 40;
    let area = Rect::new(0, 0, WIDTH, 3);
    let mut browser = tree_browser(reset_projection());
    let root = artist("reset-artist");
    expand_root(&mut browser, &root);
    assert_eq!(browser.visible_targets().len(), 3);
    let first = album("reset-first");
    let second = album("reset-second");

    fn name_slot(row: &str) -> &str {
        row.trim_start()
    }

    // The first selected title starts at the beginning of its window.
    select_target(&mut browser, &first);
    let term = music_tree_frame(&mut browser, area, WIDTH, 3);
    let first_row = music_tree_row_text(&term, 1, 0, WIDTH);
    let first_name = name_slot(&first_row);
    assert!(
        first_name.starts_with(&RESET_TITLE_FIRST[..12]),
        "the first title starts at its hold position: {first_name}"
    );

    // Selecting a different title resets the clock and starts the new window
    // from its beginning.
    select_target(&mut browser, &second);
    let term = music_tree_frame(&mut browser, area, WIDTH, 3);
    let second_row = music_tree_row_text(&term, 2, 0, WIDTH);
    let second_name = name_slot(&second_row);
    assert!(
        second_name.starts_with(&RESET_TITLE_SECOND[..12]),
        "the new title's clock reset to its hold start: {second_name}"
    );
}
