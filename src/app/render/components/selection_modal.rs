use super::super::super::palette;
use super::super::super::types_selection_modal::{
    SelectionModalFilter, SelectionModalListState, SelectionModalRow,
};
use crate::app::render::components::modal_frame::render_modal_frame;
use crate::app::render::components::widgets::{render_pill_bar, PillBar};
use crate::app::ui_util::trunc_str;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

const SELECTION_MODAL_MIN_WIDTH: u16 = 30;
const SELECTION_MODAL_MAX_WIDTH: u16 = 70;
const SELECTION_MODAL_MAX_HEIGHT: u16 = 20;

/// Snapshot passed to the App-free Selection modal painter.
pub(in crate::app) struct SelectionModalRenderModel<'a> {
    pub title: &'a str,
    pub state: &'a SelectionModalListState,
    pub cursor: usize,
    pub filter: Option<&'a SelectionModalFilter>,
}

/// Geometry painted by the Selection modal, reused by its mouse hit-testing.
pub(in crate::app) struct SelectionModalRenderGeometry {
    pub area: Rect,
    pub selector_tabs: Vec<(Rect, usize)>,
    pub rows: Vec<(Rect, usize)>,
}

/// Paint the Selection modal and return all geometry needed for interaction.
pub(in crate::app) fn render_selection_modal_content(
    f: &mut Frame,
    dim_backdrop_active: &mut bool,
    model: SelectionModalRenderModel<'_>,
) -> SelectionModalRenderGeometry {
    let title = format!(" {} ", model.title);
    let rows = model.state.rows();
    let status = model.state.status();
    let max_row_w = rows
        .iter()
        .map(|row| match row {
            SelectionModalRow::Header(name) => name.width(),
            SelectionModalRow::Item(item) => item.name.width() + item.meta.width() + 2,
        })
        .max()
        .unwrap_or_else(|| status.map_or(0, str::len));
    let inner_w =
        ((max_row_w + 4) as u16).clamp(SELECTION_MODAL_MIN_WIDTH, SELECTION_MODAL_MAX_WIDTH);
    let width = inner_w + 2;
    let filter_h: u16 = u16::from(model.filter.is_some());
    let spacer_h = filter_h;
    let status_h = u16::from(status.is_some());
    let content_h = rows.len() as u16 + status_h + filter_h + spacer_h;
    let height = (content_h + 2).min(SELECTION_MODAL_MAX_HEIGHT);
    let inner = render_modal_frame(
        f,
        dim_backdrop_active,
        &title,
        width,
        height,
        palette::SURFACE_FOCUSED,
    );

    let filter_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: filter_h,
    };
    let selector_tabs = model.filter.map_or_else(Vec::new, |filter| {
        let ids: Vec<usize> = (0..filter.labels.len()).collect();
        render_pill_bar(
            f,
            filter_area,
            PillBar {
                labels: &filter.labels,
                ids: &ids,
                selected_pos: filter.selected,
                prefix: None,
            },
        )
    });

    let spacer_area = Rect {
        x: inner.x,
        y: inner.y + filter_h,
        width: inner.width,
        height: spacer_h,
    };
    if spacer_h > 0 {
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_FOCUSED)),
            spacer_area,
        );
    }

    let list_area = Rect {
        x: inner.x,
        y: inner.y + filter_h + spacer_h,
        width: inner.width,
        height: inner.height.saturating_sub(filter_h + spacer_h),
    };
    let list_h = list_area.height.saturating_sub(status_h) as usize;
    let cursor = model.cursor;
    let scroll = if cursor >= list_h {
        cursor + 1 - list_h
    } else {
        0
    };
    let row_w = list_area.width as usize;
    let mut lines: Vec<Line> = status
        .into_iter()
        .map(|status| {
            Line::from(Span::styled(
                status,
                Style::default().fg(palette::TEXT_MUTED),
            ))
        })
        .collect();
    lines.extend(
        rows.iter()
            .enumerate()
            .skip(scroll)
            .take(list_h)
            .map(|(i, row)| match row {
                SelectionModalRow::Header(name) => Line::from(Span::styled(
                    trunc_str(name, row_w),
                    Style::default()
                        .fg(palette::TEXT_MUTED)
                        .add_modifier(Modifier::BOLD),
                )),
                SelectionModalRow::Item(item) => {
                    let focused = i == cursor;
                    let name_style = if focused {
                        Style::default().fg(palette::TEXT_PRIMARY)
                    } else {
                        Style::default().fg(palette::TEXT_SECONDARY)
                    };
                    let meta_style = if focused {
                        Style::default().fg(palette::TEXT_ACCENT_MUTED)
                    } else {
                        Style::default().fg(palette::TEXT_MUTED)
                    };
                    let arrow = if focused { "▸ " } else { "  " };
                    let meta_w = item.meta.width();
                    let name_budget = row_w.saturating_sub(arrow.width() + meta_w + 1);
                    let name = trunc_str(&item.name, name_budget);
                    let pad = row_w
                        .saturating_sub(arrow.width() + name.width() + meta_w)
                        .max(1);
                    Line::from(vec![
                        Span::raw(arrow),
                        Span::styled(name, name_style),
                        Span::raw(" ".repeat(pad)),
                        Span::styled(item.meta.clone(), meta_style),
                    ])
                }
            })
            .collect::<Vec<_>>(),
    );
    f.render_widget(Paragraph::new(lines), list_area);

    let rows = rows
        .iter()
        .enumerate()
        .skip(scroll)
        .take(list_h)
        .filter_map(|(row_index, row)| {
            matches!(row, SelectionModalRow::Item(_)).then_some((
                Rect {
                    x: list_area.x,
                    y: list_area.y + status_h + (row_index - scroll) as u16,
                    width: list_area.width,
                    height: 1,
                },
                row_index,
            ))
        })
        .collect();

    SelectionModalRenderGeometry {
        area: inner,
        selector_tabs,
        rows,
    }
}
