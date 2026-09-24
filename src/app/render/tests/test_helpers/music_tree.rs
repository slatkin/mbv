#![allow(dead_code, unused_imports)]

//! Shared Grouped Music tree fixtures: the stable-target constructors, the
//! plain fixture forests, the transition-surface shorthands, and the
//! buffer/state readers every Music tree contract test paints through.
//! The tree is driven only through the shared owner's stable targets, shared
//! transitions, retained geometry, and rendered output.

use super::super::*;
use super::draw_mounted_frame;
use super::mounted_model_at;
use crate::app::components::library_panel::{LibraryPanel, WideSkeletonGeometry};
use crate::app::components::list::tree_browser::{
    TreeBrowser, TreeMarkPolicy, TreeNode, TreeOperation, TreeTitleRole,
};
use crate::app::components::media_list::{
    queue_row_background, queue_row_zebra, MediaSemanticState,
};
use crate::app::components::music_tree_target::MusicTreeTarget;
use crate::app::components::ComponentId;
use crate::app::shell::Model;
use crate::app::state::music_grouping::ArtistKey;
use crate::app::App;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Terminal;
use std::time::{Duration, Instant};

/// A fixture album's stable tree target.
pub fn album(name: &str) -> MusicTreeTarget {
    MusicTreeTarget::Album(name.to_string())
}

/// A fixture Service artist root's stable tree target.
pub fn artist(key: &str) -> MusicTreeTarget {
    MusicTreeTarget::Artist(ArtistKey::Service(key.to_string()))
}

/// The projected artist root whose settled display name is `title` (the
/// fixture corpus groups by display identity, so its stable key is not
/// load-bearing).
pub fn artist_root(browser: &TreeBrowser<MusicTreeTarget>, title: &str) -> MusicTreeTarget {
    browser
        .roots()
        .into_iter()
        .find(|target| {
            target.is_artist()
                && browser.node(target).map(|node| node.title.as_str()) == Some(title)
        })
        .cloned()
        .expect("the settled artist root is projected")
}

/// The owner's aggregate mark state over a root's direct children, mirroring
/// the retired `TreeMarkState` read (the shared owner exposes membership, not
/// an aggregate cache).
#[derive(Debug, PartialEq, Eq)]
pub enum MarkState {
    None,
    Partial,
    Marked,
}

pub fn mark_state(browser: &TreeBrowser<MusicTreeTarget>, root: &MusicTreeTarget) -> MarkState {
    let children = browser.children_of(root).unwrap_or_default();
    let marked = children
        .iter()
        .filter(|child| browser.marked_targets().contains(child))
        .count();
    if marked == 0 {
        MarkState::None
    } else if marked == children.len() {
        MarkState::Marked
    } else {
        MarkState::Partial
    }
}

/// Mark one leaf through the shared transition surface, idempotently.
pub fn mark_leaf(browser: &mut TreeBrowser<MusicTreeTarget>, target: &MusicTreeTarget) {
    if !browser.marked_targets().contains(target) {
        browser.apply(TreeOperation::ToggleMarkTarget(target.clone()));
    }
}

/// Expand a root through the shared transition surface, idempotently.
pub fn expand_root(browser: &mut TreeBrowser<MusicTreeTarget>, root: &MusicTreeTarget) {
    if !browser.is_expanded(root) {
        browser.apply(TreeOperation::ToggleExpansionTarget(root.clone()));
    }
}

/// Select a stable target through the shared transition surface.
pub fn select_target(browser: &mut TreeBrowser<MusicTreeTarget>, target: &MusicTreeTarget) {
    browser.apply(TreeOperation::Select(target.clone()));
}

/// Inject the shared marquee clock (no sleeps): the marquee primitive resets
/// its start instant when the keyed title changes, so seeding the key and
/// instant is the state a real frame reaches after `elapsed_ms`.
pub fn set_marquee_clock(browser: &mut TreeBrowser<MusicTreeTarget>, title: &str, elapsed_ms: u64) {
    browser.set_marquee_started_at(
        title,
        Instant::now()
            .checked_sub(Duration::from_millis(elapsed_ms))
            .expect("the injected clock precedes the process start"),
    )
}

/// The painted buffer y of a target's row in the latest tree frame. The owner
/// resolves its own painted geometry, so the test never re-derives a row from
/// the projection index.
pub fn row_y(browser: &TreeBrowser<MusicTreeTarget>, target: &MusicTreeTarget) -> u16 {
    browser
        .row_rect_for(target)
        .expect("the target's row is painted in the latest frame")
        .y
}

/// The non-Wide Library-panel geometry the mounted `MusicWorkspaceComponent`
/// painted into.
pub fn mounted_music_narrow_geometry(model: &Model) -> WideSkeletonGeometry {
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

/// The Wide Library-panel fixture the Grouped Music tree row tests paint into.
pub const MUSIC_TREE_WIDE_WIDTH: u16 = 160;
pub const MUSIC_TREE_WIDE_HEIGHT: u16 = 40;

/// One tree frame: the shared owner painting into the mounted Library panel's
/// reserved browser rect on a fresh buffer.
pub fn music_tree_frame(
    browser: &mut TreeBrowser<MusicTreeTarget>,
    area: Rect,
    width: u16,
    height: u16,
) -> Terminal<TestBackend> {
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|f| tuirealm::component::Component::view(browser, f, area))
        .unwrap();
    term
}

pub fn music_tree_row_text(term: &Terminal<TestBackend>, y: u16, x0: u16, x1: u16) -> String {
    let buf = term.backend().buffer();
    (x0..x1).map(|x| buf[(x, y)].symbol().to_string()).collect()
}

/// Grouped Music deliberately paints no hierarchy or expansion symbols.
pub fn music_tree_hierarchy_glyph(c: char) -> bool {
    matches!(c, '>' | 'v' | '*' | '?' | '~' | '|' | '-' | '`')
}

/// Every visible node row keeps its title and uses only plain-space indentation.
/// The shared scrollbar is allowed in the same outside-column position used by
/// the canonical list painter.
pub fn assert_music_tree_row_within(
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
pub fn music_tree_row_slice(row: &str, start: usize, len: usize) -> String {
    row.chars().skip(start).take(len).collect()
}

pub fn music_tree_row_bg(term: &Terminal<TestBackend>, x: u16, y: u16) -> Color {
    term.backend().buffer()[(x, y)].bg
}

/// The tree and canonical list use the same scrollbar painter, position, and
/// metrics. Rendering the shared widget separately makes this a focused buffer
/// contract rather than a glyph-only assertion.
pub fn assert_music_tree_scrollbar_matches_shared(
    term: &Terminal<TestBackend>,
    browser: &TreeBrowser<MusicTreeTarget>,
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
                browser.visible_targets().len(),
                area.height as usize,
                browser.viewport_offset(),
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

pub fn assert_music_tree_title_fg(
    term: &Terminal<TestBackend>,
    area: Rect,
    y: u16,
    title: &str,
    expected: Color,
) {
    let row = music_tree_row_text(term, y, area.x, area.right());
    let x = row.find(title).expect("tree title painted") as u16 + area.x;
    assert_eq!(
        term.backend().buffer()[(x, y)].fg,
        expected,
        "tree title {title:?} keeps its selection foreground"
    );
}

/// The Queue row roles shared by the tree: base fill and zebra stripe.
pub fn music_tree_row_fill(focused: bool) -> Color {
    queue_row_background(focused)
}

pub fn music_tree_zebra_fill(focused: bool) -> Color {
    queue_row_zebra(focused)
}

/// The long album's stable tree target (its settled sort position is not
/// load-bearing).
pub fn music_tree_long_leaf() -> MusicTreeTarget {
    album("album-long")
}

/// Build a shared owner over a plain fixture projection.
pub fn tree_browser(projection: Vec<TreeNode<MusicTreeTarget>>) -> TreeBrowser<MusicTreeTarget> {
    let mut browser = TreeBrowser::new();
    browser.reconcile(projection).expect("fixture forest");
    browser
}

/// A plain node projection: one artist root over its album leaves.
pub fn album_leaf(
    parent: &MusicTreeTarget,
    target: &str,
    title: &str,
    trailing: Option<&str>,
    semantic_state: MediaSemanticState,
) -> TreeNode<MusicTreeTarget> {
    let node = TreeNode::new(
        album(target),
        Some(parent.clone()),
        title,
        title,
        semantic_state,
        TreeMarkPolicy::Direct,
    );
    let node = node.with_title_role(TreeTitleRole::Secondary);
    match trailing {
        Some(trailing) => node.with_trailing(trailing),
        None => node,
    }
}

/// A controlled one-artist corpus for the exact gutter cases: a long artist
/// root, a long year-bearing album, and a long yearless album, all wider than
/// the row so every slot is exercised at the clipping boundary.
pub const GUTTER_ARTIST: &str = "The Long Collective Artist Name That Will Not Fit One Row";
pub const GUTTER_YEARED: &str = "A Yeared Album Title Long Enough To Want A Gutter";
pub const GUTTER_YEARLESS: &str = "A Yearless Album Title Long Enough To Fill The Row";
pub const GUTTER_YEAR: &str = "2007";

pub fn gutter_projection() -> Vec<TreeNode<MusicTreeTarget>> {
    let root = artist("gutter-artist");
    vec![
        TreeNode::new(
            root.clone(),
            None,
            GUTTER_ARTIST,
            GUTTER_ARTIST,
            MediaSemanticState::Ordinary,
            TreeMarkPolicy::Aggregate,
        )
        .with_title_role(TreeTitleRole::Heading),
        album_leaf(
            &root,
            "gutter-yeared",
            GUTTER_YEARED,
            Some(GUTTER_YEAR),
            MediaSemanticState::Ordinary,
        ),
        album_leaf(
            &root,
            "gutter-yearless",
            GUTTER_YEARLESS,
            None,
            MediaSemanticState::Ordinary,
        ),
    ]
}

/// A controlled two-album corpus whose titles both overflow a narrow row, so
/// the shared marquee primitive can be observed to reset its clock when the
/// selection moves between distinct titles.
pub const RESET_TITLE_FIRST: &str =
    "A First Album Title Long Enough To Marquee Across The Browser Row";
pub const RESET_TITLE_SECOND: &str =
    "A Second Album Title Long Enough To Marquee Across The Browser Row";

pub fn reset_projection() -> Vec<TreeNode<MusicTreeTarget>> {
    let root = artist("reset-artist");
    vec![
        TreeNode::new(
            root.clone(),
            None,
            "Reset Artist",
            "Reset Artist",
            MediaSemanticState::Ordinary,
            TreeMarkPolicy::Aggregate,
        ),
        album_leaf(
            &root,
            "reset-first",
            RESET_TITLE_FIRST,
            None,
            MediaSemanticState::Ordinary,
        ),
        album_leaf(
            &root,
            "reset-second",
            RESET_TITLE_SECOND,
            None,
            MediaSemanticState::Ordinary,
        ),
    ]
}
