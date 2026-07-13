use super::super::super::ui_util::*;
use super::{natural_sort_key, parse_album_folder_name, strip_article};
use crate::app::layout::LayoutPower;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

const INLINE_ALBUM_DETAIL_INDENT: u16 = 2;

enum GroupedAlbumDisplayRow {
    ArtistHeader(String),
    AlbumDetailRule,
    Album(usize),
    AlbumDetailStart(usize),
    AlbumDetailContinuation,
    AlbumLoading,
}

struct GroupedAlbumDisplayPlan {
    order: Vec<usize>,
    rows: Vec<GroupedAlbumDisplayRow>,
    display_cursor: usize,
}

impl GroupedAlbumDisplayRow {
    fn album_index(&self) -> Option<usize> {
        match self {
            Self::Album(idx) => Some(*idx),
            _ => None,
        }
    }
}

impl App {
    fn build_grouped_album_display_plan(
        &mut self,
        albums: &[mbv_core::api::MediaItem],
        cursor: usize,
        fetch_missing_tracks: bool,
    ) -> GroupedAlbumDisplayPlan {
        let mut album_info: Vec<(String, String, String)> = Vec::with_capacity(albums.len());
        for item in albums {
            let artist = self.resolve_group_album_artist(item);
            let (year_str, album_name) = if !item.artist.is_empty() {
                let year_str = if item.production_year > 0 {
                    item.production_year.to_string()
                } else {
                    String::new()
                };
                (year_str, item.display_name())
            } else if let Some((_, year, album)) = parse_album_folder_name(&item.name) {
                let year_str = if year > 0 {
                    year.to_string()
                } else {
                    String::new()
                };
                (year_str, album)
            } else {
                (String::new(), item.display_name())
            };
            album_info.push((artist, year_str, album_name));
        }

        let mut order: Vec<usize> = (0..album_info.len()).collect();
        order.sort_by_key(|&i| natural_sort_key(strip_article(&album_info[i].0)));

        let mut rows: Vec<GroupedAlbumDisplayRow> = Vec::new();
        let mut last_artist = String::new();
        for &idx in &order {
            let artist = &album_info[idx].0;
            if artist != &last_artist {
                rows.push(GroupedAlbumDisplayRow::ArtistHeader(artist.clone()));
                last_artist = artist.clone();
            }
            if idx == cursor {
                match self.album_tracks_cache.get(&albums[idx].id) {
                    Some(tracks) if !tracks.is_empty() => {
                        let detail_rows = 1 + tracks.len();
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                        rows.push(GroupedAlbumDisplayRow::Album(idx));
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailStart(idx));
                        rows.extend(
                            std::iter::repeat_with(|| {
                                GroupedAlbumDisplayRow::AlbumDetailContinuation
                            })
                            .take(detail_rows.saturating_sub(1)),
                        );
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                    }
                    Some(_) => rows.push(GroupedAlbumDisplayRow::Album(idx)),
                    None => {
                        if fetch_missing_tracks {
                            self.fetch_album_tracks(albums[idx].id.clone());
                        }
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                        rows.push(GroupedAlbumDisplayRow::Album(idx));
                        rows.push(GroupedAlbumDisplayRow::AlbumLoading);
                        rows.push(GroupedAlbumDisplayRow::AlbumDetailRule);
                    }
                }
            } else {
                rows.push(GroupedAlbumDisplayRow::Album(idx));
            }
        }

        let display_cursor = rows
            .iter()
            .position(|row| matches!(row, GroupedAlbumDisplayRow::Album(i) if *i == cursor))
            .unwrap_or(0);

        GroupedAlbumDisplayPlan {
            order,
            rows,
            display_cursor,
        }
    }

    pub(in crate::app) fn page_power_grouped_album_cursor(
        &mut self,
        lib_idx: usize,
        page_down: bool,
    ) -> bool {
        if self.queue_view != crate::app::QUEUE_VIEW_POWER
            || self.power_left_tab != lib_idx + 1
            || !matches!(self.power_focus, crate::app::PowerFocus::Left)
            || self.libs[lib_idx].search.is_some()
            || self.libs[lib_idx].album_track_focus.is_some()
            || !self.is_viewing_album_folders(lib_idx)
        {
            return false;
        }

        let idle = self.list_image_fetches_allowed();
        self.last_nav_at = std::time::Instant::now();

        let Some(level) = self.libs[lib_idx].nav_stack.last() else {
            return false;
        };
        if level.items.is_empty() {
            return true;
        }

        let cursor = level.cursor;
        let albums = level.items.clone();
        let page = (self.layout.power.left_area.height as usize).max(1);
        let plan = self.build_grouped_album_display_plan(&albums, cursor, false);
        let target_row = if page_down {
            (plan.display_cursor + page).min(plan.rows.len().saturating_sub(1))
        } else {
            plan.display_cursor.saturating_sub(page)
        };
        let new_cursor = if page_down {
            plan.rows
                .iter()
                .skip(target_row)
                .find_map(GroupedAlbumDisplayRow::album_index)
                .unwrap_or_else(|| plan.order.last().copied().unwrap_or(cursor))
        } else {
            plan.rows[..=target_row]
                .iter()
                .rev()
                .find_map(GroupedAlbumDisplayRow::album_index)
                .unwrap_or_else(|| plan.order.first().copied().unwrap_or(cursor))
        };

        if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
            if level.cursor != new_cursor {
                level.cursor = new_cursor;
                self.libs[lib_idx].album_track_focus = None;
            }
        }
        if idle {
            self.maybe_fetch_next_page(lib_idx);
        }
        true
    }

    pub(super) fn render_power_grouped_album_rows(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        albums: &[mbv_core::api::MediaItem],
        cursor: usize,
        stored_scroll: usize,
        focused: bool,
        layout: &mut LayoutPower,
    ) -> usize {
        let visible = area.height as usize;
        let avail = (area.width as usize).saturating_sub(2);
        let mut album_info: Vec<(String, String, String)> = Vec::with_capacity(albums.len());
        for item in albums {
            let artist = self.resolve_group_album_artist(item);
            let (year_str, album_name) = if !item.artist.is_empty() {
                let year_str = if item.production_year > 0 {
                    item.production_year.to_string()
                } else {
                    String::new()
                };
                (year_str, item.display_name())
            } else if let Some((_, year, album)) = parse_album_folder_name(&item.name) {
                let year_str = if year > 0 {
                    year.to_string()
                } else {
                    String::new()
                };
                (year_str, album)
            } else {
                (String::new(), item.display_name())
            };
            album_info.push((artist, year_str, album_name));
        }

        let plan = self.build_grouped_album_display_plan(albums, cursor, true);
        layout.left_sorted_indices = plan.order.clone();
        let display_cursor = plan.display_cursor;
        let display_rows = plan.rows;
        let top_bound = if display_cursor > 0
            && matches!(
                display_rows[display_cursor - 1],
                GroupedAlbumDisplayRow::AlbumDetailRule
            ) {
            display_cursor - 1
        } else {
            display_cursor
        };
        let offset = stored_scroll.clamp(
            display_cursor
                .saturating_sub(visible.saturating_sub(1))
                .min(top_bound),
            top_bound,
        );

        let visible_rows: Vec<&GroupedAlbumDisplayRow> =
            display_rows.iter().skip(offset).take(visible).collect();
        for (row_idx, row) in visible_rows.iter().enumerate() {
            let row_area = Rect {
                x: area.x,
                y: area.y + row_idx as u16,
                width: area.width,
                height: 1,
            };
            let detail_row_area = Rect {
                x: row_area.x + INLINE_ALBUM_DETAIL_INDENT.min(row_area.width),
                width: row_area.width.saturating_sub(INLINE_ALBUM_DETAIL_INDENT),
                ..row_area
            };
            match row {
                GroupedAlbumDisplayRow::ArtistHeader(name) => {
                    let artist_label = trunc_str(name, avail);
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(artist_label, Style::default().fg(palette::YELLOW)),
                        ])),
                        row_area,
                    );
                }
                GroupedAlbumDisplayRow::AlbumDetailRule => {
                    let detail_avail = (detail_row_area.width as usize).saturating_sub(2);
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(
                                "\u{2500}".repeat(detail_avail),
                                Style::default().fg(palette::OVERLAY),
                            ),
                        ])),
                        detail_row_area,
                    );
                }
                GroupedAlbumDisplayRow::Album(idx) => {
                    let selected = *idx == cursor;
                    let (_, year_str, album_name) = &album_info[*idx];
                    let prefix_w = if year_str.is_empty() {
                        3
                    } else {
                        year_str.len() + 6
                    };
                    let name_w = avail.saturating_sub(prefix_w);
                    let trunc_name = trunc_str(album_name, name_w);
                    let fg = if focused {
                        palette::WHITE
                    } else {
                        palette::SUBTLE
                    };
                    let name_color = if selected && focused {
                        palette::IRIS
                    } else {
                        fg
                    };
                    let mut spans: Vec<Span> = Vec::new();
                    if selected {
                        spans.push(Span::styled("\u{258c}", Style::default().fg(palette::PINE)));
                    } else {
                        spans.push(Span::raw(" "));
                    }
                    if selected {
                        spans.push(Span::raw(" "));
                    } else if year_str.is_empty() {
                        spans.push(Span::raw("   "));
                    } else {
                        spans.push(Span::styled("   (", Style::default().fg(palette::SUBTLE)));
                    }
                    if selected && !year_str.is_empty() {
                        spans.push(Span::styled("(", Style::default().fg(palette::SUBTLE)));
                    }
                    if !year_str.is_empty() {
                        spans.push(Span::styled(
                            year_str.clone(),
                            Style::default().fg(palette::PINE),
                        ));
                        spans.push(Span::styled(") ", Style::default().fg(palette::SUBTLE)));
                    }
                    let name_style = if selected && focused {
                        Style::default().fg(name_color).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(name_color)
                    };
                    spans.push(Span::styled(trunc_name.to_string(), name_style));
                    let album_area = if selected { detail_row_area } else { row_area };
                    f.render_widget(Paragraph::new(Line::from(spans)), album_area);
                }
                GroupedAlbumDisplayRow::AlbumDetailStart(idx) => {
                    let height = visible_rows[row_idx..]
                        .iter()
                        .take_while(|r| {
                            matches!(
                                r,
                                GroupedAlbumDisplayRow::AlbumDetailStart(_)
                                    | GroupedAlbumDisplayRow::AlbumDetailContinuation
                            )
                        })
                        .count() as u16;
                    if let Some(tracks) = self.album_tracks_cache.get(&albums[*idx].id).cloned() {
                        let cursor = self.libs[lib_idx].album_track_focus.unwrap_or(0);
                        let detail_focused = self.libs[lib_idx].album_track_focus.is_some();
                        self.render_power_album_detail(
                            f,
                            Rect {
                                height,
                                ..detail_row_area
                            },
                            &tracks,
                            cursor,
                            detail_focused,
                            true,
                            layout,
                        );
                    }
                }
                GroupedAlbumDisplayRow::AlbumLoading => {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::styled("\u{258c}", Style::default().fg(palette::PINE)),
                            Span::raw(" "),
                            Span::styled("Loading\u{2026}", Style::default().fg(palette::MUTED)),
                        ])),
                        detail_row_area,
                    );
                }
                GroupedAlbumDisplayRow::AlbumDetailContinuation => {}
            }
        }

        if self.libs[lib_idx].album_track_focus.is_none() {
            layout.cursor_screen_y = Some(area.y + (display_cursor.saturating_sub(offset)) as u16);
        }

        layout.left_row_map = display_rows
            .iter()
            .skip(offset)
            .take(visible)
            .map(|dr| match dr {
                GroupedAlbumDisplayRow::ArtistHeader(_)
                | GroupedAlbumDisplayRow::AlbumDetailRule => None,
                GroupedAlbumDisplayRow::Album(idx) => Some(*idx),
                GroupedAlbumDisplayRow::AlbumDetailStart(_)
                | GroupedAlbumDisplayRow::AlbumDetailContinuation
                | GroupedAlbumDisplayRow::AlbumLoading => None,
            })
            .collect();

        let display_n = display_rows.len();
        if focused && display_n > visible {
            let max_off = display_n.saturating_sub(visible);
            super::render_power_scrollbar(f, area, max_off, offset);
        }

        offset
    }

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
        selected_region_gutter: bool,
        layout: &mut LayoutPower,
    ) {
        if area.height == 0 {
            return;
        }

        let n = items.len();
        if items.is_empty() {
            return;
        }
        let gutter_w = if selected_region_gutter { 2 } else { 1 };
        let inner_w = (area.width as usize).saturating_sub(gutter_w);
        let max_y = area.y + area.height;
        let mut row = area.y;

        let mut title_style = Style::default().fg(if focused {
            palette::YELLOW
        } else {
            palette::MUTED
        });
        if focused {
            title_style = title_style.add_modifier(Modifier::BOLD);
        }

        // — Album title: yellow when interactive, muted for inline preview —
        if row < max_y && !selected_region_gutter {
            let album_title = items[0].album.clone();
            let title = trunc_str(&album_title, inner_w);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(format!(" {}", title), title_style))),
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
        if row < max_y {
            if selected_region_gutter {
                let hint_w = (area.width as usize).saturating_sub(gutter_w);
                let hint = trunc_str("^P: Play all | ^A: Add to Queue | ^S: Shuffle", hint_w);
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled("\u{258c}", Style::default().fg(palette::PINE)),
                        Span::raw(" "),
                        Span::styled(hint.to_string(), Style::default().fg(palette::SUBTLE)),
                    ])),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                );
            }
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
            .saturating_sub(gutter_w + if show_length { dur_col_w + 1 } else { 0 });

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
                let marker = if selected_region_gutter || (is_cursor && focused) {
                    Span::styled("\u{258c}", Style::default().fg(palette::PINE))
                } else {
                    Span::raw(" ")
                };
                let track_num = if item.index_number > 0 {
                    format!("{}. ", item.index_number)
                } else {
                    format!("{}. ", i + 1)
                };
                let mut title_spans = vec![marker];
                if selected_region_gutter {
                    title_spans.push(Span::raw(" "));
                    if is_cursor && focused {
                        title_spans
                            .push(Span::styled("\u{258c}", Style::default().fg(palette::PINE)));
                    }
                }
                let num_w = track_num.chars().count();
                let focus_marker_w = usize::from(selected_region_gutter && is_cursor && focused);
                let title = trunc_str(
                    &item.name,
                    title_col_w.saturating_sub(num_w + focus_marker_w),
                );
                title_spans.push(Span::styled(
                    track_num,
                    Style::default().fg(palette::SUBTLE),
                ));
                title_spans.push(Span::raw(title));
                let title_cell = Cell::from(Line::from(title_spans));
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
            super::render_power_scrollbar(f, table_area, max_offset, state.offset());
        }
    }
}
