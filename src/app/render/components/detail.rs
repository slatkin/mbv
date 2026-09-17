use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::App;

impl App {
    pub(crate) fn selected_series_item(
        &self,
        lib_idx: usize,
        cursor: usize,
    ) -> Option<mbv_core::api::EmbyItem> {
        let ctx = self.library_list_render_ctx(lib_idx, cursor);
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
