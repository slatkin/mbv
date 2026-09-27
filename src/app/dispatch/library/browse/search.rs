use crate::app::{AlbumPathPart, AlbumSearchEntry, App, BrowseLevel, LibEvent, LibraryTab};
use mbv_core::api::EmbyItem;

pub(super) type AlbumIndexFetch<'a> =
    dyn FnMut(&str, usize, usize) -> Result<(Vec<EmbyItem>, usize), String> + 'a;
const ALBUM_INDEX_PAGE_SIZE: usize = 200;
// Visibility bump: private -> `pub(in crate::app)`. Exercised directly by
// `actions_tests.rs` (a submodule of `actions.rs`, so it needs an explicit
// `crate::app::dispatch::library::browse::...` import once this lives outside
// `actions.rs`'s own namespace).
pub(in crate::app) fn recursive_album_search_eligible(
    collection_type: &str,
    levels: &[String],
) -> bool {
    collection_type == "music"
        && levels.len() > 1
        && levels.last().is_some_and(|level| level == "album")
}

/// The correct fetch `limit` for an unfiltered whole-library fetch, used by
/// `spawn_all_items_prefetch`/`spawn_search_items_load` so `all_items` (the
/// set `/`-search runs over) always spans the entire library. `lvl.total_count`
/// alone is NOT enough: with a letter-range pill active it's the FILTERED
/// range's count (e.g. ~40 for `A–C` out of a 3,000-item library), which
/// would silently truncate `all_items` to the active range and make search
/// miss everything outside it. `lib.library_total` (the true count captured
/// on the library's first, unfiltered load) is the right number; `.max` is
/// just a defensive fallback for the moment before it's been captured.
// Visibility bump: private -> `pub(in crate::app)`. Same reason as
// `recursive_album_search_eligible` above -- exercised directly by
// `actions_tests.rs`.
pub(in crate::app) fn full_library_fetch_limit(lib: &LibraryTab, lvl: &BrowseLevel) -> usize {
    lib.library_total
        .unwrap_or(lvl.total_count)
        .max(lvl.total_count)
}

pub(in crate::app) fn fetch_all_album_index_items(
    parent_id: &str,
    fetch: &mut AlbumIndexFetch<'_>,
) -> Result<Vec<EmbyItem>, String> {
    let mut items = Vec::new();
    loop {
        let (page, total) = fetch(parent_id, items.len(), ALBUM_INDEX_PAGE_SIZE)?;
        if page.is_empty() {
            break;
        }
        items.extend(page);
        if items.len() >= total {
            break;
        }
    }
    Ok(items)
}

// Visibility bump: private -> `pub(in crate::app)`. Same reason as
// `recursive_album_search_eligible` above -- exercised directly by
// `actions_tests.rs`.
pub(in crate::app) fn build_album_index_with(
    library_id: &str,
    levels: &[String],
    fetch: &mut AlbumIndexFetch<'_>,
) -> Result<Vec<AlbumSearchEntry>, String> {
    fn visit(
        parent_id: &str,
        depth: usize,
        levels: &[String],
        ancestors: &mut Vec<AlbumPathPart>,
        entries: &mut Vec<AlbumSearchEntry>,
        fetch: &mut AlbumIndexFetch<'_>,
    ) -> Result<(), String> {
        let items = fetch_all_album_index_items(parent_id, fetch)?;
        if depth + 1 == levels.len() {
            // The terminal configured level is "album" by position, not by
            // Emby's `Type` field: unidentified/unmatched album folders come
            // back as plain "Folder" items rather than "MusicAlbum", so
            // filtering on the type string silently dropped them from the
            // index. `is_folder` is the same criterion the intermediate
            // levels above already use.
            for album in items.into_iter().filter(|item| item.is_folder) {
                entries.push(AlbumSearchEntry::from_chain(album, ancestors.clone()));
            }
            return Ok(());
        }

        for item in items.into_iter().filter(|item| item.is_folder) {
            ancestors.push(AlbumPathPart {
                id: item.id.clone(),
                name: item.display_name(),
            });
            visit(&item.id, depth + 1, levels, ancestors, entries, fetch)?;
            ancestors.pop();
        }
        Ok(())
    }

    let mut entries = Vec::new();
    visit(library_id, 0, levels, &mut Vec::new(), &mut entries, fetch)?;
    Ok(entries)
}

impl App {
    // Visibility bump: private -> `pub(in crate::app)`. Called from
    // `handle_lib_loaded`/`handle_lib_page_appended`, which stay behind in
    // `actions.rs`.
    pub(in crate::app) fn spawn_all_items_prefetch(&self, lib_idx: usize) {
        let lib = &self.libs[lib_idx];
        let Some(lvl) = lib.nav_stack.last() else {
            return;
        };
        // `lvl.is_fully_loaded()` compares server rows consumed (`fetched_rows`)
        // against the active range's `total_count` -- with a letter-range pill
        // active, that count is the FILTERED range's total, not the whole
        // library's, so a fully-loaded small range (e.g. 40 items in `A–C`)
        // would wrongly read as "nothing more to prefetch" while `all_items`
        // (which backs whole-library search) is still absent. `all_items` must
        // never be satisfied by just the active range.
        if lvl.letter_filter.is_none() && lvl.is_fully_loaded() {
            return;
        }
        let parent_id = lvl.parent_id.clone();
        let total_count = full_library_fetch_limit(lib, lvl);
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            if let Ok((items, _)) = client.get_items_sorted(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                0,
                total_count,
                &sort_by,
                &sort_order,
            ) {
                let _ = tx.send(LibEvent::AllItemsPrefetched {
                    lib_idx,
                    parent_id,
                    items,
                });
            }
        });
    }

    pub(in crate::app) fn spawn_search_items_load(&self, lib_idx: usize) {
        let lib = &self.libs[lib_idx];
        let Some(lvl) = lib.nav_stack.last() else {
            return;
        };
        let parent_id = lvl.parent_id.clone();
        // See `spawn_all_items_prefetch` above: always fetch the WHOLE
        // library unfiltered so search covers everything, not just an
        // active letter-range pill's slice.
        let total_count = full_library_fetch_limit(lib, lvl);
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            if let Ok((items, _)) = client.get_items_sorted(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                0,
                total_count,
                &sort_by,
                &sort_order,
            ) {
                let _ = tx.send(LibEvent::SearchItemsLoaded {
                    lib_idx,
                    parent_id,
                    items,
                });
            }
        });
    }
}
