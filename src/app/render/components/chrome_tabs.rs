//! Tab-bar painter and tab-window math (task 2.1).
//!
//! The mounted `TabPanel` Interactive Component
//! (`src/app/components/tab_panel.rs`) owns the tab bar's placement paint,
//! hit regions and click resolution; this module is its painter plus the one
//! shared visible-window computation, so the keyboard tab-cycling path
//! (`App::ensure_tab_visible`) and the painted bar cannot drift.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Tabs};
use ratatui::Frame;

use crate::app::palette;

/// Plain-data paint model for one tab bar (task 2.1). The shell projects the
/// tab titles, the selected tab's position and the scroll anchor; the
/// mounted `TabPanel` component retains the hit regions the painter resolves.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct TabBarModel<'a> {
    /// Tab titles in position order (Home, Emby libraries, Audiobookshelf
    /// libraries, Feeds when subscribed).
    pub titles: &'a [String],
    /// Parallel per-tab Latest markers; missing entries are treated as unmarked.
    pub markers: &'a [bool],
    /// The selected tab's position (its `titles` index; Home = 0).
    pub selected: usize,
    /// Scroll anchor shared with the keyboard tab-cycling path
    /// (`App::ensure_tab_visible`).
    pub scroll: usize,
    /// Full tab position currently under the pointer, if any.
    pub hovered: Option<usize>,
}

/// Tab-title widths: 2-column padding around each title. One definition for
/// the painted bar and the keyboard path's `App::tab_title_widths`.
pub(in crate::app) fn tab_title_widths(titles: &[String]) -> Vec<u16> {
    titles
        .iter()
        .map(|t| t.chars().count() as u16 + 2)
        .collect()
}

/// Which tab positions fit in `avail_w` starting at the `scroll` anchor:
/// `(visible_start, visible_end)` as positions into the full tab list.
/// Moved verbatim from the former `App::visible_tab_range` (task 2.1) so the
/// painted bar and `App::ensure_tab_visible` share one computation.
pub(in crate::app) fn visible_tab_range(
    widths: &[u16],
    scroll: usize,
    avail_w: u16,
) -> (usize, usize) {
    let n = widths.len();
    let start = scroll.min(if n > 0 { n - 1 } else { 0 });
    let left_w: u16 = if start > 0 { 2 } else { 0 };
    let mut budget = avail_w.saturating_sub(left_w);
    let mut end = start;
    while end < n {
        let tab_w: u16 = widths[end] + 2;
        let right_w: u16 = if end + 1 < n { 2 } else { 0 };
        if budget < tab_w + right_w && end > start {
            break;
        }
        budget = budget.saturating_sub(tab_w);
        end += 1;
    }
    (start, end)
}

/// Paint the tab bar within the 3-row `area` (the `RootFrame.tab` placement)
/// and resolve each painted tab's hit region into `hits` as
/// `(screen_rect, tab_position)`.
pub(in crate::app) fn render_tab_bar(
    f: &mut Frame,
    area: Rect,
    model: &TabBarModel<'_>,
    hits: &mut Vec<(Rect, usize)>,
) {
    // Fill the tab bar area with the tab box's own background. `Clear`
    // blanks every cell's symbol first (task 12.2): a bare `Block::style`
    // only recolors a cell, it never overwrites a stale glyph left by a
    // prior frame, so the placement's padding rows (never touched by the
    // tab/title painting below) would otherwise keep showing whatever was
    // painted underneath before this panel owned the placement.
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(
        ratatui::widgets::Block::default().style(
            ratatui::style::Style::default().bg(palette::surface_colors(
                palette::Surface::TabBar,
                false,
            )
            .fill),
        ),
        area,
    );

    // Tabs render on the second row; first row is padding inside the box.
    let tab_row = Rect {
        y: area.y + 1,
        height: 1,
        ..area
    };

    let tabs_x = area.x + 1;
    let tabs_w = crate::app::render::arrangements::chrome::tab_strip_text_width(area.width);

    let widths = tab_title_widths(model.titles);
    let (vis_start, vis_end) = visible_tab_range(&widths, model.scroll, tabs_w);
    let has_left = vis_start > 0;
    let has_right = vis_end < widths.len();
    let ind_style = Style::default().fg(palette::TEXT_STRONG);
    let left_w: u16 = if has_left { 2 } else { 0 };
    let right_w: u16 = if has_right { 2 } else { 0 };
    if has_left {
        f.render_widget(
            Paragraph::new("« ").style(ind_style),
            Rect {
                x: tabs_x,
                y: tab_row.y,
                width: 2,
                height: 1,
            },
        );
    }
    if has_right {
        f.render_widget(
            Paragraph::new(" »").style(ind_style),
            Rect {
                x: tabs_x + tabs_w.saturating_sub(2),
                y: tab_row.y,
                width: 2,
                height: 1,
            },
        );
    }
    let inner_tabs = Rect {
        x: tabs_x + left_w,
        y: tab_row.y,
        width: tabs_w.saturating_sub(left_w + right_w),
        height: area.height,
    };
    let tab_pos = model.selected;
    let selected_tab = if tab_pos < vis_start || tab_pos >= vis_end {
        usize::MAX
    } else {
        tab_pos - vis_start
    };
    let mut tab_x = inner_tabs.x;
    let tab_titles: Vec<Line> = model.titles[vis_start..vis_end]
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let n = n.to_uppercase();
            let position = vis_start + i;
            let marked = model.markers.get(position).copied().unwrap_or(false);
            let marker_span = |style: Style| {
                if marked {
                    Span::styled("•", Style::default().fg(palette::ACCENT_ACTIVE))
                } else {
                    Span::styled(" ", style)
                }
            };
            let line = if i == selected_tab {
                let style = Style::default()
                    .fg(palette::TEXT_STRONG)
                    .add_modifier(Modifier::BOLD);
                Line::from(vec![
                    Span::styled("▐", Style::default().fg(palette::ACCENT)),
                    Span::styled(format!(" {n}"), style),
                    marker_span(style),
                    Span::styled(" ", style),
                ])
            } else {
                let style = if model.hovered == Some(position) {
                    Style::default().fg(palette::TEXT_STRONG)
                } else {
                    Style::default().fg(palette::TEXT_MUTED)
                };
                Line::from(vec![
                    Span::styled(format!("  {n}"), style),
                    marker_span(style),
                    Span::styled(" ", style),
                ])
            };
            let width = line.width() as u16;
            hits.push((
                Rect {
                    x: tab_x,
                    y: inner_tabs.y,
                    width,
                    height: 1,
                },
                position,
            ));
            tab_x += width;
            line
        })
        .collect();
    f.render_widget(
        Tabs::new(tab_titles)
            .select(usize::MAX)
            .style(Style::default().fg(palette::TEXT_SECONDARY))
            .highlight_style(Style::default())
            .divider(Span::raw(""))
            .padding("", ""),
        inner_tabs,
    );
}
