use super::test_helpers::{
    buffer_to_string, draw_mounted_frame, make_music_group_app, make_music_tree_group_app,
    mounted_model_at, mounted_music_tree_browser, mounted_music_wide_geometry,
    MUSIC_TREE_ALPHA_ROOT, MUSIC_TREE_BETA_LEAF_0, MUSIC_TREE_BETA_LEAF_1, MUSIC_TREE_BETA_ROOT,
    MUSIC_TREE_EXPANDED_PROJECTION_LEN, MUSIC_TREE_LONG_TITLE, MUSIC_TREE_LONG_TITLE_YEAR,
};
use super::*;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::style::Modifier;
use ratatui::Terminal;
use tui_treelistview::{TreeHit, TreeMarkState};

use crate::app::components::media_list::MediaSemanticState;
use crate::app::components::music_tree::{MusicTreeBrowser, MusicTreeEntry, MusicTreeModel};
use crate::app::music_grouping::ArtistKey;

/// Narrow grouped Music is painted by the mounted `MusicWorkspaceComponent`
/// now (task 3.8), so route the narrow characterization renders through the
/// real `Model::draw_frame` shell path.
fn render_narrow_music(app: App, width: u16, height: u16) -> String {
    let mut model = mounted_model_at(app, width, height);
    draw_mounted_frame(&mut model, width, height)
}

fn render_music_legacy(app: &mut App, width: u16, height: u16, _focused: bool) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut layout = Rect::default();
    terminal
        .draw(|f| {
            app.reserve_library_area(f, Rect::new(0, 0, width, height), &mut layout, None);
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn music_buffer_characterization_covers_wide_unfocused_narrow_and_selected_states() {
    // Wide grouped Music: the legacy base frame is now geometry-only — the
    // mounted `MusicWorkspaceComponent` is the sole painter (#613), so
    // `render_library` paints no grouped-album rows at the wide breakpoint.
    for (width, height, focused) in [(120, 30, true), (120, 30, false)] {
        let mut app = make_music_group_app();
        app.libs[0].nav_stack[1].set_resting_cursor(0);
        let output = render_music_legacy(&mut app, width, height, focused);
        assert!(
            !output.contains("First Album"),
            "wide legacy frame must not paint music rows in {width}x{height}: {output:?}"
        );
    }

    // Narrow grouped Music is painted by the mounted `MusicWorkspaceComponent`
    // now (task 3.8), reached through the full `Model::draw_frame` path.
    for (width, height) in [(60, 30), (60, 20)] {
        let mut app = make_music_group_app();
        app.libs[0].nav_stack[1].set_resting_cursor(0);
        let output = render_narrow_music(app, width, height);
        assert!(
            output.contains("First Album"),
            "narrow music row missing in {width}x{height}: {output:?}"
        );
    }
}

#[test]
fn narrow_grouped_music_shows_group_pill_bar() {
    // Task 3.6a: the narrow branch reserves a group pill row above the album
    // rows, mirroring narrow TV and the narrow browser. Before the fix the
    // narrow branch rendered album rows straight into the full area and never
    // published `selector_tabs`.
    let app = make_music_group_app();
    let mut model = mounted_model_at(app, 60, 30);
    let output = draw_mounted_frame(&mut model, 60, 30);
    let selector_tabs = model
        .application
        .get_component(&crate::app::components::ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .expect("Library panel mounted")
        .test_selector_hits()
        .regions()
        .to_vec();
    assert!(
        !selector_tabs.is_empty(),
        "narrow grouped Music must publish group selector pills:\n{output}"
    );
    assert!(
        output.contains("Beta"),
        "group pill labels must paint:\n{output}"
    );
}

// --- Grouped Music tree rows ---------------------------------------------

/// The Wide Library-panel fixture the Grouped Music tree row tests paint into.
const MUSIC_TREE_WIDE_WIDTH: u16 = 160;
const MUSIC_TREE_WIDE_HEIGHT: u16 = 40;

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

/// A character-column slice of a row (glyphs are single-width, so char index
/// matches display column here).
fn music_tree_row_slice(row: &str, start: usize, len: usize) -> String {
    row.chars().skip(start).take(len).collect()
}

fn music_tree_row_bg(term: &Terminal<TestBackend>, x: u16, y: u16) -> ratatui::style::Color {
    term.backend().buffer()[(x, y)].bg
}

/// The tree and canonical list use the same scrollbar painter, position, and
/// metrics. Rendering the shared widget separately makes this a focused buffer
/// contract rather than a glyph-only assertion.
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

/// The zebra fill the tree resolves for its rows: the focused Green2 fill
/// settles to the tree's `#272e33` fill when focus moves elsewhere.
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
    browser.expand_root(MUSIC_TREE_ALPHA_ROOT);
    browser.expand_root(MUSIC_TREE_BETA_ROOT);
    assert_eq!(browser.projection_len(), MUSIC_TREE_EXPANDED_PROJECTION_LEN);
    let long_leaf = music_tree_long_leaf_id(&browser);
    let long_row = music_tree_projection_row_of(&browser, long_leaf);
    // The stable album target the shell keys survives into the tree leaf.
    assert_eq!(browser.target_of(long_leaf), Some("album-long"));

    // Frame A rests at the top with the root selected, so the zebra rows
    // below it paint unselected.
    browser.select_index(MUSIC_TREE_ALPHA_ROOT);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let buf = term.backend().buffer();
    assert_eq!(browser.offset(), 0, "a top selection needs no scroll");

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
    let second_row_id = browser.projected_nodes()[1].id();
    match browser.hit_test(Position {
        x: list_area.x + 5,
        y: list_area.y + 1,
    }) {
        Some(TreeHit::Row { id, .. }) if id == second_row_id => {}
        other => panic!("expected the painted row's node, got {other:?}"),
    }
    assert!(matches!(
        browser.hit_test(Position {
            x: scrollbar_x,
            y: list_area.y + 1
        }),
        Some(TreeHit::VerticalScrollbar)
    ));

    // Clipping: scrolled so the unselected long album leaf paints at the
    // viewport's last row, it truncates with an ellipsis, its year sits
    // right-aligned inside the fixed six-column gutter with a two-column
    // trailing gap, and no glyph lands
    // outside the browser rectangle.
    browser.scroll_to((long_row + 1).saturating_sub(list_area.height as usize));
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let long_y = music_tree_row_y(&browser, list_area, long_row);
    let table_right = list_area.x + list_area.width - 1;
    let long_row_text = music_tree_row_text(&term, long_y, list_area.x, list_area.right());
    assert!(
        long_row_text.contains('\u{2026}'),
        "long title truncates: {long_row_text}"
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
    assert_music_tree_row_within(&term, long_y, list_area, MUSIC_TREE_WIDE_WIDTH);

    // Frame B: selecting the second Beta leaf scrolls it into view; its bar
    // spans the full row and overrides the group band, while Beta's first leaf
    // keeps the neighbouring unstriped phase.
    browser.select_index(MUSIC_TREE_BETA_LEAF_1);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let buf = term.backend().buffer();
    assert!(browser.offset() > 0, "the bottom Beta leaf forces a scroll");
    let beta_bar_y = music_tree_row_y(&browser, list_area, MUSIC_TREE_BETA_LEAF_1);
    for x in list_area.x..table_right {
        assert_eq!(
            buf[(x, beta_bar_y)].bg,
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
        "Beta leaf 0 keeps the neighbouring unstriped phase"
    );
    assert_ne!(palette::SELECTED_ROW_BG, fill);

    // Aggregate marks: one of Beta's two leaves marked leaves the Beta root
    // Partial; marking the second lifts it to Marked, and each aggregate state
    // paints its own semantic role on the root's title.
    browser.set_marked(MUSIC_TREE_BETA_LEAF_0, true);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    assert_eq!(
        browser.mark_state(MUSIC_TREE_BETA_ROOT),
        TreeMarkState::Partial
    );
    assert_eq!(
        term.backend().buffer()[(
            list_area.x + 2,
            music_tree_row_y(&browser, list_area, MUSIC_TREE_BETA_ROOT)
        )]
            .fg,
        palette::TEXT_ACCENT_MUTED,
        "a Partial artist root paints the muted aggregate role"
    );
    browser.set_marked(MUSIC_TREE_BETA_LEAF_1, true);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    assert_eq!(
        browser.mark_state(MUSIC_TREE_BETA_ROOT),
        TreeMarkState::Marked
    );
    assert_eq!(
        term.backend().buffer()[(
            list_area.x + 2,
            music_tree_row_y(&browser, list_area, MUSIC_TREE_BETA_ROOT)
        )]
            .fg,
        palette::STATUS_AVAILABLE,
        "a Marked artist root paints the positive aggregate role"
    );
}

/// The focused selected row marquees its title through the injected clock (no
/// sleeps) while an unfocused row truncates with an ellipsis instead.
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
    browser.expand_root(MUSIC_TREE_ALPHA_ROOT);
    let long_leaf = music_tree_long_leaf_id(&browser);
    let long_row = music_tree_projection_row_of(&browser, long_leaf);
    browser.select_index(long_row);

    // The marquee window strips to the row's name slot (after the hierarchy
    // plain-space indentation the crate composes).
    fn name_slot(row: &str) -> &str {
        row.trim_start()
    }

    // Hold phase: the window rests on the title's beginning — no ellipsis.
    browser.set_marquee_clock_for_test(MUSIC_TREE_LONG_TITLE, 0);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let resting_row = music_tree_row_text(
        &term,
        music_tree_row_y(&browser, list_area, long_row),
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

    // Seven steps past the hold: the window has travelled seven columns.
    browser.set_marquee_clock_for_test(MUSIC_TREE_LONG_TITLE, 600 + 150 * 7);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let scrolled_row = music_tree_row_text(
        &term,
        music_tree_row_y(&browser, list_area, long_row),
        list_area.x,
        list_area.right(),
    );
    let scrolled = name_slot(&scrolled_row);
    assert!(
        scrolled.starts_with(&MUSIC_TREE_LONG_TITLE[7..14]),
        "the focused title window advanced: {scrolled}"
    );

    // Unfocused, the same row truncates with an ellipsis instead of marqueeing.
    browser.set_focused(false);
    browser.set_marquee_clock_for_test(MUSIC_TREE_LONG_TITLE, 600 + 150 * 7);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let unfocused_row = music_tree_row_text(
        &term,
        music_tree_row_y(&browser, list_area, long_row),
        list_area.x,
        list_area.right(),
    );
    let unfocused = name_slot(&unfocused_row);
    assert!(
        unfocused.contains('\u{2026}'),
        "unfocused title truncates: {unfocused}"
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
    browser.expand_root(MUSIC_TREE_ALPHA_ROOT);
    browser.expand_root(MUSIC_TREE_BETA_ROOT);
    let table_right = list_area.x + list_area.width - 1;
    let zebra = music_tree_zebra_fill(true);
    assert_ne!(palette::SELECTED_ROW_BG, zebra, "the bar is not the stripe");

    // Focused single selection of Beta's first member: the bar spans
    // the whole content row (the scrollbar column keeps the parent
    // background) and overrides the zebra stripe, and the title keeps the
    // ordinary primary foreground with no bold modifier.
    browser.select_index(MUSIC_TREE_BETA_LEAF_0);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let selected_y = music_tree_row_y(&browser, list_area, MUSIC_TREE_BETA_LEAF_0);
    let buf = term.backend().buffer();
    for x in list_area.x..table_right {
        assert_eq!(
            buf[(x, selected_y)].bg,
            palette::SELECTED_ROW_BG,
            "selected-row bar reaches column {x}"
        );
    }
    let title_x = list_area.x + 6;
    assert_eq!(buf[(title_x, selected_y)].fg, palette::TEXT_PRIMARY);
    assert!(
        !buf[(title_x, selected_y)].modifier.contains(Modifier::BOLD),
        "the selected title is not bold"
    );

    // Artist roots are Heading-equivalent grouping rows and carry their own
    // group's band. Paint the Alpha header at the top so this assertion does
    // not depend on the selected Beta leaf's scrolled viewport.
    browser.select_index(MUSIC_TREE_ALPHA_ROOT);
    browser.scroll_to(0);
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
    browser.select_index(MUSIC_TREE_ALPHA_ROOT + 1);
    browser.scroll_to(0);
    let unfocused_zebra = music_tree_zebra_fill(false);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let cursor_y = music_tree_row_y(&browser, list_area, MUSIC_TREE_ALPHA_ROOT + 1);
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
    browser.select_index(MUSIC_TREE_BETA_LEAF_1);
    browser.set_marked(MUSIC_TREE_BETA_LEAF_0, true);
    let term = music_tree_frame(
        &mut browser,
        list_area,
        MUSIC_TREE_WIDE_WIDTH,
        MUSIC_TREE_WIDE_HEIGHT,
    );
    let marked_y = music_tree_row_y(&browser, list_area, MUSIC_TREE_BETA_LEAF_0);
    let buf = term.backend().buffer();
    for x in list_area.x..table_right {
        assert_eq!(
            buf[(x, marked_y)].bg,
            palette::SELECTED_ROW_BG,
            "multi-selected bar reaches column {x}"
        );
    }
    assert_ne!(
        buf[(
            title_x,
            music_tree_row_y(&browser, list_area, MUSIC_TREE_BETA_LEAF_1)
        )]
            .bg,
        palette::SELECTED_ROW_BG,
        "an unmarked, unfocused cursor row paints no bar"
    );
}

/// A controlled one-artist corpus for the exact gutter cases: a long artist
/// root, a long year-bearing album, and a long yearless album, all wider than
/// the row so every slot is exercised at the clipping boundary.
const GUTTER_ARTIST: &str = "The Long Collective Artist Name That Will Not Fit One Row";
const GUTTER_YEARED: &str = "A Yeared Album Title Long Enough To Want A Gutter";
const GUTTER_YEARLESS: &str = "A Yearless Album Title Long Enough To Fill The Row";
const GUTTER_YEAR: &str = "2007";

fn gutter_entries() -> Vec<MusicTreeEntry> {
    let key = ArtistKey::Service("gutter-artist".into());
    vec![
        MusicTreeEntry {
            artist: GUTTER_ARTIST.into(),
            artist_key: key.clone(),
            title: GUTTER_YEARED.into(),
            year: Some(GUTTER_YEAR.into()),
            target: "gutter-yeared".into(),
            semantic_state: MediaSemanticState::Ordinary,
        },
        MusicTreeEntry {
            artist: GUTTER_ARTIST.into(),
            artist_key: key,
            title: GUTTER_YEARLESS.into(),
            year: None,
            target: "gutter-yearless".into(),
            semantic_state: MediaSemanticState::Ordinary,
        },
    ]
}

#[test]
fn music_tree_ignores_played_rows_but_keeps_live_playback_emphasis() {
    let key = ArtistKey::Service("semantic-artist".into());
    let entries = vec![
        MusicTreeEntry {
            artist: "Semantic Artist".into(),
            artist_key: key.clone(),
            title: "Played Album".into(),
            year: None,
            target: "played-album".into(),
            semantic_state: MediaSemanticState::Played,
        },
        MusicTreeEntry {
            artist: "Semantic Artist".into(),
            artist_key: key,
            title: "Active Album".into(),
            year: None,
            target: "active-album".into(),
            semantic_state: MediaSemanticState::active(Some(50)),
        },
    ];
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&entries));
    browser.expand_root(browser.projected_nodes()[0].id());
    browser.set_focused(false);
    let area = Rect::new(0, 0, 40, 3);
    let term = music_tree_frame(&mut browser, area, 40, 3);
    let buf = term.backend().buffer();

    assert_eq!(
        buf[(4, 1)].fg,
        palette::TEXT_PRIMARY,
        "a played music row remains ordinary primary text"
    );
    assert_eq!(
        buf[(4, 2)].fg,
        palette::TEXT_EMPHASIS,
        "live playback keeps tree-row emphasis"
    );
}

/// The pinned year-gutter contract (design D8), painted through the crate's
/// label/column seams: one right-aligned fixed six-column `STATUS_AVAILABLE`
/// cell on a year-bearing album row followed by a two-column trailing gap,
/// reserved nowhere on the artist root or the yearless leaf (their titles
/// reach the last column), and no inline or second year column. The state
/// glyph and title roles are pinned here too.
#[test]
fn music_tree_year_gutter_is_reserved_only_on_the_album_that_carries_a_year() {
    const WIDTH: u16 = 40;
    const FRAME_WIDTH: u16 = WIDTH + 4;
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&gutter_entries()));
    let root = browser.projected_nodes()[0].id();
    assert!(
        browser.target_of(root).is_none(),
        "level 0 is the artist root"
    );
    browser.expand_root(root);
    assert_eq!(browser.projection_len(), 3);
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
    let gutter = (WIDTH - crate::app::components::music_tree::YEAR_GUTTER_WIDTH - 2) as usize;
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
    assert_eq!(buf[(0, 0)].fg, palette::MUSIC_HEADER, "root title role");

    // Year-bearing leaf: the title stops before the gutter, and the year
    // right-aligns in the fixed six-column `STATUS_AVAILABLE` cell before
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
    assert_eq!(buf[(4, 1)].fg, palette::TEXT_PRIMARY, "leaf title role");
    assert_eq!(
        buf[(gutter as u16 - 1, 1)].fg,
        palette::TEXT_PRIMARY,
        "an ordinary album leaf keeps the primary role"
    );
    assert_eq!(
        buf[(gutter as u16 + 4, 1)].fg,
        palette::STATUS_AVAILABLE,
        "the year paints in the STATUS_AVAILABLE role"
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
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&gutter_entries()));
    let root = browser.projected_nodes()[0].id();
    browser.expand_root(root);
    browser.select_index(1);
    browser.set_marquee_clock_for_test(GUTTER_YEARED, 0);

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
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&gutter_entries()));
    browser.set_focused(false);
    let root = browser.projected_nodes()[0].id();
    assert!(
        browser.target_of(root).is_none(),
        "level 0 is the artist root"
    );

    let term = music_tree_frame(&mut browser, area, WIDTH, 3);
    let collapsed_root_row = music_tree_row_text(&term, 0, 0, WIDTH);
    assert_eq!(collapsed_root_row.chars().next(), Some('T'));

    browser.expand_root(root);
    assert_eq!(browser.projection_len(), 3);
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

/// A controlled two-album corpus whose titles both overflow a narrow row, so
/// the shared marquee primitive can be observed to reset its clock when the
/// selection moves between distinct titles.
const RESET_TITLE_FIRST: &str = "A First Album Title Long Enough To Marquee Across The Browser Row";
const RESET_TITLE_SECOND: &str =
    "A Second Album Title Long Enough To Marquee Across The Browser Row";

fn reset_entries() -> Vec<MusicTreeEntry> {
    let key = ArtistKey::Service("reset-artist".into());
    vec![
        MusicTreeEntry {
            artist: "Reset Artist".into(),
            artist_key: key.clone(),
            title: RESET_TITLE_FIRST.into(),
            year: None,
            target: "reset-first".into(),
            semantic_state: MediaSemanticState::Ordinary,
        },
        MusicTreeEntry {
            artist: "Reset Artist".into(),
            artist_key: key,
            title: RESET_TITLE_SECOND.into(),
            year: None,
            target: "reset-second".into(),
            semantic_state: MediaSemanticState::Ordinary,
        },
    ]
}

/// The shared marquee clock keys on the marqueed title text: changing the
/// selection to a row with a different title resets the clock to the frame's
/// start, so the new title paints its hold window instead of inheriting the
/// previous row's scroll position. The clock is injected directly (no sleeps);
/// the hold window is what the reset produces.
#[test]
fn music_tree_title_clock_resets_when_the_selection_changes() {
    const WIDTH: u16 = 40;
    let area = Rect::new(0, 0, WIDTH, 3);
    let mut browser = MusicTreeBrowser::new(MusicTreeModel::from_entries(&reset_entries()));
    let root = browser.projected_nodes()[0].id();
    browser.expand_root(root);
    assert_eq!(browser.projection_len(), 3);
    let first = browser.projected_nodes()[1].id();
    let second = browser.projected_nodes()[2].id();

    fn name_slot(row: &str) -> &str {
        row.trim_start()
    }

    // Select the first title and inject a mid-scroll clock: its window has
    // travelled eight columns, not its hold start.
    browser.select_id(first);
    browser.set_marquee_clock_for_test(RESET_TITLE_FIRST, 600 + 150 * 8);
    let term = music_tree_frame(&mut browser, area, WIDTH, 3);
    let first_row = music_tree_row_text(&term, 1, 0, WIDTH);
    let first_name = name_slot(&first_row);
    assert!(
        first_name.starts_with(&RESET_TITLE_FIRST[8..20]),
        "the injected clock is mid-scroll: {first_name}"
    );

    // Select the second title without touching the clock. Its title text
    // differs, so the primitive resets the clock and it paints its hold
    // window from the start.
    browser.select_id(second);
    let term = music_tree_frame(&mut browser, area, WIDTH, 3);
    let second_row = music_tree_row_text(&term, 2, 0, WIDTH);
    let second_name = name_slot(&second_row);
    assert!(
        second_name.starts_with(&RESET_TITLE_SECOND[..12]),
        "the new title's clock reset to its hold start: {second_name}"
    );
}
