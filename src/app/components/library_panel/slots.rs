//! Slot Render Components for the Library panel's Browser-pane rows
//! (task 5.1): the Selector row (one pill bar + the panel's spacer, with the
//! bar's `HitRegions` retained by the panel) and the List controls row
//! (optional pills + optional label). Painters only: typed content in,
//! painted rects out; no state, no effects, no destination arm.

use ratatui::layout::Alignment;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::components::mouse::hit::HitRegions;
use crate::app::palette;
use crate::app::render::{render_pill_bar, PillBar, PillBarWindow};

use super::content::{ListControls, SelectorRow};

/// Paints one pill-bar row from `labels` plus the active pill's index into
/// `area`, pushing the painted pills' hitboxes into `hits` and refreshing
/// `window` — the row's sticky overflow window the caller retains across
/// frames — with this frame's painted window. The one shared pill-row
/// painter for every panel site (Selector row, List controls row,
/// pre-5.6 Workspace selector). `active: None` paints NO active pill (the
/// `SelectorRow` contract in `content.rs`); an empty label list or a zero
/// area paints nothing. The caller keeps its own `HitRegions` registry and
/// window.
pub(in crate::app) fn paint_pill_bar_row(
    f: &mut Frame,
    area: Rect,
    labels: &[String],
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
        PillBar {
            labels,
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

/// Paints one List controls row as a right-aligned plain-text label.
pub(in crate::app) fn paint_list_controls_row(
    f: &mut Frame,
    area: Rect,
    controls: &ListControls,
    _hits: &mut HitRegions<usize>,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    f.render_widget(
        Paragraph::new(controls.label.clone())
            .style(Style::default().fg(palette::TEXT_SECONDARY))
            .alignment(Alignment::Right),
        Rect { height: 1, ..area },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn draw<F: FnOnce(&mut Frame)>(width: u16, height: u16, paint: F) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(paint).unwrap();
        terminal
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

    #[test]
    fn selector_row_paints_pills_and_resolves_a_painted_pill() {
        let bar = Rect::new(2, 1, 20, 1);
        let spacer = Rect::new(2, 2, 20, 1);
        let row = SelectorRow {
            pills: vec!["Movies".into(), "TV".into()],
            active: Some(1),
        };
        let mut hits = HitRegions::new();
        let mut window = PillBarWindow::default();
        let terminal = draw(24, 4, |f| {
            paint_selector_row(
                f,
                bar,
                spacer,
                &row,
                None,
                &mut hits,
                &mut window,
                palette::Surface::PillRowGap,
                false,
            );
        });
        let buf = terminal.backend().buffer();
        assert!(text_in(buf, bar, "Movies") && text_in(buf, bar, "TV"));
        // The active pill is the selected chip; a different chip is unselected.
        let selected_fg = palette::PILL_SELECTED_FG;
        let unselected_fg = palette::TEXT_MUTED;
        assert!(hits.regions().len() == 2, "both painted pills retained");
        let (active_rect, active_id) = hits.regions()[1];
        assert_eq!(active_id, 1);
        let cell = &buf[(active_rect.x + 1, active_rect.y)];
        assert_eq!(cell.style().fg, Some(selected_fg));
        let (other_rect, _) = hits.regions()[0];
        let other_cell = &buf[(other_rect.x + 1, other_rect.y)];
        assert_ne!(other_cell.style().fg, Some(selected_fg));
        assert_eq!(other_cell.style().fg, Some(unselected_fg));
        // A hit test inside a painted pill resolves that pill's index.
        let point = ratatui::layout::Position {
            x: other_rect.x + 1,
            y: other_rect.y,
        };
        assert_eq!(hits.resolve(point), Some(&0));
        // The spacer row paints the panel's gap band, not text.
        let gap_bg = palette::surface_colors(palette::Surface::PillRowGap, false).fill;
        assert_eq!(buf[(spacer.x, spacer.y)].bg, gap_bg);
    }

    #[test]
    fn selector_row_hover_precedes_resting_but_selected_wins() {
        let bar = Rect::new(2, 1, 20, 1);
        let spacer = Rect::new(2, 2, 20, 1);
        let row = SelectorRow {
            pills: vec!["Movies".into(), "TV".into()],
            active: Some(1),
        };
        let mut hits = HitRegions::new();
        let mut window = PillBarWindow::default();
        let terminal = draw(24, 4, |f| {
            paint_selector_row(
                f,
                bar,
                spacer,
                &row,
                Some(0),
                &mut hits,
                &mut window,
                palette::Surface::PillRowGap,
                false,
            );
        });
        let buf = terminal.backend().buffer();
        let (hovered_rect, _) = hits.regions()[0];
        let (selected_rect, _) = hits.regions()[1];
        assert!(bar.contains(ratatui::layout::Position::new(
            hovered_rect.x,
            hovered_rect.y
        )));
        assert_eq!(
            buf[(hovered_rect.x + 1, hovered_rect.y)].bg,
            palette::surface_colors(palette::Surface::PillChip, true).fill
        );
        assert_eq!(
            buf[(selected_rect.x + 1, selected_rect.y)].bg,
            palette::PILL_SELECTED_BG
        );
        assert_eq!(
            buf[(hovered_rect.x + 1, hovered_rect.y)].fg,
            palette::TEXT_EMPHASIS
        );
        assert_ne!(
            buf[(hovered_rect.x + 1, hovered_rect.y)].fg,
            buf[(hovered_rect.x + 1, hovered_rect.y)].bg,
            "hovered pill text must remain legible against its surface"
        );
        assert_eq!(
            buf[(selected_rect.x + 1, selected_rect.y)].fg,
            palette::PILL_SELECTED_FG
        );
    }

    #[test]
    fn selector_row_with_active_none_paints_no_active_pill() {
        let bar = Rect::new(2, 1, 20, 1);
        let spacer = Rect::new(2, 2, 20, 1);
        let row = SelectorRow {
            pills: vec!["Movies".into(), "TV".into()],
            active: None,
        };
        let mut hits = HitRegions::new();
        let mut window = PillBarWindow::default();
        let terminal = draw(24, 4, |f| {
            paint_selector_row(
                f,
                bar,
                spacer,
                &row,
                None,
                &mut hits,
                &mut window,
                palette::Surface::PillRowGap,
                false,
            );
        });
        let buf = terminal.backend().buffer();
        // Both pills paint (unselected); no cell anywhere in the bar carries
        // the selected chip's surface, and every pill's label cell paints the
        // unselected pill foreground.
        assert!(text_in(buf, bar, "Movies") && text_in(buf, bar, "TV"));
        assert_eq!(hits.regions().len(), 2, "both pills painted and retained");
        for (rect, _) in hits.regions() {
            for y in rect.top()..rect.bottom() {
                for x in rect.left()..rect.right() {
                    assert_ne!(
                        buf[(x, y)].bg,
                        palette::PILL_SELECTED_BG,
                        "active: None must paint no active pill"
                    );
                }
            }
            let cell = &buf[(rect.x + 1, rect.y)];
            assert_eq!(cell.style().fg, Some(palette::TEXT_MUTED));
        }
    }

    #[test]
    fn selector_row_without_pills_paints_no_bar_but_keeps_the_spacer() {
        let bar = Rect::new(2, 1, 20, 1);
        let spacer = Rect::new(2, 2, 20, 1);
        let row = SelectorRow {
            pills: Vec::new(),
            active: None,
        };
        let mut hits = HitRegions::new();
        let mut window = PillBarWindow::default();
        let terminal = draw(24, 4, |f| {
            paint_selector_row(
                f,
                bar,
                spacer,
                &row,
                None,
                &mut hits,
                &mut window,
                palette::Surface::PillRowGap,
                false,
            );
        });
        let buf = terminal.backend().buffer();
        assert!(hits.regions().is_empty(), "no pill painted, no hit kept");
        assert!(
            !text_in(buf, bar, "Movies") && !text_in(buf, bar, "◢"),
            "empty Selector row paints no bar"
        );
        let gap_bg = palette::surface_colors(palette::Surface::PillRowGap, false).fill;
        assert_eq!(buf[(spacer.x, spacer.y)].bg, gap_bg);
    }

    #[test]
    fn list_controls_row_paints_its_label() {
        let area = Rect::new(2, 3, 24, 1);
        let controls = ListControls {
            label: "17 items".into(),
        };
        let mut hits = HitRegions::new();
        let terminal = draw(28, 5, |f| {
            paint_list_controls_row(f, area, &controls, &mut hits);
        });
        assert!(text_in(terminal.backend().buffer(), area, "17 items"));
        assert!(hits.regions().is_empty());
    }
}
