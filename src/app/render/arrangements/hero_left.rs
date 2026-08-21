//! Hero-on-left arrangement: geometry, borders, recessed content blocks,
//! and wrapped-text rendering for the two-pane layout shared by Music,
//! Home, and future screens (design.md decisions 4–6).

use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use textwrap::wrap;

/// Minimum outer content-area height for the hero-on-left arrangement's
/// two-pane split.
pub(in crate::app::render) const HERO_ON_LEFT_MIN_AREA_HEIGHT: u16 = 6;
/// Minimum width either hero-on-left pane may shrink to (decision 5's
/// minimum pane width).
const HERO_ON_LEFT_MIN_PANE_WIDTH: u16 = 40;
/// Empty columns separating the hero-on-left arrangement's two panes.
const HERO_ON_LEFT_PANE_GAP: u16 = 2;
/// Height of the pill row at the top of the hero-on-left arrangement's right
/// (list) pane.
const HERO_ON_LEFT_PILLS_ROW_HEIGHT: u16 = 1;
/// Blank rows below the pill row before the list starts.
const HERO_ON_LEFT_PILLS_GAP_ROWS: u16 = 1;

/// Symmetric interior padding shared by every hero-on-left surface's panes
/// (hero content, list panel, recessed boxes). One definition; surfaces that
/// previously carried their own `PANE_PAD_X`/`PANE_PAD_Y` (or `HOME_HERO_PAD_*`)
/// copy now import these.
pub(in crate::app::render) const PANE_PAD_X: u16 = 2;
pub(in crate::app::render) const PANE_PAD_Y: u16 = 1;

/// Resolves the only shared responsive decision for hero-bearing browsers and
/// returns the pane geometry when the wide presentation fits. Callers provide
/// content; they do not own a breakpoint or a height threshold.
pub(in crate::app::render) fn shared_hero_presentation(content_area: Rect) -> Option<(Rect, Rect)> {
    (content_area.width >= crate::app::TWO_COLUMN_THRESHOLD
        && content_area.height.saturating_sub(1) >= HERO_ON_LEFT_MIN_AREA_HEIGHT)
        .then(|| hero_on_left_panes(content_area))
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn shared_presentation_owns_breakpoint_and_pane_geometry() {
        let wide = shared_hero_presentation(Rect {
            x: 3,
            y: 4,
            width: crate::app::TWO_COLUMN_THRESHOLD,
            height: HERO_ON_LEFT_MIN_AREA_HEIGHT + 1,
        });
        assert!(wide.is_some());
        let Some((left, right)) = wide else {
            unreachable!();
        };
        assert_eq!(left.x, 3);
        assert_eq!(right.x, left.right() + 2);

        assert!(shared_hero_presentation(Rect {
            x: 0,
            y: 0,
            width: crate::app::TWO_COLUMN_THRESHOLD - 1,
            height: 20,
        })
        .is_none());
        assert!(shared_hero_presentation(Rect {
            x: 0,
            y: 0,
            width: crate::app::TWO_COLUMN_THRESHOLD,
            height: HERO_ON_LEFT_MIN_AREA_HEIGHT,
        })
        .is_none());
    }
}

/// Returns `(left_pane, right_pane)` for the hero-on-left arrangement's
/// horizontal split: a `HERO_ON_LEFT_PANE_GAP`-column gutter between a
/// ~40%-width left (hero) pane and the remaining right (list) pane, each
/// floored at `HERO_ON_LEFT_MIN_PANE_WIDTH`.
pub(in crate::app::render) fn hero_on_left_panes(content_area: Rect) -> (Rect, Rect) {
    let left_w = ((content_area.width as u32 * 2 / 5) as u16)
        .max(HERO_ON_LEFT_MIN_PANE_WIDTH)
        .min(
            content_area
                .width
                .saturating_sub(HERO_ON_LEFT_MIN_PANE_WIDTH),
        );
    let right_w = content_area
        .width
        .saturating_sub(left_w)
        .saturating_sub(HERO_ON_LEFT_PANE_GAP);
    (
        Rect {
            x: content_area.x,
            y: content_area.y,
            width: left_w,
            height: content_area.height,
        },
        Rect {
            x: content_area.x + left_w + HERO_ON_LEFT_PANE_GAP,
            y: content_area.y,
            width: right_w,
            height: content_area.height,
        },
    )
}

/// The hero-on-left arrangement's right (list) pane geometry: a one-row pill
/// bar flush with the pane's top, then the list panel below it (decision
/// 6's "pill row at top of list pane"). `right_panel` is the pane's full
/// rect (its `y`/`height` anchor the pill row and the panel's bottom);
/// `right_area` is the vertically-inset pane used for the pill row's
/// x/width. `bottom_pad` is the caller's own trailing padding reserve
/// (grouped Music's `PANE_PAD_Y`), kept as a parameter rather than a second
/// constant here so the two files do not each own a copy of the same value.
pub(in crate::app::render) struct HeroOnLeftRightPane {
    pub pills_area: Rect,
    pub list_panel: Rect,
}

pub(in crate::app::render) fn hero_on_left_right_pane(
    right_panel: Rect,
    right_area: Rect,
    bottom_pad: u16,
) -> HeroOnLeftRightPane {
    let pills_area = Rect {
        x: right_area.x,
        y: right_panel.y,
        width: right_area.width,
        height: HERO_ON_LEFT_PILLS_ROW_HEIGHT,
    };
    let browser_y = right_panel.y + HERO_ON_LEFT_PILLS_ROW_HEIGHT + HERO_ON_LEFT_PILLS_GAP_ROWS;
    let list_panel = Rect {
        x: right_area.x,
        y: browser_y,
        width: right_area.width,
        height: right_panel.height.saturating_sub(
            HERO_ON_LEFT_PILLS_ROW_HEIGHT + HERO_ON_LEFT_PILLS_GAP_ROWS + bottom_pad,
        ),
    };
    HeroOnLeftRightPane {
        pills_area,
        list_panel,
    }
}

/// Paints the focused browser rail's border using the shared framing primitive:
/// a `▔` top row and a `▁` bottom row, with a focus-resolved background, one
/// row inside `list_panel`'s own top/bottom edge. The rail uses the shared
/// painter's focused-rail framing arm; its fixed window mirrors `hero_block_shell`'s
/// (`offset = 0`, fully visible, padding rows `[1, height - 2]`); this is
/// hero-on-left's thin shell entry point, the same role `hero_block_shell`
/// plays for inline presentation.
pub(in crate::app::render) fn hero_on_left_list_panel_border(
    f: &mut Frame,
    list_panel: Rect,
    focused: bool,
) {
    if list_panel.height == 0 {
        return;
    }
    let background = palette::resolve_surface_focus(focused);
    for y in list_panel.y..list_panel.bottom() {
        for x in list_panel.x..list_panel.right() {
            let cell = f.buffer_mut().cell_mut((x, y)).expect("panel cell exists");
            cell.set_bg(background);
        }
    }
    crate::app::render::render_selected_block_borders(
        f,
        list_panel,
        0,
        list_panel.height as usize,
        1,
        (list_panel.height as usize).saturating_sub(2),
        crate::app::render::SelectedBlockBorderStyle::FocusedRail { focused },
    );
}

/// Paints the hero-on-left component's standard recessed content block:
/// a [`palette::SURFACE_BACKDROP`] rect inset by `pad_x` from `area`'s
/// horizontal edges, with an inner content rect further inset by
/// `pad_x` / `pad_y`.  Returns both rects so callers can use `panel` for
/// full-bleed row backgrounds and `content` for text layout.
///
/// Shared by Music's track panel and Home's overview block.
pub(in crate::app::render) fn hero_on_left_recessed_box(
    f: &mut Frame,
    area: Rect,
    pad_x: u16,
    pad_y: u16,
) -> (Rect, Rect) {
    use ratatui::widgets::Block;

    let panel = Rect {
        x: area.x.saturating_add(pad_x),
        width: area.width.saturating_sub(pad_x * 2),
        ..area
    };
    f.render_widget(
        Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
        panel,
    );
    let content = Rect {
        x: panel.x.saturating_add(pad_x),
        y: panel.y.saturating_add(pad_y),
        width: panel.width.saturating_sub(pad_x * 2),
        height: panel.height.saturating_sub(pad_y * 2),
    };
    (panel, content)
}

/// One line of the `Hero` component's hero-on-left text block. Unlike
/// inline presentation's single-row, truncated [`super::hero::HeroLine`], hero-on-left
/// text wraps across as many rows as it needs (design.md decision 2's
/// "Consequence": text wrapping moves into `Hero`, screens hand over
/// unwrapped strings). Style is screen-chosen (e.g. focus-derived bold),
/// matching how `HeroContent::meta_color` lets an inline browser pick its
/// own colour.
pub(in crate::app::render) struct WrappedHeroLine<'a> {
    pub text: &'a str,
    pub style: Style,
}

/// Paints `lines` wrapped to `area`'s width, top to bottom, stopping at
/// `area`'s bottom edge; empty line text is skipped. Returns the first
/// unpainted row.
pub(in crate::app::render) fn paint_hero_on_left_text(
    f: &mut Frame,
    area: Rect,
    lines: &[WrappedHeroLine],
) -> u16 {
    if area.height == 0 || area.width < 3 {
        return area.y;
    }
    let mut row = area.y;
    let wrap_width = (area.width as usize).saturating_sub(1);
    for line in lines {
        if line.text.is_empty() {
            continue;
        }
        for wrapped in wrap(line.text, wrap_width.max(1)) {
            if row >= area.bottom() {
                return row;
            }
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(wrapped.into_owned(), line.style))),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }
    }
    row
}
