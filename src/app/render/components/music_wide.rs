//! Grouped Music's wide hero-on-left component.

use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::hero_left::{self, WrappedHeroLine, PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::arrangements::library as library_arrangement;
use crate::app::render::arrangements::music::{self as music_arrangement, WideMusicLeftLayout};
use crate::app::render::arrangements::padded_rect;
use crate::app::render::components::album::{
    render_grouped_album_rows_with_ctx, AlbumRowsCursorCtx, GroupedAlbumRenderCtx,
};
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::components::music_wide_browser::render_wide_right_album_browser_with_ctx;
use crate::app::render::components::music_wide_tracks::render_wide_left_tracks;
use crate::app::render::MusicImagePaint;
use crate::app::{palette, App, PanelFocus};
use mbv_core::api::EmbyItem;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::Frame;
use std::collections::HashMap;

#[derive(Clone)]
pub(in crate::app) struct MusicWideRenderCtx {
    pub(in crate::app) list: LibraryListRenderCtx,
    pub(in crate::app) selected_album: Option<EmbyItem>,
    pub(in crate::app) album_artist: String,
    pub(in crate::app) groups: Vec<EmbyItem>,
    pub(in crate::app) group_cursor: usize,
    pub(in crate::app) album_info: Vec<(String, String, String)>,
    pub(in crate::app) album_order: Vec<usize>,
    pub(in crate::app) focused: bool,
    pub(in crate::app) images_enabled: bool,
    pub(in crate::app) album_tracks: Option<Vec<EmbyItem>>,
    pub(in crate::app) album_tracks_loading: bool,
    pub(in crate::app) track_cursor: Option<usize>,
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
        focused: bool,
        images_enabled: bool,
        album_tracks: Option<Vec<EmbyItem>>,
        album_tracks_loading: bool,
        track_cursor: Option<usize>,
    ) -> Self {
        Self {
            list,
            selected_album,
            album_artist,
            groups,
            group_cursor,
            album_info,
            album_order,
            focused,
            images_enabled,
            album_tracks,
            album_tracks_loading,
            track_cursor,
        }
    }

    pub(in crate::app) fn with_local_state(
        mut self,
        cursor: usize,
        scroll: usize,
        track_cursor: Option<usize>,
    ) -> Self {
        self.list = self.list.with_cursor_scroll(cursor, scroll);
        self.track_cursor = track_cursor;
        self
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

        let Some(panes) = library_arrangement::wide_library_panes(area, 0, PANE_PAD_Y) else {
            return None;
        };
        let left_layout = music_arrangement::wide_music_left_layout(
            panes.left_area,
            self.selected_album.is_some() && self.images_enabled,
            self.album_tracks.as_ref().map_or(0, Vec::len),
        );
        layout.wide_music_right_area = panes.right_area;
        layout.left_area = panes.left_area;
        layout.hero_area = left_layout.hero_area;
        if self.selected_album.is_some() {
            layout.wide_music_art_area = left_layout.art_area;
        }
        Some((panes, left_layout))
    }
}

#[derive(Default)]
pub(in crate::app) struct MusicWideRenderOutput {
    pub(in crate::app) final_scroll: usize,
    pub(in crate::app) image_paint: Option<MusicImagePaint>,
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
    pub(in crate::app) fn wide_music_render_ctx(
        &self,
        lib_idx: usize,
        cursor_scroll: Option<(usize, usize)>,
    ) -> MusicWideRenderCtx {
        let list = self.library_list_render_ctx(
            lib_idx,
            true,
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
            matches!(self.effective_panel_focus(), PanelFocus::Library),
            self.images_enabled(),
            album_tracks,
            album_tracks_loading,
            // The App side never owns inline track focus: the wide
            // `MusicWorkspaceComponent` repaints over this underpaint with
            // its local cursor, and narrow keeps track focus explicitly off.
            None,
        )
    }
}

/// App-free grouped Music painter. The legacy `App` path still performs the
/// track fetch before building this context; this function consumes only its
/// resolved presentation data.
/// Paint grouped Music in Normal geometry: the album rows and Model A inline hero.
pub(in crate::app) fn render_narrow_music_group_with_ctx(
    f: &mut Frame,
    area: Rect,
    ctx: &MusicWideRenderCtx,
    layout: &mut LayoutMain,
) -> MusicWideRenderOutput {
    let mut album_tracks = HashMap::new();
    if let (Some(album), Some(tracks)) = (&ctx.selected_album, &ctx.album_tracks) {
        album_tracks.insert(album.id.clone(), tracks.clone());
    }

    // Group pill bar above the album rows, mirroring the narrow browser
    // (`list_narrow.rs`) and the wide sibling's right-pane pill slot. Album
    // rows then render into the reduced content area.
    let content_area = if ctx.groups.is_empty() {
        area
    } else {
        let areas = hero_left::pill_bar_areas(area);
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

    let (offset, image_paint) = render_grouped_album_rows_with_ctx(
        f,
        content_area,
        &ctx.list.items,
        AlbumRowsCursorCtx {
            cursor: ctx.list.cursor,
            stored_scroll: ctx.list.scroll,
        },
        ctx.focused,
        true,
        1,
        layout,
        GroupedAlbumRenderCtx {
            album_info: ctx.album_info.clone(),
            order: ctx.album_order.clone(),
            in_music_group_view: true,
            playing_track_id: None,
            images_enabled: ctx.images_enabled,
            album_tracks: &album_tracks,
        },
    );
    MusicWideRenderOutput {
        final_scroll: offset,
        image_paint,
    }
}

pub(in crate::app) fn render_wide_music_group_with_ctx(
    f: &mut Frame,
    area: Rect,
    ctx: &MusicWideRenderCtx,
    layout: &mut LayoutMain,
) -> MusicWideRenderOutput {
    let mut output = MusicWideRenderOutput::default();
    // The pure arrangement is computed exactly once here in
    // `publish_geometry`; the paint path below consumes the returned panes
    // and left layout rather than recomputing them.
    let Some((panes, left_layout)) = ctx.publish_geometry(area, layout) else {
        return output;
    };
    layout.wide_music_track_hitmap.clear();
    let left_panel = panes.left_panel;
    let right_panel = panes.right_panel;
    f.render_widget(
        ratatui::widgets::Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
        Rect {
            x: left_panel.x,
            y: left_panel.bottom(),
            width: left_panel.width,
            height: 1,
        },
    );

    let left_area = panes.left_area;
    let right_area = panes.right_area;
    let track_active = ctx.track_cursor.is_some();
    let left_focused = ctx.focused && track_active;
    let right_focused = ctx.focused && !track_active;
    // `left_layout` is the arrangement returned from `publish_geometry`; no
    // recomputation here.
    f.render_widget(
        ratatui::widgets::Block::default()
            .style(Style::default().bg(palette::resolve_surface_focus(left_focused))),
        left_panel,
    );

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
        render_wide_left_tracks(
            f,
            &left_layout.track_area,
            album,
            ctx.album_tracks.as_deref(),
            ctx.album_tracks_loading,
            ctx.track_cursor,
            left_focused,
            ctx.focused,
            layout,
        );
    } else {
        crate::app::render::render_placeholder(f, left_area, " Loading\u{2026}");
    }

    f.render_widget(
        ratatui::widgets::Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
        right_panel,
    );
    let right_pane = hero_left::hero_on_left_right_pane(right_panel, right_area, PANE_PAD_Y);
    if ctx.list.is_search_active() {
        crate::app::render::components::hero::render_search_box(
            f,
            right_pane.pills_area,
            ctx.list.search_query.as_deref().unwrap_or_default(),
            ctx.list.search_loading,
        );
    } else if right_pane.pills_area.y + right_pane.pills_area.height <= right_area.bottom() {
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
    if browser_area.height > 0 && browser_area.width > 0 {
        if ctx.list.is_search_active() {
            let cols = crate::app::library_column_width::library_column_count(browser_area.width);
            output.final_scroll = super::list_plain::render_plain_rows(
                f,
                ctx.list.rows(browser_area, cols, right_focused, 0),
                layout,
            );
        } else {
            output.final_scroll = render_wide_right_album_browser_with_ctx(
                f,
                browser_area,
                list_panel,
                &ctx.album_info,
                &ctx.album_order,
                &ctx.list,
                right_focused,
                layout,
            );
        }
    }
    hero_left::hero_on_left_list_panel_border(f, list_panel, right_focused);
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
    hero_left::paint_hero_on_left_text(f, left_layout.text_area, &hero_lines);

    if images_enabled && left_layout.art_area.width > 0 && left_layout.art_area.height > 0 {
        return Some(MusicImagePaint::Album {
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
