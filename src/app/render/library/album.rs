use super::super::super::palette;
use super::super::super::ui_util::{fmt_duration, trunc_str};
use super::super::super::App;
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;
use tui_scrollbar::{GlyphSet, ScrollBar, ScrollLengths};

impl App {
    pub(super) fn render_album_view(&mut self, f: &mut Frame, area: Rect, lib_idx: usize) {
        if lib_idx >= self.libs.len() {
            return;
        }
        let (items, cursor, album_id) = {
            let lvl = match self.libs[lib_idx].nav_stack.last() {
                Some(l) => l,
                None => return,
            };
            (lvl.items.clone(), lvl.cursor, lvl.parent_id.clone())
        };
        let n = items.len();
        if n == 0 {
            f.render_widget(
                Paragraph::new("  (empty)").style(Style::default().fg(palette::MUTED)),
                area,
            );
            return;
        }

        let first = &items[0];
        let album_name = self.libs[lib_idx]
            .nav_stack
            .last()
            .map(|l| l.title.clone())
            .unwrap_or_else(|| first.album.clone());
        let artist = first.artist.clone();
        let year = first.production_year;

        let left_w = ((area.width as u32 * 2 / 5) as u16).clamp(20, 60);
        let right_x = area.x + left_w + 1;
        let right_w = area.width.saturating_sub(left_w + 1);
        let left_area = Rect {
            x: area.x,
            y: area.y,
            width: left_w,
            height: area.height,
        };
        let right_area = Rect {
            x: right_x,
            y: area.y,
            width: right_w,
            height: area.height,
        };

        let cache_key = format!("{}:{}", album_id, crate::config::IMAGE_CACHE_SUFFIX_LIBRARY);
        self.fetch_list_card_image_when_idle(
            cache_key.clone(),
            album_id,
            String::new(),
            &["AudioChild", "Primary"],
        );
        let mut meta_parts: Vec<String> = Vec::new();
        if year > 0 {
            meta_parts.push(format!("{}", year));
        }
        meta_parts.push(format!("{} tracks", n));
        let ep_tag = meta_parts.join("  ");
        self.render_card_slot(
            f,
            left_area,
            true,
            true,
            false,
            true,
            true,
            false,
            &cache_key,
            &album_name,
            &artist,
            &ep_tag,
            0,
            0,
            0,
            false,
            None,
            None,
            true,
        );

        let playback = self.effective_playback_state();
        let now_playing_id: Option<String> = if playback.active {
            self.playback_queue()
                .items
                .get(playback.active_idx)
                .map(|i| i.id.clone())
        } else {
            None
        };

        let show_length = right_w > 40;
        let dur_col_w: usize = if show_length { 7 } else { 0 };
        let title_col_w =
            (right_w as usize).saturating_sub(1 + if show_length { dur_col_w + 1 } else { 0 });

        let rows: Vec<Row> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_cursor = i == cursor;
                let is_playing = now_playing_id.as_deref() == Some(item.id.as_str());
                let row_style = if is_playing {
                    Style::default()
                        .fg(palette::GREEN)
                        .add_modifier(Modifier::BOLD)
                } else if is_cursor {
                    Style::default().fg(palette::YELLOW)
                } else {
                    Style::default().fg(palette::WHITE)
                };
                let marker = if is_cursor {
                    Span::styled("▌", Style::default().fg(palette::AQUA))
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
                    fmt_duration(len_secs)
                } else {
                    "—".to_string()
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
        f.render_stateful_widget(table, right_area, &mut state);

        let total_rows = n;
        let visible_rows = right_area.height as usize;
        if total_rows > visible_rows {
            let scrollbar = ScrollBar::vertical(ScrollLengths {
                content_len: total_rows,
                viewport_len: visible_rows,
            })
            .offset(state.offset())
            .glyph_set(super::super::thin_vertical_thumb(GlyphSet::minimal()))
            .track_style(Style::default().fg(palette::SCROLLBAR))
            .thumb_style(Style::default().fg(palette::SCROLLBAR));
            f.render_widget(
                &scrollbar,
                Rect {
                    x: right_area.x + right_area.width.saturating_sub(1),
                    width: 1,
                    ..right_area
                },
            );
        }
    }
}
