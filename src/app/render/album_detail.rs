use super::super::ui_util::*;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;

const INLINE_ALBUM_TITLE_EXTRA_INDENT: u16 = 1;
const INLINE_ALBUM_TRACK_EXTRA_INDENT: u16 = 2;

impl App {
    /// Renders the music album detail panel (track list) into `area` — the lib
    /// slot below the card. The card itself already shows the album art (handled
    /// in `render_card`). Mirrors `render_compact_detail` for movies.
    ///
    /// Takes `items`/`cursor` explicitly rather than reading `nav_stack`
    /// internally (#145) so it can render either the legacy drilled-in
    /// nav_stack level or the inline-album-detail cache (the currently
    /// highlighted album in the album-folder listing, fetched proactively
    /// via `fetch_album_tracks`) with the same code path.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_album_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        items: &[mbv_core::api::EmbyItem],
        cursor: usize,
        focused: bool,
        show_title: bool,
        selected_region_gutter: bool,
        flush_left: bool,
        show_hint: bool,
        art_reserved_w: u16,
        active_marker_x: Option<u16>,
        layout: &mut LayoutMain,
    ) {
        if area.height == 0 {
            return;
        }

        let n = items.len();
        if items.is_empty() {
            return;
        }
        let gutter_w = if selected_region_gutter { 2 } else { 1 };
        let max_y = area.y + area.height;
        let mut row = area.y;

        // — Album title (only when no separate row already shows it — the
        // drilled-in single-pane view has no Album(idx) row above this, unlike
        // the inline/grouped call site) —
        if show_title && row < max_y {
            let album_title = items[0].album.clone();
            let title = trunc_str(&album_title, (area.width as usize).saturating_sub(1));
            let title_style = if focused {
                Style::default()
                    .fg(palette::YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::YELLOW)
            };
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(format!(" {title}"), title_style))),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }

        // — Inline album actions / spacer row —
        if row < max_y && show_hint {
            if selected_region_gutter {
                let hint_w = (area.width as usize).saturating_sub(gutter_w);
                // `focused` mirrors `album_track_focus.is_some()` at the call
                // site: once track-selection mode is entered, swap the
                // "show tracks" hint for the exit hint.
                let trailing_hint = if focused {
                    "BACK: Exit"
                } else {
                    "ENTER: Show tracks"
                };
                let hint = trunc_str(
                    &format!("^P: Play | ^A: Enqueue | ^S: Shuffle | {trailing_hint}"),
                    hint_w,
                );
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        super::selection_marker(true),
                        Span::raw(" "),
                        Span::styled(hint.to_string(), Style::default().fg(palette::MUTED)),
                    ])),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                );
                row += 1;
                if row + 1 < max_y {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            super::selection_marker(true),
                            Span::raw(" "),
                        ])),
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
            if !selected_region_gutter {
                let trailing_hint = if focused {
                    "BACK: Exit"
                } else {
                    "ENTER: Show tracks"
                };
                let hint_width = area
                    .width
                    .saturating_sub(art_reserved_w)
                    .saturating_sub(1)
                    .max(1) as usize;
                let hint_lines: Vec<Line> = wrap(
                    &format!("^P: Play | ^A: Enqueue | ^S: Shuffle | {trailing_hint}"),
                    hint_width,
                )
                .into_iter()
                .map(|line| {
                    Line::from(vec![
                        Span::raw(" "),
                        Span::styled(line.into_owned(), Style::default().fg(palette::SOFT_WHITE)),
                    ])
                })
                .collect();
                f.render_widget(
                    Paragraph::new(hint_lines.clone()),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width.saturating_sub(art_reserved_w),
                        height: hint_lines.len() as u16,
                    },
                );
                row += hint_lines.len() as u16 + 1;
            }
        }

        // — Scrollable track list —
        let track_indent = if flush_left || selected_region_gutter {
            0
        } else {
            INLINE_ALBUM_TRACK_EXTRA_INDENT
                .saturating_sub(INLINE_ALBUM_TITLE_EXTRA_INDENT)
                .min(area.width)
        };
        let table_area = Rect {
            x: area.x + track_indent,
            y: row,
            width: area
                .width
                .saturating_sub(track_indent)
                .saturating_sub(art_reserved_w),
            height: max_y.saturating_sub(row),
        };
        if table_area.height == 0 {
            return;
        }

        if focused {
            if let Some(marker_x) = active_marker_x {
                let scroll_offset = cursor.saturating_sub(table_area.height as usize - 1);
                f.render_widget(
                    Paragraph::new(Span::styled("\u{258D}", Style::default().fg(palette::AQUA))),
                    Rect {
                        x: marker_x,
                        y: table_area.y + cursor.saturating_sub(scroll_offset) as u16,
                        width: 1,
                        height: 1,
                    },
                );
            }
        }

        let playback = self.effective_playback_state();
        let now_playing_id: Option<String> = if playback.active {
            self.playback_queue()
                .items
                .get(playback.active_idx)
                .map(|i| i.id.clone())
        } else {
            None
        };

        let show_length = table_area.width > 40;
        let dur_col_w: usize = if show_length { 7 } else { 0 };
        let title_col_w = (table_area.width as usize)
            .saturating_sub(gutter_w + 2 + if show_length { dur_col_w + 1 } else { 0 });

        let max_track_num = items
            .iter()
            .map(|item| item.index_number.max(0) as usize)
            .max()
            .unwrap_or(n);
        let track_num_width = max_track_num.to_string().len();

        let rows: Vec<Row> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_cursor = i == cursor;
                let is_playing = now_playing_id.as_deref() == Some(item.id.as_str());
                let row_style = if is_cursor && focused {
                    Style::default().fg(palette::YELLOW).bg(palette::BG_GREEN)
                } else if focused {
                    Style::default().fg(palette::WHITE)
                } else {
                    Style::default().fg(palette::SUBTLE)
                };
                let marker = if is_cursor && focused && active_marker_x.is_none() {
                    Span::styled("\u{258D}", Style::default().fg(palette::AQUA))
                } else {
                    super::selection_marker(selected_region_gutter)
                };
                let track_num = if item.index_number > 0 {
                    format!("{:>width$}. ", item.index_number, width = track_num_width)
                } else {
                    format!("{:>width$}. ", i + 1, width = track_num_width)
                };
                let mut title_spans = vec![marker];
                if selected_region_gutter {
                    title_spans.push(Span::raw(" "));
                }
                let num_w = track_num.chars().count();
                let play_icon_w = if is_playing { 2 } else { 0 };
                let title_width = title_col_w.saturating_sub(num_w + play_icon_w).max(1);
                let title_lines = wrap(&item.name, title_width);
                let mut wrapped_title_lines = Vec::with_capacity(title_lines.len());
                for (line_idx, line) in title_lines.into_iter().enumerate() {
                    if line_idx == 0 {
                        let mut first_line = title_spans.clone();
                        first_line.push(Span::styled(
                            track_num.clone(),
                            Style::default().fg(palette::SOFT_WHITE),
                        ));
                        first_line.push(Span::styled(
                            line.into_owned(),
                            Style::default().fg(palette::SOFT_WHITE),
                        ));

                        wrapped_title_lines.push(Line::from(first_line));
                    } else {
                        wrapped_title_lines.push(Line::from(vec![
                            Span::raw(
                                " ".repeat(1 + if selected_region_gutter { 1 } else { 0 } + num_w),
                            ),
                            Span::styled(
                                line.into_owned(),
                                Style::default().fg(palette::SOFT_WHITE),
                            ),
                        ]));
                    }
                }
                let title_height = wrapped_title_lines.len() as u16;
                let title_cell = Cell::from(Text::from(wrapped_title_lines));
                let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
                let length = if len_secs > 0 {
                    fmt_duration_mmss(len_secs)
                } else {
                    "\u{2014}".to_string()
                };
                if show_length {
                    Row::new([
                        title_cell,
                        Cell::from(Line::from(length).alignment(Alignment::Right))
                            .style(Style::default().fg(palette::SUBTLE)),
                        Cell::from(""),
                    ])
                    .height(title_height)
                    .style(row_style)
                } else {
                    Row::new([title_cell, Cell::from(""), Cell::from("")])
                        .height(title_height)
                        .style(row_style)
                }
            })
            .collect();

        let mut state = TableState::default();
        state.select(Some(cursor));
        let table = Table::new(
            rows,
            [
                Constraint::Min(10),
                Constraint::Length(if show_length { dur_col_w as u16 } else { 0 }),
                Constraint::Length(1),
            ],
        )
        .column_spacing(1)
        .row_highlight_style(Style::default());
        f.render_stateful_widget(table, table_area, &mut state);
        layout.cursor_screen_y =
            Some(table_area.y + (cursor.saturating_sub(state.offset())) as u16);

        let visible_rows = table_area.height as usize;
        if !selected_region_gutter && n > visible_rows {
            let max_offset = n.saturating_sub(visible_rows);
            super::render_right_scrollbar(f, table_area, max_offset, state.offset());
        }
    }
}
