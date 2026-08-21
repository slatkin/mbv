//! Grouped Music's wide hero-on-left component.

use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::hero_left::{self, WrappedHeroLine};
use crate::app::render::arrangements::library as library_arrangement;
use crate::app::render::arrangements::music::{
    self as music_arrangement, WideMusicLeftLayout, PANE_PAD_X, PANE_PAD_Y,
};
use crate::app::{palette, App, PanelFocus};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

/// Reserved width for the right-aligned track duration column plus its
/// leading space (`fmt_duration_mmss` output is unbounded but rarely
/// exceeds this).
const DURATION_COL_W: usize = 8;

fn wide_album_metadata(album: &mbv_core::api::EmbyItem, artist: &str) -> (String, u32) {
    let display_name = album.display_name();
    if let Some((parsed_artist, parsed_year, title)) =
        crate::app::render::parse_album_folder_name(&display_name)
    {
        let year_matches = album.production_year == 0 || album.production_year == parsed_year;
        if parsed_artist == artist && year_matches {
            return (title, album.production_year.max(parsed_year));
        }
    }

    let prefix = if album.production_year > 0 {
        format!("{artist} ({}) ", album.production_year)
    } else {
        format!("{artist} ")
    };
    let title = display_name
        .strip_prefix(&prefix)
        .unwrap_or(&display_name)
        .to_string();
    (title, album.production_year)
}

impl App {
    /// Renders the wide grouped Music layout: a left pane with album hero
    /// and persistent tracks, and a right pane with music-group pills and
    /// a one-column album browser.
    pub(in crate::app::render) fn render_wide_music_group(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        layout.wide_music_track_hitmap.clear();
        layout.wide_music_art_area = Rect::default();

        let Some(panes) = library_arrangement::wide_library_panes(area, 0, PANE_PAD_Y) else {
            // Too narrow for wide mode — fall back to narrow rendering.
            self.render_list(f, area, focused, layout);
            return;
        };
        let left_panel = panes.left_panel;
        let right_panel = panes.right_panel;

        // Keep a library-side separator row below the left pane, while the
        // right pane remains flush with the status bar below the library area.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
            Rect {
                x: left_panel.x,
                y: left_panel.bottom(),
                width: left_panel.width,
                height: 1,
            },
        );

        let album = self.selected_album_item(lib_idx);

        // The shared library content area already provides the outer
        // horizontal gutter. Only retain vertical breathing room here so the
        // wide panes do not acquire a second two-column frame.
        let left_area = panes.left_area;
        let right_area = panes.right_area;
        layout.wide_music_right_area = right_area;

        // ── Focus state ──────────────────────────────────────────────────
        // Derive internal pane focus from outer PanelFocus and
        // album_track_focus without adding persisted focus state.
        let library_focused = matches!(self.effective_panel_focus(), PanelFocus::Library);
        let track_active = self.libs[lib_idx].album_track_focus.is_some();
        // In wide mode: track_active → left focused; otherwise → right focused.
        let left_focused = library_focused && track_active;
        let right_focused = library_focused && !track_active;

        // ── Fetch and cache tracks ──────────────────────────────────────
        let track_count = album
            .as_ref()
            .and_then(|album| self.album_tracks_cache.get(&album.id))
            .map_or(0, Vec::len);
        if let Some(album) = album.as_ref() {
            if !self.album_tracks_cache.contains_key(&album.id)
                && !self.album_tracks_loading.contains(&album.id)
            {
                self.fetch_album_tracks(album.id.clone());
            }
        }

        // ── Left pane: hero + tracks ────────────────────────────────────
        let left_layout = music_arrangement::wide_music_left_layout(
            left_area,
            album.is_some() && self.images_enabled(),
            track_count,
        );
        layout.left_area = left_area;
        layout.hero_area = left_layout.hero_area;

        // Pane background (reciprocal palette).
        let left_bg = palette::resolve_surface_focus(left_focused);
        f.render_widget(
            Block::default().style(Style::default().bg(left_bg)),
            left_panel,
        );

        if let Some(album) = album.as_ref() {
            self.render_wide_left_hero(
                f,
                &left_layout,
                album,
                left_focused,
                library_focused,
                layout,
            );
        } else {
            crate::app::render::render_placeholder(f, left_area, " Loading\u{2026}");
        }
        layout.wide_music_art_area = left_layout.art_area;

        if let Some(album) = album.as_ref() {
            self.render_wide_left_tracks(
                f,
                &left_layout.track_area,
                album,
                lib_idx,
                left_focused,
                library_focused,
                layout,
            );
        }

        // ── Right pane: pills + album browser ───────────────────────────
        // Keep the pill bar on the library-side surface. The focused green
        // surface belongs only to the album list below it.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
            right_panel,
        );

        // Pills at the top of the right rail, then the album browser below
        // them (design.md decision 6's "pill row at top of list pane"). The
        // list panel uses the same one-row padding and upper/lower
        // three-quarter borders as Home's wide list panel; the browser
        // itself is inset inside it.
        let right_pane = hero_left::hero_on_left_right_pane(right_panel, right_area, PANE_PAD_Y);
        let pills_area = right_pane.pills_area;
        let search_active = self.libs[lib_idx].search.is_some();

        // The fuzzy search box replaces the pill bar at the top of the right
        // rail while search is active (never pills and filtering at once),
        // occupying the exact one-row pill slot -- the gap row and browser
        // panel below it are already reserved either way.
        if search_active {
            let s = self.libs[lib_idx].search.as_ref().unwrap();
            crate::app::render::components::hero::render_search_box(
                f, pills_area, &s.query, s.loading,
            );
        } else if pills_area.y + pills_area.height <= right_area.bottom() {
            self.render_music_group_pills_row(f, pills_area, lib_idx, layout);
        }

        let list_panel = right_pane.list_panel;
        // `render_wide_right_album_browser` renders text one cell right of
        // `browser_area.x` (the row painters' own leading gutter), so inset
        // one cell less than the panel's other padded content to land text
        // at the panel's standard two-column interior inset.
        let browser_area = music_arrangement::wide_music_browser_area(list_panel);
        if list_panel.height > 0 {
            let list_bg = palette::resolve_surface_focus(right_focused);
            f.render_widget(
                Block::default().style(Style::default().bg(list_bg)),
                list_panel,
            );
        }
        if browser_area.height > 0 && browser_area.width > 0 {
            if search_active {
                // Fuzzy search results, a plain one/multi-column list fed from
                // `lib.search` (same construction as the narrow path in
                // `render_list`), replacing the grouped album browser.
                let s = self.libs[lib_idx].search.as_ref().unwrap();
                let items: Vec<mbv_core::api::EmbyItem> = s
                    .results
                    .iter()
                    .filter_map(|&i| {
                        s.items
                            .get(i)
                            .map(|item| self.recursive_album_display_item(lib_idx, i, item.clone()))
                    })
                    .collect();
                let cols =
                    crate::app::library_column_width::library_column_count(browser_area.width);
                let scroll = self.render_plain_rows(
                    f,
                    crate::app::render::components::list_rows::ListRenderCtx {
                        content_area: browser_area,
                        items: &items,
                        cursor: s.cursor,
                        stored_scroll: s.scroll,
                        cols,
                        focused: right_focused,
                        hero_rows: 0,
                    },
                    layout,
                );
                if let Some(sl) = self.libs[lib_idx].search.as_mut() {
                    sl.scroll = scroll;
                }
            } else {
                self.render_wide_right_album_browser(
                    f,
                    browser_area,
                    list_panel,
                    lib_idx,
                    right_focused,
                    layout,
                );
            }
        }
        hero_left::hero_on_left_list_panel_border(f, list_panel, right_focused);
    }

    /// Renders the wide left pane's hero: album title, metadata, and
    /// large centered artwork.
    fn render_wide_left_hero(
        &mut self,
        f: &mut Frame,
        left_layout: &WideMusicLeftLayout,
        album: &mbv_core::api::EmbyItem,
        left_focused: bool,
        library_focused: bool,
        layout: &mut LayoutMain,
    ) {
        let text = &left_layout.text_area;
        let artist = self.resolve_group_album_artist(album);
        let (title, release_year) = wide_album_metadata(album, &artist);
        let title_style = if left_focused || library_focused {
            Style::default()
                .fg(palette::TEXT_FOCUS_ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::TEXT_FOCUS_ACCENT)
        };
        let show_artist = !artist.is_empty() && artist != "Unknown Artist";
        let year_text = (release_year > 0).then(|| release_year.to_string());
        let mut hero_lines = vec![WrappedHeroLine {
            text: &title,
            style: title_style,
        }];
        if show_artist {
            hero_lines.push(WrappedHeroLine {
                text: &artist,
                style: Style::default().fg(palette::TEXT_METADATA),
            });
        }
        if let Some(year) = year_text.as_deref() {
            hero_lines.push(WrappedHeroLine {
                text: year,
                style: Style::default().fg(palette::TEXT_SECONDARY),
            });
        }
        hero_left::paint_hero_on_left_text(f, *text, &hero_lines);

        // ── Artwork ──
        if left_layout.art_area.width > 0 && left_layout.art_area.height > 0 {
            if left_layout.stack_metadata {
                self.render_inline_album_art_centered(f, left_layout.art_area, album, layout);
            } else {
                self.render_inline_album_art(f, left_layout.art_area, album, layout);
            }
        }
    }

    /// Renders the persistent track list in the wide left pane's lower
    /// region. Shows tracks regardless of `album_track_focus`.
    /// Keeps the focused track visible and records mouse hit targets.
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

        // Standard hero-on-left recessed content block: same pattern as
        // Home's overview block, using the shared `hero_on_left_recessed_box`.
        let (track_panel, track_content) =
            hero_left::hero_on_left_recessed_box(f, *track_area, PANE_PAD_X, PANE_PAD_Y);
        if track_content.height == 0 || track_content.width == 0 {
            return;
        }

        let list_area = track_content;
        if list_area.height == 0 {
            return;
        }

        match self.album_tracks_cache.get(&album.id) {
            None => {
                // Loading state.
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "Loading\u{2026}",
                        Style::default().fg(palette::TEXT_MUTED),
                    ))),
                    list_area,
                );
            }
            Some(tracks) if tracks.is_empty() => {
                // Empty state.
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "(no tracks)",
                        Style::default().fg(palette::TEXT_MUTED),
                    ))),
                    list_area,
                );
            }
            Some(tracks) => {
                // Track list.
                let n = tracks.len();
                let visible = list_area.height as usize;
                // Preview mode starts from the beginning (offset 0);
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

                let title_col_w = (list_area.width as usize).saturating_sub(DURATION_COL_W);

                layout.wide_music_track_hitmap.clear();

                for vi in 0..visible {
                    let ti = scroll + vi;
                    if ti >= n {
                        break;
                    }
                    let track = &tracks[ti];
                    let row_y = list_area.y + vi as u16;
                    let row_rect = Rect {
                        x: track_panel.x,
                        y: row_y,
                        width: track_panel.width,
                        height: 1,
                    };

                    let is_cursor = Some(ti) == track_cursor;
                    let selected = is_cursor && left_focused;

                    let text_fg = if selected {
                        palette::TEXT_FOCUS_ACCENT
                    } else if left_focused {
                        palette::TEXT_STRONG
                    } else {
                        palette::TEXT_EMPHASIS
                    };

                    // Focused-track cursor highlight.
                    if is_cursor && left_focused {
                        f.render_widget(
                            Block::default().style(Style::default().bg(palette::SURFACE_FOCUSED)),
                            row_rect,
                        );
                    }

                    let track_num = if track.index_number > 0 {
                        format!("{:>2}. ", track.index_number)
                    } else {
                        format!("{:>2}. ", ti + 1)
                    };
                    let name_w = title_col_w.saturating_sub(track_num.chars().count());
                    let name = crate::app::ui_util::trunc_str(&track.name, name_w);
                    let duration = if track.runtime_ticks > 0 {
                        crate::app::ui_util::fmt_duration_mmss(
                            track.runtime_ticks / mbv_core::api::TICKS_PER_SECOND,
                        )
                    } else {
                        "\u{2014}".to_string()
                    };

                    let used = track_num.chars().count() + name.chars().count();
                    let mut spans = vec![
                        crate::app::render::selection_marker(
                            selected,
                            crate::app::render::MarkerEdge::Left,
                        ),
                        Span::raw(" "),
                    ];
                    spans.push(Span::styled(track_num, Style::default().fg(text_fg)));
                    spans.push(Span::styled(name, Style::default().fg(text_fg)));
                    // Duration right-aligned.
                    let pad = (list_area.width as usize).saturating_sub(used + duration.len() + 1);
                    if pad > 0 {
                        spans.push(Span::raw(" ".repeat(pad)));
                    }
                    spans.push(Span::styled(
                        format!(" {duration}"),
                        Style::default().fg(palette::STATUS_AVAILABLE),
                    ));

                    f.render_widget(Paragraph::new(Line::from(spans)), row_rect);

                    // Record the hit target for this track.
                    layout.wide_music_track_hitmap.push((row_rect, ti));
                }

                // Scrollbar if needed.
                if n > visible && library_focused {
                    let max_offset = n.saturating_sub(visible);
                    crate::app::render::render_right_scrollbar(
                        f,
                        list_area,
                        max_offset,
                        scroll,
                        palette::SCROLLBAR,
                    );
                }

                // Publish the selected track rect as the keyboard anchor.
                if let Some(cursor) = track_cursor {
                    if cursor >= scroll && cursor < scroll + visible {
                        let cy = list_area.y + (cursor - scroll) as u16;
                        layout.selected_item_rect = Some(Rect {
                            x: list_area.x,
                            y: cy,
                            width: list_area.width,
                            height: 1,
                        });
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::wide_album_metadata;
    use crate::app::tests::make_item;
    #[test]
    fn wide_album_metadata_removes_artist_and_year_prefix() {
        let mut album = make_item("Bob Dylan (1970) New Morning", "MusicAlbum");
        album.artist = "Bob Dylan".into();
        album.production_year = 1970;

        assert_eq!(
            wide_album_metadata(&album, "Bob Dylan"),
            ("New Morning".to_string(), 1970)
        );
    }
}
