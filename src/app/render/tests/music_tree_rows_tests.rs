//! Wide Library-panel grouped-row painting contracts: the full-row selection
//! bar, header-group zebra bands, the shared scrollbar, latest-render hit
//! testing, and aggregate marks.

use super::test_helpers::{
    album, artist_root, assert_music_tree_row_within, assert_music_tree_scrollbar_matches_shared,
    draw_mounted_frame, expand_root, make_music_tree_group_app, mark_leaf, mark_state,
    mounted_model_at, mounted_music_tree_browser, mounted_music_wide_geometry, music_tree_frame,
    music_tree_long_leaf, music_tree_row_bg, music_tree_row_fill, music_tree_row_text,
    music_tree_zebra_fill, row_y, select_target, MarkState, MUSIC_TREE_EXPANDED_PROJECTION_LEN,
    MUSIC_TREE_LONG_TITLE_YEAR, MUSIC_TREE_WIDE_HEIGHT, MUSIC_TREE_WIDE_WIDTH,
};
use super::*;
use ratatui::layout::Position;
use ratatui::style::Modifier;

use crate::app::render::components::tree_browser::{
    tree_metadata_gutter_width, TREE_METADATA_SLOT_WIDTH,
};

/// The tree's row contracts at the Wide Library-panel fixture, painted through
/// the crate's supported model/state/renderer/style seams: the full-row
/// selected bar, header-group zebra bands, scrollbar, clipping with the fixed
/// year gutter, latest-render hit testing, and aggregate marks.
#[test]
fn wide_music_tree_rows_paint_the_grouped_row_contracts() {
    let mut model = mounted_model_at(
        make_music_tree_group_app(),
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let _ = draw_mounted_frame(&mut model, MUSIC_TREE_WIDE_WIDTH, MUSIC_TREE_WIDE_HEIGHT);
    let list_area = mounted_music_wide_geometry(&model).list_area;
    assert!(
        list_area.width > 10 && list_area.height > 4,
        "wide browser rect reserved"
    );
    let scrollbar_x = if list_area.right() < MUSIC_TREE_WIDE_WIDTH {
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
    let long_leaf = music_tree_long_leaf();
    // The stable album target the shell keys survives into the tree leaf.
    assert_eq!(long_leaf.album_leaf_target(), Some("album-long"));

    // Frame A rests at the top with the root selected, so the zebra rows
    // below it paint unselected.
    select_target(&mut browser, &alpha_root);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let buf = term.backend().buffer();
    assert_eq!(
        browser.viewport_offset(),
        0,
        "a top selection needs no scroll"
    );

    // The selected artist root keeps its title, and the scrollbar follows the
    // canonical list's outside-column placement.
    assert_music_tree_row_within(&term, list_area.y, list_area, MUSIC_TREE_WIDE_WIDTH);
    let scrollbar_cell = &buf[(scrollbar_x, list_area.y + 1)];
    assert_ne!(scrollbar_cell.symbol(), " ");
    assert_eq!(
        scrollbar_cell.fg,
        palette::SCROLLBAR,
        "the crate's scrollbar takes the SCROLLBAR role"
    );

    // Header-group zebra: the Alpha header and every expanded descendant
    // share one continuous band; the adjacent Beta group takes the opposite
    // phase.
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

    // The shared scrollbar matches the canonical widget exactly; the crate's
    // default scrollbar does not remain in the tree column.
    assert_music_tree_scrollbar_matches_shared(
        &term,
        &browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );

    // Latest-render hit testing: the painted second row (frame A is at the
    // top, so it is the row at the viewport's first line) resolves its node,
    // and a point on the scrollbar resolves to the scrollbar region.
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
    // The scrollbar column is painted just outside the claimed rect, so the
    // shared owner's row-only retained geometry resolves no target there.
    assert_eq!(
        browser.resolve_current_point(Position {
            x: scrollbar_x,
            y: list_area.y + 1
        }),
        None
    );

    // Clipping: selected and unfocused so the long album leaf paints at the
    // viewport's last row without marqueeing; it truncates with an ellipsis,
    // its year sits right-aligned inside the fixed six-column gutter with a
    // two-column trailing gap, and no glyph lands outside the browser
    // rectangle.
    select_target(&mut browser, &long_leaf);
    browser.set_focused(false);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let long_y = row_y(&browser, &long_leaf);
    let table_right = list_area.x + list_area.width - 1;
    let long_row_text = music_tree_row_text(&term, long_y, list_area.x, list_area.right());
    assert!(
        long_row_text.contains('\u{2026}'),
        "long title truncates: {long_row_text}"
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
    assert_music_tree_row_within(&term, long_y, list_area, MUSIC_TREE_WIDE_WIDTH);

    // Frame B: selecting the second Beta leaf scrolls it into view; its bar
    // spans the full row and overrides the group band, while Beta's first leaf
    // keeps the neighbouring unstriped phase.
    browser.set_focused(true);
    select_target(&mut browser, &album("album-beta-2"));
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let buf = term.backend().buffer();
    assert!(
        browser.viewport_offset() > 0,
        "the bottom Beta leaf forces a scroll"
    );
    let beta_bar_y = row_y(&browser, &album("album-beta-2"));
    for x in list_area.x..table_right {
        assert_eq!(
            buf[(x, beta_bar_y)].bg,
            palette::SELECTED_ROW_BG,
            "selected-row bar reaches column {x}"
        );
    }
    assert_eq!(
        music_tree_row_bg(&term, probe_x, row_y(&browser, &album("album-beta-1"))),
        music_tree_row_fill(true),
        "Beta leaf 0 uses the Queue base fill"
    );
    assert_ne!(palette::SELECTED_ROW_BG, fill);

    // Aggregate marks: one of Beta's two leaves marked leaves the Beta root
    // Partial; marking the second lifts it to Marked, and each aggregate state
    // paints its own semantic role on the root's title.
    mark_leaf(&mut browser, &album("album-beta-1"));
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    assert_eq!(mark_state(&browser, &beta_root), MarkState::Partial);
    assert_eq!(
        term.backend().buffer()[(list_area.x + 2, row_y(&browser, &beta_root))].fg,
        palette::TEXT_ACCENT_MUTED,
        "a Partial artist root paints the muted aggregate role"
    );
    mark_leaf(&mut browser, &album("album-beta-2"));
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    assert_eq!(mark_state(&browser, &beta_root), MarkState::Marked);
    assert_eq!(
        term.backend().buffer()[(list_area.x + 2, row_y(&browser, &beta_root))].fg,
        palette::STATUS_AVAILABLE,
        "a Marked artist root paints the positive aggregate role"
    );
}

/// The selected-row bar's focus and multi-selection treatment at the Wide
/// fixture: a focused selection paints the bar across the whole row and
/// overrides a striped row's zebra, every span keeps the ordinary non-bold
/// foreground, an unfocused tree paints no bar for its cursor row, and a
/// multi-selected (marked) album leaf paints the bar even while unfocused.
/// The artist root, the Heading-equivalent grouping row, keeps the surface
/// fill and never takes the bar.
#[test]
fn wide_music_tree_selected_and_multi_selected_rows_paint_the_bar() {
    let mut model = mounted_model_at(
        make_music_tree_group_app(),
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let _ = draw_mounted_frame(&mut model, MUSIC_TREE_WIDE_WIDTH, MUSIC_TREE_WIDE_HEIGHT);
    let list_area = mounted_music_wide_geometry(&model).list_area;
    let mut browser = mounted_music_tree_browser(&model);
    let alpha_root = artist_root(&browser, "Alpha");
    let beta_root = artist_root(&browser, "Beta");
    expand_root(&mut browser, &alpha_root);
    expand_root(&mut browser, &beta_root);
    let table_right = list_area.x + list_area.width - 1;
    let zebra = music_tree_zebra_fill(true);
    assert_ne!(palette::SELECTED_ROW_BG, zebra, "the bar is not the stripe");

    // Focused single selection of Beta's first member: the bar spans
    // the whole content row (the scrollbar column keeps the parent
    // background) and overrides the zebra stripe, and the title keeps the
    // ordinary primary foreground with no bold modifier.
    select_target(&mut browser, &album("album-beta-1"));
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let selected_y = row_y(&browser, &album("album-beta-1"));
    let buf = term.backend().buffer();
    for x in list_area.x..table_right {
        assert_eq!(
            buf[(x, selected_y)].bg,
            palette::SELECTED_ROW_BG,
            "selected-row bar reaches column {x}"
        );
    }
    let title_x = list_area.x + 6;
    assert_eq!(buf[(title_x, selected_y)].fg, palette::SELECTED_ROW_FG);
    assert!(
        !buf[(title_x, selected_y)].modifier.contains(Modifier::BOLD),
        "the selected title is not bold"
    );

    // Artist roots are Heading-equivalent grouping rows and carry their own
    // group's band. Paint the Alpha header at the top so this assertion does
    // not depend on the selected Beta leaf's scrolled viewport.
    select_target(&mut browser, &alpha_root);
    browser.set_focused(false);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    assert_eq!(
        music_tree_row_bg(&term, list_area.x + 10, list_area.y),
        music_tree_zebra_fill(false),
        "the Alpha header is striped"
    );

    // Unfocused: the cursor row paints no bar; the focused Alpha band settles
    // from the focused Green2 fill to the tree's `#272e33` fill.
    browser.set_focused(false);
    select_target(&mut browser, &album("album-1"));
    let unfocused_zebra = music_tree_zebra_fill(false);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let cursor_y = row_y(&browser, &album("album-1"));
    // The unfocused tree's zebra intentionally shares the selected-row bar's
    // `#272e33` value; the focus-gated bar policy is not distinguishable by
    // background colour alone.
    assert_ne!(unfocused_zebra, zebra, "focus changes the zebra role");
    assert_eq!(
        music_tree_row_bg(&term, title_x, cursor_y),
        unfocused_zebra,
        "an unfocused cursor row keeps the tree's unfocused zebra fill"
    );
    let scrollbar_x = if list_area.right() < MUSIC_TREE_WIDE_WIDTH {
        list_area.right()
    } else {
        list_area.right().saturating_sub(1)
    };
    for y in list_area.y..list_area.bottom() {
        assert_eq!(
            term.backend().buffer()[(scrollbar_x, y)].symbol(),
            " ",
            "unfocused tree has no scrollbar glyph"
        );
    }

    // Unfocused multi-selection: with the cursor moved off the marked album
    // leaf, the mark alone paints the bar across its whole row, again
    // overriding the zebra stripe.
    select_target(&mut browser, &album("album-beta-2"));
    mark_leaf(&mut browser, &album("album-beta-1"));
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let marked_y = row_y(&browser, &album("album-beta-1"));
    let buf = term.backend().buffer();
    for x in list_area.x..table_right {
        assert_eq!(
            buf[(x, marked_y)].bg,
            palette::SELECTED_ROW_BG,
            "multi-selected bar reaches column {x}"
        );
    }
    assert_eq!(
        buf[(title_x, marked_y)].fg,
        palette::SELECTED_ROW_FG,
        "multi-selected text uses the selected-row foreground"
    );
    assert_ne!(
        buf[(title_x, row_y(&browser, &album("album-beta-2")))].bg,
        palette::SELECTED_ROW_BG,
        "an unmarked, unfocused cursor row paints no bar"
    );
}
