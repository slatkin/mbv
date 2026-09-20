//! Grouped Music's wide Wide hero component.

use crate::app::music_grouping::ArtistKey;
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::App;
use mbv_core::api::EmbyItem;
use std::collections::HashMap;

#[derive(Clone)]
pub(in crate::app) struct MusicWideRenderCtx {
    pub(in crate::app) list: LibraryListRenderCtx,
    pub(in crate::app) album_targets: Vec<String>,
    pub(in crate::app) selected_album: Option<EmbyItem>,
    pub(in crate::app) groups: Vec<EmbyItem>,
    pub(in crate::app) group_cursor: usize,
    pub(in crate::app) album_info: Vec<(String, String, String)>,
    /// Stable artist identity per album (parallel to `album_info`, design
    /// D2): the settled catalog's resolved `ArtistItems` identity or its
    /// deterministic fallback key. The Grouped Music tree owner groups the
    /// settled albums into artist roots by this key.
    pub(in crate::app) album_artist_keys: Vec<ArtistKey>,
    pub(in crate::app) album_order: Vec<usize>,
    pub(in crate::app) focused: bool,
    pub(in crate::app) album_tracks: Option<Vec<EmbyItem>>,
}

impl MusicWideRenderCtx {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::app) fn new(
        list: LibraryListRenderCtx,
        selected_album: Option<EmbyItem>,
        _album_artist: String,
        groups: Vec<EmbyItem>,
        group_cursor: usize,
        album_info: Vec<(String, String, String)>,
        album_artist_keys: Vec<ArtistKey>,
        album_order: Vec<usize>,
        album_tracks: Option<Vec<EmbyItem>>,
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
            groups,
            group_cursor,
            album_info,
            album_artist_keys,
            album_order,
            // Framework focus is owned by `MusicWorkspaceComponent` and applied
            // from `Attribute::Focus`; content projection never sets it.
            focused: false,
            album_tracks,
        }
    }
}

impl App {
    pub(in crate::app) fn wide_music_render_ctx(
        &self,
        lib_idx: usize,
        cursor: Option<usize>,
    ) -> MusicWideRenderCtx {
        let list = self.library_list_render_ctx(lib_idx, cursor.unwrap_or(0));
        let lib = &self.libs[lib_idx];
        let level = lib.nav_stack.last();
        let selected_cursor = cursor.unwrap_or(0);
        let selected_album = level
            .and_then(|level| level.items.get(selected_cursor))
            .cloned();
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
        let album_artist_keys = crate::app::render::screens::album_plan::group_album_artist_keys(
            &self.album_artist_cache,
            &albums,
            catalog.as_ref(),
        );
        let album_tracks = selected_album
            .as_ref()
            .and_then(|album| self.album_tracks_cache.get(&album.id).cloned());

        MusicWideRenderCtx::new(
            list,
            selected_album,
            String::new(),
            groups,
            group_cursor,
            album_info,
            album_artist_keys,
            album_order,
            album_tracks,
        )
    }
}
