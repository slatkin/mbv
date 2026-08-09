use super::super::ui_util::*;
use super::detail_series::{
    series_meta_line, wrap_overview_lines, SERIES_DETAIL_DIVIDER_ROWS,
    SERIES_DETAIL_EPISODE_ROWS_ESTIMATE, SERIES_DETAIL_TRAILING_BLANK_ROWS, SERIES_IMAGE_COLS,
    SERIES_IMAGE_PLACEHOLDER_ROWS, SERIES_IMAGE_ROWS,
};
use super::RENDER_FILTER;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    /// Renders the selected Series' season pills + episode table into the
    /// inline hero slot (`render_list` reserves `area`'s rows via
    /// `series_inline_detail_rows` and paints the surrounding block
    /// border/background itself -- this draws only the content, mirroring
    /// how `render_compact_detail` is the movie hero's content-only
    /// counterpart).
    pub(super) fn render_series_inline_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        show_title: bool,
        _layout: &mut LayoutMain,
    ) {
        if area.height == 0 {
            return;
        }

        let Some(item) = self.selected_series_item(lib_idx) else {
            return;
        };
        let (in_selection, _) = self.series_selection_state(lib_idx, &item.id);

        // Fetch series detail if not cached
        if !item.id.is_empty() {
            self.fetch_series_detail(item.id.clone());
        }

        let inner_x = area.x;
        let inner_w = (area.width as usize).saturating_sub(1);
        let inner_w16 = area.width.saturating_sub(1);
        let max_y = area.y + area.height;
        let mut row = area.y;

        let text_color = if focused {
            palette::WHITE
        } else {
            palette::SUBTLE
        };

        // — Title row (two-column lists only) —
        // Mirrors the movie hero's top-row title (`render_compact_detail`
        // in `detail.rs`): the selected item's name in yellow, pushing the
        // poster/meta content down a row. Skipped for one-column lists, where
        // the full-width list-row title directly above the block already
        // shows the name.
        if show_title {
            row = super::detail::render_hero_title_row(
                f,
                inner_x,
                row,
                max_y,
                inner_w16,
                &item.display_name(),
                focused,
            );
        }

        // ── Series Primary image (right-aligned, text wraps around it) ───
        let img_start_row = row;
        let primary_cache_key = format!("{}:ser_primary", item.id);
        if !item.id.is_empty() && self.images_enabled() {
            self.fetch_card_image(
                primary_cache_key.clone(),
                item.id.clone(),
                String::new(),
                &["Primary"],
            );
        }
        let img_loading = !item.id.is_empty()
            && self.images_enabled()
            && self.card_image_loading.contains(&primary_cache_key);
        let (img_actual_w, img_height, img_is_placeholder): (u16, u16, bool) = {
            if let Some(state) = self.cached_image_protocol_mut(&primary_cache_key) {
                let avail = ratatui::layout::Size {
                    width: SERIES_IMAGE_COLS,
                    height: SERIES_IMAGE_ROWS,
                };
                match state.size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), avail) {
                    Some(actual) => (actual.width, actual.height, false),
                    None => (SERIES_IMAGE_COLS, SERIES_IMAGE_PLACEHOLDER_ROWS, true),
                }
            } else if img_loading {
                (SERIES_IMAGE_COLS, SERIES_IMAGE_PLACEHOLDER_ROWS, true)
            } else {
                (0, 0, false)
            }
        };
        let img_x = area.x + area.width.saturating_sub(img_actual_w);
        let img_end_row = img_start_row + img_height;
        let narrow_w = inner_w.saturating_sub(img_actual_w as usize);
        let narrow_w16 = inner_w16.saturating_sub(img_actual_w);
        let text_dims = |r: u16| -> (usize, u16) {
            if img_height > 0 && r >= img_start_row && r < img_end_row {
                (narrow_w, narrow_w16)
            } else {
                (inner_w, inner_w16)
            }
        };

        // Series metadata (year range + genre)
        let ser_meta = series_meta_line(&item);
        if !ser_meta.is_empty() && row < max_y {
            let (tw, tw16) = text_dims(row);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    trunc_str(&ser_meta, tw),
                    Style::default().fg(palette::SUBTLE),
                ))),
                Rect {
                    x: inner_x,
                    y: row,
                    width: tw16,
                    height: 1,
                },
            );
            row += 1;
        }

        // Blank spacer after series block
        if row < max_y {
            row += 1;
        }

        // Overview (word-wrapped, respects image shadow width)
        if !item.overview.is_empty() && row < max_y {
            let overview_start_row = row;
            let lines = wrap_overview_lines(&item.overview, |line_idx| {
                text_dims(overview_start_row + line_idx as u16).0
            });
            // Cap at available rows minus space for the season row and episode list --
            // shares SERIES_DETAIL_* constants with `series_inline_detail_rows`
            // so the reserved space and what's actually drawn stay in sync.
            let reserved_for_below = (SERIES_DETAIL_DIVIDER_ROWS
                + if in_selection {
                    SERIES_DETAIL_EPISODE_ROWS_ESTIMATE
                } else {
                    0
                }
                + SERIES_DETAIL_TRAILING_BLANK_ROWS) as u16;
            let available_rows =
                (max_y.saturating_sub(row).saturating_sub(reserved_for_below)) as usize;
            for line_text in lines.iter().take(available_rows) {
                if row >= max_y {
                    break;
                }
                let (_, tw16) = text_dims(row);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        line_text.clone(),
                        Style::default().fg(text_color),
                    ))),
                    Rect {
                        x: inner_x,
                        y: row,
                        width: tw16,
                        height: 1,
                    },
                );
                row += 1;
            }
            if !lines.is_empty() && row < max_y {
                row += 1;
            }
        }

        // ── Render series image last so it layers over text ───────────────
        if img_height > 0 {
            let img_rect = Rect {
                x: img_x,
                y: img_start_row,
                width: img_actual_w,
                height: img_height,
            };
            if img_is_placeholder {
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::OVERLAY)),
                    img_rect,
                );
            } else if let Some(state) = self.cached_image_protocol_mut(&primary_cache_key) {
                type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
                    img_rect,
                    state,
                );
            }
        }

        // ── Grey divider with season tabs overlaid ───────────────────────
        // Fetch series detail from cache
        let series_detail = self.series_detail_cache.get(&item.id).cloned();
        let season_cursor = self.libs[lib_idx].series_season_cursor;
        let ep_cursor = self.libs[lib_idx].series_selection;
        if let Some(ref detail) = series_detail {
            if !detail.seasons.is_empty() && row < max_y {
                let (_, season_w16) = text_dims(row);
                let prefix_w = "Series: ".width();
                if in_selection {
                    let tab_labels: Vec<String> = detail
                        .seasons
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let n = if s.index_number > 0 {
                                s.index_number as usize
                            } else {
                                i + 1
                            };
                            format!("{:02}", n)
                        })
                        .collect();
                    let n_tabs = tab_labels.len();
                    let ids: Vec<usize> = (0..n_tabs).collect();
                    // Render the `Series:` label separately, then delegate the
                    // remaining row to the shared pill bar so season choices
                    // share the canonical pill appearance and keep the
                    // selected season visible on overflow. The bar's returned
                    // hitboxes are discarded: season navigation is
                    // keyboard-only and must not alter library click targets.
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            "Series: ",
                            Style::default()
                                .fg(palette::YELLOW)
                                .add_modifier(Modifier::BOLD),
                        ))),
                        Rect {
                            x: area.x,
                            y: row,
                            width: prefix_w as u16,
                            height: 1,
                        },
                    );
                    super::render_pill_bar(
                        f,
                        Rect {
                            x: area.x + prefix_w as u16,
                            y: row,
                            width: season_w16.saturating_sub(prefix_w as u16),
                            height: 1,
                        },
                        super::PillBar {
                            labels: &tab_labels,
                            ids: &ids,
                            selected_pos: season_cursor,
                            prefix: Some(" ⌘ "),
                        },
                    );
                    row += 1;
                } else {
                    let spans: Vec<Span> = vec![
                        Span::styled(
                            "Series: ",
                            Style::default()
                                .fg(palette::YELLOW)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            detail.seasons.len().to_string(),
                            Style::default().fg(palette::SOFT_WHITE),
                        ),
                    ];
                    f.render_widget(
                        Paragraph::new(Line::from(spans)),
                        Rect {
                            x: area.x,
                            y: row,
                            width: season_w16,
                            height: 1,
                        },
                    );
                    row += 1;
                }
            }

            // ── Episode list ─────────────────────────────────────────────
            let active_season = detail.seasons.get(season_cursor);
            let episodes = active_season
                .and_then(|s| detail.episodes.get(&s.id))
                .map(|eps| eps.as_slice())
                .unwrap_or(&[]);
            if in_selection && !episodes.is_empty() && row < max_y {
                let (_, table_width) = text_dims(row);
                let table_area = Rect {
                    x: area.x,
                    y: row,
                    width: table_width,
                    height: max_y
                        .saturating_sub(row)
                        .saturating_sub(SERIES_DETAIL_TRAILING_BLANK_ROWS as u16),
                };
                if table_area.height > 0 {
                    let show_length = table_area.width > 40;
                    let dur_col_w: usize = if show_length { 7 } else { 0 };
                    let title_col_w = (table_area.width as usize)
                        .saturating_sub(1 + if show_length { dur_col_w + 1 } else { 0 });

                    let playback = self.effective_playback_state();
                    let now_playing_id: Option<String> = if playback.active {
                        self.playback_queue()
                            .items
                            .get(playback.active_idx)
                            .map(|i| i.id.clone())
                    } else {
                        None
                    };

                    let rows: Vec<Row> = episodes
                        .iter()
                        .enumerate()
                        .map(|(i, ep)| {
                            let is_cursor = ep_cursor == Some(i);
                            let is_playing = now_playing_id.as_deref() == Some(ep.id.as_str());
                            let row_style = if is_cursor && focused {
                                Style::default().fg(palette::YELLOW)
                            } else if focused {
                                Style::default().fg(palette::WHITE)
                            } else {
                                Style::default().fg(palette::SUBTLE)
                            };
                            let marker = if is_cursor {
                                super::selection_marker(focused)
                            } else {
                                ratatui::text::Span::raw(" ")
                            };
                            let ep_num_w = episodes.len().to_string().len();
                            let ep_label = if ep.index_number > 0 {
                                format!("{:>ep_num_w$}. ", ep.index_number)
                            } else {
                                format!("{:>ep_num_w$}. ", i + 1)
                            };
                            let label_w = ep_label.chars().count();
                            let play_icon_w = if is_playing { 2 } else { 0 };
                            let title = trunc_str(
                                &ep.name,
                                title_col_w.saturating_sub(label_w + play_icon_w),
                            );
                            let mut title_spans = vec![marker];
                            title_spans
                                .push(Span::styled(ep_label, Style::default().fg(palette::MUTED)));
                            title_spans.push(Span::raw(title));

                            let title_cell = Cell::from(Line::from(title_spans));
                            let len_secs = ep.runtime_ticks / TICKS_PER_SECOND;
                            let length = if len_secs > 0 {
                                fmt_duration_approx(len_secs)
                            } else {
                                "\u{2014}".to_string()
                            };
                            if show_length {
                                Row::new([
                                    title_cell,
                                    Cell::from(Line::from(length).alignment(Alignment::Right))
                                        .style(Style::default().fg(palette::MUTED)),
                                    Cell::from(""),
                                ])
                                .style(row_style)
                            } else {
                                Row::new([title_cell, Cell::from(""), Cell::from("")])
                                    .style(row_style)
                            }
                        })
                        .collect();

                    let mut state = TableState::default();
                    state.select(ep_cursor);
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
                }
            }
        } else if row < max_y {
            // Loading state
            super::render_placeholder(
                f,
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
                " Loading\u{2026}",
            );
        }
    }
}
