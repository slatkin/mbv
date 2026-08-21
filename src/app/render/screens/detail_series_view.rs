use super::detail_series::{
    series_meta_line, wrap_overview_lines, SERIES_DETAIL_DIVIDER_ROWS,
    SERIES_DETAIL_EPISODE_ROWS_ESTIMATE, SERIES_DETAIL_TRAILING_BLANK_ROWS, SERIES_IMAGE_COLS,
    SERIES_IMAGE_PLACEHOLDER_ROWS, SERIES_IMAGE_ROWS,
};
use crate::app::layout::LayoutMain;
use crate::app::render::components::hero::{HeroContent, HeroImage, HeroLine, ImageTop};
use crate::app::render::components::list_rows::selection_marker;
use crate::app::render::{render_pill_bar, render_placeholder, MarkerEdge, PillBar, RENDER_FILTER};
use crate::app::ui_util::*;
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
    pub(in crate::app::render) fn render_series_inline_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        show_title: bool,
        persistent: bool,
        layout: &mut LayoutMain,
    ) {
        if area.height == 0 {
            return;
        }

        let Some(item) = self.selected_series_item(lib_idx) else {
            return;
        };
        let (in_selection, _) = self.series_selection_state(lib_idx, &item.id);
        let show_episodes = persistent || in_selection;

        // Fetch series detail if not cached
        if !item.id.is_empty() {
            self.fetch_series_detail(item.id.clone());
        }

        let inner_w = (area.width as usize).saturating_sub(1);
        let inner_w16 = area.width.saturating_sub(1);
        let max_y = area.y + area.height;

        // ── Series Primary image sizing (right-aligned, text wraps around
        //    it) -- resolved here (needs `self`'s image cache) and handed to
        //    the `Hero` component to lay text out around ───────────────────
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

        // Series metadata (year range + genre) and overview need the same
        // width-narrowing `text_dims` the movie hero uses, computed here
        // (before the image's actual on-screen row is known) only for the
        // overview's line-by-line wrap width -- the title row hasn't
        // rendered yet, so estimate the image's top row the same way the
        // `Hero` component will (`ImageTop::AfterTitle`: right after the
        // title row, or `area.y` if there's no title).
        let title_rows = if show_title { 1 } else { 0 };
        let img_start_row_estimate = area.y + title_rows;
        let img_end_row_estimate = img_start_row_estimate + img_height;
        let narrow_w = inner_w.saturating_sub(img_actual_w as usize);
        let text_dims_pre = |r: u16| -> usize {
            if img_height > 0 && r >= img_start_row_estimate && r < img_end_row_estimate {
                narrow_w
            } else {
                inner_w
            }
        };

        let ser_meta = series_meta_line(&item);
        // Row the overview starts on: title (0/1) + meta (0/1) + spacer (1,
        // unconditional -- see `unconditional_spacer_after_meta`).
        let overview_start_row = area.y + title_rows + (!ser_meta.is_empty()) as u16 + 1;
        let overview_lines = if !item.overview.is_empty() {
            let lines = wrap_overview_lines(&item.overview, |line_idx| {
                text_dims_pre(overview_start_row + line_idx as u16)
            });
            // Cap at available rows minus space for the season row and episode list --
            // shares SERIES_DETAIL_* constants with `series_inline_detail_rows`
            // so the reserved space and what's actually drawn stay in sync.
            let reserved_for_below = (SERIES_DETAIL_DIVIDER_ROWS
                + if show_episodes {
                    SERIES_DETAIL_EPISODE_ROWS_ESTIMATE
                } else {
                    0
                }
                + SERIES_DETAIL_TRAILING_BLANK_ROWS) as u16;
            let available_rows = (max_y
                .saturating_sub(overview_start_row)
                .saturating_sub(reserved_for_below)) as usize;
            lines.into_iter().take(available_rows).collect()
        } else {
            Vec::new()
        };
        let hero_lines: Vec<HeroLine> = overview_lines.into_iter().map(HeroLine::Plain).collect();
        let has_overview_lines = !hero_lines.is_empty();
        let title = item.display_name();

        let hero_content = HeroContent {
            title: show_title.then_some(title.as_str()),
            meta_line: (!ser_meta.is_empty()).then_some(ser_meta.as_str()),
            meta_color: palette::TEXT_DETAIL_META,
            show_playing: false,
            unconditional_spacer_after_meta: true,
            lines: &hero_lines,
            image: (img_height > 0).then_some(HeroImage {
                actual_w: img_actual_w,
                height: img_height,
                top: ImageTop::AfterTitle,
            }),
        };
        let result = crate::app::render::components::hero::paint_hero_content(
            f,
            area,
            &hero_content,
            focused,
        );
        // Trailing spacer after the overview block, matching the original's
        // "if !lines.is_empty() && row < max_y { row += 1 }".
        let mut row = if has_overview_lines && result.next_row < max_y {
            result.next_row + 1
        } else {
            result.next_row
        };

        // Reconstruct `text_dims` from the Hero's actual painted image rect
        // (not the pre-title estimate above) for the season row/table below,
        // which still need to narrow around the image exactly as before.
        let (img_start_row, img_actual_w, img_height) = match result.img_rect {
            Some(r) => (r.y, r.width, r.height),
            None => (0, 0, 0),
        };
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

        // ── Render series image last so it layers over text ───────────────
        if let Some(img_rect) = result.img_rect {
            if img_is_placeholder {
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::BORDER_UNFOCUSED)),
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
                if persistent || in_selection {
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
                                .fg(palette::TEXT_FOCUS_ACCENT)
                                .add_modifier(Modifier::BOLD),
                        ))),
                        Rect {
                            x: area.x,
                            y: row,
                            width: prefix_w as u16,
                            height: 1,
                        },
                    );
                    let season_tabs = render_pill_bar(
                        f,
                        Rect {
                            x: area.x + prefix_w as u16,
                            y: row,
                            width: season_w16.saturating_sub(prefix_w as u16),
                            height: 1,
                        },
                        PillBar {
                            labels: &tab_labels,
                            ids: &ids,
                            selected_pos: season_cursor,
                            prefix: Some(" ⌘ "),
                        },
                    );
                    if persistent {
                        layout.tv_wide_season_tabs = season_tabs;
                    }
                    row += 1;
                } else {
                    let spans: Vec<Span> = vec![
                        Span::styled(
                            "Series: ",
                            Style::default()
                                .fg(palette::TEXT_FOCUS_ACCENT)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            detail.seasons.len().to_string(),
                            Style::default().fg(palette::TEXT_EMPHASIS),
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
            if show_episodes && !episodes.is_empty() && row < max_y {
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
                    let show_length = table_area.width > 30;
                    let dur_col_w: usize = if show_length { 7 } else { 0 };
                    let title_col_w = (table_area.width as usize)
                        .saturating_sub(1 + if show_length { dur_col_w + 1 } else { 0 });

                    let visible = table_area.height as usize;
                    let start = if persistent {
                        ep_cursor
                            .map(|cursor| cursor.saturating_sub(visible.saturating_sub(1)))
                            .unwrap_or(0)
                            .min(episodes.len().saturating_sub(visible))
                    } else {
                        0
                    };
                    let rows: Vec<Row> = episodes
                        .iter()
                        .enumerate()
                        .skip(start)
                        .take(visible)
                        .map(|(i, ep)| {
                            let is_cursor = ep_cursor == Some(i);
                            let row_style = if is_cursor && focused {
                                Style::default().fg(palette::TEXT_FOCUS_ACCENT)
                            } else if focused {
                                Style::default().fg(palette::TEXT_STRONG)
                            } else {
                                Style::default().fg(palette::TEXT_SECONDARY)
                            };
                            let marker = selection_marker(is_cursor, MarkerEdge::Left);
                            let ep_num_w = episodes.len().to_string().len();
                            let ep_label = if ep.index_number > 0 {
                                format!("{:>ep_num_w$}. ", ep.index_number)
                            } else {
                                format!("{:>ep_num_w$}. ", i + 1)
                            };
                            let label_w = ep_label.chars().count();
                            let title = trunc_str(&ep.name, title_col_w.saturating_sub(label_w));
                            let mut title_spans = vec![marker];
                            title_spans.push(Span::styled(
                                ep_label,
                                Style::default().fg(palette::TEXT_MUTED),
                            ));
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
                                        .style(Style::default().fg(palette::TEXT_MUTED)),
                                    Cell::from(""),
                                ])
                                .style(row_style)
                            } else {
                                Row::new([title_cell, Cell::from(""), Cell::from("")])
                                    .style(row_style)
                            }
                        })
                        .collect();
                    for (visible_idx, (i, _)) in episodes
                        .iter()
                        .enumerate()
                        .skip(start)
                        .take(visible)
                        .enumerate()
                    {
                        let row_rect = Rect {
                            x: table_area.x,
                            y: table_area.y + visible_idx as u16,
                            width: table_area.width,
                            height: 1,
                        };
                        layout.tv_wide_episode_rows.push((row_rect, i));
                    }
                    f.render_widget(
                        Table::new(
                            rows,
                            [
                                Constraint::Min(10),
                                Constraint::Length(if show_length { dur_col_w as u16 } else { 0 }),
                                Constraint::Length(1),
                            ],
                        )
                        .column_spacing(1),
                        table_area,
                    );
                }
            }
        } else if row < max_y {
            // Loading state
            render_placeholder(
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
