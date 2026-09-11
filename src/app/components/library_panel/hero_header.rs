//! The Wide Hero header (task 5.5, design D5): three arms — Landscape
//! (16:9 artwork full content width above title/meta), Portrait (2:3) and
//! Square (1:1) (title/meta left, artwork right) — behind one title/meta
//! painter that colours meta row *n* with `HERO_META_ROLES[n % 3]` and owns
//! truncation and wrapping. The arm comes only from the artwork policy's
//! shape (`HeroHeader::from`), never from a destination. The artwork box is
//! sized by the paint-free [`hero_artwork_box`], the one layout site the
//! shell projection (task 5.10) shares with the painter.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::palette;
use crate::app::render::{
    paint_wide_hero_text, render_artwork_placeholder, wide_hero_hero_content_box,
    wrap_overview_lines, WrappedHeroLine, PANE_PAD_X, PANE_PAD_Y,
};

use super::content::{HeroContent, HeroFacts, HeroHeader};

/// Blank rows between the artwork box and the text block (the shared
/// `wide_hero_slots` convention: metadata starts at `img_area.bottom() + 1`).
const ARTWORK_TEXT_GAP_ROWS: u16 = 1;

/// Vertical room a present Workspace keeps below the header (design D5: the
/// artwork shrinks before a Workspace viewport would drop). Two padding rows
/// plus a few visible list rows.
const WORKSPACE_MIN_ROWS: u16 = 6;

/// Minimum columns the title/meta block keeps beside a right-aligned artwork
/// box (Portrait/Square arms).
const HERO_MIN_TEXT_COLS: u16 = 16;

/// The paint-free artwork box for one hero pane's content area (design D5):
/// Landscape fills the content width at the top with its 16:9 height; the
/// Portrait (2:3) and Square (1:1) boxes are sized from the available height
/// and right-aligned, capped so the text block keeps room. A present
/// Workspace caps the box height first, so the artwork shrinks before a
/// Workspace viewport would drop.
///
/// One layout site: the header painter calls this and the shell projection
/// (task 5.10) calls it with the Library panel's area, so the fetched image
/// is always encoded for the box that paints it.
pub(in crate::app) fn hero_artwork_box(area: Rect, content: &HeroContent<'_>) -> Rect {
    let header = HeroHeader::from(content.facts.artwork.shape);
    // The artwork shrinks before a Workspace viewport would drop (design D5).
    let max_h = if content.workspace.is_some() {
        area.height.saturating_sub(WORKSPACE_MIN_ROWS)
    } else {
        area.height
    };
    let (width, height) = match header.arm() {
        super::content::HeroHeaderArm::Landscape => {
            // 16:9 in terminal cells (cells are ~2x taller than wide).
            let h = (area.width.saturating_mul(9).saturating_add(31) / 32).max(1);
            (area.width, h.min(max_h))
        }
        super::content::HeroHeaderArm::Portrait => box_from_height(max_h, 4, 3, area),
        super::content::HeroHeaderArm::Square => box_from_height(max_h, 2, 1, area),
    };
    Rect {
        x: area.right().saturating_sub(width).max(area.x),
        y: area.y,
        width: width.min(area.width),
        height,
    }
}

/// Right-aligned box of `numerator:denominator` display aspect, sized from
/// `max_h` and capped so the text block keeps [`HERO_MIN_TEXT_COLS`] columns.
/// Cell aspect (~2x taller than wide) is folded into the ratio: a 2:3
/// portrait box is `h*4/3` cells wide, a square `h*2`.
fn box_from_height(max_h: u16, num: u16, den: u16, area: Rect) -> (u16, u16) {
    let cap_w = area
        .width
        .saturating_sub(HERO_MIN_TEXT_COLS)
        .min(area.width);
    let w = ((max_h.saturating_mul(num).saturating_add(den - 1)) / den).min(cap_w);
    let h = (w.saturating_mul(den) / num).min(max_h);
    (w, h)
}

/// Paints the Hero header and — when overview text exists — the overview
/// Main content box below it (design D5). One title/meta painter for all
/// three arms, truncation and wrapping owned here; meta row *n* is coloured
/// `HERO_META_ROLES[n % 3]`. The artwork box renders the shared placeholder
/// at full box size (the projected image paints the same rect once the
/// projection lands, task 5.10). Returns the first unpainted row, the anchor
/// the skeleton places the Workspace below.
pub(in crate::app) fn paint_hero_pane_content(
    f: &mut Frame,
    area: Rect,
    content: &HeroContent<'_>,
) -> u16 {
    let header = HeroHeader::from(content.facts.artwork.shape);
    let artwork = hero_artwork_box(area, content);
    if artwork.width > 0 && artwork.height > 0 {
        // Placeholder at full box size while loading (spec: the artwork box
        // keeps its size and shows the shared placeholder).
        render_artwork_placeholder(f, artwork);
    }

    let text_area = match header.arm() {
        super::content::HeroHeaderArm::Landscape => Rect {
            y: artwork.bottom().saturating_add(ARTWORK_TEXT_GAP_ROWS),
            height: area
                .bottom()
                .saturating_sub(artwork.bottom().saturating_add(ARTWORK_TEXT_GAP_ROWS)),
            ..area
        },
        // Portrait/Square: title/meta left of the artwork box.
        _ => Rect {
            width: artwork.x.saturating_sub(area.x).saturating_sub(1),
            ..area
        },
    };
    let mut next_row = paint_title_and_meta(f, text_area, &content.facts);
    // The header's painted bottom edge includes a right-side artwork box the
    // text block may not reach.
    if header.arm() != super::content::HeroHeaderArm::Landscape {
        next_row = next_row.max(artwork.bottom());
    }

    // The overview Main content box only when overview text exists (design
    // D5); without it the Workspace moves up.
    if let Some(overview) = content
        .overview
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        let content_w = area.width.saturating_sub(PANE_PAD_X * 4) as usize;
        let lines = wrap_overview_lines(overview, |_| content_w);
        let room = area.bottom().saturating_sub(next_row);
        let box_height = (lines.len() as u16)
            .max(1)
            .saturating_add(PANE_PAD_Y * 2)
            .min(room);
        // Room only for padding: no box renders (the shared padding is part
        // of the box's reserved rows).
        if box_height > PANE_PAD_Y * 2 {
            let box_area = Rect {
                y: next_row,
                height: box_height,
                ..area
            };
            let (_, box_content) = wide_hero_hero_content_box(f, box_area);
            // Plain text, never destination-styled; the hero pane's own focus
            // (derived from the Workspace, design D6) picks the row colour.
            let focused = content
                .workspace
                .as_ref()
                .is_some_and(|workspace| workspace.focused);
            let fg = if focused {
                palette::TEXT_STRONG
            } else {
                palette::TEXT_MUTED
            };
            let lines: Vec<WrappedHeroLine<'_>> = lines
                .iter()
                .map(|line| WrappedHeroLine {
                    text: line,
                    style: ratatui::style::Style::default().fg(fg),
                })
                .collect();
            paint_wide_hero_text(f, box_content, &lines);
            next_row = box_area.bottom();
        }
    }
    next_row
}

/// The one title/meta painter for all three arms: the title in
/// [`palette::TEXT_STRONG`], then meta row *n* in `HERO_META_ROLES[n % 3]`,
/// wrapped to the text block's width (truncation and wrapping owned here,
/// design D5). Returns the first unpainted row.
fn paint_title_and_meta(f: &mut Frame, area: Rect, facts: &HeroFacts) -> u16 {
    let mut lines: Vec<WrappedHeroLine<'_>> = Vec::with_capacity(1 + facts.meta_rows.len());
    lines.push(WrappedHeroLine {
        text: &facts.title,
        style: ratatui::style::Style::default().fg(palette::TEXT_STRONG),
    });
    for (index, row) in facts.meta_rows.iter().enumerate() {
        lines.push(WrappedHeroLine {
            text: row,
            style: ratatui::style::Style::default()
                .fg(palette::HERO_META_ROLES[index % palette::HERO_META_ROLES.len()]),
        });
    }
    paint_wide_hero_text(f, area, &lines)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod hero_header_tests {
    use super::*;
    use crate::app::components::library_panel::content::PanelList;
    use crate::app::components::library_panel::content::{ArtworkShape, HeroArtwork, Workspace};
    use crate::app::palette;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// A stub `PanelList` so a Workspace can be present for the shrink rule.
    struct NoopList;
    impl PanelList for NoopList {
        fn view(&mut self, _f: &mut Frame, _rect: Rect) {}
    }

    const AREA: Rect = Rect::new(0, 0, 60, 30);

    fn facts(shape: ArtworkShape) -> HeroFacts {
        HeroFacts {
            title: "Dune".into(),
            meta_rows: vec!["2021".into()],
            artwork: HeroArtwork {
                shape,
                source: None,
            },
        }
    }

    fn content(shape: ArtworkShape) -> HeroContent<'static> {
        HeroContent {
            facts: facts(shape),
            overview: None,
            workspace: None,
        }
    }

    fn draw_pane(width: u16, height: u16, pane: &HeroContent<'_>) -> ratatui::buffer::Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let area = Rect::new(0, 0, width, height);
        let pane_area = area;
        terminal
            .draw(|f| {
                paint_hero_pane_content(f, pane_area, pane);
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
        let artwork = hero_artwork_box(AREA, &pane);
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
        let artwork = hero_artwork_box(AREA, &pane);
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
        let artwork = hero_artwork_box(AREA, &pane);
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

    #[test]
    fn meta_rows_cycle_the_three_roles_one_two_three_one() {
        let pane_facts = HeroFacts {
            title: "T".into(),
            meta_rows: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            artwork: HeroArtwork {
                shape: ArtworkShape::Landscape,
                source: None,
            },
        };
        let pane = HeroContent {
            facts: pane_facts,
            overview: None,
            workspace: None,
        };
        let artwork = hero_artwork_box(AREA, &pane);
        let buf = draw_pane(AREA.width, AREA.height, &pane);
        // Title first, then one row per meta row; row n uses role n % 3.
        for (index, expected) in palette::HERO_META_ROLES.iter().cycle().take(4).enumerate() {
            let y = artwork.bottom() + 2 + index as u16;
            assert_eq!(
                buf[(AREA.x, y)].style().fg,
                Some(*expected),
                "meta row {index} colour"
            );
        }
    }

    #[test]
    fn overview_box_presents_only_with_overview_text() {
        let box_fill = palette::surface_colors(palette::Surface::MainContentBox, false).fill;
        let pane = content(ArtworkShape::Landscape);
        let artwork = hero_artwork_box(AREA, &pane);
        let below = Rect {
            y: artwork.bottom() + 3,
            height: AREA.bottom().saturating_sub(artwork.bottom() + 3),
            ..AREA
        };
        // Without overview: no Main content box surface below the header.
        let buf = draw_pane(AREA.width, AREA.height, &pane);
        assert!(
            (below.top()..below.bottom()).all(|y| buf[(AREA.x + 2, y)].bg != box_fill),
            "no overview box without overview text"
        );
        // With overview text: the Main content box renders below the header
        // and its text paints inside.
        let with = HeroContent {
            facts: facts(ArtworkShape::Landscape),
            overview: Some("A very long overview.".into()),
            workspace: None,
        };
        let buf = draw_pane(AREA.width, AREA.height, &with);
        assert!(
            (below.top()..below.bottom()).any(|y| buf[(AREA.x + 2, y)].bg == box_fill),
            "overview box painted below the header"
        );
        assert!(text_in(&buf, below, "A very long overview."));
    }

    #[test]
    fn artwork_shrinks_before_a_workspace_viewport_drops() {
        let free = content(ArtworkShape::Landscape);
        let small = Rect::new(0, 0, 60, 14);
        let without_workspace = hero_artwork_box(small, &free);
        let mut workspace_list = NoopList;
        let constrained = HeroContent {
            facts: facts(ArtworkShape::Landscape),
            overview: None,
            workspace: Some(Workspace {
                selector: None,
                list: &mut workspace_list,
                focused: false,
            }),
        };
        let with_workspace = hero_artwork_box(small, &constrained);
        // The landscape box would fill the pane; a present Workspace caps it
        // so the Workspace keeps its minimum rows.
        assert_eq!(without_workspace.height, small.height);
        assert_eq!(
            with_workspace.height,
            small.height.saturating_sub(WORKSPACE_MIN_ROWS)
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
}
