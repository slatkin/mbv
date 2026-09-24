//! Paint-free Wide Hero artwork-box geometry: sizes the Landscape (16:9),
//! Portrait (2:3), and Square (1:1) artwork boxes from the pane area,
//! the short-pane compact caps, and the Workspace/text-starvation shrink
//! rules — the one layout site the painter and the shell image projection
//! share.

use ratatui::layout::Rect;

use super::super::content::{HeroFacts, HeroHeader, HeroHeaderArm};

/// Blank rows between the artwork box and the text block (the shared
/// `wide_hero_slots` convention: metadata starts at `img_area.bottom() + 1`).
pub(super) const ARTWORK_TEXT_GAP_ROWS: u16 = 1;

/// Vertical room a present Workspace keeps below the header (design D5: the
/// artwork shrinks before a Workspace viewport would drop). Two padding rows
/// plus a few visible list rows.
pub(super) const WORKSPACE_MIN_ROWS: u16 = 6;

/// Maximum artwork height for the Wide hero box, shared by Landscape, Portrait,
/// and Square artwork.
pub(super) const HERO_ARTWORK_MAX_ROWS: u16 = 25;

/// Maximum artwork height for the non-landscape (Portrait/Square) arms. Their
/// boxes are sized from the available height, so they reach the cap directly
/// and a taller block reads as oversized beside the title/meta text.
pub(super) const HERO_NON_LANDSCAPE_ARTWORK_MAX_ROWS: u16 = 20;

/// Panes whose *terminal* is this short (or fewer rows) use the compact caps
/// below so the header leaves room for the text block, overview, and
/// Workspace beneath it. Measured on the terminal, not the pane, so panel
/// chrome does not change the decision.
pub(in crate::app) const HERO_SHORT_PANE_MAX_HEIGHT: u16 = 50;

/// The compact-cap decision, shared by the artwork box, the overview box,
/// and the shell's image projection: the threshold is the terminal height.
pub(in crate::app) fn short_pane(terminal_height: u16) -> bool {
    terminal_height <= HERO_SHORT_PANE_MAX_HEIGHT
}

/// Compact caps for short panes: 15 rows for Landscape and non-landscape arms
/// alike.
pub(super) const HERO_SHORT_LANDSCAPE_MAX_ROWS: u16 = 15;
pub(super) const HERO_SHORT_NON_LANDSCAPE_MAX_ROWS: u16 = 15;

/// Minimum columns the title/meta block keeps beside a right-aligned artwork
/// box (Portrait/Square arms).
const HERO_MIN_TEXT_COLS: u16 = 16;

/// The paint-free artwork box for one hero pane's content area (design D5):
/// Landscape fills the content width at the top with its 16:9 height; the
/// Portrait (2:3) and Square (1:1) boxes are sized from the available height
/// and right-aligned, capped so the text block keeps room. A present
/// Workspace caps the box height first (the artwork shrinks before a
/// Workspace viewport would drop), and the Landscape box additionally
/// shrinks before the wrapped title/meta block below it is starved out of
/// the pane (the legacy wide Emby card's rule, now universal). Tall panes
/// cap all three artwork boxes at 25 rows; panes 50 rows or shorter use the
/// compact 15-row cap.
///
/// One layout site: the header painter calls this and the shell projection
/// (task 5.10) calls it with the Library panel's area, so the fetched image
/// is always encoded for the box that paints it. `terminal_height` is the
/// terminal's row count: the compact caps apply at [`HERO_SHORT_PANE_MAX_HEIGHT`]
/// terminal rows or fewer.
pub(in crate::app) fn hero_artwork_box(
    area: Rect,
    facts: &HeroFacts,
    workspace_present: bool,
    terminal_height: u16,
) -> Rect {
    let header = HeroHeader::from(facts.artwork.painted_shape());
    // Short terminals leave room for the text block, overview, and Workspace
    // beneath the header: compact 15-row caps at 50 rows or fewer.
    let short = short_pane(terminal_height);
    let landscape_cap = if short {
        HERO_SHORT_LANDSCAPE_MAX_ROWS
    } else {
        HERO_ARTWORK_MAX_ROWS
    };
    let non_landscape_cap = if short {
        HERO_SHORT_NON_LANDSCAPE_MAX_ROWS
    } else {
        HERO_NON_LANDSCAPE_ARTWORK_MAX_ROWS
    };
    // The artwork shrinks before a Workspace viewport would drop (design D5).
    let max_h = if workspace_present {
        area.height.saturating_sub(WORKSPACE_MIN_ROWS)
    } else {
        area.height
    }
    .min(landscape_cap);
    let (width, height) = match header.arm() {
        HeroHeaderArm::Landscape => {
            // 16:9 in terminal cells (cells are ~2x taller than wide).
            let h = (area.width.saturating_mul(9).saturating_add(31) / 32).max(1);
            // The artwork shrinks before the title/meta block below it is
            // starved out of the pane: the box keeps room for the wrapped
            // text rows plus the gap row between the two blocks.
            let text_rows = landscape_text_rows(area.width, facts);
            let room_for_text = area
                .height
                .saturating_sub(text_rows)
                .saturating_sub(ARTWORK_TEXT_GAP_ROWS);
            (area.width, h.min(max_h).min(room_for_text))
        }
        HeroHeaderArm::Portrait => box_from_height(max_h.min(non_landscape_cap), 4, 3, area),
        HeroHeaderArm::Square => box_from_height(max_h.min(non_landscape_cap), 2, 1, area),
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

/// Minimum columns each landscape-grid column keeps: the text block
/// beside Portrait/Square art never goes narrower, so neither does a
/// grid column. Below twice that width the Landscape arm falls back to
/// the stacked rows.
pub(super) fn landscape_grid_columns(width: u16) -> Option<(u16, u16)> {
    if width < 2 * HERO_MIN_TEXT_COLS {
        return None;
    }
    let left = width / 2;
    Some((left, width.saturating_sub(left)))
}

/// Rows the Landscape title/meta block needs: the two-column grid packs
/// two non-empty entries per row (cells truncate, never wrap), or — on a
/// text block too narrow for two columns — the same wrap
/// [`paint_wide_hero_text`] applies (`width - 1`), so the box's
/// text-starvation cap matches what the painter actually paints below it.
fn landscape_text_rows(width: u16, facts: &HeroFacts) -> u16 {
    if landscape_grid_columns(width).is_some() {
        facts.live_entries().count().div_ceil(2) as u16
    } else {
        let wrap_width = (width as usize).saturating_sub(1).max(1);
        facts
            .live_entries()
            .map(|line| textwrap::wrap(line, wrap_width).len() as u16)
            .sum()
    }
}
