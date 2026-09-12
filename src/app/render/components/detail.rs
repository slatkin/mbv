use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::App;

/// Cache key for the compact movie banner's poster image, under which
/// `fetch_card_image`/`fetch_list_card_image_when_idle` store and look up the
/// resized/encoded image state. Shared by the Home hero's Movies/HomeVideos
/// image chain (`home_hero_emby.rs`) and the prefetch loop in
/// `list_narrow.rs`'s `fetch_nearby_movie_posters` (#287) so the two can
/// never format the key differently and silently miss each other's cache
/// entries.
pub(in crate::app::render) fn compact_banner_image_cache_key(item_id: &str) -> String {
    format!("{item_id}:cmp_primary")
}

impl App {
    pub(crate) fn selected_series_item(
        &self,
        lib_idx: usize,
        cursor: usize,
    ) -> Option<mbv_core::api::EmbyItem> {
        let ctx = self.library_list_render_ctx(lib_idx, cursor, 0);
        self.selected_series_item_with_ctx(lib_idx, &ctx)
    }

    fn selected_series_item_with_ctx(
        &self,
        lib_idx: usize,
        ctx: &LibraryListRenderCtx,
    ) -> Option<mbv_core::api::EmbyItem> {
        let lib = self.libs.get(lib_idx)?;
        if lib.library.collection_type != "tvshows" {
            return None;
        }

        let item = ctx.items.get(ctx.cursor)?.clone();

        if item.item_type != "Series" {
            return None;
        }

        Some(item)
    }
}
