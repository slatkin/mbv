use super::test_helpers::*;
use super::*;
use crate::app::components::library_panel::{LibraryPanel, WideSkeletonGeometry};
use crate::app::components::music_tree::MusicTreeBrowser;
use crate::app::components::ComponentId;
use crate::app::shell::Model;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use tui_treelistview::{TreeHit, TreeMarkState};

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

fn mounted_music_narrow_geometry(model: &Model) -> WideSkeletonGeometry {
    model
        .application
        .get_component(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("LibraryPanel")
        .test_narrow_geometry()
        .expect("non-Wide Music skeleton painted")
}

/// One tree frame: the tree adapter painting into the mounted Library panel's
/// reserved browser rect on a fresh buffer.
fn music_tree_frame(
    browser: &mut MusicTreeBrowser,
    area: Rect,
    width: u16,
    height: u16,
) -> Terminal<TestBackend> {
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|f| browser.view(f, area)).unwrap();
    term
}

/// The buffer y of a projection row in the latest frame (the row must be
/// visible: `projection_row` in `browser.offset()..offset + viewport`).
fn music_tree_row_y(browser: &MusicTreeBrowser, list_area: Rect, projection_row: usize) -> u16 {
    list_area.y + (projection_row - browser.offset()) as u16
}

fn music_tree_row_text(term: &Terminal<TestBackend>, y: u16, x0: u16, x1: u16) -> String {
    let buf = term.backend().buffer();
    (x0..x1).map(|x| buf[(x, y)].symbol().to_string()).collect()
}

fn assert_music_tree_scrollbar_matches_shared(
    term: &Terminal<TestBackend>,
    browser: &MusicTreeBrowser,
    area: Rect,
    width: u16,
    height: u16,
) {
    let mut expected = Terminal::new(TestBackend::new(width, height)).unwrap();
    expected
        .draw(|frame| {
            crate::app::render::components::widgets::render_right_scrollbar_with_viewport(
                frame,
                area,
                browser.projection_len(),
                area.height as usize,
                browser.offset(),
                palette::SCROLLBAR,
            );
        })
        .unwrap();
    let scrollbar_x = if area.right() < width {
        area.right()
    } else {
        area.right().saturating_sub(1)
    };
    for y in area.y..area.bottom() {
        let actual = &term.backend().buffer()[(scrollbar_x, y)];
        let shared = &expected.backend().buffer()[(scrollbar_x, y)];
        assert_eq!(actual.symbol(), shared.symbol(), "scrollbar glyph at y={y}");
        assert_eq!(actual.fg, shared.fg, "scrollbar foreground at y={y}");
    }
}

/// Grouped Music deliberately paints no hierarchy or expansion symbols.
fn music_tree_hierarchy_glyph(c: char) -> bool {
    matches!(c, '>' | 'v' | '*' | '?' | '~' | '|' | '-' | '`')
}

/// Every visible node row keeps its title and uses only plain-space indentation.
/// The shared scrollbar is allowed in the same outside-column position used by
/// the canonical list painter.
fn assert_music_tree_row_within(
    term: &Terminal<TestBackend>,
    row_y: u16,
    list_area: Rect,
    frame_width: u16,
) {
    let row = music_tree_row_text(term, row_y, list_area.x, list_area.right());
    assert!(
        !row.chars().any(music_tree_hierarchy_glyph),
        "row paints no hierarchy or expansion symbols: {row:?}"
    );
    assert!(
        row.chars().any(|c| !c.is_whitespace() && c != '…'),
        "row keeps at least one title cell: {row:?}"
    );
    let scrollbar_x = if list_area.right() < frame_width {
        list_area.right()
    } else {
        list_area.right().saturating_sub(1)
    };
    for x in list_area.right()..frame_width {
        if x != scrollbar_x {
            assert_eq!(
                term.backend().buffer()[(x, row_y)].symbol(),
                " ",
                "only the shared scrollbar may occupy the outside column"
            );
        }
    }
}

fn music_tree_row_bg(term: &Terminal<TestBackend>, x: u16, y: u16) -> ratatui::style::Color {
    term.backend().buffer()[(x, y)].bg
}

/// The zebra fill the tree resolves for its rows: the focused Green2 fill
/// settles to the resting-content Storm when focus moves elsewhere.
fn music_tree_zebra_fill(focused: bool) -> ratatui::style::Color {
    palette::music_tree_zebra(focused)
}

/// The long album's node id, found by its title in the arena (its settled
/// sort position is not load-bearing).
fn music_tree_long_leaf_id(browser: &MusicTreeBrowser) -> usize {
    (0..browser.projection_len() * 2)
        .find(|&id| browser.title_of(id) == MUSIC_TREE_LONG_TITLE)
        .expect("long album interned")
}

/// A node's projection row, if visible in the settled projection.
fn music_tree_projection_row_of(browser: &MusicTreeBrowser, id: usize) -> usize {
    browser
        .projected_nodes()
        .iter()
        .position(|node| node.id() == id)
        .expect("node projected")
}

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
    browser.expand_root(MUSIC_TREE_ALPHA_ROOT);
    browser.expand_root(MUSIC_TREE_BETA_ROOT);
    assert_eq!(browser.projection_len(), MUSIC_TREE_EXPANDED_PROJECTION_LEN);
    let long_leaf = music_tree_long_leaf_id(&browser);
    let long_row = music_tree_projection_row_of(&browser, long_leaf);

    // Frame A rests at the top with the root selected, so the Alpha zebra
    // rows below paint unselected.
    browser.select_index(MUSIC_TREE_ALPHA_ROOT);
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
    let second_row_id = browser.projected_nodes()[1].id();
    match browser.hit_test(Position {
        x: list_area.x + 5,
        y: list_area.y + 1,
    }) {
        Some(TreeHit::Row { id, .. }) if id == second_row_id => {}
        other => panic!("expected the painted row's node, got {other:?}"),
    }

    // Clipping: scrolled so the unselected long album leaf paints, it
    // truncates with an ellipsis, its year keeps its gutter inside the rect,
    // and no glyph lands outside the browser rect.
    browser.scroll_to((long_row + 1).saturating_sub(list_area.height as usize));
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    let long_y = music_tree_row_y(&browser, list_area, long_row);
    let table_right = list_area.x + list_area.width - 1;
    let long_row_text = music_tree_row_text(&term, long_y, list_area.x, list_area.right());
    assert!(
        long_row_text.contains('\u{2026}'),
        "long title truncates at {MUSIC_TREE_NON_WIDE_WIDTH}: {long_row_text}"
    );
    let gutter_start =
        list_area.right() - crate::app::components::music_tree::YEAR_GUTTER_WIDTH - 2;
    let gutter_text: String = music_tree_row_text(
        &term,
        long_y,
        gutter_start,
        gutter_start + crate::app::components::music_tree::YEAR_GUTTER_WIDTH,
    )
    .trim()
    .to_string();
    assert_eq!(
        gutter_text,
        MUSIC_TREE_LONG_TITLE_YEAR.to_string(),
        "year right-aligned in the fixed gutter"
    );
    assert_music_tree_row_within(&term, long_y, list_area, MUSIC_TREE_NON_WIDE_WIDTH);

    // Scrollbar on the overflowing projection remains the shared painter.
    assert_music_tree_scrollbar_matches_shared(
        &term,
        &browser,
        list_area,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );

    // Frame B: the second Beta leaf's bar spans the narrower full row, and
    // Beta's first leaf keeps the neighbouring group's unstriped band.
    browser.select_index(MUSIC_TREE_BETA_LEAF_1);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    let buf = term.backend().buffer();
    for x in list_area.x..table_right {
        assert_eq!(
            buf[(
                x,
                music_tree_row_y(&browser, list_area, MUSIC_TREE_BETA_LEAF_1)
            )]
                .bg,
            palette::SELECTED_ROW_BG,
            "selected-row bar reaches column {x}"
        );
    }
    assert_ne!(
        music_tree_row_bg(
            &term,
            probe_x,
            music_tree_row_y(&browser, list_area, MUSIC_TREE_BETA_LEAF_0)
        ),
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
    stale.draw(|f| browser.view(f, Rect::ZERO)).unwrap();
    assert_eq!(
        browser.hit_test(Position {
            x: list_area.x + 5,
            y: list_area.y + 1
        }),
        None
    );

    // Aggregate marks: both Beta leaves marked lift the Beta root to Marked.
    browser.set_marked(MUSIC_TREE_BETA_LEAF_0, true);
    browser.set_marked(MUSIC_TREE_BETA_LEAF_1, true);
    music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_NON_WIDE_WIDTH,
        MUSIC_TREE_NON_WIDE_HEIGHT,
    );
    assert_eq!(
        browser.mark_state(MUSIC_TREE_BETA_ROOT),
        TreeMarkState::Marked
    );
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
    browser.expand_root(MUSIC_TREE_ALPHA_ROOT);
    browser.expand_root(MUSIC_TREE_BETA_ROOT);
    browser.select_index(MUSIC_TREE_ALPHA_ROOT);
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

    // The row content leaves the right inset clear; the shared scrollbar stays
    // at the full-width claim edge rather than moving into the inset.
    assert_eq!(
        term.backend().buffer()[(claim.right() - 2, content.y)].symbol(),
        " "
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
