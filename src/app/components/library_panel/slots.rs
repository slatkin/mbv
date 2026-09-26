//! Slot Render Components for the Library panel's Browser-pane rows
//! (task 5.1): the Selector row (one pill bar + the panel's spacer, with the
//! bar's `HitRegions` retained by the panel). Painters only: typed content in,
//! painted rects out; no state, no effects, no destination arm.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::components::mouse::hit::HitRegions;
use crate::app::palette;
use crate::app::render::{render_pill_bar, PillBar, PillBarWindow};

use super::content::SelectorRow;

/// Paints one pill-bar row from `labels` plus the active pill's index into
/// `area`, pushing the painted pills' hitboxes into `hits` and refreshing
/// `window` — the row's sticky overflow window the caller retains across
/// frames — with this frame's painted window. The one shared pill-row
/// painter for every panel site (Selector row and Workspace selector).
/// `active: None` paints NO active pill (the
/// `SelectorRow` contract in `content.rs`); an empty label list or a zero
/// area paints nothing. The caller keeps its own `HitRegions` registry and
/// window.
pub(in crate::app) fn paint_pill_bar_row(
    f: &mut Frame,
    area: Rect,
    labels: &[String],
    markers: &[bool],
    active: Option<usize>,
    hovered: Option<usize>,
    prefix: Option<&str>,
    hits: &mut HitRegions<usize>,
    window: &mut PillBarWindow,
) {
    let ids: Vec<usize> = (0..labels.len()).collect();
    // No active pill: a position past the end selects nothing, so every
    // painted pill renders unselected.
    let selected_pos = active.unwrap_or(labels.len());
    let (tabs, painted_window) = render_pill_bar(
        f,
        area,
        &PillBar {
            labels,
            markers,
            ids: &ids,
            selected_pos,
            hovered,
            prefix,
            window: *window,
        },
    );
    *window = painted_window;
    for (rect, id) in tabs {
        hits.push(rect, id);
    }
}

/// The Selector row's leading glyph, every library destination's pill bar
/// before the D8 migration (`home_pills.rs`, `music.rs`, `feeds.rs`,
/// `tv_wide.rs`, both Audiobookshelf destinations) and restored here as the
/// one shared painter's prefix rather than re-duplicated per destination.
pub(in crate::app) const SELECTOR_ROW_PREFIX: &str = " \u{2318} ";

/// Paints one Selector row: the single pill bar into `bar_area` — pushing
/// the painted pills' hitboxes into `hits` — followed by the panel's spacer
/// row. An empty pill list paints no bar (the row's place stays reserved for
/// it), and the spacer always paints: the row is the bar plus the spacer.
///
/// `body`/`body_focused` are the owning panel's own body surface and focus
/// bit: the spacer row is the panel showing through, so the caller supplies
/// what "the panel" means in its geometry (Wide's chrome gap, or the non-Wide
/// body) rather than this painter choosing one.
pub(in crate::app) fn paint_selector_row(
    f: &mut Frame,
    bar_area: Rect,
    spacer_area: Rect,
    row: &SelectorRow,
    hovered: Option<usize>,
    hits: &mut HitRegions<usize>,
    window: &mut PillBarWindow,
    body: palette::Surface,
    body_focused: bool,
) {
    if !row.pills.is_empty() && bar_area.height > 0 && bar_area.width > 0 {
        paint_pill_bar_row(
            f,
            bar_area,
            &row.pills,
            &row.markers,
            row.active,
            hovered,
            Some(SELECTOR_ROW_PREFIX),
            hits,
            window,
        );
    }
    paint_pill_row_gap(f, spacer_area, body, body_focused);
}

/// Paints the panel's blank spacer row below a pill bar: the owning panel's
/// own body surface (`body`, resolved with `focused`), never a separate
/// backdrop of its own. One owner: the slot paints it, so destinations never
/// carry Home's private spacer paint again (design D8).
pub(in crate::app) fn paint_pill_row_gap(
    f: &mut Frame,
    area: Rect,
    body: palette::Surface,
    focused: bool,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let background = palette::surface_colors(body, focused).fill;
    f.render_widget(
        Paragraph::new(" ".repeat(area.width as usize)).style(Style::default().bg(background)),
        area,
    );
}
