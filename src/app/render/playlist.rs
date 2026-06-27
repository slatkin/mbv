use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use crate::api::TICKS_PER_SECOND;
use super::super::{App, palette};
use super::super::ui_util::{fmt_duration, trunc_str};

impl App {
    pub(super) fn render_combined(&mut self, f: &mut Frame, area: Rect) {
        self.home_rect = area;
        self.layout_carousel_left_arrow = None;
        self.layout_carousel_right_arrow = None;
        self.layout_carousel_up_arrow = None;
        self.layout_carousel_down_arrow = None;
        self.layout_home_card_strips.clear();
        if self.home_search.is_some() {
            self.render_home_search(f, area);
        } else if self.home_card_view {
            self.render_home_cards(f, area);
        } else {
            self.render_home_panel(f, area);
        }
    }

    pub(super) fn render_playlist_panel(&mut self, f: &mut Frame, area: Rect) {
        if self.home_search.is_some() {
            self.render_home_search(f, area);
            return;
        }
        let (active, current_idx, live_pos, live_runtime, _live_paused) = self.effective_playback_state();

        self.playlist_rect = area;

        let inner = area;
        self.layout_playlist_inner = inner;

        if self.player_tab.items.is_empty() {
            f.render_widget(
                Paragraph::new("Add items with p from Home or library tabs")
                    .style(Style::default().fg(palette::MUTED)),
                inner,
            );
            return;
        }

        let cursor = self.player_tab.playlist_cursor;
        let table_area = inner;
        let show_ep_cols = self.player_tab.items.iter().any(|it| it.item_type == "Episode");

        // Fixed column widths + inter-column gaps of 1.
        let title_col_width = (table_area.width as i32
            - if show_ep_cols { 21 } else { 13 }).max(0) as usize;

        const SPINNER_FRAMES: &[&str] = &["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"];
        let spinner_char: &str = {
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            SPINNER_FRAMES[(ms / 150) as usize % SPINNER_FRAMES.len()]
        };

        let rows: Vec<Row> = self.player_tab.items.iter().enumerate().map(|(i, item)| {
            let now_playing = i == current_idx && active;
            let row_style = if i == cursor {
                Style::default().fg(palette::YELLOW)
            } else {
                Style::default().fg(palette::WHITE)
            };

            let indicator = if i == cursor {
                Cell::from("▐").style(Style::default().fg(palette::IRIS))
            } else {
                Cell::from(" ")
            };

            let title = item.playback_label();
            let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
            let length = if len_secs > 0 { fmt_duration(len_secs) } else { "—".to_string() };
            let (pos_ticks, rt_ticks) = if i == current_idx && active {
                let pos = if live_pos > 0 { live_pos } else { item.playback_position_ticks };
                (pos, live_runtime)
            } else {
                (item.playback_position_ticks, item.runtime_ticks)
            };
            // Spinner prefix "⠋ " costs 2 chars when now-playing.
            let spin_w: usize = if now_playing { 2 } else { 0 };
            // Now-playing title text is emby blue (not bold); others inherit row_style.
            let title_span_style = if now_playing {
                Style::default().fg(palette::FOAM)
            } else {
                Style::default()
            };
            let title_cell = if pos_ticks > 0 && rt_ticks > 0 && !item.is_audio() {
                let pct = (pos_ticks * 100 / rt_ticks.max(1)) as u64;
                // Now-playing progress is green; other in-progress rows are grey.
                let pct_style = if now_playing { palette::IRIS } else { palette::SUBTLE };
                let pct_str = format!(" {pct}%");
                let max_title = title_col_width.saturating_sub(pct_str.chars().count() + spin_w);
                let mut spans: Vec<Span> = if now_playing {
                    vec![Span::styled(spinner_char.to_string(), Style::default().fg(palette::IRIS)), Span::raw(" ")]
                } else { vec![] };
                spans.push(Span::styled(trunc_str(&title, max_title), title_span_style));
                spans.push(Span::styled(pct_str, Style::default().fg(pct_style)));
                Cell::from(Line::from(spans))
            } else {
                let max_title = title_col_width.saturating_sub(spin_w);
                let mut spans: Vec<Span> = if now_playing {
                    vec![Span::styled(spinner_char.to_string(), Style::default().fg(palette::IRIS)), Span::raw(" ")]
                } else { vec![] };
                spans.push(Span::styled(trunc_str(&title, max_title), title_span_style));
                Cell::from(Line::from(spans))
            };

            if show_ep_cols {
                let ep_tag = if item.item_type == "Episode" && item.parent_index_number > 0 {
                    format!("S{:02}/E{:02}", item.parent_index_number, item.index_number)
                } else { String::new() };
                Row::new([
                    indicator,
                    title_cell,
                    Cell::from(Line::from(ep_tag).alignment(Alignment::Right)).style(Style::default().fg(palette::SUBTLE)),
                    Cell::from(Line::from(length).alignment(Alignment::Right)),
                    Cell::from(""),
                ]).style(row_style)
            } else {
                Row::new([
                    indicator,
                    title_cell,
                    Cell::from(""),
                    Cell::from(Line::from(length).alignment(Alignment::Right)),
                    Cell::from(""),
                ]).style(row_style)
            }
        }).collect();

        let mut state = TableState::default();
        state.select(Some(cursor));
        let table = Table::new(rows, [
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(if show_ep_cols { 8 } else { 0 }),
            Constraint::Length(7),
            Constraint::Length(1),
        ])
        .column_spacing(1)
        .row_highlight_style(Style::default());
        f.render_stateful_widget(table, table_area, &mut state);
    }
}
