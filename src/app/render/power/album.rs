use super::super::super::ui_util::*;
use crate::app::layout::LayoutPower;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

impl App {
    /// Renders the music album detail panel (track list) into `area` — the lib
    /// slot below the card. The card itself already shows the album art (handled
    /// in `render_power_card`). Mirrors `render_power_detail` for movies.
    ///
    /// Takes `items`/`cursor` explicitly rather than reading `nav_stack`
    /// internally (#145) so it can render either the legacy drilled-in
    /// nav_stack level or the inline-album-detail cache (the currently
    /// highlighted album in the album-folder listing, fetched proactively
    /// via `fetch_album_tracks`) with the same code path.
    pub(super) fn render_power_album_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        items: &[mbv_core::api::MediaItem],
        cursor: usize,
        focused: bool,
        layout: &mut LayoutPower,
    ) {
        if area.height == 0 {
            return;
        }

        let n = items.len();
        if items.is_empty() {
            return;
        }
        let first = &items[0];
        let album_title = first.album.clone();
        let album_artist = first.artist.clone();
        let album_year = first.production_year;

        let inner_w = area.width as usize;
        let max_y = area.y + area.height;
        let mut row = area.y;

        // — Album title: yellow, left-aligned, no background —
        if row < max_y {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" {}", trunc_str(&album_title, inner_w.saturating_sub(1))),
                    Style::default()
                        .fg(palette::YELLOW)
                        .add_modifier(Modifier::BOLD),
                ))),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }

        // — Album artist: same colour as inactive tabs (SUBTLE) —
        if row < max_y && !album_artist.is_empty() {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" {}", trunc_str(&album_artist, inner_w.saturating_sub(1))),
                    Style::default().fg(palette::SUBTLE),
                ))),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }

        // — Release year: same colour as the VOL label (MUTED) —
        if row < max_y && album_year > 0 {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" {}", album_year),
                    Style::default().fg(palette::MUTED),
                ))),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }

        // — Blank spacer row —
        if row < max_y {
            row += 1;
        }

        // — Scrollable track list —
        let table_area = Rect {
            x: area.x,
            y: row,
            width: area.width,
            height: max_y.saturating_sub(row),
        };
        if table_area.height == 0 {
            return;
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
            .saturating_sub(1 + if show_length { dur_col_w + 1 } else { 0 });

        let rows: Vec<Row> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_cursor = i == cursor;
                let is_playing = now_playing_id.as_deref() == Some(item.id.as_str());
                let row_style = if is_playing {
                    Style::default()
                        .fg(palette::FOAM)
                        .add_modifier(Modifier::BOLD)
                } else if is_cursor && focused {
                    Style::default().fg(palette::YELLOW)
                } else if focused {
                    Style::default().fg(palette::WHITE)
                } else {
                    Style::default().fg(palette::SUBTLE)
                };
                let marker = if is_cursor && focused {
                    Span::styled("\u{258c}", Style::default().fg(palette::PINE))
                } else {
                    Span::raw(" ")
                };
                let track_num = if item.index_number > 0 {
                    format!("{}. ", item.index_number)
                } else {
                    format!("{}. ", i + 1)
                };
                let num_w = track_num.chars().count();
                let title = trunc_str(&item.name, title_col_w.saturating_sub(num_w));
                let title_cell = Cell::from(Line::from(vec![
                    marker,
                    Span::styled(track_num, Style::default().fg(palette::SUBTLE)),
                    Span::raw(title),
                ]));
                let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
                let length = if len_secs > 0 {
                    fmt_duration_approx(len_secs)
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
                    .style(row_style)
                } else {
                    Row::new([title_cell, Cell::from(""), Cell::from("")]).style(row_style)
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
        if n > visible_rows {
            let max_offset = n.saturating_sub(visible_rows);
            let mut sb_state = ScrollbarState::new(max_offset + 1).position(state.offset());
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("\u{2590}")
                    .track_symbol(Some(" "))
                    .begin_symbol(None)
                    .end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                table_area,
                &mut sb_state,
            );
        }
    }
}
