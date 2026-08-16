//! Grouped Music's wide (hero-on-left) rendering: `compute_wide_left_layout`
//! below is design.md's hero-on-left geometry source (component catalogue,
//! decision 4), the counterpart to `hero.rs`'s hero-on-top
//! `top_hero_layout`. It stays in this file rather than moving into
//! `hero.rs` because its sizing constants (`PANE_PAD_X`, `PANE_PAD_Y`, ...)
//! are shared with this file's non-hero list-pane layout below; the pane
//! split, right-pane pill/list geometry, and hero text paint moved into
//! `hero.rs` in phase 5 ("Assemble hero-on-left") since those have no
//! remaining dependency on this file's local constants.

use super::album_art::INLINE_ALBUM_ART_RESERVED;
use super::hero::{self, WrappedHeroLine};
use crate::app::layout::LayoutMain;
use crate::app::{palette, App, PanelFocus, TWO_COLUMN_THRESHOLD};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

/// Padding inside recessed wide-music blocks, matching the Home overview block.
const PANE_PAD_X: u16 = 2;
const PANE_PAD_Y: u16 = 1;
/// Minimum left-pane height needed to draw a hero/track separator row.
const MIN_LEFT_HEIGHT_FOR_SEPARATOR: u16 = 6;
/// Minimum width for the hero metadata column to remain beside the artwork.
/// Narrower columns move the metadata below the artwork instead.
const MIN_HERO_METADATA_SIDE_WIDTH: u16 = 15;
/// Reserved width for the right-aligned track duration column plus its
/// leading space (`fmt_duration_mmss` output is unbounded but rarely
/// exceeds this).
const DURATION_COL_W: usize = 8;

fn wide_album_metadata(album: &mbv_core::api::EmbyItem, artist: &str) -> (String, u32) {
    let display_name = album.display_name();
    if let Some((parsed_artist, parsed_year, title)) = super::parse_album_folder_name(&display_name)
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

/// Tracks the vertical sub-areas of the wide Music left pane so the
/// hero and track regions can be allocated independently.
struct WideLeftLayout {
    /// Title + metadata + artwork region at the top of the left pane.
    hero_area: Rect,
    /// Track list region below the hero.
    track_area: Rect,
    /// Artwork sub-rect aligned one row below the pane top and flush with the
    /// track block's right edge.
    art_area: Rect,
    /// Text sub-rect inside `hero_area` (left of artwork).
    text_area: Rect,
    /// Whether the hero uses the narrow stacked artwork/metadata layout.
    stack_metadata: bool,
}

/// Computes the vertical split of the wide Music left pane between the
/// hero (title, metadata, artwork) and the persistent track list.
///
/// The hero is sized to the artwork (or its compact metadata when images are
/// disabled), so the track block starts directly below the visible hero.
/// The track region reserves its own padding and at least one visible content
/// row when tracks exist.
fn compute_wide_left_layout(
    left_area: Rect,
    images_enabled: bool,
    track_count: usize,
) -> WideLeftLayout {
    let total_h = left_area.height;
    // Keep the hero/banner inset against the left pane, like the track block
    // below it, while leaving the pane itself visible around the content.
    let hero_content_area = Rect {
        x: left_area.x.saturating_add(PANE_PAD_X),
        width: left_area.width.saturating_sub(PANE_PAD_X * 2),
        ..left_area
    };
    let art_available = images_enabled && hero_content_area.width >= INLINE_ALBUM_ART_RESERVED;
    let side_metadata_width = hero_content_area
        .width
        .saturating_sub(INLINE_ALBUM_ART_RESERVED);
    let stack_metadata = art_available && side_metadata_width < MIN_HERO_METADATA_SIDE_WIDTH;
    // Reserve a separator row between hero and tracks.
    let sep: u16 = if total_h > MIN_LEFT_HEIGHT_FOR_SEPARATOR {
        1
    } else {
        0
    };

    // The track block is exactly the loaded track rows plus one padding row at
    // each edge. Loading and empty states still need one content row.
    let track_rows = track_count.max(1) as u16;
    let requested_track_h = track_rows.saturating_add(PANE_PAD_Y * 2);

    // Keep the hero no taller than the artwork. This keeps the track block
    // directly below the visible cover instead of leaving an empty tail under
    // the artwork just because the pane happens to be tall.
    let hero_ideal = if art_available {
        super::album_art::INLINE_ALBUM_ART_ROWS.saturating_add(if stack_metadata { 3 } else { 0 })
    } else {
        2
    }
    .min(total_h.saturating_sub(sep + PANE_PAD_Y * 2));
    let track_h = requested_track_h.min(total_h.saturating_sub(hero_ideal + sep));
    let hero_h = hero_ideal.min(total_h.saturating_sub(track_h + sep));

    let hero_area = Rect {
        x: hero_content_area.x,
        y: left_area.y,
        width: hero_content_area.width,
        height: hero_h,
    };
    let track_area = Rect {
        x: left_area.x,
        y: left_area.y + hero_h + sep,
        width: left_area.width,
        height: track_h,
    };

    let art_area = if art_available && hero_area.width >= INLINE_ALBUM_ART_RESERVED {
        let art_width = if stack_metadata {
            hero_area.width
        } else {
            INLINE_ALBUM_ART_RESERVED
        };
        Rect {
            x: if stack_metadata {
                hero_area.x
            } else {
                hero_area.x.saturating_add(
                    hero_area
                        .width
                        .saturating_sub(INLINE_ALBUM_ART_RESERVED)
                        .saturating_add(PANE_PAD_X),
                )
            },
            y: hero_area.y,
            width: art_width,
            height: if stack_metadata {
                super::album_art::INLINE_ALBUM_ART_ROWS.min(hero_area.height)
            } else {
                hero_area.height
            },
        }
    } else {
        Rect::default()
    };
    let text_area = if stack_metadata {
        Rect {
            x: hero_area.x,
            y: hero_area.y.saturating_add(art_area.height),
            width: hero_area.width,
            height: hero_area.height.saturating_sub(art_area.height),
        }
    } else {
        Rect {
            width: hero_area.width.saturating_sub(art_area.width),
            ..hero_area
        }
    };

    WideLeftLayout {
        hero_area,
        track_area,
        art_area,
        text_area,
        stack_metadata,
    }
}

fn inset_pane_vertically(area: Rect) -> Rect {
    Rect {
        y: area.y.saturating_add(PANE_PAD_Y),
        height: area.height.saturating_sub(PANE_PAD_Y * 2),
        ..area
    }
}

impl App {
    /// Renders the wide grouped Music layout: a left pane with album hero
    /// and persistent tracks, and a right pane with music-group pills and
    /// a one-column album browser.
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

        let left_content_area = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };
        if area.width < TWO_COLUMN_THRESHOLD
            || left_content_area.height < hero::HERO_ON_LEFT_MIN_AREA_HEIGHT
        {
            // Too narrow for wide mode — fall back to narrow rendering.
            self.render_list(f, area, focused, layout);
            return;
        }

        let (mut left_panel, right_panel) = hero::hero_on_left_panes(area);
        left_panel.height = left_content_area.height;

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
        let left_area = inset_pane_vertically(left_panel);
        let right_area = inset_pane_vertically(right_panel);
        layout.wide_music_right_area = right_area;

        // ── Focus state ──────────────────────────────────────────────────
        // Derive internal pane focus from outer PanelFocus and
        // album_track_focus without adding persisted focus state.
        let library_focused = matches!(self.panel_focus, PanelFocus::Library);
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
        let left_layout = compute_wide_left_layout(
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
            super::render_placeholder(f, left_area, " Loading\u{2026}");
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
        let right_pane = hero::hero_on_left_right_pane(right_panel, right_area, PANE_PAD_Y);
        let pills_area = right_pane.pills_area;
        if pills_area.y + pills_area.height <= right_area.bottom() {
            self.render_music_group_pills_row(f, pills_area, lib_idx, layout);
        }

        let list_panel = right_pane.list_panel;
        let browser_area = Rect {
            x: list_panel.x.saturating_add(PANE_PAD_X),
            y: list_panel.y.saturating_add(PANE_PAD_Y),
            width: list_panel.width.saturating_sub(PANE_PAD_X * 2),
            height: list_panel.height.saturating_sub(PANE_PAD_Y * 2),
        };
        if list_panel.height > 0 {
            let list_bg = palette::resolve_surface_focus(right_focused);
            f.render_widget(
                Block::default().style(Style::default().bg(list_bg)),
                list_panel,
            );
        }
        if browser_area.height > 0 && browser_area.width > 0 {
            self.render_wide_right_album_browser(
                f,
                browser_area,
                list_panel,
                lib_idx,
                right_focused,
                layout,
            );
        }
        hero::hero_on_left_list_panel_border(f, list_panel, right_focused);
    }

    /// Renders the wide left pane's hero: album title, metadata, and
    /// large centered artwork.
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
        let artist = self.resolve_group_album_artist(album);
        let (title, release_year) = wide_album_metadata(album, &artist);
        let title_style = if left_focused || library_focused {
            Style::default()
                .fg(palette::YELLOW)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::YELLOW)
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
                style: Style::default().fg(palette::FOAM),
            });
        }
        if let Some(year) = year_text.as_deref() {
            hero_lines.push(WrappedHeroLine {
                text: year,
                style: Style::default().fg(palette::SUBTLE),
            });
        }
        hero::paint_hero_on_left_text(f, *text, &hero_lines);

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

        // Match the recessed overview block used on Home: two columns of
        // outer inset, one row of vertical padding, and two cells of internal
        // horizontal padding around the track list.
        let track_panel = Rect {
            x: track_area.x.saturating_add(PANE_PAD_X),
            width: track_area.width.saturating_sub(PANE_PAD_X * 2),
            ..*track_area
        };
        let track_panel_bg = if left_focused {
            palette::SURFACE_ACCENT_SOFT
        } else {
            palette::SURFACE_BACKDROP
        };
        f.render_widget(
            Block::default().style(Style::default().bg(track_panel_bg)),
            track_panel,
        );
        let track_content = Rect {
            x: track_panel.x.saturating_add(PANE_PAD_X),
            y: track_panel.y.saturating_add(PANE_PAD_Y),
            width: track_panel.width.saturating_sub(PANE_PAD_X * 2),
            height: track_panel.height.saturating_sub(PANE_PAD_Y * 2),
        };
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
                        palette::YELLOW
                    } else if left_focused {
                        palette::WHITE
                    } else {
                        palette::SOFT_WHITE
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
                    let name = super::super::ui_util::trunc_str(&track.name, name_w);
                    let duration = if track.runtime_ticks > 0 {
                        super::super::ui_util::fmt_duration_mmss(
                            track.runtime_ticks / mbv_core::api::TICKS_PER_SECOND,
                        )
                    } else {
                        "\u{2014}".to_string()
                    };

                    let used = track_num.chars().count() + name.chars().count();
                    let mut spans = vec![
                        super::selection_marker(selected, super::MarkerEdge::Left),
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
                        Style::default().fg(palette::GREEN),
                    ));

                    f.render_widget(Paragraph::new(Line::from(spans)), row_rect);

                    // Record the hit target for this track.
                    layout.wide_music_track_hitmap.push((row_rect, ti));
                }

                // Scrollbar if needed.
                if n > visible && library_focused {
                    let max_offset = n.saturating_sub(visible);
                    super::render_right_scrollbar(
                        f,
                        list_area,
                        max_offset,
                        scroll,
                        palette::SCROLLBAR,
                    );
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
