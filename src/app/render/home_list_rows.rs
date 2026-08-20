use super::home_video::home_panel_scroll;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

#[derive(Clone)]
pub(super) enum DisplayRow {
    Empty,
    Item(usize, Box<QueueItem>),
    Hero,
}

pub(super) fn render_home_list_rows(
    app: &mut App,
    f: &mut Frame,
    rows: &[DisplayRow],
    list_area: Rect,
    selection_bg_full: Rect,
    selection_bg: Color,
    cursor: usize,
    focused: bool,
    layout: &mut LayoutMain,
    hero_rows: u16,
) {
    let cursor_row = rows
        .iter()
        .position(|row| matches!(row, DisplayRow::Item(flat_idx, _) if *flat_idx == cursor))
        .unwrap_or(0) as u16;
    let mut display_rows = rows.to_vec();
    if hero_rows > 0 {
        display_rows.splice(
            cursor_row as usize + 1..cursor_row as usize + 1,
            (0..hero_rows).map(|_| DisplayRow::Hero),
        );
    }
    let display_content_h = display_rows.len() as u16;
    let needs_scrollbar = display_content_h > list_area.height;
    let list_w = super::content_width(list_area.width, needs_scrollbar) as u16;
    let scroll_y = if hero_rows > 0 {
        super::hero::inline_detail_flow(
            cursor_row as usize,
            hero_rows,
            list_area.height,
            app.home.home_scroll,
        )
        .map(|flow| flow.offset as u16)
        .unwrap_or(0)
    } else {
        home_panel_scroll(
            app.home.home_scroll as u16,
            cursor_row,
            cursor_row + 1,
            display_content_h,
            list_area.height,
        )
    };
    app.home.home_scroll = scroll_y as usize;

    let mut hitmap: Vec<(Rect, usize)> = Vec::new();

    let visible = list_area
        .height
        .min(display_content_h.saturating_sub(scroll_y));
    for k in 0..visible {
        let row_idx = scroll_y as usize + k as usize;
        let sy = list_area.y + k;
        let row_x = selection_bg_full.x;
        let row_rect = Rect {
            x: row_x,
            y: sy,
            width: list_w.saturating_add(list_area.x.saturating_sub(row_x)),
            height: 1,
        };
        if row_idx >= display_rows.len() {
            continue;
        }
        match &display_rows[row_idx] {
            DisplayRow::Empty => {
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::raw(" "),
                        Span::styled("(empty)", Style::default().fg(palette::MUTED)),
                    ])),
                    row_rect,
                );
            }
            DisplayRow::Hero => {}
            DisplayRow::Item(flat_idx, item) => {
                let selected_row = *flat_idx == cursor;
                if selected_row {
                    layout.selected_item_rect = Some(row_rect);
                }

                if selected_row && focused {
                    f.render_widget(
                        Block::default().style(Style::default().bg(selection_bg)),
                        Rect {
                            x: selection_bg_full.x,
                            y: sy,
                            width: selection_bg_full.width,
                            height: 1,
                        },
                    );
                    let gutter_x = if list_area.x > row_x {
                        row_x
                    } else {
                        row_x.saturating_sub(2)
                    };
                    f.render_widget(
                        Block::default().style(Style::default().bg(selection_bg)),
                        Rect {
                            x: gutter_x,
                            y: sy,
                            width: 2,
                            height: 1,
                        },
                    );
                    f.render_widget(
                        Paragraph::new(super::selection_marker(true, super::MarkerEdge::Left)),
                        Rect {
                            x: gutter_x,
                            y: sy,
                            width: 1,
                            height: 1,
                        },
                    );
                }

                let text_rect = Rect {
                    x: row_x + 2,
                    width: row_rect.width.saturating_sub(2),
                    ..row_rect
                };

                let Some(emby) = item.as_emby() else {
                    super::home_latest_row::render_home_latest_row(
                        f,
                        text_rect,
                        item,
                        selected_row,
                        focused,
                    );
                    hitmap.push((row_rect, *flat_idx));
                    continue;
                };
                super::home_latest_row::render_home_emby_row(
                    f,
                    text_rect,
                    emby,
                    selected_row,
                    focused,
                );
                hitmap.push((row_rect, *flat_idx));
            }
        }
    }

    layout.home.hitmap = hitmap;

    if hero_rows > 0 {
        let detail_row = cursor_row as usize + 1;
        if detail_row >= scroll_y as usize && detail_row < display_rows.len() {
            layout.hero_area = Rect {
                x: list_area.x,
                y: list_area.y + (detail_row as u16 - scroll_y),
                width: list_area.width,
                height: hero_rows.min(
                    list_area
                        .bottom()
                        .saturating_sub(list_area.y + (detail_row as u16 - scroll_y)),
                ),
            };
        }
    }

    if display_content_h > list_area.height && focused {
        let max_off = display_content_h.saturating_sub(list_area.height) as usize;
        super::render_right_scrollbar(f, list_area, max_off, scroll_y as usize, palette::SCROLLBAR);
    }
}
