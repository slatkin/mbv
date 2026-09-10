//! Grouped Music's wide Wide hero component.

use crate::app::components::inline_search::InlineSearch;
use crate::app::components::media_list::{
    InlineMediaBrowser, InlineMediaBrowserPaintPolicy, MediaKind, MediaListRow, MediaSemanticState,
    SelectedRowSurface, WideMediaList, WideMediaListPaintPolicy,
};
use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::library as library_arrangement;
use crate::app::render::arrangements::library::selected_detail_content_area;
use crate::app::render::arrangements::music::{self as music_arrangement, WideMusicLeftLayout};
use crate::app::render::arrangements::padded_rect;
use crate::app::render::arrangements::wide_hero::{self, WrappedHeroLine, PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::components::album_detail::album_hero_detail_rows;
use crate::app::render::components::detail_series_view::{SERIES_IMAGE_COLS, SERIES_IMAGE_ROWS};
use crate::app::render::components::hero::{
    paint_hero_content, selected_detail_shell, HeroContent, HeroImage, HERO_BLOCK_EXTRA_ROWS,
};
use crate::app::render::components::list_rows::{
    LibraryListRenderCtx, SELECTED_BLOCK_SIDE_PADDING,
};
use crate::app::render::components::music_wide_browser::render_wide_right_album_browser_with_ctx;
use crate::app::render::MusicImagePaint;
use crate::app::{palette, App};
use mbv_core::api::EmbyItem;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::Frame;
use std::collections::HashMap;
use tuirealm::component::Component;

#[derive(Clone)]
pub(in crate::app) struct MusicWideRenderCtx {
    pub(in crate::app) list: LibraryListRenderCtx,
    pub(in crate::app) album_targets: Vec<String>,
    pub(in crate::app) selected_album: Option<EmbyItem>,
    pub(in crate::app) album_artist: String,
    pub(in crate::app) groups: Vec<EmbyItem>,
    pub(in crate::app) group_cursor: usize,
    pub(in crate::app) album_info: Vec<(String, String, String)>,
    pub(in crate::app) album_order: Vec<usize>,
    pub(in crate::app) focused: bool,
    pub(in crate::app) images_enabled: bool,
    pub(in crate::app) album_tracks: Option<Vec<EmbyItem>>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) album_tracks_loading: bool,
    /// Parent-owned track-pane focus (design.md D5). The track owner stays
    /// authoritative for the selected track; this only says whether the track
    /// pane currently paints focused.
    pub(in crate::app) track_focused: bool,
}

impl MusicWideRenderCtx {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::app) fn new(
        list: LibraryListRenderCtx,
        selected_album: Option<EmbyItem>,
        album_artist: String,
        groups: Vec<EmbyItem>,
        group_cursor: usize,
        album_info: Vec<(String, String, String)>,
        album_order: Vec<usize>,
        images_enabled: bool,
        album_tracks: Option<Vec<EmbyItem>>,
        album_tracks_loading: bool,
        track_focused: bool,
    ) -> Self {
        let mut counts = HashMap::<&str, usize>::new();
        for album in &list.items {
            *counts.entry(&album.id).or_default() += 1;
        }
        let album_targets = list
            .items
            .iter()
            .enumerate()
            .map(|(index, album)| {
                if counts[album.id.as_str()] > 1 {
                    format!("{}\u{0}{index}", album.id)
                } else {
                    album.id.clone()
                }
            })
            .collect();
        Self {
            list,
            album_targets,
            selected_album,
            album_artist,
            groups,
            group_cursor,
            album_info,
            album_order,
            // Framework focus is owned by `MusicWorkspaceComponent` and applied
            // from `Attribute::Focus`; content projection never sets it.
            focused: false,
            images_enabled,
            album_tracks,
            album_tracks_loading,
            track_focused,
        }
    }

    /// Publish the geometry shared by the legacy underpaint and the mounted
    /// Music workspace before the component view runs, and return the pure
    /// arrangement so the paint path consumes the same computed panes and
    /// left layout instead of recomputing them.
    pub(in crate::app) fn publish_geometry(
        &self,
        area: Rect,
        layout: &mut LayoutMain,
    ) -> Option<(library_arrangement::WideLibraryPanes, WideMusicLeftLayout)> {
        layout.wide_music_area = area;
        layout.wide_music_art_area = Rect::default();

        let panes = library_arrangement::wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y)?;
        let left_layout = music_arrangement::wide_music_left_layout(
            panes.hero_panel,
            self.selected_album.is_some() && self.images_enabled,
            self.album_tracks.as_ref().map_or(0, Vec::len),
        );
        layout.wide_music_right_area = panes.browser_area;
        layout.left_area = panes.hero_area;
        layout.hero_area = left_layout.hero_area;
        if self.selected_album.is_some() {
            layout.wide_music_art_area = left_layout.art_area;
        }
        Some((panes, left_layout))
    }
}

impl MusicWideRenderCtx {
    /// Canonical row projection shared by the wide `WideMediaList` and the
    /// narrow `InlineMediaBrowser`: one `Heading` per artist group, a `Spacer`
    /// between groups, and one selectable `Item` per album keyed by its stable
    /// id. Grouped Music album rows carry no played/active state (parity with
    /// the wide rail and the legacy painter).
    pub(in crate::app) fn grouped_rows(&self) -> Vec<MediaListRow<String>> {
        grouped_album_rows_with_targets(&self.album_info, &self.album_order, &self.album_targets)
    }
}

fn grouped_album_rows_with_targets(
    album_info: &[(String, String, String)],
    order: &[usize],
    targets: &[String],
) -> Vec<MediaListRow<String>> {
    let mut rows = Vec::new();
    let mut start = 0;
    while start < order.len() {
        let artist = album_info[order[start]].0.clone();
        let mut end = start + 1;
        while end < order.len() && album_info[order[end]].0 == artist {
            end += 1;
        }
        if start > 0 {
            rows.push(MediaListRow::Spacer);
        }
        rows.push(MediaListRow::Heading { text: artist });
        for &idx in &order[start..end] {
            let (_, year, name) = &album_info[idx];
            rows.push(MediaListRow::Item {
                target: targets[idx].clone(),
                primary: name.clone(),
                trailing: (!year.is_empty()).then(|| year.clone()),
                duration: None,
                kind: MediaKind::Collection,
                semantic_state: MediaSemanticState::Ordinary,
            });
        }
        start = end;
    }
    rows
}

#[derive(Default)]
pub(in crate::app) struct MusicWideRenderOutput {
    pub(in crate::app) image_paint: Option<MusicImagePaint>,
}

/// The active album-row presentation Grouped Music hands to the render layer
/// this frame (design.md D1/D2). The component derives it from the painted
/// breakpoint; the same shared owner moves between the two adapters.
pub(in crate::app) enum MusicAlbumPresentation<'a> {
    Wide(&'a mut WideMediaList<String>),
    Inline(&'a mut InlineMediaBrowser<String>),
}

/// The active track-row presentation Grouped Music hands to the render layer
/// this frame. Tracks paint fixed one-column rows through the Wide
/// presentation (design.md D1/D2).
pub(in crate::app) enum MusicTrackPresentation<'a> {
    Wide(&'a mut WideMediaList<String>),
}

/// The wide Music right-rail browser content height for `area`, computed from
/// the same arrangement the painter consumes. Grouped Music uses it to restore
/// the responsive anchor against the receiving presentation's painted viewport
/// rather than a stale retained rect.
pub(in crate::app) fn wide_music_browser_content_height(area: Rect) -> Option<usize> {
    let panes = library_arrangement::wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y)?;
    let right_pane = wide_hero::wide_hero_browser_pane(panes.browser_panel, panes.browser_area);
    let browser_area = padded_rect(right_pane.list_panel, PANE_PAD_X, PANE_PAD_Y);
    Some(browser_area.height as usize)
}

/// Strips the "Artist (Year) " folder-name prefix from an album's display
/// name, returning the bare title and resolved release year.
pub(in crate::app::render) fn wide_album_metadata(album: &EmbyItem, artist: &str) -> (String, u32) {
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
    /// Pre-warm nearby album art for narrow grouped Music after navigation has
    /// gone idle. `order` is the already-resolved display order, so this does
    /// not repeat the grouping or sorting work used to build the render context.
    ///
    /// Shell-side effect, not painting: like `fetch_nearby_movie_posters`
    /// (`list_narrow.rs`), this is an `App` image-fetch helper that lives in the
    /// render module because the album cache-key/art-type constants are render-
    /// scoped. Called only from `shell_music_workspace.rs` after the mounted
    /// component has established its authoritative cursor — never from
    /// `MusicWorkspaceComponent::view`, which must stay free of `App` effects.
    pub(in crate::app) fn prewarm_grouped_music_album_images(
        &mut self,
        albums: &[EmbyItem],
        cursor: usize,
        order: &[usize],
    ) {
        const PREFETCH_AHEAD: usize = 3;
        const PREFETCH_BEHIND: usize = 1;

        let Some(cursor_pos) = order.iter().position(|&idx| idx == cursor) else {
            return;
        };
        let start = cursor_pos.saturating_sub(PREFETCH_BEHIND);
        let end = (cursor_pos + PREFETCH_AHEAD + 1).min(order.len());
        for (offset, &idx) in order[start..end].iter().enumerate() {
            if start + offset == cursor_pos {
                continue;
            }
            let Some(album) = albums.get(idx) else {
                continue;
            };
            self.fetch_list_card_image_when_idle(
                crate::app::render::components::album_art::inline_album_art_cache_key(&album.id),
                album.id.clone(),
                album.series_id.clone(),
                crate::app::render::MUSIC_ALBUM_IMAGE_TYPES,
            );
        }
    }

    pub(in crate::app) fn wide_music_render_ctx(
        &self,
        lib_idx: usize,
        cursor_scroll: Option<(usize, usize)>,
    ) -> MusicWideRenderCtx {
        let list = self.library_list_render_ctx(
            lib_idx,
            cursor_scroll.map_or(0, |v| v.0),
            cursor_scroll.map_or(0, |v| v.1),
        );
        let lib = &self.libs[lib_idx];
        let level = lib.nav_stack.last();
        let selected_cursor = cursor_scroll.map_or(0, |(cursor, _)| cursor);
        let selected_album = level
            .and_then(|level| level.items.get(selected_cursor))
            .cloned();
        let album_artist = selected_album
            .as_ref()
            .map(|album| self.resolve_group_album_artist(album))
            .unwrap_or_default();
        let (groups, group_cursor) = if lib.nav_stack.len() >= 2 {
            let group = &lib.nav_stack[lib.nav_stack.len() - 2];
            (group.items.clone(), group.resting().cursor())
        } else {
            (Vec::new(), 0)
        };
        let albums = level.map(|level| level.items.clone()).unwrap_or_default();
        let catalog = level
            .and_then(|level| level.music_grouping.as_ref())
            .and_then(|state| state.settled.clone());
        let album_info = crate::app::render::screens::album_plan::group_album_info(
            &self.album_artist_cache,
            &albums,
            catalog.as_ref(),
        );
        let album_order = catalog
            .as_ref()
            .map(|catalog| {
                catalog
                    .entries
                    .iter()
                    .map(|entry| entry.album_index)
                    .filter(|&index| index < albums.len())
                    .collect()
            })
            .unwrap_or_else(|| crate::app::render::sorted_group_album_order(&album_info));
        let (album_tracks, album_tracks_loading) = selected_album
            .as_ref()
            .map(|album| {
                (
                    self.album_tracks_cache.get(&album.id).cloned(),
                    self.album_tracks_loading.contains(&album.id),
                )
            })
            .unwrap_or((None, false));

        MusicWideRenderCtx::new(
            list,
            selected_album,
            album_artist,
            groups,
            group_cursor,
            album_info,
            album_order,
            self.images_enabled(),
            album_tracks,
            album_tracks_loading,
            // The App side never owns inline track focus: the wide
            // `MusicWorkspaceComponent` repaints over this underpaint with
            // its parent-owned track-pane focus, and narrow keeps track focus
            // explicitly off (design.md D5).
            false,
        )
    }
}

/// Paint grouped Music in Normal geometry through the canonical persistent
/// `InlineMediaBrowser`: the group pill bar, one-column album rows with artist
/// headings/spacers, and the selected album's inline detail block (title /
/// artist / year / art) reserved in the row flow by the control. Mirrors the
/// narrow TV series composition in `list_narrow.rs`. The control is owned and
/// fed by `MusicWorkspaceComponent`; this function only paints and publishes
/// the flow geometry.
pub(in crate::app) fn render_narrow_music_group_with_ctx(
    f: &mut Frame,
    area: Rect,
    ctx: &MusicWideRenderCtx,
    layout: &mut LayoutMain,
    presentation: MusicAlbumPresentation<'_>,
) -> MusicWideRenderOutput {
    let MusicAlbumPresentation::Inline(browser) = presentation else {
        return MusicWideRenderOutput::default();
    };
    browser.invalidate_paint();
    // Group pill bar above the album rows, mirroring the narrow browser
    // (`list_narrow.rs`) and the wide sibling's right-pane pill slot. Album
    // rows then render into the reduced content area.
    let content_area = if ctx.groups.is_empty() {
        area
    } else {
        let areas = wide_hero::pill_bar_areas(area);
        if ctx.list.is_search_active() {
            crate::app::render::components::hero::render_search_box(
                f,
                areas.pills_area,
                ctx.list.search_query.as_deref().unwrap_or_default(),
                ctx.list.search_loading,
            );
        } else {
            crate::app::render::components::music::render_music_group_pills_row_with_ctx(
                f,
                areas.pills_area,
                &ctx.groups,
                ctx.group_cursor,
                layout,
            );
        }
        areas.content_area
    };

    if ctx.list.items.is_empty() {
        crate::app::render::render_placeholder(
            f,
            content_area,
            if ctx.list.loading {
                " Loading\u{2026}"
            } else {
                " (empty)"
            },
        );
        return MusicWideRenderOutput { image_paint: None };
    }

    let images_enabled = ctx.images_enabled;
    let hero_rows = album_hero_detail_rows(images_enabled) + HERO_BLOCK_EXTRA_ROWS as usize;
    let focused = ctx.focused;
    let cursor = ctx.list.cursor;

    browser.set_geometry(content_area, content_area);
    browser.set_paint_policy(InlineMediaBrowserPaintPolicy::new(
        focused,
        SelectedRowSurface::ListBackdrop,
        hero_rows,
    ));
    Component::view(browser, f, content_area);

    let mut image_paint = None;
    match browser.current_detail_rect() {
        Some(hero_area) => {
            layout.hero_area = hero_area;
            layout.inline_hero_area = hero_area;
            layout.selected_item_rect = Some(hero_area);
            selected_detail_shell(f, hero_area, hero_rows as u16, focused);
            let content_rect = selected_detail_content_area(
                hero_area,
                SELECTED_BLOCK_SIDE_PADDING,
                HERO_BLOCK_EXTRA_ROWS,
            );
            if let Some((artist, year, title)) = ctx.album_info.get(cursor) {
                let meta = if year.is_empty() {
                    artist.clone()
                } else {
                    format!("{artist} \u{2022} {year}")
                };
                let image = images_enabled.then_some(HeroImage {
                    actual_w: SERIES_IMAGE_COLS,
                    height: SERIES_IMAGE_ROWS,
                });
                let content = HeroContent {
                    title: Some(title.as_str()),
                    meta_line: (!meta.is_empty()).then_some(meta.as_str()),
                    meta_color: palette::TEXT_DETAIL_META,
                    show_playing: false,
                    unconditional_spacer_after_meta: true,
                    lines: &[],
                    image,
                };
                let painted = paint_hero_content(f, content_rect, &content, false);
                image_paint = painted.img_rect.and_then(|img_rect| {
                    ctx.list
                        .items
                        .get(cursor)
                        .filter(|_| images_enabled && img_rect.width >= 4 && img_rect.height >= 2)
                        .map(|album| MusicImagePaint {
                            area: img_rect,
                            album: Box::new(album.clone()),
                            centered: false,
                        })
                });
            }
        }
        None => {
            layout.selected_item_rect = browser.current_selected_row_rect();
        }
    }

    MusicWideRenderOutput { image_paint }
}

/// Wide grouped Music paints one full-width album row at a time. The mounted
/// component's navigation uses that same one-dimensional geometry.
pub(in crate::app) fn render_wide_music_group_with_ctx(
    f: &mut Frame,
    area: Rect,
    ctx: &MusicWideRenderCtx,
    layout: &mut LayoutMain,
    album_presentation: MusicAlbumPresentation<'_>,
    track_presentation: MusicTrackPresentation<'_>,
    inline_search: &mut InlineSearch,
) -> MusicWideRenderOutput {
    let mut output = MusicWideRenderOutput::default();
    let MusicAlbumPresentation::Wide(album_list) = album_presentation else {
        return output;
    };
    let MusicTrackPresentation::Wide(track_list) = track_presentation;
    album_list.invalidate_paint();
    // The pure arrangement is computed exactly once here in
    // `publish_geometry`; the paint path below consumes the returned panes
    // and left layout rather than recomputing them.
    let Some((panes, left_layout)) = ctx.publish_geometry(area, layout) else {
        return output;
    };
    let browser_panel = panes.browser_panel;
    let browser_area = panes.browser_area;
    let track_active = ctx.track_focused;
    let left_focused = ctx.focused && track_active;
    let right_focused = ctx.focused && !track_active;
    let Some(left_area) = wide_hero::wide_hero_hero_pane(
        f,
        area,
        wide_hero::LeftPaneFocus::Workspace(ctx.focused && ctx.track_focused),
    ) else {
        return output;
    };
    layout.left_area = left_area;
    track_list.invalidate_paint();

    if let Some(album) = ctx.selected_album.as_ref() {
        output.image_paint = render_wide_left_hero(
            f,
            &left_layout,
            album,
            &ctx.album_artist,
            left_focused,
            ctx.focused,
            ctx.images_enabled,
        );
        let track_area = left_layout.track_area;
        if track_area.height > 0 && track_area.width > 0 && !track_list.is_empty() {
            let track_surface = if left_focused {
                crate::app::render::arrangements::wide_hero::WideHeroContentBoxSurface::FocusedTrackList
            } else {
                crate::app::render::arrangements::wide_hero::WideHeroContentBoxSurface::Backdrop
            };
            let (track_panel, track_content_area) = crate::app::render::arrangements::wide_hero::
                wide_hero_hero_content_box_with_surface(f, track_area, track_surface);
            track_list.set_geometry(track_panel, track_content_area);
            track_list.set_paint_policy(WideMediaListPaintPolicy::new(
                left_focused,
                SelectedRowSurface::OwningSurface,
                None,
            ));
            Component::view(track_list, f, track_area);
            layout.selected_item_rect = track_list.current_selected_row_rect();
        }
    } else {
        crate::app::render::render_placeholder(f, left_area, " Loading\u{2026}");
    }

    f.render_widget(
        ratatui::widgets::Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
        browser_panel,
    );
    let right_pane = wide_hero::wide_hero_browser_pane(browser_panel, browser_area);
    if !inline_search.is_active() && ctx.list.is_search_active() {
        crate::app::render::components::hero::render_search_box(
            f,
            right_pane.pills_area,
            ctx.list.search_query.as_deref().unwrap_or_default(),
            ctx.list.search_loading,
        );
    } else if !inline_search.is_active()
        && right_pane.pills_area.y + right_pane.pills_area.height <= browser_area.bottom()
    {
        crate::app::render::components::music::render_music_group_pills_row_with_ctx(
            f,
            right_pane.pills_area,
            &ctx.groups,
            ctx.group_cursor,
            layout,
        );
    }

    let list_panel = right_pane.list_panel;
    let browser_area = padded_rect(list_panel, PANE_PAD_X, PANE_PAD_Y);
    if list_panel.height > 0 {
        f.render_widget(
            ratatui::widgets::Block::default()
                .style(Style::default().bg(palette::resolve_surface_focus(right_focused))),
            list_panel,
        );
    }
    // Paint the rail frame before the rows: `wide_hero_browser_border`
    // rewrites every panel cell's background, so it must not run after the
    // canonical list (which owns the selected-row background). Mirrors
    // `render_wide_tv_with_ctx`.
    wide_hero::wide_hero_browser_border(f, list_panel, right_focused);
    if browser_area.height > 0 && browser_area.width > 0 {
        if inline_search.is_active() {
            // Wide hero Wide passes only the right-rail library-list area
            // (design.md D3); the Hero pane and track pane painted above
            // remain visible, and the ordinary grouped album rail does not
            // also paint `browser_area`.
            let items = inline_search.ordered_items();
            let query = inline_search.query().to_string();
            let loading = inline_search.loading();
            let cursor = inline_search.cursor();
            let scroll_in = inline_search.scroll();
            let new_scroll = crate::app::render::render_inline_search(
                f,
                right_pane.pills_area,
                browser_area,
                &query,
                loading,
                items,
                cursor,
                scroll_in,
                right_focused,
                1,
                inline_search.layout_mut(),
            );
            inline_search.set_scroll(new_scroll);
        } else if ctx.list.is_search_active() {
            // The search-results grid is not the canonical album rail; keep
            // the rail control empty so a stray mouse hit resolves to nothing.
            let cols = crate::app::library_column_width::library_column_count(browser_area.width);
            let _ = super::media_list::render_plain_rows(
                f,
                ctx.list.rows(browser_area, cols, right_focused, 0),
                layout,
            );
        } else {
            render_wide_right_album_browser_with_ctx(
                f,
                browser_area,
                list_panel,
                &ctx.list,
                right_focused,
                layout,
                album_list,
            );
        }
    }
    output
}

fn render_wide_left_hero(
    f: &mut Frame,
    left_layout: &WideMusicLeftLayout,
    album: &EmbyItem,
    artist: &str,
    left_focused: bool,
    library_focused: bool,
    images_enabled: bool,
) -> Option<MusicImagePaint> {
    let (title, release_year) = wide_album_metadata(album, artist);
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
            text: artist,
            style: Style::default().fg(palette::TEXT_METADATA),
        });
    }
    if let Some(year) = year_text.as_deref() {
        hero_lines.push(WrappedHeroLine {
            text: year,
            style: Style::default().fg(palette::TEXT_SECONDARY),
        });
    }
    wide_hero::paint_wide_hero_text(f, left_layout.text_area, &hero_lines);

    if images_enabled && left_layout.art_area.width > 0 && left_layout.art_area.height > 0 {
        return Some(MusicImagePaint {
            area: left_layout.art_area,
            album: Box::new(album.clone()),
            centered: left_layout.stack_metadata,
        });
    }
    None
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
