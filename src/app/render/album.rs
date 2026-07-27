use super::super::ui_util::trunc_str;
use super::album_art::{INLINE_ALBUM_ART_RESERVED, INLINE_ALBUM_ART_ROWS};
use super::album_plan::GroupedAlbumDisplayRow;
use super::parse_album_folder_name;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;

impl App {
    pub(super) fn render_power_grouped_album_rows(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        albums: &[mbv_core::api::MediaItem],
        cursor: usize,
        stored_scroll: usize,
        focused: bool,
        layout: &mut LayoutMain,
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

        layout.inline_image_rect = None;

        let selected = self.selected_power_music_artist_header(lib_idx);
        let selectable_headers = self.is_music_group_view(lib_idx);
        // When an artist header is the focused row, the album under the
        // cursor must not also render as selected -- only one row group
        // (header or album) is ever the actual focus target at a time.
        let header_selected = selected.is_some();
        // Inline track expansion for the selected album: in the music-group
        // (pill selector) view, only expand once the user has pressed Enter
        // to enter track-selection mode (`album_track_focus`); elsewhere
        // (plain album-folder browsing) the existing always-expand behavior
        // is unchanged.
        let expand_selected = !selectable_headers || self.libs[lib_idx].album_track_focus.is_some();
        let plan = self.build_grouped_album_display_plan(
            albums,
            cursor,
            true,
            selectable_headers,
            selected.as_ref(),
            expand_selected,
            Some((
                area.width,
                if self.images_enabled() && area.width >= INLINE_ALBUM_ART_RESERVED + 20 {
                    INLINE_ALBUM_ART_RESERVED
                } else {
                    0
                },
            )),
        );
        if selected.is_some() && !plan.selected_artist_header_valid {
            self.clear_artist_header_focus(lib_idx);
        }
        layout.left_sorted_indices = plan.order.clone();
        let display_cursor = plan.display_cursor;
        let display_rows = plan.rows;
        let selected_block_bounds = plan.selected_block_bounds;
        let track_detail_bounds = plan.track_detail_bounds;
        let selected_art_reserved_w = if self.images_enabled()
            && selected_block_bounds.is_some()
            && area.width >= INLINE_ALBUM_ART_RESERVED + 20
        {
            INLINE_ALBUM_ART_RESERVED
        } else {
            0
        };
        let selected_art_abs_rows =
            selected_block_bounds.and_then(|(top_pad_abs, bottom_pad_abs)| {
                if selected_art_reserved_w == 0 {
                    return None;
                }
                let art_top = top_pad_abs + 1;
                let art_bottom = (art_top + INLINE_ALBUM_ART_ROWS as usize).min(bottom_pad_abs);
                (art_bottom > art_top).then_some((art_top, art_bottom))
            });
        let top_bound = selected_block_bounds
            .map(|(top, _)| top.saturating_sub(1))
            .unwrap_or(display_cursor);
        let rows_below_block = selected_block_bounds
            .map(|(_, bottom_pad_abs)| (bottom_pad_abs + 1).saturating_sub(display_cursor))
            .unwrap_or(0);
        let lower_bound = (display_cursor + rows_below_block)
            .saturating_sub(visible.saturating_sub(1))
            .min(top_bound);
        let offset = stored_scroll.clamp(lower_bound, top_bound);

        // Paint the colored background block before rendering row content
        if let Some((top_pad_abs, bottom_pad_abs)) = selected_block_bounds {
            let bg = if focused {
                palette::MEDIA_SELECTED_BG
            } else {
                palette::PLAYBACK_PANEL_BG
            };
            super::render_selected_block_background(
                f,
                area,
                offset,
                visible,
                top_pad_abs,
                bottom_pad_abs,
                bg,
            );
        }

        // Paint the track detail block background
        if let Some((track_start, track_end)) = track_detail_bounds {
            let vis_top = track_start.max(offset);
            let vis_bot = (track_end.saturating_sub(1)).min(offset + visible.saturating_sub(1));
            if vis_top <= vis_bot {
                let block_y = area.y + (vis_top - offset) as u16;
                let block_h = (vis_bot - vis_top + 1) as u16;
                let block_x = area.x + 4;
                let block_w = area
                    .width
                    .saturating_sub(6)
                    .saturating_sub(selected_art_reserved_w);
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::TRACK_BLOCK_BG)),
                    Rect {
                        x: block_x,
                        y: block_y,
                        width: block_w,
                        height: block_h,
                    },
                );
            }
        }

        let visible_rows: Vec<&GroupedAlbumDisplayRow> =
            display_rows.iter().skip(offset).take(visible).collect();
        for (row_idx, row) in visible_rows.iter().enumerate() {
            let row_area = Rect {
                x: area.x,
                y: area.y + row_idx as u16,
                width: area.width,
                height: 1,
            };
            let abs_row_idx = offset + row_idx;
            match row {
                GroupedAlbumDisplayRow::ArtistHeader(selection) => {
                    let selected = selectable_headers
                        && self.libs[lib_idx]
                            .artist_header_focus
                            .as_ref()
                            .is_some_and(|focused| focused == selection);
                    let in_selected_block = selected_block_bounds
                        .is_some_and(|(top, bottom)| abs_row_idx > top && abs_row_idx < bottom);
                    let grouped_block = selectable_headers && in_selected_block;
                    let label_area = if in_selected_block {
                        Rect {
                            width: row_area.width.saturating_sub(selected_art_reserved_w),
                            ..row_area
                        }
                    } else {
                        row_area
                    };
                    let gutter_w = if grouped_block { 2 } else { 1 };
                    let label_avail = (label_area.width as usize).saturating_sub(gutter_w);
                    let artist_label = trunc_str(&selection.artist_label, label_avail);
                    let label_style = if selected && grouped_block {
                        Style::default()
                            .fg(palette::FOAM)
                            .add_modifier(Modifier::BOLD)
                    } else if selected && focused {
                        Style::default()
                            .fg(palette::YELLOW)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(palette::YELLOW)
                    };
                    let mut spans = Vec::with_capacity(3);
                    if grouped_block {
                        spans.push(super::selection_marker(selected));
                        spans.push(Span::raw(" "));
                    } else {
                        spans.push(Span::raw(" "));
                    }
                    spans.push(Span::styled(artist_label, label_style));
                    f.render_widget(Paragraph::new(Line::from(spans)), label_area);
                }
                GroupedAlbumDisplayRow::ArtistGroupSpacer => {}
                GroupedAlbumDisplayRow::AlbumDetailRule => {
                    // Padding rows for the colored block; the background is painted separately.
                    // This row renders as empty, letting the background block show through.
                }
                GroupedAlbumDisplayRow::AlbumWrappedContinuation => {}
                GroupedAlbumDisplayRow::Album(idx) => {
                    let selected = *idx == cursor && !header_selected;
                    let (_, year_str, album_name) = &album_info[*idx];
                    let suffix_w = if year_str.is_empty() {
                        0
                    } else {
                        year_str.chars().count() + 3
                    };
                    let lead_w = if selected { 2 } else { 1 };
                    let name_w = avail.saturating_sub(lead_w + suffix_w);
                    let trunc_name = trunc_str(album_name, name_w);
                    let in_selected_block = selected_block_bounds
                        .is_some_and(|(top, bottom)| abs_row_idx > top && abs_row_idx < bottom);
                    let grouped_block = selectable_headers && in_selected_block;
                    if grouped_block {
                        let content_width = row_area
                            .width
                            .saturating_sub(selected_art_reserved_w)
                            .saturating_sub(2);
                        let suffix = if year_str.is_empty() {
                            String::new()
                        } else {
                            format!(" • {year_str}")
                        };
                        let suffix_width = suffix.chars().count() as u16;
                        let title_width = content_width.saturating_sub(suffix_width).max(1);
                        let wrapped = wrap(album_name, title_width as usize);
                        let wrapped_len = wrapped.len();
                        let title_lines: Vec<Line> = wrapped
                            .into_iter()
                            .enumerate()
                            .map(|(line_idx, line)| {
                                let mut spans = if line_idx == 0 {
                                    vec![super::selection_marker(selected), Span::raw(" ")]
                                } else {
                                    vec![Span::raw("  ")]
                                };
                                let title_style = if selected {
                                    Style::default()
                                        .fg(palette::WHITE)
                                        .add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(palette::WHITE)
                                };
                                spans.push(Span::styled(line.into_owned(), title_style));
                                if line_idx + 1 == wrapped_len && !suffix.is_empty() {
                                    spans.push(Span::styled(
                                        " • ",
                                        Style::default().fg(palette::YELLOW),
                                    ));
                                    spans.push(Span::styled(
                                        year_str.as_str(),
                                        Style::default().fg(palette::AQUA),
                                    ));
                                }
                                Line::from(spans)
                            })
                            .collect();
                        f.render_widget(
                            Paragraph::new(title_lines.clone()),
                            Rect {
                                width: row_area.width.saturating_sub(selected_art_reserved_w),
                                height: title_lines.len() as u16,
                                ..row_area
                            },
                        );
                        continue;
                    }
                    // Detect if this album is inside a colored block frame
                    // Check the absolute row index (not the display cursor) to see if it's
                    // the first content row after the top border of the block
                    let has_block = selected
                        && selected_block_bounds
                            .is_some_and(|(top_pad_abs, _)| abs_row_idx == top_pad_abs + 1);

                    if has_block {
                        let content_width = row_area
                            .width
                            .saturating_sub(selected_art_reserved_w)
                            .saturating_sub(1);
                        let suffix = if year_str.is_empty() {
                            String::new()
                        } else {
                            format!(" • {year_str}")
                        };
                        let suffix_width = suffix.chars().count();
                        let title_lines: Vec<Line> = wrap(
                            album_name,
                            content_width.saturating_sub(suffix_width as u16).max(1) as usize,
                        )
                        .into_iter()
                        .enumerate()
                        .map(|(line_idx, line)| {
                            let mut spans = vec![
                                Span::raw(" "),
                                Span::styled(
                                    line.into_owned(),
                                    Style::default()
                                        .fg(palette::WHITE)
                                        .add_modifier(Modifier::BOLD),
                                ),
                            ];
                            if line_idx + 1
                                == wrap(
                                    album_name,
                                    content_width.saturating_sub(suffix_width as u16).max(1)
                                        as usize,
                                )
                                .len()
                                && !suffix.is_empty()
                            {
                                spans.push(Span::styled(
                                    " • ",
                                    Style::default().fg(palette::YELLOW),
                                ));
                                spans.push(Span::styled(
                                    year_str.as_str(),
                                    Style::default().fg(palette::AQUA),
                                ));
                            }
                            Line::from(spans)
                        })
                        .collect();
                        f.render_widget(
                            Paragraph::new(title_lines.clone()),
                            Rect {
                                width: row_area.width.saturating_sub(selected_art_reserved_w),
                                height: title_lines.len() as u16,
                                ..row_area
                            },
                        );
                        continue;
                    }

                    let mut spans: Vec<Span> = Vec::new();
                    if has_block {
                        // Movie-style: 1-col leading pad, no ▌ marker
                        spans.push(Span::raw(" "));
                    } else if selected {
                        // Legacy style: ▌ AQUA marker
                        spans.push(super::selection_marker(true));
                    } else {
                        // Unselected: plain space
                        spans.push(Span::raw(" "));
                    }

                    if !has_block && selected {
                        spans.push(Span::raw(" "));
                    }

                    let title_style = if selected && focused {
                        Style::default()
                            .fg(palette::WHITE)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(palette::WHITE)
                    };
                    spans.push(Span::styled(trunc_name, title_style));
                    if !year_str.is_empty() {
                        spans.push(Span::styled(" • ", Style::default().fg(palette::YELLOW)));
                        spans.push(Span::styled(
                            year_str.as_str(),
                            Style::default().fg(palette::AQUA),
                        ));
                    }

                    let album_area = row_area;
                    f.render_widget(Paragraph::new(Line::from(spans)), album_area);
                }
                GroupedAlbumDisplayRow::AlbumActionHint => {
                    let in_selected_block = selected_block_bounds
                        .is_some_and(|(top, bottom)| abs_row_idx > top && abs_row_idx < bottom);
                    let hint = if selectable_headers
                        && in_selected_block
                        && self.libs[lib_idx].album_track_focus.is_some()
                    {
                        "^P: Play | ^A: Enqueue | ^S: Shuffle | BACK: Exit"
                    } else {
                        "^P: Play | ^A: Enqueue | ^S: Shuffle | ENTER: Show tracks"
                    };
                    let gutter_w = if selectable_headers && in_selected_block {
                        2
                    } else {
                        1
                    };
                    let hint_width = row_area
                        .width
                        .saturating_sub(selected_art_reserved_w)
                        .saturating_sub(gutter_w)
                        .max(1) as usize;
                    let hint_lines: Vec<Line> = wrap(hint, hint_width)
                        .into_iter()
                        .map(|line| {
                            Line::from(vec![
                                Span::raw(" ".repeat(gutter_w as usize)),
                                Span::styled(
                                    line.into_owned(),
                                    Style::default().fg(palette::SOFT_WHITE),
                                ),
                            ])
                        })
                        .collect();
                    f.render_widget(
                        Paragraph::new(hint_lines.clone()),
                        Rect {
                            width: row_area.width.saturating_sub(selected_art_reserved_w),
                            height: hint_lines.len() as u16,
                            ..row_area
                        },
                    );
                }
                GroupedAlbumDisplayRow::ArtistActionHint => {
                    let in_selected_block = selected_block_bounds
                        .is_some_and(|(top, bottom)| abs_row_idx > top && abs_row_idx < bottom);
                    let gutter_w = if selectable_headers && in_selected_block {
                        2
                    } else {
                        1
                    };
                    let hint_w = row_area
                        .width
                        .saturating_sub(selected_art_reserved_w)
                        .saturating_sub(gutter_w) as usize;
                    let hint = trunc_str("^P: Play | ^A: Enqueue | ^S: Shuffle", hint_w);
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::raw(" ".repeat(gutter_w as usize)),
                            Span::styled(
                                hint.to_string(),
                                Style::default().fg(palette::SOFT_WHITE),
                            ),
                        ])),
                        Rect {
                            width: row_area.width.saturating_sub(selected_art_reserved_w),
                            ..row_area
                        },
                    );
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
                        let track_area = Rect {
                            x: row_area.x + 6,
                            y: row_area.y,
                            width: row_area
                                .width
                                .saturating_sub(10)
                                .saturating_sub(selected_art_reserved_w),
                            height,
                        };
                        self.render_power_album_detail(
                            f,
                            track_area,
                            &tracks,
                            cursor,
                            detail_focused,
                            false, // show_title: Album(idx) row above already shows it
                            false,
                            true,
                            false, // show_hint: AlbumActionHint row at top already shows it
                            0,     // art_reserved_w: already accounted for in track_area
                            layout,
                        );
                    }
                }
                GroupedAlbumDisplayRow::AlbumLoading => {
                    let loading = "Loading…";
                    let loading_width = row_area
                        .width
                        .saturating_sub(selected_art_reserved_w)
                        .saturating_sub(2)
                        .max(1) as usize;
                    let loading_lines: Vec<Line> = wrap(loading, loading_width)
                        .into_iter()
                        .map(|line| {
                            Line::from(vec![
                                super::selection_marker(true),
                                Span::raw(" "),
                                Span::styled(
                                    line.into_owned(),
                                    Style::default().fg(palette::MUTED),
                                ),
                            ])
                        })
                        .collect();
                    f.render_widget(
                        Paragraph::new(loading_lines.clone()),
                        Rect {
                            width: row_area.width.saturating_sub(selected_art_reserved_w),
                            height: loading_lines.len() as u16,
                            ..row_area
                        },
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
                | GroupedAlbumDisplayRow::ArtistGroupSpacer
                | GroupedAlbumDisplayRow::AlbumDetailRule
                | GroupedAlbumDisplayRow::AlbumWrappedContinuation => None,
                GroupedAlbumDisplayRow::Album(idx) => Some(*idx),
                GroupedAlbumDisplayRow::AlbumActionHint
                | GroupedAlbumDisplayRow::ArtistActionHint
                | GroupedAlbumDisplayRow::AlbumDetailStart(_)
                | GroupedAlbumDisplayRow::AlbumDetailContinuation
                | GroupedAlbumDisplayRow::AlbumLoading => None,
            })
            .collect();
        layout.left_row_targets = display_rows
            .iter()
            .skip(offset)
            .take(visible)
            .map(|dr| dr.row_target(selectable_headers))
            .collect();

        let display_n = display_rows.len();
        if focused && display_n > visible {
            let max_off = display_n.saturating_sub(visible);
            super::render_power_right_scrollbar(f, area, max_off, offset);
        }

        if let Some((art_top, art_bottom)) = selected_art_abs_rows {
            if art_top >= offset && art_top < offset + visible {
                let visible_bottom = art_bottom.min(offset + visible);
                let art_rect = Rect {
                    x: area.x,
                    y: area.y + (art_top - offset) as u16,
                    width: area.width,
                    height: (visible_bottom - art_top) as u16,
                };
                if let Some(selection) = &selected {
                    // Collage: the selected artist header's albums, in the
                    // already-sorted `left_sorted_indices` order, first 4.
                    let header_albums: Vec<mbv_core::api::MediaItem> = layout
                        .left_sorted_indices
                        .iter()
                        .filter(|&&idx| album_info[idx].0 == selection.artist_label)
                        .filter_map(|&idx| albums.get(idx).cloned())
                        .collect();
                    self.render_inline_artist_collage(f, art_rect, &header_albums, layout);
                } else if let Some(album) = albums.get(cursor) {
                    self.render_inline_album_art(f, art_rect, album, layout);
                }
            }
        }

        // Paint the ▁/▔ border rows around the colored block (after content/scrollbar)
        if let Some((top_pad_abs, bottom_pad_abs)) = selected_block_bounds {
            super::render_selected_block_borders(
                f,
                area,
                offset,
                visible,
                top_pad_abs,
                bottom_pad_abs,
            );
        }

        offset
    }
}
