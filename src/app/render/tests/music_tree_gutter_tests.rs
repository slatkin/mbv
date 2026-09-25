//! The Music tree row's horizontal slot contracts: the plain-space
//! indentation, the fixed year metadata gutter, and the label budget those
//! slots cost under clipping and overflow.

use super::test_helpers::{
    album, artist, expand_root, gutter_projection, music_tree_frame, music_tree_hierarchy_glyph,
    music_tree_row_slice, music_tree_row_text, select_target, tree_browser, GUTTER_ARTIST,
    GUTTER_YEAR, GUTTER_YEARED,
};
use super::*;

use crate::app::render::components::tree_browser::TREE_METADATA_SLOT_WIDTH;

/// The pinned year-gutter contract (design D8), painted through the crate's
/// label/column seams: one right-aligned fixed six-column green status
/// cell on a year-bearing album row followed by a two-column trailing gap,
/// reserved nowhere on the artist root or the yearless leaf (their titles
/// reach the last column), and no inline or second year column. The state
/// glyph and title roles are pinned here too.
#[test]
fn music_tree_year_gutter_is_reserved_only_on_the_album_that_carries_a_year() {
    const WIDTH: u16 = 40;
    const FRAME_WIDTH: u16 = WIDTH + 4;
    let mut browser = tree_browser(gutter_projection());
    let root = artist("gutter-artist");
    assert!(root.is_artist(), "level 0 is the artist root");
    expand_root(&mut browser, &root);
    assert_eq!(browser.visible_targets().len(), 3);
    // Unfocused so no row marquees: every row paints its ordinary truncation,
    // which is what makes the per-row budget observable.
    browser.set_focused(false);

    let area = Rect::new(0, 0, WIDTH, 3);
    let term = music_tree_frame(&mut browser, area, FRAME_WIDTH, 3);
    let buf = term.backend().buffer();
    for y in 0..area.height {
        for x in area.right()..FRAME_WIDTH {
            assert_eq!(
                buf[(x, y)].symbol(),
                " ",
                "no-overflow tree paints no scrollbar gutter"
            );
        }
    }

    // Three rows in three lines never overflow, so no scrollbar takes a
    // column: the tree column is the full browser width and the gutter is its
    // six-column date plus its two-column trailing gap.
    let gutter = (WIDTH - TREE_METADATA_SLOT_WIDTH as u16 - 2) as usize;
    let rows: Vec<String> = (0..3)
        .map(|y| music_tree_row_text(&term, y, 0, WIDTH))
        .collect();

    // Artist root: an ordinary grouping row in the metadata role, its long
    // title using the full width (no gutter reserved).
    let root_row = &rows[0];
    assert!(
        root_row.starts_with(GUTTER_ARTIST.chars().next().unwrap()),
        "the expanded root starts directly with its title: {root_row:?}"
    );
    assert_eq!(
        root_row.chars().last(),
        Some('…'),
        "the root title reaches the last column (no gutter): {root_row:?}"
    );
    assert_eq!(buf[(0, 0)].fg, palette::TEXT_EMPHASIS, "heading title role");

    // Year-bearing leaf: the title stops before the gutter, and the year
    // right-aligns in the fixed six-column green status cell before
    // its two-column trailing gap.
    let yeared_row = &rows[1];
    assert!(
        yeared_row.starts_with("  "),
        "the leaf keeps the reduced plain-space indentation: {yeared_row:?}"
    );
    assert_eq!(
        music_tree_row_slice(yeared_row, gutter, 6),
        format!("{GUTTER_YEAR:>6}"),
        "the year is right-aligned before its trailing gap: {yeared_row:?}"
    );
    assert_eq!(
        music_tree_row_slice(yeared_row, gutter + 6, 2),
        "  ",
        "the year keeps its two-column trailing gap"
    );
    assert_eq!(
        yeared_row.chars().nth(gutter - 1),
        Some('…'),
        "the yeared title truncates before the gutter: {yeared_row:?}"
    );
    assert_eq!(
        buf[(4, 1)].fg,
        palette::TEXT_FOCUS_ACCENT,
        "leaf title role"
    );
    assert_eq!(
        buf[(gutter as u16 - 1, 1)].fg,
        palette::TEXT_FOCUS_ACCENT,
        "an ordinary album leaf keeps its secondary title role"
    );
    assert_eq!(
        buf[(gutter as u16 + 4, 1)].fg,
        palette::STATUS_AVAILABLE,
        "the year paints in the green status role"
    );

    // Yearless leaf: no gutter is reserved, so its long title reaches the
    // last column and the gutter columns carry title text, never a year.
    let yearless_row = &rows[2];
    assert_eq!(
        yearless_row.chars().last(),
        Some('…'),
        "the yearless title uses the full width: {yearless_row:?}"
    );
    let yearless_gutter = music_tree_row_slice(yearless_row, gutter, 6);
    assert!(
        !yearless_gutter.chars().any(|c| c.is_ascii_digit())
            && !yearless_gutter.contains(GUTTER_YEAR),
        "the yearless row reserves no year: {yearless_gutter:?}"
    );
    assert!(
        !rows.iter().any(|row| row.contains('%')),
        "music tree rows never paint resume progress"
    );
}

/// When overflow has room for the scrollbar outside the claimed area, the
/// crate's extra render column must not reduce the tree cell's label budget.
/// This keeps the year gutter and marquee aligned with the canonical lists.
#[test]
fn music_tree_overflow_with_room_keeps_cell_budget_aligned() {
    const WIDTH: u16 = 40;
    const FRAME_WIDTH: u16 = WIDTH + 4;
    let mut browser = tree_browser(gutter_projection());
    let root = artist("gutter-artist");
    expand_root(&mut browser, &root);
    select_target(&mut browser, &album("gutter-yeared"));

    let area = Rect::new(0, 0, WIDTH, 2);
    let term = music_tree_frame(&mut browser, area, FRAME_WIDTH, area.height);
    let row = music_tree_row_text(&term, 1, 0, WIDTH);

    assert_eq!(row.chars().take(2).collect::<String>(), "  ");
    assert_eq!(
        row.chars().skip(2).take(30).collect::<String>(),
        GUTTER_YEARED.chars().take(30).collect::<String>(),
        "marquee receives the full tree-cell budget before the year gutter"
    );
    assert_eq!(
        row.chars()
            .skip((WIDTH - 6 - 2) as usize)
            .take(6)
            .collect::<String>(),
        format!("{GUTTER_YEAR:>6}"),
        "year remains right-aligned before its trailing gap"
    );
    assert_eq!(
        term.backend().buffer()[(WIDTH, 1)].fg,
        palette::SCROLLBAR,
        "overflow scrollbar remains just outside the claimed area"
    );
}

/// The tree never paints expansion or hierarchy symbols, regardless of the
/// global Nerd Font display setting used by other surfaces.
#[test]
fn music_tree_uses_plain_indentation_in_every_state() {
    const WIDTH: u16 = 40;
    const PLAIN_LEAF_PREFIX: &str = "  ";

    let area = Rect::new(0, 0, WIDTH, 3);
    let mut browser = tree_browser(gutter_projection());
    browser.set_focused(false);
    let root = artist("gutter-artist");
    assert!(root.is_artist(), "level 0 is the artist root");

    let term = music_tree_frame(&mut browser, area, WIDTH, 3);
    let collapsed_root_row = music_tree_row_text(&term, 0, 0, WIDTH);
    assert_eq!(collapsed_root_row.chars().next(), Some('T'));

    expand_root(&mut browser, &root);
    assert_eq!(browser.visible_targets().len(), 3);
    let term = music_tree_frame(&mut browser, area, WIDTH, 3);
    let root_row = music_tree_row_text(&term, 0, 0, WIDTH);
    assert_eq!(root_row.chars().next(), Some('T'));
    let leaf_row = music_tree_row_text(&term, 1, 0, WIDTH);
    assert!(leaf_row.starts_with(PLAIN_LEAF_PREFIX), "{leaf_row:?}");
    assert!(
        !leaf_row.chars().any(music_tree_hierarchy_glyph),
        "{leaf_row:?}"
    );
}
