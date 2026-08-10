use super::album_art::INLINE_ALBUM_ART_RESERVED;
use super::album_plan::HeaderFocusCtx;
use super::album_rows::AlbumRowCtx;
use super::widgets::COLUMN_GAP;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App, PanelFocus, TWO_COLUMN_THRESHOLD};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

/// Height of the music-group pills row inside the right rail.
const PILLS_ROW_HEIGHT: u16 = 1;
/// Blank rows below the pills before the album list starts.
const PILLS_GAP_ROWS: u16 = 1;
/// Padding inside each wide music pane, matching the Home overview block.
const PANE_PAD_X: u16 = 2;
const PANE_PAD_Y: u16 = 1;

/// Tracks the vertical sub-areas of the wide Music left pane so the
/// hero and track regions can be allocated independently.
struct WideLeftLayout {
    /// Title + metadata + artwork region at the top of the left pane.
    hero_area: Rect,
    /// Track list region below the hero.
    track_area: Rect,
    /// Artwork sub-rect aligned to the top and right edge of the pane. It
    /// intentionally reaches into the pane's outer padding so the cover has
    /// no blank row above it and its rendered edge aligns with the track block.
    art_area: Rect,
    /// Text sub-rect inside `hero_area` (left of artwork).
    text_area: Rect,
}

/// Computes the vertical split of the wide Music left pane between the
/// hero (title, metadata, artwork) and the persistent track list.
///
/// The hero is sized to the artwork (or its compact metadata when images are
/// disabled), so the track block starts directly below the visible hero.
/// The track region reserves its own padding, label, and at least one visible
/// row when tracks exist.
fn compute_wide_left_layout(
    left_area: Rect,
    images_enabled: bool,
    track_count: usize,
) -> WideLeftLayout {
    let total_h = left_area.height;
    let art_available = images_enabled && left_area.width >= INLINE_ALBUM_ART_RESERVED;
    // Reserve a separator row between hero and tracks.
    let sep: u16 = if total_h > 6 { 1 } else { 0 };

    // The track block is exactly the loaded track rows plus one padding row at
    // each edge. Loading and empty states still need one content row.
    let track_rows = track_count.max(1) as u16;
    let requested_track_h = track_rows.saturating_add(PANE_PAD_Y * 2);

    // Keep the hero no taller than the artwork. This keeps the track block
    // directly below the visible cover instead of leaving an empty tail under
    // the artwork just because the pane happens to be tall.
    let hero_ideal = if art_available {
        super::album_art::INLINE_ALBUM_ART_ROWS
    } else {
        2
    }
    .min(total_h.saturating_sub(sep + 2));
    let track_h = requested_track_h.min(total_h.saturating_sub(hero_ideal + sep));
    let hero_h = hero_ideal.min(total_h.saturating_sub(track_h + sep));

    let hero_area = Rect {
        x: left_area.x,
        y: left_area.y,
        width: left_area.width,
        height: hero_h,
    };
    let track_area = Rect {
        x: left_area.x,
        y: left_area.y + hero_h + sep,
        width: left_area.width,
        height: track_h,
    };

    let art_area = if art_available && hero_area.width >= INLINE_ALBUM_ART_RESERVED {
        Rect {
            x: hero_area.x.saturating_add(
                hero_area
                    .width
                    .saturating_sub(INLINE_ALBUM_ART_RESERVED)
                    .saturating_add(PANE_PAD_X),
            ),
            y: hero_area.y.saturating_sub(PANE_PAD_Y),
            width: INLINE_ALBUM_ART_RESERVED,
            height: hero_area.height.saturating_add(PANE_PAD_Y),
        }
    } else {
        Rect::default()
    };
    let text_area = Rect {
        width: hero_area.width.saturating_sub(art_area.width),
        ..hero_area
    };

    WideLeftLayout {
        hero_area,
        track_area,
        art_area,
        text_area,
    }
}

/// Returns `(left_pane, right_pane)` for the wide Music horizontal split.
/// Uses the Home-style 40/60 ratio with a two-column gap between panes.
fn wide_music_panes(content_area: Rect) -> (Rect, Rect) {
    let left_w = ((content_area.width as u32 * 2 / 5) as u16)
        .max(40)
        .min(content_area.width.saturating_sub(40));
    let right_w = content_area
        .width
        .saturating_sub(left_w)
        .saturating_sub(COLUMN_GAP);
    (
        Rect {
            x: content_area.x,
            y: content_area.y,
            width: left_w,
            height: content_area.height,
        },
        Rect {
            x: content_area.x + left_w + COLUMN_GAP,
            y: content_area.y,
            width: right_w,
            height: content_area.height,
        },
    )
}

fn inset_pane(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(PANE_PAD_X),
        y: area.y.saturating_add(PANE_PAD_Y),
        width: area.width.saturating_sub(PANE_PAD_X * 2),
        height: area.height.saturating_sub(PANE_PAD_Y * 2),
    }
}

impl App {
    /// Renders the wide grouped Music layout: a left pane with album hero
    /// and persistent tracks, and a right pane with music-group pills and
    /// a one-column album browser.
    ///
    /// Tasks 1.3, 2.1-2.4, 3.1-3.4, 4.1-4.2, 5.1.
    pub(super) fn render_wide_music_group(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        layout.wide_music_track_hitmap.clear();
        layout.wide_music_art_area = Rect::default();

        if area.width < TWO_COLUMN_THRESHOLD || area.height < 6 {
            // Too narrow for wide mode — fall back to narrow rendering.
            self.render_list(f, area, focused, layout);
            return;
        }

        let Some(album) = self.selected_album_item(lib_idx) else {
            super::render_placeholder(f, area, " (no album selected)");
            return;
        };

        let (left_area, right_area) = wide_music_panes(area);
        let left_panel = left_area;
        let right_panel = right_area;
        let left_area = inset_pane(left_panel);
        let right_area = inset_pane(right_panel);
        layout.wide_music_right_area = right_area;

        // ── Focus state (Task 4.1) ──────────────────────────────────────
        // Derive internal pane focus from outer PanelFocus and
        // album_track_focus without adding persisted focus state.
        let library_focused = matches!(self.panel_focus, PanelFocus::Library);
        let track_active = self.libs[lib_idx].album_track_focus.is_some();
        // In wide mode: track_active → left focused; otherwise → right focused.
        let left_focused = library_focused && track_active;
        let right_focused = library_focused && !track_active;

        // ── Fetch and cache tracks ──────────────────────────────────────
        let tracks = self.album_tracks_cache.get(&album.id).cloned();
        if tracks.is_none() && !self.album_tracks_loading.contains(&album.id) {
            self.fetch_album_tracks(album.id.clone());
        }
        let track_count = tracks.as_ref().map_or(0, Vec::len);

        // ── Left pane: hero + tracks ────────────────────────────────────
        let left_layout = compute_wide_left_layout(left_area, self.images_enabled(), track_count);
        layout.left_area = left_area;
        layout.hero_area = left_layout.hero_area;

        // Pane background (Task 4.2: reciprocal palette).
        let left_bg = if left_focused {
            palette::BG_GREEN
        } else {
            palette::LIBRARY_SIDE_BG
        };
        f.render_widget(
            Block::default().style(Style::default().bg(left_bg)),
            left_panel,
        );

        self.render_wide_left_hero(
            f,
            &left_layout,
            &album,
            left_focused,
            library_focused,
            layout,
        );
        layout.wide_music_art_area = left_layout.art_area;

        self.render_wide_left_tracks(
            f,
            &left_layout.track_area,
            &album,
            lib_idx,
            left_focused,
            library_focused,
            layout,
        );

        // ── Right pane: pills + album browser ───────────────────────────
        // Keep the pill bar on the library-side surface. The focused green
        // surface belongs only to the album list below it.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::LIBRARY_SIDE_BG)),
            right_panel,
        );

        // Pills at the top of the right rail (Task 3.1).
        let pills_area = Rect {
            x: right_area.x,
            y: right_panel.y,
            width: right_area.width,
            height: PILLS_ROW_HEIGHT,
        };
        if pills_area.y + pills_area.height <= right_area.bottom() {
            self.render_music_group_pills_row(f, pills_area, lib_idx, layout);
        }

        // Album browser below the pills (Tasks 3.2-3.4). The list panel uses
        // the same one-row padding and upper/lower three-quarter borders as
        // Home's wide list panel; the browser itself is inset inside it.
        let browser_y = right_panel.y + PILLS_ROW_HEIGHT + PILLS_GAP_ROWS;
        let list_panel = Rect {
            x: right_area.x,
            y: browser_y,
            width: right_area.width,
            height: right_panel
                .height
                .saturating_sub(PILLS_ROW_HEIGHT + PILLS_GAP_ROWS + PANE_PAD_Y),
        };
        let browser_area = Rect {
            x: list_panel.x.saturating_add(PANE_PAD_X),
            y: list_panel.y.saturating_add(PANE_PAD_Y),
            width: list_panel.width.saturating_sub(PANE_PAD_X * 2),
            height: list_panel.height.saturating_sub(PANE_PAD_Y * 2 + 1),
        };
        if list_panel.height > 0 {
            let list_bg = if right_focused {
                palette::BG_GREEN
            } else {
                palette::PLAYBACK_PANEL_BG
            };
            f.render_widget(
                Block::default().style(Style::default().bg(list_bg)),
                list_panel,
            );
        }
        if browser_area.height > 0 && browser_area.width > 0 {
            self.render_wide_right_album_browser(f, browser_area, lib_idx, right_focused, layout);
        }
        if list_panel.height > 0 {
            let border_style = Style::default().fg(palette::SEEK_TRACK);
            for (y, glyph) in [
                (list_panel.y, '\u{2594}'),
                (list_panel.bottom().saturating_sub(1), '\u{2581}'),
            ] {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        glyph.to_string().repeat(list_panel.width as usize),
                        border_style,
                    ))),
                    Rect {
                        y,
                        height: 1,
                        ..list_panel
                    },
                );
            }
        }
    }

    /// Renders the wide left pane's hero: album title, metadata, and
    /// large centered artwork. Task 2.1.
    fn render_wide_left_hero(
        &mut self,
        f: &mut Frame,
        left_layout: &WideLeftLayout,
        album: &mbv_core::api::EmbyItem,
        left_focused: bool,
        library_focused: bool,
        layout: &mut LayoutMain,
    ) {
        let text = &left_layout.text_area;
        if text.height > 0 && text.width >= 3 {
            // ── Title ──
            let title = album.display_name();
            let title_trunc =
                super::super::ui_util::trunc_str(&title, (text.width as usize).saturating_sub(1));
            let title_style = if left_focused || library_focused {
                Style::default()
                    .fg(palette::YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::YELLOW)
            };
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" {title_trunc}"),
                    title_style,
                ))),
                Rect {
                    x: text.x,
                    y: text.y,
                    width: text.width,
                    height: 1,
                },
            );

            // ── Metadata line (year, artist) ──
            if text.height > 1 {
                let mut meta_parts = Vec::new();
                if album.production_year > 0 {
                    meta_parts.push(album.production_year.to_string());
                }
                // Artist from the grouping catalog or fallback.
                let artist = self.resolve_group_album_artist(album);
                if !artist.is_empty() && artist != "Unknown Artist" {
                    meta_parts.push(artist);
                }
                let meta_text = meta_parts.join(" \u{2022} ");
                let meta_trunc = super::super::ui_util::trunc_str(
                    &meta_text,
                    (text.width as usize).saturating_sub(1),
                );
                let meta_fg = if library_focused {
                    palette::SUBTLE
                } else {
                    palette::MUTED
                };
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        format!(" {meta_trunc}"),
                        Style::default().fg(meta_fg),
                    ))),
                    Rect {
                        x: text.x,
                        y: text.y + 1,
                        width: text.width,
                        height: 1,
                    },
                );
            }
        }

        // ── Artwork ──
        if left_layout.art_area.width > 0 && left_layout.art_area.height > 0 {
            self.render_inline_album_art(f, left_layout.art_area, album, layout);
        }
    }

    /// Renders the persistent track list in the wide left pane's lower
    /// region. Shows tracks regardless of `album_track_focus` (Task 2.3).
    /// Keeps focused track visible (Task 2.4). Records hit targets (Task 5.1).
    fn render_wide_left_tracks(
        &mut self,
        f: &mut Frame,
        track_area: &Rect,
        album: &mbv_core::api::EmbyItem,
        lib_idx: usize,
        left_focused: bool,
        library_focused: bool,
        layout: &mut LayoutMain,
    ) {
        if track_area.height == 0 {
            return;
        }

        // Match the recessed overview block used on Home: one row of vertical
        // padding and two cells of horizontal padding around the track list.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::PLAYBACK_PANEL_BG)),
            *track_area,
        );
        let track_content = Rect {
            x: track_area.x.saturating_add(PANE_PAD_X),
            y: track_area.y.saturating_add(PANE_PAD_Y),
            width: track_area.width.saturating_sub(PANE_PAD_X * 2),
            height: track_area.height.saturating_sub(PANE_PAD_Y * 2),
        };
        if track_content.height == 0 || track_content.width == 0 {
            return;
        }

        let list_area = track_content;
        if list_area.height == 0 {
            return;
        }

        match self.album_tracks_cache.get(&album.id).cloned() {
            None => {
                // Loading state (Task 2.3).
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "Loading\u{2026}",
                        Style::default().fg(palette::MUTED),
                    ))),
                    list_area,
                );
            }
            Some(tracks) if tracks.is_empty() => {
                // Empty state.
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "(no tracks)",
                        Style::default().fg(palette::MUTED),
                    ))),
                    list_area,
                );
            }
            Some(tracks) => {
                // Track list (Tasks 2.3, 2.4, 5.1).
                let n = tracks.len();
                let visible = list_area.height as usize;
                // Task 2.4: preview starts from beginning (offset 0),
                // focused mode keeps the selected track visible.
                let track_cursor = self.libs[lib_idx].album_track_focus;
                let scroll = if let Some(cursor) = track_cursor {
                    // Keep cursor visible.
                    let max_scroll = n.saturating_sub(visible);
                    let want = cursor.saturating_sub(visible.saturating_sub(1));
                    want.min(max_scroll)
                } else {
                    // Preview mode: start from the beginning.
                    0
                };

                let gutter_w: usize = 1;
                let title_col_w = (list_area.width as usize).saturating_sub(gutter_w + 2 + 8); // 8 for duration

                layout.wide_music_track_hitmap.clear();

                for vi in 0..visible {
                    let ti = scroll + vi;
                    if ti >= n {
                        break;
                    }
                    let track = &tracks[ti];
                    let row_y = list_area.y + vi as u16;
                    let row_rect = Rect {
                        x: list_area.x,
                        y: row_y,
                        width: list_area.width,
                        height: 1,
                    };

                    let is_cursor = Some(ti) == track_cursor;
                    let selected = is_cursor && left_focused;

                    let text_fg = if selected {
                        palette::YELLOW
                    } else if left_focused {
                        palette::WHITE
                    } else {
                        palette::SOFT_WHITE
                    };

                    // Focused-track cursor highlight (Task 4.2).
                    if is_cursor && left_focused {
                        f.render_widget(
                            Block::default().style(Style::default().bg(palette::BG_GREEN)),
                            row_rect,
                        );
                    }

                    let marker = super::selection_marker(selected);
                    let track_num = if track.index_number > 0 {
                        format!("{:>2}. ", track.index_number)
                    } else {
                        format!("{:>2}. ", ti + 1)
                    };
                    let name_w = title_col_w.saturating_sub(track_num.chars().count());
                    let name = super::super::ui_util::trunc_str(&track.name, name_w);
                    let duration = if track.runtime_ticks > 0 {
                        super::super::ui_util::fmt_duration_mmss(
                            track.runtime_ticks / mbv_core::api::TICKS_PER_SECOND,
                        )
                    } else {
                        "\u{2014}".to_string()
                    };

                    let mut spans = vec![marker, Span::raw(" ")];
                    spans.push(Span::styled(track_num, Style::default().fg(text_fg)));
                    spans.push(Span::styled(name.to_string(), Style::default().fg(text_fg)));
                    // Duration right-aligned.
                    let used: usize = spans.iter().map(|s| s.content.len()).sum::<usize>() + 1; // +1 for the marker space
                    let pad = (list_area.width as usize).saturating_sub(used + duration.len() + 1);
                    if pad > 0 {
                        spans.push(Span::raw(" ".repeat(pad)));
                    }
                    spans.push(Span::styled(
                        format!(" {duration}"),
                        Style::default().fg(if selected {
                            palette::YELLOW
                        } else {
                            palette::MUTED
                        }),
                    ));

                    f.render_widget(Paragraph::new(Line::from(spans)), row_rect);

                    // Record hit target for this track (Task 5.1).
                    layout.wide_music_track_hitmap.push((row_rect, ti));
                }

                // Scrollbar if needed.
                if n > visible && library_focused {
                    let max_offset = n.saturating_sub(visible);
                    super::render_right_scrollbar(f, list_area, max_offset, scroll);
                }

                // Update cursor_screen_y for the focused track.
                if let Some(cursor) = track_cursor {
                    if cursor >= scroll && cursor < scroll + visible {
                        layout.cursor_screen_y = Some(list_area.y + (cursor - scroll) as u16);
                    }
                }
            }
        }
    }

    /// Renders the wide right pane's album browser: a one-column
    /// artist-grouped album list. Tasks 3.2-3.4.
    fn render_wide_right_album_browser(
        &mut self,
        f: &mut Frame,
        browser_area: Rect,
        lib_idx: usize,
        right_focused: bool,
        layout: &mut LayoutMain,
    ) {
        let Some(level) = self.libs[lib_idx].nav_stack.last() else {
            return;
        };
        let albums = level.items.clone();
        let cursor = level.cursor;

        if albums.is_empty() {
            let msg = if level.loading {
                " Loading\u{2026}"
            } else {
                " (empty)"
            };
            super::render_placeholder(f, browser_area, msg);
            return;
        }

        let (album_info, order) = {
            let catalog = self.libs[lib_idx]
                .nav_stack
                .last()
                .and_then(|l| l.music_grouping.as_ref())
                .and_then(|s| s.settled.as_ref());
            match catalog {
                Some(cat) => {
                    let info = self.group_album_info(&albums, Some(cat));
                    let order: Vec<usize> = cat
                        .entries
                        .iter()
                        .map(|e| e.album_index)
                        .filter(|&i| i < albums.len())
                        .collect();
                    (info, order)
                }
                None => {
                    let info = self.group_album_info(&albums, None);
                    let order = super::sorted_group_album_order(&info);
                    (info, order)
                }
            }
        };

        // Task 3.3: One column only — no two-column packing.
        // No selectable headers in wide right rail (design Decision 5).
        let plan = self.build_grouped_album_display_plan(
            &albums,
            &album_info,
            &order,
            cursor,
            true,
            HeaderFocusCtx {
                selectable_headers: false,
                selected_artist_header: None,
                expand_selected: false,
            },
            None, // No wrap_widths needed for one-column.
            true,
        );

        // Scroll to keep the selected album visible.
        let display_cursor = plan.display_cursor;
        let total_rows = plan.rows.len();
        let visible = browser_area.height as usize;
        let stored_scroll = self.libs[lib_idx]
            .nav_stack
            .last()
            .map(|l| l.scroll)
            .unwrap_or(0);
        let max_offset = total_rows.saturating_sub(visible);
        let mut offset = stored_scroll.min(max_offset);
        if display_cursor < offset {
            offset = display_cursor;
        } else if display_cursor >= offset + visible {
            offset = display_cursor
                .saturating_add(1)
                .saturating_sub(visible)
                .min(max_offset);
        }

        // Update scroll.
        if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
            level.scroll = offset;
        }

        let avail = (browser_area.width as usize).saturating_sub(2);
        let visible_rows: Vec<_> = plan
            .rows
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible)
            .collect();

        for (row_idx, row) in &visible_rows {
            let screen_y = (*row_idx - offset) as u16;
            let row_area = Rect {
                x: browser_area.x,
                y: browser_area.y + screen_y,
                width: browser_area.width,
                height: 1,
            };

            match row {
                super::album_plan::GroupedAlbumDisplayRow::ArtistHeader(header) => {
                    // Wide right rail: no selectable headers (design Decision 5).
                    self.render_artist_header_row(
                        f,
                        row_area,
                        header,
                        false, // selectable_headers: not in wide right rail
                        None,  // No selected block in wide right rail.
                        *row_idx,
                        0, // No art reservation in right rail.
                        right_focused,
                        lib_idx,
                    );
                }
                super::album_plan::GroupedAlbumDisplayRow::ArtistGroupSpacer => {}
                super::album_plan::GroupedAlbumDisplayRow::Album(idx) => {
                    let selected = *idx == cursor;
                    if selected {
                        layout.cursor_screen_y = Some(browser_area.y + screen_y);
                    }
                    self.render_album_row(
                        f,
                        AlbumRowCtx {
                            row_area,
                            idx: *idx,
                            album_info: &album_info,
                            cursor,
                            header_selected: false,
                            avail,
                            selected_block_bounds: None,
                            selectable_headers: false,
                            abs_row_idx: *row_idx,
                            selected_art_reserved_w: 0,
                            focused: right_focused,
                        },
                    );
                }
                _ => {}
            }
        }

        // Scrollbar.
        if total_rows > visible && right_focused {
            let max_off = total_rows.saturating_sub(visible);
            super::render_right_scrollbar(f, browser_area, max_off, offset);
        }

        // Update left_sorted_indices for cursor navigation.
        layout.left_sorted_indices = plan.order;
    }
}
