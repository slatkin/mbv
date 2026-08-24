use super::list_rows::LibraryListRenderCtx;
use crate::app::App;

impl App {
    pub(in crate::app::render) fn library_list_render_ctx(
        &self,
        lib_idx: usize,
    ) -> LibraryListRenderCtx {
        let lib = &self.libs[lib_idx];
        let (items, cursor, scroll, total_count, search_query, search_loading) =
            if let Some(search) = &lib.search {
                let items = search
                    .results
                    .iter()
                    .filter_map(|&idx| {
                        search.items.get(idx).map(|item| {
                            self.recursive_album_display_item(lib_idx, idx, item.clone())
                        })
                    })
                    .collect::<Vec<_>>();
                let total = items.len();
                (
                    items,
                    search.cursor,
                    search.scroll,
                    total,
                    Some(search.query.clone()),
                    search.loading,
                )
            } else {
                match lib.nav_stack.last() {
                    Some(level) => (
                        level.items.clone(),
                        level.cursor,
                        level.scroll,
                        level.total_count,
                        None,
                        false,
                    ),
                    None => (Vec::new(), 0, 0, 0, None, false),
                }
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
            search_query,
            search_loading,
        }
    }
}
