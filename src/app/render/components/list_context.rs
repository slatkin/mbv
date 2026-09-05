use super::list_rows::LibraryListRenderCtx;
use crate::app::App;

impl App {
    pub(in crate::app) fn library_list_render_ctx(
        &self,
        lib_idx: usize,
        cursor: usize,
        scroll: usize,
    ) -> LibraryListRenderCtx {
        let lib = &self.libs[lib_idx];
        let (items, cursor, scroll, total_count) = match lib.nav_stack.last() {
            Some(level) => (level.items.clone(), cursor, scroll, level.total_count),
            None => (Vec::new(), 0, 0, 0),
        };

        LibraryListRenderCtx {
            items,
            cursor,
            scroll,
            total_count,
            library_total: lib.library_total,
            letter_filter: lib
                .nav_stack
                .last()
                .and_then(|level| level.letter_filter.as_ref())
                .cloned(),
            loading: lib.nav_stack.last().is_some_and(|level| level.loading),
            search_query: None,
            search_loading: false,
            group_pills: false,
        }
    }
}
