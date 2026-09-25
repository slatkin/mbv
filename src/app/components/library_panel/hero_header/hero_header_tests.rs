//! Wide Hero header characterization tests: artwork-box geometry, arm
//! derivation, title/meta role cycling, and the Workspace/text-starvation
//! shrink rules — gated behind the `#[cfg(test)]` declaration in `hero_header.rs`.

use super::super::content::{
    ArtworkShape, HeroArtwork, HeroContent, HeroFacts, HeroHeader, PanelList, Workspace,
};
use super::artwork_box::*;
use super::title_meta::*;
use crate::app::palette;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Frame;
use ratatui::Terminal;

/// A stub `PanelList` so a Workspace can be present for the shrink rule.
struct NoopList;
impl PanelList for NoopList {
    fn clamp_viewport(&mut self, _viewport_height: usize) {}

    fn set_paint_policy(
        &mut self,
        _policy: crate::app::components::library_panel::content::PanelListPaintPolicy,
    ) {
    }

    fn view(&mut self, _f: &mut Frame, _rect: Rect) {}
}

const AREA: Rect = Rect::new(0, 0, 60, 30);

fn facts(shape: ArtworkShape) -> HeroFacts {
    HeroFacts {
        title: "Dune".into(),
        meta_rows: vec!["2021".into()],
        duration_row: None,
        progress_row: None,
        links: Vec::new(),
        artwork: HeroArtwork {
            shape,
            source: None,
            decoration: None,
            image: crate::app::components::library_panel::content::HeroImageState::None,
        },
    }
}

fn content(shape: ArtworkShape) -> HeroContent<'static> {
    HeroContent {
        facts: facts(shape),
        overview: None,
        credits: None,
        workspace: None,
    }
}

fn draw_pane(width: u16, height: u16, pane: &HeroContent<'_>) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let area = Rect::new(0, 0, width, height);
    let pane_area = area;
    terminal
        .draw(|f| {
            paint_hero_pane_content(
                f,
                pane_area,
                pane,
                0,
                None,
                &mut crate::app::components::mouse::hit::HitRegions::new(),
                palette::Surface::HeroPane,
                height,
            );
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn text_in(buf: &ratatui::buffer::Buffer, area: Rect, needle: &str) -> bool {
    for y in area.top()..area.bottom() {
        let mut line = String::new();
        for x in area.left()..area.right() {
            line.push_str(buf[(x, y)].symbol());
        }
        if line.contains(needle) {
            return true;
        }
    }
    false
}

fn placeholder_fill() -> ratatui::style::Color {
    palette::surface_colors(palette::Surface::ArtworkPlaceholder, false).fill
}

#[test]
fn landscape_header_paints_art_above_text() {
    let pane = content(ArtworkShape::Landscape);
    let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
    let tall = Rect::new(0, 0, 113, 51);
    let capped = hero_artwork_box(tall, &pane.facts, false, tall.height);
    assert_eq!(capped.height, HERO_ARTWORK_MAX_ROWS);
    let tall_buf = draw_pane(tall.width, tall.height, &pane);
    assert_eq!(
        tall_buf[(capped.right() - 1, capped.bottom() - 1)].bg,
        placeholder_fill()
    );
    let buf = draw_pane(AREA.width, AREA.height, &pane);
    // The artwork box spans the full content width at the top.
    assert_eq!(artwork.x, AREA.x);
    assert_eq!(artwork.width, AREA.width);
    // The box carries the shared placeholder at its full size.
    assert_eq!(buf[(artwork.x + 1, artwork.y + 1)].bg, placeholder_fill());
    assert_eq!(
        buf[(artwork.right() - 1, artwork.bottom() - 1)].bg,
        placeholder_fill()
    );
    // The title paints below the box, never inside it.
    let below = Rect {
        y: artwork.bottom(),
        height: AREA.bottom() - artwork.bottom(),
        ..AREA
    };
    assert!(text_in(&buf, below, "Dune"));
    assert!(text_in(&buf, below, "2021"));
    let inside = (artwork.y..artwork.bottom())
        .map(|y| {
            (artwork.x..artwork.right())
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .any(|line| line.contains("Dune"));
    assert!(!inside, "no title inside the artwork box");
}

#[test]
fn square_header_paints_art_right_of_text() {
    let pane = content(ArtworkShape::Square);
    let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
    // A pane wide enough that the row cap, not the text column minimum, is
    // the binding constraint.
    let tall = Rect::new(0, 0, 100, 51);
    let capped = hero_artwork_box(tall, &pane.facts, false, tall.height);
    assert_eq!(capped.height, HERO_NON_LANDSCAPE_ARTWORK_MAX_ROWS);
    let tall_buf = draw_pane(tall.width, tall.height, &pane);
    assert_eq!(
        tall_buf[(capped.right() - 1, capped.bottom() - 1)].bg,
        placeholder_fill()
    );
    assert_eq!(
        hero_artwork_box(Rect::new(0, 0, 60, 20), &pane.facts, true, 20).height,
        14
    );
    let buf = draw_pane(AREA.width, AREA.height, &pane);
    // The artwork box is right-aligned; title and meta paint left of it.
    assert_eq!(artwork.right(), AREA.right());
    assert!(artwork.x > AREA.x + 12, "art is on the right");
    let text_area = Rect {
        width: artwork.x.saturating_sub(AREA.x + 1),
        ..AREA
    };
    assert!(text_in(&buf, text_area, "Dune"));
    assert!(text_in(&buf, text_area, "2021"));
    // The placeholder fills the whole box.
    assert_eq!(buf[(artwork.x + 1, artwork.y + 1)].bg, placeholder_fill());
    assert_eq!(
        buf[(artwork.right() - 1, artwork.bottom() - 1)].bg,
        placeholder_fill()
    );
}

#[test]
fn portrait_header_paints_art_right_of_text() {
    let pane = content(ArtworkShape::Portrait);
    let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
    let tall = Rect::new(0, 0, 60, 51);
    let capped = hero_artwork_box(tall, &pane.facts, false, tall.height);
    assert_eq!(capped.height, HERO_NON_LANDSCAPE_ARTWORK_MAX_ROWS);
    let tall_buf = draw_pane(tall.width, tall.height, &pane);
    assert_eq!(
        tall_buf[(capped.right() - 1, capped.bottom() - 1)].bg,
        placeholder_fill()
    );
    assert_eq!(
        hero_artwork_box(Rect::new(0, 0, 60, 20), &pane.facts, true, 20).height,
        14
    );
    let buf = draw_pane(AREA.width, AREA.height, &pane);
    assert_eq!(artwork.right(), AREA.right());
    let text_area = Rect {
        width: artwork.x.saturating_sub(AREA.x + 1),
        ..AREA
    };
    assert!(text_in(&buf, text_area, "Dune"));
    // The placeholder fills the whole box.
    assert_eq!(buf[(artwork.x + 1, artwork.y + 1)].bg, placeholder_fill());
}

/// A `Ready` projected image with `decoded` source pixels, on top of the
/// given declared policy shape — the agreement the painter must honour.
fn with_decoded_image(shape: ArtworkShape, decoded: (u32, u32)) -> HeroContent<'static> {
    HeroContent {
        facts: HeroFacts {
            title: "Dune".into(),
            meta_rows: vec!["2021".into()],
            duration_row: None,
            progress_row: None,
            links: Vec::new(),
            artwork: HeroArtwork {
                shape,
                source: None,
                decoration: None,
                image: crate::app::components::library_panel::content::HeroImageState::Ready {
                    cache_key: "k".into(),
                    decoded: Some(decoded),
                },
            },
        },
        overview: None,
        credits: None,
        workspace: None,
    }
}

/// The image the fetch actually resolved governs the arm: a series whose
/// policy pinned Landscape from a declared thumb but whose chain resolved
/// to the portrait poster paints the Portrait arm — title/meta beside a
/// right-aligned box, never stacked underneath the art.
#[test]
fn landscape_declared_series_with_a_portrait_poster_paints_text_beside_art() {
    let pane = with_decoded_image(ArtworkShape::Landscape, (200, 300));
    let artwork = hero_artwork_box(AREA, &pane.facts, false, AREA.height);
    // Right-aligned side-by-side box, never the full content width.
    assert_eq!(artwork.right(), AREA.right(), "art is right-aligned");
    assert!(
        artwork.width < AREA.width,
        "not the Landscape full-width box"
    );
    let buf = draw_pane(AREA.width, AREA.height, &pane);
    let beside = Rect {
        width: artwork.x.saturating_sub(AREA.x + 1),
        ..AREA
    };
    assert!(text_in(&buf, beside, "Dune"));
    assert!(text_in(&buf, beside, "2021"));
    let below = Rect {
        y: artwork.bottom(),
        height: AREA.bottom() - artwork.bottom(),
        ..AREA
    };
    assert!(!text_in(&buf, below, "Dune"), "no title stacked below");
}

/// Genuinely 16:9 artwork keeps the Landscape arm: full-width art above
/// the title/meta block.
#[test]
fn landscape_declared_series_with_a_landscape_thumb_keeps_art_above_text() {
    let pane = with_decoded_image(ArtworkShape::Landscape, (1600, 900));
    let artwork = hero_artwork_box(AREA, &pane.facts, false, AREA.height);
    assert_eq!(artwork.x, AREA.x);
    assert_eq!(artwork.width, AREA.width);
    let buf = draw_pane(AREA.width, AREA.height, &pane);
    let below = Rect {
        y: artwork.bottom(),
        height: AREA.bottom() - artwork.bottom(),
        ..AREA
    };
    assert!(text_in(&buf, below, "Dune"));
    assert!(text_in(&buf, below, "2021"));
}

/// Square artwork (music) keeps the Square arm: side-by-side, unchanged.
#[test]
fn square_policy_with_square_art_keeps_text_beside_art() {
    let pane = with_decoded_image(ArtworkShape::Square, (500, 500));
    let artwork = hero_artwork_box(AREA, &pane.facts, false, AREA.height);
    assert_eq!(artwork.right(), AREA.right());
    assert!(
        artwork.width < AREA.width,
        "not the Landscape full-width box"
    );
    let buf = draw_pane(AREA.width, AREA.height, &pane);
    let beside = Rect {
        width: artwork.x.saturating_sub(AREA.x + 1),
        ..AREA
    };
    assert!(text_in(&buf, beside, "Dune"));
    assert!(text_in(&buf, beside, "2021"));
}

#[test]
fn meta_rows_cycle_the_three_roles_across_the_grid() {
    let pane_facts = HeroFacts {
        title: "T".into(),
        meta_rows: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        duration_row: None,
        progress_row: None,
        links: Vec::new(),
        artwork: HeroArtwork {
            shape: ArtworkShape::Landscape,
            source: None,
            decoration: None,
            image: crate::app::components::library_panel::content::HeroImageState::None,
        },
    };
    let pane = HeroContent {
        facts: pane_facts,
        overview: None,
        credits: None,
        workspace: None,
    };
    let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
    let buf = draw_pane(AREA.width, AREA.height, &pane);
    // Grid entries [T, a, b, c, d]: title top-left, then alternating
    // columns — left cells keep the indent column, right cells end at
    // the area's right edge. Colours still cycle per meta row.
    let y0 = artwork.bottom() + 1;
    assert_eq!(buf[(AREA.x, y0)].symbol(), "T");
    assert_eq!(
        buf[(AREA.x, y0)].style().fg,
        Some(palette::TEXT_HERO_TITLE),
        "title colour"
    );
    let cells = [
        (AREA.right() - 1, y0, "a", palette::HERO_META_ROLES[0]),
        (AREA.x, y0 + 1, "b", palette::HERO_META_ROLES[1]),
        (AREA.right() - 1, y0 + 1, "c", palette::HERO_META_ROLES[2]),
        (AREA.x, y0 + 2, "d", palette::HERO_META_ROLES[0]),
    ];
    for (x, y, symbol, expected) in cells {
        assert_eq!(buf[(x, y)].symbol(), symbol);
        assert_eq!(
            buf[(x, y)].style().fg,
            Some(expected),
            "meta {symbol} colour"
        );
    }
    assert_eq!(
        buf[(AREA.x + AREA.width / 2, y0)].symbol(),
        " ",
        "right cell opens with alignment padding"
    );
}

/// The duration row paints the `DURATION` role (the sage) and the cycle
/// skips it, so the remaining rows keep adjacent cycle colours.
#[test]
fn duration_row_paints_the_duration_role_and_the_cycle_skips_it() {
    let mut pane_facts = facts(ArtworkShape::Landscape);
    pane_facts.meta_rows = vec!["a".into(), "b".into(), "c".into()];
    pane_facts.duration_row = Some(1);
    let pane = HeroContent {
        facts: pane_facts,
        overview: None,
        credits: None,
        workspace: None,
    };
    let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
    let buf = draw_pane(AREA.width, AREA.height, &pane);
    // Grid entries [Dune, a, b, c]: the duration row keeps its grid
    // cell and the cycle still skips it.
    let y0 = artwork.bottom() + 1;
    assert_eq!(
        buf[(AREA.right() - 1, y0)].style().fg,
        Some(palette::HERO_META_ROLES[0]),
        "row before the duration keeps its cycle colour"
    );
    assert_eq!(
        buf[(AREA.x, y0 + 1)].style().fg,
        Some(palette::DURATION),
        "duration row paints the DURATION role"
    );
    assert_eq!(buf[(AREA.x, y0 + 1)].symbol(), "b");
    assert_eq!(
        buf[(AREA.right() - 1, y0 + 1)].style().fg,
        Some(palette::HERO_META_ROLES[1]),
        "the cycle skips the duration row"
    );
}

/// The resume-percentage row paints the `PROGRESS_PERCENT` role (the
/// orange) and the cycle skips it, so the metadata rows around it keep
/// adjacent cycle colours.
#[test]
fn progress_row_paints_the_progress_role_and_the_cycle_skips_it() {
    let mut pane_facts = facts(ArtworkShape::Landscape);
    pane_facts.meta_rows = vec!["a".into(), "47%".into(), "c".into()];
    pane_facts.progress_row = Some(1);
    let pane = HeroContent {
        facts: pane_facts,
        overview: None,
        credits: None,
        workspace: None,
    };
    let artwork = hero_artwork_box(AREA, &pane.facts, pane.workspace.is_some(), AREA.height);
    let buf = draw_pane(AREA.width, AREA.height, &pane);
    // Grid entries [Dune, a, 47%, c]: the percentage row keeps its grid
    // cell and the cycle still skips it.
    let y0 = artwork.bottom() + 1;
    assert_eq!(
        buf[(AREA.right() - 1, y0)].style().fg,
        Some(palette::HERO_META_ROLES[0]),
        "row before the progress row keeps its cycle colour"
    );
    assert_eq!(buf[(AREA.x, y0 + 1)].symbol(), "4");
    assert_eq!(
        buf[(AREA.x, y0 + 1)].style().fg,
        Some(palette::PROGRESS_PERCENT),
        "the percentage row paints the progress role"
    );
    assert_eq!(
        buf[(AREA.right() - 1, y0 + 1)].style().fg,
        Some(palette::HERO_META_ROLES[1]),
        "the cycle skips the progress row"
    );
}

/// A text block too narrow for two columns keeps the stacked rows:
/// entries paint top to bottom in the left column, colours unchanged.
#[test]
fn narrow_landscape_falls_back_to_stacked_rows() {
    let pane = content(ArtworkShape::Landscape);
    let narrow = Rect::new(0, 0, 24, 30);
    assert!(
        landscape_grid_columns(narrow.width).is_none(),
        "24 columns fit no two text columns"
    );
    let artwork = hero_artwork_box(narrow, &pane.facts, pane.workspace.is_some(), narrow.height);
    let buf = draw_pane(narrow.width, narrow.height, &pane);
    let y0 = artwork.bottom() + 1;
    assert_eq!(buf[(narrow.x, y0)].symbol(), "D");
    assert_eq!(buf[(narrow.x, y0 + 1)].symbol(), "2");
    assert_eq!(
        buf[(narrow.x, y0 + 1)].style().fg,
        Some(palette::HERO_META_ROLES[0]),
        "stacked fallback keeps the cycle colour"
    );
}

#[test]
fn artwork_shrinks_before_a_workspace_viewport_drops() {
    let free = content(ArtworkShape::Landscape);
    let small = Rect::new(0, 0, 60, 14);
    let without_workspace =
        hero_artwork_box(small, &free.facts, free.workspace.is_some(), small.height);
    let mut workspace_list = NoopList;
    let constrained = HeroContent {
        facts: facts(ArtworkShape::Landscape),
        overview: None,
        credits: None,
        workspace: Some(Workspace {
            header: None,
            selector: None,
            list: &mut workspace_list,
            focused: false,
        }),
    };
    let with_workspace = hero_artwork_box(
        small,
        &constrained.facts,
        constrained.workspace.is_some(),
        small.height,
    );
    // The landscape box no longer fills the pane even without a
    // Workspace: it leaves room for the wrapped title/meta rows below
    // it, and a present Workspace caps it further so the Workspace
    // keeps its minimum rows.
    assert!(without_workspace.height < small.height);
    assert_eq!(
        with_workspace.height,
        small.height.saturating_sub(WORKSPACE_MIN_ROWS)
    );
}

#[test]
fn landscape_artwork_shrinks_before_the_title_meta_block_is_starved() {
    // A wide, short pane: the raw 16:9 box (113 wide → 32 rows) would
    // fill the whole pane and leave the title/meta block no rows.
    let pane = content(ArtworkShape::Landscape);
    let small = Rect::new(0, 0, 113, 19);
    let artwork = hero_artwork_box(small, &pane.facts, pane.workspace.is_some(), small.height);
    assert!(
        artwork.height < small.height,
        "artwork leaves room for the text block below it"
    );
    // The title and meta paint below the capped box.
    let buf = draw_pane(small.width, small.height, &pane);
    let below = Rect {
        y: artwork.bottom(),
        height: small.bottom() - artwork.bottom(),
        ..small
    };
    assert!(text_in(&buf, below, "Dune"));
    assert!(text_in(&buf, below, "2021"));
}

#[test]
fn short_pane_uses_compact_caps() {
    let landscape = content(ArtworkShape::Landscape);
    assert_eq!(
        hero_artwork_box(Rect::new(0, 0, 113, 50), &landscape.facts, false, 50).height,
        HERO_SHORT_LANDSCAPE_MAX_ROWS
    );
    let square = content(ArtworkShape::Square);
    assert_eq!(
        hero_artwork_box(Rect::new(0, 0, 100, 50), &square.facts, false, 50).height,
        HERO_SHORT_NON_LANDSCAPE_MAX_ROWS
    );
    let portrait = content(ArtworkShape::Portrait);
    assert_eq!(
        hero_artwork_box(Rect::new(0, 0, 60, 50), &portrait.facts, false, 50).height,
        HERO_SHORT_NON_LANDSCAPE_MAX_ROWS
    );
}

#[test]
fn header_arm_is_derived_from_the_policy_shape_only() {
    use super::super::content::HeroHeaderArm;
    // `HeroHeader::from` is the only constructor: the arm is always the
    // policy shape's arm (no destination-selectable constructor exists —
    // the field is private to the content module).
    assert_eq!(
        HeroHeader::from(ArtworkShape::Landscape).arm(),
        HeroHeaderArm::Landscape
    );
    assert_eq!(
        HeroHeader::from(ArtworkShape::Portrait).arm(),
        HeroHeaderArm::Portrait
    );
    assert_eq!(
        HeroHeader::from(ArtworkShape::Square).arm(),
        HeroHeaderArm::Square
    );
}
