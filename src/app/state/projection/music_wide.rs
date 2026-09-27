use crate::app::App;
use mbv_render::MusicWideRenderCtx;

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
        let (album_info, album_artist_keys) =
            mbv_render::group_album_plan(&self.album_artist_cache, &albums, catalog.as_ref());
        let album_order = catalog.as_ref().map_or_else(
            || mbv_render::sorted_group_album_order(&album_info),
            |catalog| {
                catalog
                    .entries
                    .iter()
                    .map(|entry| entry.album_index)
                    .filter(|&index| index < albums.len())
                    .collect()
            },
        );
        let album_tracks = selected_album
            .as_ref()
            .and_then(|album| self.album_tracks_cache.get(&album.id).cloned());
        let catalog_revision = catalog.as_ref().map_or(0, |catalog| catalog.revision);

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
        .with_catalog_revision(catalog_revision)
    }
}
