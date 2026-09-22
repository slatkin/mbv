//! Grouped Music's non-Wide Library-panel contracts: the shared skeleton
//! geometry, the grouped row contracts at the smallest non-Wide width, and the
//! panel-inset claim geometry.

use super::test_helpers::*;
use super::*;
use crate::app::render::components::tree_browser::{
    tree_metadata_gutter_width, TREE_METADATA_SLOT_WIDTH,
};
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
#[test]
fn wide_music_panel_uses_shared_skeleton_geometry() {
    let mut model = mounted_model_at(make_music_group_app(), 160, 24);
    let rendered = draw_mounted_frame(&mut model, 160, 24);
    assert!(rendered.contains("First Album"));
    let geometry = super::test_helpers::mounted_music_wide_geometry(&model);
    assert!(geometry.list_panel.width > 0);
    assert!(geometry.hero.width > 0);
}

// --- Grouped Music tree rows at the smallest non-Wide width --------------

/// The smallest supported non-Wide Library-panel width: below the
/// `TWO_COLUMN_THRESHOLD` (82), above the `MINI_VIEW_THRESHOLD` (80).
const MUSIC_TREE_NON_WIDE_WIDTH: u16 = crate::app::TWO_COLUMN_THRESHOLD - 1;
const MUSIC_TREE_NON_WIDE_HEIGHT: u16 = 30;

/// The tree's row contracts at the smallest supported non-Wide Library-panel
/// width: the hierarchy glyph and title fit inside the browser rect with no
/// overrun, the narrow scrollbar keeps the `SCROLLBAR` role, the zebra reset,
/// the full-row selected bar, clipping with the fixed gutter, latest-render hit
/// testing, and stale-geometry invalidation all survive the narrower column.
#[test]
fn non_wide_music_tree_rows_paint_the_grouped_row_contracts() {
    let mut model = mounted_model_at(
        make_music_tree_group_app(),
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    let _ = draw_mounted_frame(
        &mut model,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    let list_area = mounted_music_narrow_geometry(&model).list_area;
    assert!(
        list_area.width > 10 && list_area.height > 4,
        "non-Wide browser rect reserved"
    );
    let scrollbar_x = if list_area.right() < MUSIC_TREE_NON_WIDE_WIDTH {
        list_area.right()
    } else {
        list_area.right().saturating_sub(1)
    };

    let mut browser = mounted_music_tree_browser(&model);
    let alpha_root = artist_root(&browser, "Alpha");
    let beta_root = artist_root(&browser, "Beta");
    expand_root(&mut browser, &alpha_root);
    expand_root(&mut browser, &beta_root);
    assert_eq!(
        browser.visible_targets().len(),
        MUSIC_TREE_EXPANDED_PROJECTION_LEN
    );

    // Frame A rests at the top with the root selected, so the Alpha zebra
    // rows below paint unselected.
    select_target(&mut browser, &alpha_root);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    let buf = term.backend().buffer();
    assert_music_tree_row_within(&term, list_area.y, list_area, MUSIC_TREE_NON_WIDE_WIDTH);
    assert_eq!(
        buf[(scrollbar_x, list_area.y + 1)].fg,
        palette::SCROLLBAR,
        "the shared scrollbar takes the SCROLLBAR role"
    );
    assert_music_tree_scrollbar_matches_shared(
        &term,
        &browser,
        list_area,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );

    // The Alpha header establishes one band for the whole expanded group.
    let fill = music_tree_zebra_fill(true);
    let probe_x = list_area.x + 10;
    assert_eq!(
        music_tree_row_bg(&term, probe_x, list_area.y),
        palette::SELECTED_ROW_BG,
        "the selected Alpha header keeps its selection bar"
    );
    assert_eq!(
        music_tree_row_bg(&term, probe_x, list_area.y + 1),
        fill,
        "Alpha leaf 0 shares the header band"
    );
    assert_eq!(
        music_tree_row_bg(&term, probe_x, list_area.y + 2),
        fill,
        "Alpha leaf 1 shares the header band"
    );

    // Latest-render hit testing resolves the painted second row (frame A is
    // at the top, so it is the row at the viewport's first line).
    let second_row = album("album-1");
    let second_row_rect = browser
        .row_rect_for(&second_row)
        .expect("the painted second row");
    assert_eq!(
        browser.resolve_current_point(Position {
            x: second_row_rect.x + 5,
            y: second_row_rect.y,
        }),
        Some(&second_row)
    );

    // Clipping: selected and unfocused so the long album leaf paints without
    // marqueeing, it truncates with an ellipsis, its year keeps its gutter
    // inside the rect, and no glyph lands outside the browser rect.
    select_target(&mut browser, &music_tree_long_leaf());
    browser.set_focused(false);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    let long_y = row_y(&browser, &music_tree_long_leaf());
    let table_right = list_area.x + list_area.width - 1;
    let long_row_text = music_tree_row_text(&term, long_y, list_area.x, list_area.right());
    assert!(
        long_row_text.contains('\u{2026}'),
        "long title truncates at {MUSIC_TREE_NON_WIDE_WIDTH}: {long_row_text}"
    );
    let gutter_start = list_area.right()
        - tree_metadata_gutter_width(MUSIC_TREE_LONG_TITLE_YEAR.to_string().as_str()) as u16;
    let gutter_text: String = music_tree_row_text(
        &term,
        long_y,
        gutter_start,
        gutter_start + TREE_METADATA_SLOT_WIDTH as u16,
    )
    .trim()
    .to_string();
    assert_eq!(
        gutter_text,
        MUSIC_TREE_LONG_TITLE_YEAR.to_string(),
        "year right-aligned in the fixed gutter"
    );
    assert_music_tree_row_within(&term, long_y, list_area, MUSIC_TREE_NON_WIDE_WIDTH);

    // Scrollbar on the overflowing projection remains the shared painter
    // (refocused: the shared scrollbar is focus-gated).
    browser.set_focused(true);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    assert_music_tree_scrollbar_matches_shared(
        &term,
        &browser,
        list_area,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );

    // Frame B: the second Beta leaf's bar spans the narrower full row, and
    // Beta's first leaf keeps the neighbouring group's unstriped band.
    select_target(&mut browser, &album("album-beta-2"));
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    let buf = term.backend().buffer();
    for x in list_area.x..table_right {
        assert_eq!(
            buf[(x, row_y(&browser, &album("album-beta-2")))].bg,
            palette::SELECTED_ROW_BG,
            "selected-row bar reaches column {x}"
        );
    }
    let beta_leaf_0_y = row_y(&browser, &album("album-beta-1"));
    let beta_leaf_1_y = row_y(&browser, &album("album-beta-2"));
    assert_music_tree_title_fg(
        &term,
        list_area,
        beta_leaf_1_y,
        "Beta Nights",
        palette::SELECTED_ROW_FG,
    );
    assert_music_tree_title_fg(
        &term,
        list_area,
        beta_leaf_0_y,
        "Beta Session",
        palette::TEXT_FOCUS_ACCENT,
    );
    assert_ne!(
        music_tree_row_bg(&term, probe_x, beta_leaf_0_y),
        fill,
        "Beta leaf 0 keeps the neighbouring group's unstriped band"
    );

    // Stale geometry cannot claim input: an empty-area render invalidates the
    // latest hit map.
    let mut stale = Terminal::new(TestBackend::new(
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    ))
    .unwrap();
    stale
        .draw(|f| tuirealm::component::Component::view(&mut browser, f, Rect::ZERO))
        .unwrap();
    assert_eq!(
        browser.resolve_current_point(Position {
            x: list_area.x + 5,
            y: list_area.y + 1
        }),
        None
    );

    // Aggregate marks: both Beta leaves marked lift the Beta root to Marked.
    mark_leaf(&mut browser, &album("album-beta-1"));
    mark_leaf(&mut browser, &album("album-beta-2"));
    music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    assert_eq!(mark_state(&browser, &beta_root), MarkState::Marked);
}

#[test]
fn music_tree_panel_inset_keeps_rows_inside_claim_and_scrollbar_at_claim_edge() {
    let mut model = mounted_model_at(
        make_music_tree_group_app(),
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    let _ = draw_mounted_frame(
        &mut model,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    let content = mounted_music_narrow_geometry(&model).list_area;
    let claim = Rect {
        x: content.x.saturating_sub(2),
        width: content.width.saturating_add(4),
        ..content
    };
    assert!(claim.right() <= MUSIC_TREE_NON_WIDE_WIDTH);

    let mut browser = mounted_music_tree_browser(&model);
    let alpha_root = artist_root(&browser, "Alpha");
    let beta_root = artist_root(&browser, "Beta");
    expand_root(&mut browser, &alpha_root);
    expand_root(&mut browser, &beta_root);
    select_target(&mut browser, &alpha_root);
    browser.set_geometry(claim, content);
    let term = music_tree_frame(
        &mut browser,
        content,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    let row = music_tree_row_text(&term, content.y, claim.x, claim.right());
    assert!(row[..2].chars().all(char::is_whitespace));
    assert!(
        row.chars()
            .nth(2)
            .is_some_and(|character| !character.is_whitespace()),
        "tree title starts at the panel's two-column inset: {row:?}"
    );

    // The selected bar bleeds through the claim rectangle's side pads, while
    // the group's zebra band stops at the content rectangle's two-column
    // insets.
    assert_eq!(
        term.backend().buffer()[(claim.x, content.y)].bg,
        palette::SELECTED_ROW_BG
    );
    assert_eq!(
        term.backend().buffer()[(claim.right() - 1, content.y)].bg,
        palette::SELECTED_ROW_BG
    );
    assert_eq!(
        term.backend().buffer()[(claim.x, content.y + 1)].bg,
        ratatui::style::Color::Reset
    );
    assert_eq!(
        term.backend().buffer()[(claim.x + 2, content.y + 1)].bg,
        music_tree_zebra_fill(true)
    );
    assert_eq!(
        term.backend().buffer()[(claim.right() - 1, content.y + 1)].bg,
        ratatui::style::Color::Reset
    );
    let scrollbar_x = if claim.right() < MUSIC_TREE_NON_WIDE_WIDTH {
        claim.right()
    } else {
        claim.right().saturating_sub(1)
    };
    assert_eq!(
        term.backend().buffer()[(scrollbar_x, content.y + 1)].fg,
        palette::SCROLLBAR
    );
}
