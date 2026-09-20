//! Task 1.1 dependency gate: a one-frame spike of the locked
//! `tui-treelistview` 0.2.2 adapter (`components::music_tree`) at the
//! existing Wide Library-panel fixture and the smallest supported non-Wide
//! Library-panel width, proving through the crate's supported
//! model/state/renderer/style seams only: the full-row selected bar,
//! group-relative zebra, scrollbar, focused marquee, clipping, latest-render
//! hit testing, and aggregate marks. Any missing seam or failed proof here
//! rejects the dependency before tasks 1.2–6.5 (design D8).

use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use tui_treelistview::{TreeHit, TreeMarkState};

use super::test_helpers::{
    draw_mounted_frame, make_music_group_app, mounted_model_at, mounted_music_wide_geometry,
};
use super::*;
use crate::app::components::library_panel::{LibraryPanel, WideSkeletonGeometry};
use crate::app::components::media_list::MediaSemanticState;
use crate::app::components::music_tree::{MusicTreeBrowser, MusicTreeEntry, MusicTreeModel};
use crate::app::components::ComponentId;
use crate::app::music_grouping::ArtistKey;
use crate::app::shell::Model;
use crate::app::tests::make_item;
use crate::app::PanelFocus;

const WIDE_WIDTH: u16 = 160;
const WIDE_HEIGHT: u16 = 40;
/// The smallest supported non-Wide Library-panel width: below the
/// `TWO_COLUMN_THRESHOLD` (82), above the `MINI_VIEW_THRESHOLD` (80).
const NON_WIDE_WIDTH: u16 = crate::app::TWO_COLUMN_THRESHOLD - 1;
const NON_WIDE_HEIGHT: u16 = 30;

/// The long title the clipping and marquee proofs share; it cannot fit any
/// supported browser width.
const LONG_TITLE: &str = "A Suspiciously Long Album Title That Cannot Fit Any Library Browser Row";
const LONG_TITLE_YEAR: u32 = 1999;

/// Node ids of the spike projection (arena order follows the settled entry
/// order): the Alpha root, its 41 leaves, the Beta root, its 2 leaves.
const ALPHA_ROOT: usize = 0;
const BETA_ROOT: usize = 42;
const BETA_LEAF_0: usize = 43;
const BETA_LEAF_1: usize = 44;

/// The repository's existing Wide Grouped Music fixture corpus
/// (`music_app_many_albums`'s 40 Alpha albums) plus a long-titled album and a
/// second artist group, so the group-relative zebra reset has two groups to
/// cross.
fn spike_app() -> App {
    let mut app = make_music_group_app();
    app.panel_focus = PanelFocus::Library;
    let level = app.libs[0].nav_stack.last_mut().unwrap();
    for i in 1..40 {
        let mut album = make_item(&format!("Album {i:02}"), "MusicAlbum");
        album.id = format!("album-extra-{i}");
        album.artist = "Alpha".into();
        level.items.push(album);
    }
    let mut long_album = make_item(LONG_TITLE, "MusicAlbum");
    long_album.id = "album-long".into();
    long_album.artist = "Alpha".into();
    long_album.production_year = LONG_TITLE_YEAR;
    level.items.push(long_album);

    let mut beta_one = make_item("Beta Session", "MusicAlbum");
    beta_one.id = "album-beta-1".into();
    beta_one.artist = "Beta".into();
    level.items.push(beta_one);
    let mut beta_two = make_item("Beta Nights", "MusicAlbum");
    beta_two.id = "album-beta-2".into();
    beta_two.artist = "Beta".into();
    level.items.push(beta_two);

    level.total_count = level.items.len();
    app
}

/// The spike tree over the fixture's settled Grouped Music projection: the
/// same `MusicWideRenderCtx` facts (`album_info`/`album_order`/targets) the
/// production browser flattens, grouped into artist roots and album leaves.
fn spike_browser(model: &Model) -> MusicTreeBrowser {
    let ctx = model.app.wide_music_render_ctx(0, None);
    let entries: Vec<MusicTreeEntry> = ctx
        .album_order
        .iter()
        .map(|&index| {
            let (artist, year, name) = &ctx.album_info[index];
            MusicTreeEntry {
                artist: artist.clone(),
                // The spike fixture's settled order groups each artist's
                // albums consecutively, so the deterministic fallback key
                // reproduces the same roots as the real `ArtistItems`
                // identity would (task 2.3 wires the settled keys in).
                artist_key: ArtistKey::Fallback(artist.clone()),
                title: name.clone(),
                year: (!year.is_empty()).then(|| year.clone()),
                target: ctx.album_targets[index].clone(),
                // The spike projects the fixture's settled album facts; the
                // real pane derives this through `MediaSemanticState::from_emby`
                // (music collapse keeps it ordinary).
                semantic_state: MediaSemanticState::Ordinary,
            }
        })
        .collect();
    MusicTreeBrowser::new(MusicTreeModel::from_entries(&entries))
}

fn mounted_narrow_geometry(model: &Model) -> WideSkeletonGeometry {
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

/// One spike frame: the tree adapter painting into the mounted Library
/// panel's reserved browser rect on a fresh buffer.
fn render_one_frame(
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
fn row_y(browser: &MusicTreeBrowser, list_area: Rect, projection_row: usize) -> u16 {
    list_area.y + (projection_row - browser.offset()) as u16
}

fn row_text(term: &Terminal<TestBackend>, y: u16, x0: u16, x1: u16) -> String {
    let buf = term.backend().buffer();
    (x0..x1).map(|x| buf[(x, y)].symbol().to_string()).collect()
}

/// The tree's hierarchy/branch/state glyphs (the crate's Unicode set).
fn hierarchy_glyph(c: char) -> bool {
    matches!(c, '▶' | '▼' | '•' | '├' | '└' | '│' | '─')
}

/// Every visible node row keeps a hierarchy glyph and at least one title cell
/// and paints nothing past the browser rect (task 3.1's row contract).
fn assert_hierarchy_and_title_within(
    term: &Terminal<TestBackend>,
    row_y: u16,
    list_area: Rect,
    frame_width: u16,
) {
    let row = row_text(term, row_y, list_area.x, list_area.right());
    assert!(
        row.chars().any(hierarchy_glyph),
        "row keeps a hierarchy glyph: {row:?}"
    );
    assert!(
        row.chars()
            .any(|c| !c.is_whitespace() && !hierarchy_glyph(c) && c != '…'),
        "row keeps at least one title cell: {row:?}"
    );
    let outside = row_text(term, row_y, list_area.right(), frame_width);
    assert!(
        outside.chars().all(|c| c == ' '),
        "nothing overruns the browser rect: {outside:?}"
    );
}

/// A character-column slice of a row (glyphs are single-width, so char index
/// matches display column here).
fn row_slice(row: &str, start: usize, len: usize) -> String {
    row.chars().skip(start).take(len).collect()
}

fn row_bg(term: &Terminal<TestBackend>, x: u16, y: u16) -> ratatui::style::Color {
    term.backend().buffer()[(x, y)].bg
}

/// The zebra fill the tree resolves for its rows: the canonical grouped
/// list's fixed resting-content Storm in both focus states.
fn zebra_fill(focused: bool) -> ratatui::style::Color {
    palette::surface_colors(palette::Surface::SidebarBody, focused).fill
}

/// The expanded spike projection: one Alpha root over 41 leaves, one Beta
/// root over 2 leaves.
fn expanded_projection_len() -> usize {
    45
}

/// The long album's node id, found by its title in the arena (its settled
/// sort position is not load-bearing).
fn long_leaf_id(browser: &MusicTreeBrowser) -> usize {
    (0..browser.projection_len() * 2)
        .find(|&id| browser.title_of(id) == LONG_TITLE)
        .expect("long album interned")
}

/// A node's projection row, if visible in the settled projection.
fn projection_row_of(browser: &MusicTreeBrowser, id: usize) -> usize {
    browser
        .projected_nodes()
        .iter()
        .position(|node| node.id() == id)
        .expect("node projected")
}

#[test]
fn wide_fixture_spike_proves_the_visual_contracts() {
    let mut model = mounted_model_at(spike_app(), WIDE_WIDTH, WIDE_HEIGHT);
    let _ = draw_mounted_frame(&mut model, WIDE_WIDTH, WIDE_HEIGHT);
    let list_area = mounted_music_wide_geometry(&model).list_area;
    assert!(
        list_area.width > 10 && list_area.height > 4,
        "wide browser rect reserved"
    );

    let mut browser = spike_browser(&model);
    browser.expand_root(ALPHA_ROOT);
    browser.expand_root(BETA_ROOT);
    assert_eq!(browser.projection_len(), expanded_projection_len());
    let long_leaf = long_leaf_id(&browser);
    let long_row = projection_row_of(&browser, long_leaf);
    // The stable album target the shell keys survives into the tree leaf.
    assert_eq!(browser.target_of(long_leaf), Some("album-long"));

    // Frame A rests at the top with the root selected, so the zebra rows
    // below it paint unselected.
    browser.select_index(ALPHA_ROOT);
    let term = render_one_frame(&mut browser, list_area, WIDE_WIDTH, WIDE_HEIGHT);
    let buf = term.backend().buffer();
    assert_eq!(browser.offset(), 0, "a top selection needs no scroll");

    // The selected artist root keeps its hierarchy glyph and at least one
    // title cell and paints nothing past the browser rect (task 3.1).
    assert_hierarchy_and_title_within(&term, list_area.y, list_area, WIDE_WIDTH);
    // The overflowing projection's scrollbar takes the `SCROLLBAR` role; the
    // crate exposes no scrollbar style field, so the block style's foreground
    // (applied to the whole browser area) is what its unstyled scrollbar
    // inherits.
    let scrollbar_cell = &buf[(list_area.right() - 1, list_area.y + 1)];
    assert_ne!(scrollbar_cell.symbol(), " ");
    assert_eq!(
        scrollbar_cell.fg,
        palette::SCROLLBAR,
        "the crate's scrollbar takes the SCROLLBAR role"
    );

    // Group-relative zebra: each group's first member takes the secondary
    // fill, every second member reverts, and the phase resets across the
    // group boundary (Beta's first leaf stripes again after Alpha's 41).
    let fill = zebra_fill(true);
    let probe_x = list_area.x + 10;
    assert_eq!(
        row_bg(&term, probe_x, list_area.y + 1),
        fill,
        "Alpha leaf 0 stripes"
    );
    assert_ne!(
        row_bg(&term, probe_x, list_area.y + 2),
        fill,
        "Alpha leaf 1 rests"
    );

    // Scrollbar: the overflowing projection paints the crate's vertical
    // scrollbar in the column right of the table.
    assert_ne!(
        buf[(list_area.right() - 1, list_area.y + 1)].symbol(),
        " ",
        "vertical scrollbar painted"
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
            x: list_area.right() - 1,
            y: list_area.y + 1
        }),
        Some(TreeHit::VerticalScrollbar)
    ));

    // Clipping: scrolled so the unselected long album leaf paints at the
    // viewport's last row, it truncates with an ellipsis, its year sits
    // right-aligned inside the fixed six-column gutter, and no glyph lands
    // outside the browser rectangle.
    browser.scroll_to((long_row + 1).saturating_sub(list_area.height as usize));
    let term = render_one_frame(&mut browser, list_area, WIDE_WIDTH, WIDE_HEIGHT);
    let long_y = row_y(&browser, list_area, long_row);
    let table_right = list_area.x + list_area.width - 1;
    let long_row_text = row_text(&term, long_y, list_area.x, list_area.right());
    assert!(
        long_row_text.contains('\u{2026}'),
        "long title truncates: {long_row_text}"
    );
    let gutter_start = table_right - crate::app::components::music_tree::YEAR_GUTTER_WIDTH;
    let gutter_text: String = row_text(&term, long_y, gutter_start, table_right)
        .trim()
        .to_string();
    assert_eq!(
        gutter_text,
        LONG_TITLE_YEAR.to_string(),
        "year right-aligned in the fixed gutter"
    );
    assert_hierarchy_and_title_within(&term, long_y, list_area, WIDE_WIDTH);

    // Frame B: selecting the second Beta leaf scrolls it into view; its bar
    // spans the full row and overrides the zebra, and Beta's first leaf
    // stripes again above it (the group-relative reset).
    browser.select_index(BETA_LEAF_1);
    let term = render_one_frame(&mut browser, list_area, WIDE_WIDTH, WIDE_HEIGHT);
    let buf = term.backend().buffer();
    assert!(browser.offset() > 0, "the bottom Beta leaf forces a scroll");
    let beta_bar_y = row_y(&browser, list_area, BETA_LEAF_1);
    for x in list_area.x..table_right {
        assert_eq!(
            buf[(x, beta_bar_y)].bg,
            palette::SELECTED_ROW_BG,
            "selected-row bar reaches column {x}"
        );
    }
    assert_eq!(
        row_bg(&term, probe_x, row_y(&browser, list_area, BETA_LEAF_0)),
        fill,
        "Beta leaf 0 stripes again"
    );
    assert_ne!(palette::SELECTED_ROW_BG, fill);

    // Aggregate marks: one of Beta's two leaves marked leaves the Beta root
    // Partial; marking the second lifts it to Marked, and each aggregate state
    // paints its own semantic role on the root's title.
    browser.set_marked(BETA_LEAF_0, true);
    let term = render_one_frame(&mut browser, list_area, WIDE_WIDTH, WIDE_HEIGHT);
    assert_eq!(browser.mark_state(BETA_ROOT), TreeMarkState::Partial);
    assert_eq!(
        term.backend().buffer()[(list_area.x + 2, row_y(&browser, list_area, BETA_ROOT))].fg,
        palette::TEXT_ACCENT_MUTED,
        "a Partial artist root paints the muted aggregate role"
    );
    browser.set_marked(BETA_LEAF_1, true);
    let term = render_one_frame(&mut browser, list_area, WIDE_WIDTH, WIDE_HEIGHT);
    assert_eq!(browser.mark_state(BETA_ROOT), TreeMarkState::Marked);
    assert_eq!(
        term.backend().buffer()[(list_area.x + 2, row_y(&browser, list_area, BETA_ROOT))].fg,
        palette::STATUS_AVAILABLE,
        "a Marked artist root paints the positive aggregate role"
    );
}

#[test]
fn wide_fixture_marquee_scrolls_the_focused_selected_title() {
    let mut model = mounted_model_at(spike_app(), WIDE_WIDTH, WIDE_HEIGHT);
    let _ = draw_mounted_frame(&mut model, WIDE_WIDTH, WIDE_HEIGHT);
    let list_area = mounted_music_wide_geometry(&model).list_area;

    let mut browser = spike_browser(&model);
    browser.expand_root(ALPHA_ROOT);
    let long_leaf = long_leaf_id(&browser);
    let long_row = projection_row_of(&browser, long_leaf);
    browser.select_index(long_row);

    // The marquee window strips to the row's name slot (after the hierarchy
    // glyph prefix the crate composes).
    fn name_slot(row: &str) -> &str {
        row.split_once("\u{2022} ")
            .map(|(_, rest)| rest)
            .unwrap_or(row)
    }

    // Hold phase: the window rests on the title's beginning — no ellipsis.
    browser.set_marquee_clock_for_test(LONG_TITLE, 0);
    let term = render_one_frame(&mut browser, list_area, WIDE_WIDTH, WIDE_HEIGHT);
    let resting_row = row_text(
        &term,
        row_y(&browser, list_area, long_row),
        list_area.x,
        list_area.right(),
    );
    let resting = name_slot(&resting_row);
    assert!(
        resting.starts_with(&LONG_TITLE[..12]),
        "hold shows the title start: {resting}"
    );
    assert!(
        !resting.contains('\u{2026}'),
        "the marquee window carries no ellipsis"
    );

    // Seven steps past the hold: the window has travelled seven columns.
    browser.set_marquee_clock_for_test(LONG_TITLE, 600 + 150 * 7);
    let term = render_one_frame(&mut browser, list_area, WIDE_WIDTH, WIDE_HEIGHT);
    let scrolled_row = row_text(
        &term,
        row_y(&browser, list_area, long_row),
        list_area.x,
        list_area.right(),
    );
    let scrolled = name_slot(&scrolled_row);
    assert!(
        scrolled.starts_with(&LONG_TITLE[7..14]),
        "the focused title window advanced: {scrolled}"
    );

    // Unfocused, the same row truncates with an ellipsis instead of marqueeing.
    browser.set_focused(false);
    browser.set_marquee_clock_for_test(LONG_TITLE, 600 + 150 * 7);
    let term = render_one_frame(&mut browser, list_area, WIDE_WIDTH, WIDE_HEIGHT);
    let unfocused_row = row_text(
        &term,
        row_y(&browser, list_area, long_row),
        list_area.x,
        list_area.right(),
    );
    let unfocused = name_slot(&unfocused_row);
    assert!(
        unfocused.contains('\u{2026}'),
        "unfocused title truncates: {unfocused}"
    );
}

#[test]
fn non_wide_fixture_spike_proves_the_visual_contracts() {
    let mut model = mounted_model_at(spike_app(), NON_WIDE_WIDTH, NON_WIDE_HEIGHT);
    let _ = draw_mounted_frame(&mut model, NON_WIDE_WIDTH, NON_WIDE_HEIGHT);
    let list_area = mounted_narrow_geometry(&model).list_area;
    assert!(
        list_area.width > 10 && list_area.height > 4,
        "non-Wide browser rect reserved"
    );

    let mut browser = spike_browser(&model);
    browser.expand_root(ALPHA_ROOT);
    browser.expand_root(BETA_ROOT);
    assert_eq!(browser.projection_len(), expanded_projection_len());
    let long_leaf = long_leaf_id(&browser);
    let long_row = projection_row_of(&browser, long_leaf);

    // Frame A rests at the top with the root selected, so the Alpha zebra
    // rows below paint unselected.
    browser.select_index(ALPHA_ROOT);
    let term = render_one_frame(&mut browser, list_area, NON_WIDE_WIDTH, NON_WIDE_HEIGHT);
    let buf = term.backend().buffer();
    assert_hierarchy_and_title_within(&term, list_area.y, list_area, NON_WIDE_WIDTH);
    assert_eq!(
        buf[(list_area.right() - 1, list_area.y + 1)].fg,
        palette::SCROLLBAR,
        "the narrow scrollbar takes the SCROLLBAR role"
    );

    // The Alpha phase alternates from its first member.
    let fill = zebra_fill(true);
    let probe_x = list_area.x + 10;
    assert_eq!(
        row_bg(&term, probe_x, list_area.y + 1),
        fill,
        "Alpha leaf 0 stripes"
    );
    assert_ne!(
        row_bg(&term, probe_x, list_area.y + 2),
        fill,
        "Alpha leaf 1 rests"
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
    let term = render_one_frame(&mut browser, list_area, NON_WIDE_WIDTH, NON_WIDE_HEIGHT);
    let long_y = row_y(&browser, list_area, long_row);
    let table_right = list_area.x + list_area.width - 1;
    let long_row_text = row_text(&term, long_y, list_area.x, list_area.right());
    assert!(
        long_row_text.contains('\u{2026}'),
        "long title truncates at {NON_WIDE_WIDTH}: {long_row_text}"
    );
    let gutter_start = table_right - crate::app::components::music_tree::YEAR_GUTTER_WIDTH;
    let gutter_text: String = row_text(&term, long_y, gutter_start, table_right)
        .trim()
        .to_string();
    assert_eq!(
        gutter_text,
        LONG_TITLE_YEAR.to_string(),
        "year right-aligned in the fixed gutter"
    );
    let outside: String = row_text(&term, long_y, list_area.right(), NON_WIDE_WIDTH);
    assert!(
        outside.chars().all(|c| c == ' '),
        "nothing overruns the browser rect: '{outside}'"
    );
    assert_hierarchy_and_title_within(&term, long_y, list_area, NON_WIDE_WIDTH);

    // Scrollbar on the overflowing projection.
    assert_ne!(buf[(list_area.right() - 1, list_area.y + 1)].symbol(), " ");

    // Frame B: the second Beta leaf's bar spans the narrower full row, and
    // Beta's first leaf stripes again above it.
    browser.select_index(BETA_LEAF_1);
    let term = render_one_frame(&mut browser, list_area, NON_WIDE_WIDTH, NON_WIDE_HEIGHT);
    let buf = term.backend().buffer();
    for x in list_area.x..table_right {
        assert_eq!(
            buf[(x, row_y(&browser, list_area, BETA_LEAF_1))].bg,
            palette::SELECTED_ROW_BG,
            "selected-row bar reaches column {x}"
        );
    }
    assert_eq!(
        row_bg(&term, probe_x, row_y(&browser, list_area, BETA_LEAF_0)),
        fill,
        "Beta leaf 0 stripes again"
    );

    // Stale geometry cannot claim input: an empty-area render invalidates the
    // latest hit map.
    let mut stale = Terminal::new(TestBackend::new(NON_WIDE_WIDTH, NON_WIDE_HEIGHT)).unwrap();
    stale.draw(|f| browser.view(f, Rect::ZERO)).unwrap();
    assert_eq!(
        browser.hit_test(Position {
            x: list_area.x + 5,
            y: list_area.y + 1
        }),
        None
    );

    // Aggregate marks: both Beta leaves marked lift the Beta root to Marked.
    browser.set_marked(BETA_LEAF_0, true);
    browser.set_marked(BETA_LEAF_1, true);
    render_one_frame(&mut browser, list_area, NON_WIDE_WIDTH, NON_WIDE_HEIGHT);
    assert_eq!(browser.mark_state(BETA_ROOT), TreeMarkState::Marked);
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

/// The pinned year-gutter contract (design D8), painted through the crate's
/// label/column seams: one right-aligned fixed six-column `STATUS_AVAILABLE`
/// cell on a year-bearing album row, reserved nowhere on the artist root or
/// the yearless leaf (their titles reach the last column), and no inline or
/// second year column. The state glyph and title roles are pinned here too.
#[test]
fn year_gutter_is_reserved_only_on_the_album_that_carries_a_year() {
    const WIDTH: u16 = 40;
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
    let term = render_one_frame(&mut browser, area, WIDTH, 3);
    let buf = term.backend().buffer();

    // Three rows in three lines never overflow, so no scrollbar takes a
    // column: the tree column is the full browser width and the gutter is its
    // last six columns.
    let gutter = (WIDTH - crate::app::components::music_tree::YEAR_GUTTER_WIDTH) as usize;
    let rows: Vec<String> = (0..3).map(|y| row_text(&term, y, 0, WIDTH)).collect();

    // Artist root: an ordinary grouping row in the metadata role, its long
    // title using the full width (no gutter reserved).
    let root_row = &rows[0];
    assert!(
        root_row.starts_with('▼'),
        "the expanded root keeps its state glyph: {root_row:?}"
    );
    assert_eq!(
        root_row.chars().last(),
        Some('…'),
        "the root title reaches the last column (no gutter): {root_row:?}"
    );
    assert_eq!(buf[(0, 0)].fg, palette::TEXT_MUTED, "root state glyph role");
    assert_eq!(buf[(2, 0)].fg, palette::TEXT_METADATA, "root title role");

    // Year-bearing leaf: the title stops before the gutter, and the year
    // right-aligns in the fixed six-column `STATUS_AVAILABLE` cell.
    let yeared_row = &rows[1];
    assert!(
        yeared_row.starts_with("├── • "),
        "the leaf keeps its guides and state glyph: {yeared_row:?}"
    );
    assert_eq!(
        row_slice(yeared_row, gutter, 6),
        format!("{GUTTER_YEAR:>6}"),
        "the year is right-aligned in the last six columns: {yeared_row:?}"
    );
    assert_eq!(
        yeared_row.chars().nth(gutter - 1),
        Some('…'),
        "the yeared title truncates before the gutter: {yeared_row:?}"
    );
    assert_eq!(buf[(4, 1)].fg, palette::TEXT_MUTED, "leaf state glyph role");
    assert_eq!(
        buf[(gutter as u16 - 1, 1)].fg,
        palette::TEXT_EMPHASIS,
        "an ordinary album leaf keeps the emphasis role"
    );
    assert_eq!(
        buf[(gutter as u16 + 2, 1)].fg,
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
    let yearless_gutter = row_slice(yearless_row, gutter, 6);
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
