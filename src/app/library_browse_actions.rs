use super::ui_util::sort_episodes;
use super::{AlbumPathPart, AlbumSearchEntry, App, BrowseLevel, LibEvent, LibraryTab, PAGE_SIZE};
use mbv_core::api::EmbyItem;

type BrowseRefresh = (
    usize,
    String,
    Option<String>,
    bool,
    String,
    String,
    usize,
    Option<super::render::LetterFilter>,
);
type AlbumIndexFetch<'a> =
    dyn FnMut(&str, usize, usize) -> Result<(Vec<EmbyItem>, usize), String> + 'a;
const ALBUM_INDEX_PAGE_SIZE: usize = 200;

// Visibility bump: private -> `pub(super)`. Exercised directly by
// `actions_tests.rs` (a submodule of `actions.rs`, so it needs an explicit
// `crate::app::library_browse_actions::...` import once this lives outside
// `actions.rs`'s own namespace).
pub(super) fn recursive_album_search_eligible(collection_type: &str, levels: &[String]) -> bool {
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
// Visibility bump: private -> `pub(super)`. Same reason as
// `recursive_album_search_eligible` above -- exercised directly by
// `actions_tests.rs`.
pub(super) fn full_library_fetch_limit(lib: &LibraryTab, lvl: &BrowseLevel) -> usize {
    lib.library_total
        .unwrap_or(lvl.total_count)
        .max(lvl.total_count)
}

pub(super) fn fetch_all_album_index_items(
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

// Visibility bump: private -> `pub(super)`. Same reason as
// `recursive_album_search_eligible` above -- exercised directly by
// `actions_tests.rs`.
pub(super) fn build_album_index_with(
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
                let mut labels: Vec<String> =
                    ancestors.iter().map(|part| part.name.clone()).collect();
                labels.push(album.display_name());
                let display_label = labels.join(" / ");
                entries.push(AlbumSearchEntry {
                    album,
                    ancestors: ancestors.clone(),
                    search_text: display_label.clone(),
                    display_label,
                });
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
    pub(super) fn ensure_lib_loaded_for(&mut self, idx: usize) {
        if idx >= self.libs.len() {
            return;
        }
        if self.tab.emby_library_index() == Some(idx) && self.is_feed_home_video_library(idx) {
            self.ensure_feed_home_video_root_loaded(idx);
            return;
        }
        if self.tab.emby_library_index() == Some(idx) && self.is_podcast_library(idx) {
            self.ensure_podcast_root_loaded(idx);
            return;
        }
        if self.libs[idx].nav_stack.is_empty() {
            if let Some(saved) = self.saved_library_position(idx) {
                if let Some(root) = saved.levels.first() {
                    self.libs[idx].nav_stack.push(BrowseLevel {
                        parent_id: root.parent_id.clone(),
                        title: root.title.clone(),
                        items: Vec::new(),
                        total_count: 0,
                        cursor: 0,
                        item_types: root.item_types.clone(),
                        unplayed_only: root.unplayed_only,
                        sort_by: root.sort_by.clone(),
                        sort_order: root.sort_order.clone(),
                        loading: true,
                        scroll: 0,
                        all_items: None,
                        letter_filter: None,
                        music_grouping: None,
                    });
                    self.spawn_restore_library_position(idx, saved);
                    return;
                }
            }
            let lib_id = self.libs[idx].library.id.clone();
            let lib_name = self.libs[idx].library.name.clone();
            let is_feed_view = {
                let c = self.config.lock().unwrap();
                c.feed_view_libraries.contains(&lib_name.to_lowercase())
            };
            let (item_types, unplayed_only, sort_by, sort_order) =
                match self.libs[idx].library.collection_type.as_str() {
                    "movies" => (Some("Movie".to_string()), false, "SortName", "Ascending"),
                    "tvshows" => (Some("Series".to_string()), false, "SortName", "Ascending"),
                    _ if is_feed_view => {
                        (Some("Video".to_string()), true, "DateCreated", "Ascending")
                    }
                    _ => (None, false, "SortName", "Ascending"),
                };
            self.libs[idx].nav_stack.push(BrowseLevel {
                parent_id: lib_id.clone(),
                title: lib_name.clone(),
                items: vec![],
                total_count: 0,
                cursor: 0,
                item_types: item_types.clone(),
                unplayed_only,
                sort_by: sort_by.into(),
                sort_order: sort_order.into(),
                loading: true,
                scroll: 0,
                all_items: None,
                letter_filter: None,
                music_grouping: None,
            });
            self.spawn_browse(
                idx,
                lib_id,
                lib_name,
                item_types,
                unplayed_only,
                sort_by.into(),
                sort_order.into(),
            );
        }
    }

    pub(super) fn spawn_restore_library_position(
        &self,
        lib_idx: usize,
        saved: crate::config::LibraryPosition,
    ) {
        let visible_rows = self.lib_page_size();
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let restored = super::restore_library_position(&saved, visible_rows, |saved_level| {
                let letter_filter = saved_level
                    .letter_filter_index
                    .and_then(super::render::LetterFilter::for_index);
                let (name_ge, name_lt) = letter_filter
                    .as_ref()
                    .map(|f| (f.name_ge, f.name_lt))
                    .unwrap_or((None, None));
                let (items, total_count) = client.get_items_sorted_ranged(
                    &saved_level.parent_id,
                    saved_level.item_types.as_deref(),
                    saved_level.unplayed_only,
                    0,
                    PAGE_SIZE,
                    &saved_level.sort_by,
                    &saved_level.sort_order,
                    name_ge,
                    name_lt,
                )?;
                if total_count > items.len() {
                    client.get_items_sorted_ranged(
                        &saved_level.parent_id,
                        saved_level.item_types.as_deref(),
                        saved_level.unplayed_only,
                        0,
                        total_count,
                        &saved_level.sort_by,
                        &saved_level.sort_order,
                        name_ge,
                        name_lt,
                    )
                } else {
                    Ok((items, total_count))
                }
            });
            match restored {
                Ok(Some((position, nav_stack))) => {
                    let _ = tx.send(LibEvent::RestoreLibraryPosition {
                        lib_idx,
                        requested_position: saved,
                        position,
                        nav_stack,
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    pub(super) fn refresh_after_stop(&mut self) {
        let _ = self.fetch_home();
        if self.last_played_completed {
            if let Some(ref item_id) = self.last_played_item_id.clone() {
                for lib_idx in 0..self.libs.len() {
                    if self.is_feed_home_video_group_view(lib_idx)
                        || self.is_feed_home_video_library(lib_idx)
                    {
                        self.remove_item_from_feed_home_video_cache(lib_idx, item_id);
                        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                            state.loading = true;
                        }
                        self.log_feed_home_video_state(lib_idx, "refresh_after_stop_completed");
                    }
                }
            }
        }
        let fetches: Vec<BrowseRefresh> = self
            .libs
            .iter()
            .enumerate()
            .filter_map(|(i, lib)| {
                lib.nav_stack.last().map(|lvl| {
                    (
                        i,
                        lvl.parent_id.clone(),
                        lvl.item_types.clone(),
                        lvl.unplayed_only,
                        lvl.sort_by.clone(),
                        lvl.sort_order.clone(),
                        lvl.items.len(),
                        lvl.letter_filter.clone(),
                    )
                })
            })
            .collect();
        for (
            lib_idx,
            parent_id,
            item_types,
            unplayed_only,
            sort_by,
            sort_order,
            loaded_count,
            letter_filter,
        ) in fetches
        {
            self.spawn_refresh(
                lib_idx,
                parent_id,
                item_types,
                unplayed_only,
                sort_by,
                sort_order,
                loaded_count,
                letter_filter,
            );
        }
    }

    pub(super) fn spawn_browse(
        &self,
        lib_idx: usize,
        parent_id: String,
        title: String,
        item_types: Option<String>,
        unplayed_only: bool,
        sort_by: String,
        sort_order: String,
    ) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        let spawn_started = std::time::Instant::now();
        std::thread::spawn(move || {
            match client.get_items_sorted(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                0,
                PAGE_SIZE,
                &sort_by,
                &sort_order,
            ) {
                Ok((items, total_count)) => {
                    log::info!(target: "browse", "Loaded lib_idx={lib_idx} parent={parent_id} total={total_count} got={} thread_total={}ms first3={:?}",
                        items.len(),
                        spawn_started.elapsed().as_millis(),
                        items.iter().take(3).map(|i| format!("{}:{}", i.id, i.name)).collect::<Vec<_>>());
                    let _ = tx.send(LibEvent::Loaded {
                        lib_idx,
                        parent_id: parent_id.clone(),
                        level: Box::new(BrowseLevel {
                            parent_id,
                            title,
                            items,
                            total_count,
                            cursor: 0,
                            item_types,
                            unplayed_only,
                            sort_by,
                            sort_order,
                            loading: false,
                            scroll: 0,
                            all_items: None,
                            letter_filter: None,
                            music_grouping: None,
                        }),
                    });
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    pub(super) fn spawn_navigate_to_item(
        &self,
        item_id: String,
        item_type: String,
        libs: Vec<(usize, String, String)>,
    ) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            // Match library by collection_type since CollectionFolder IDs never appear in ancestors
            let target_ctype = match item_type.as_str() {
                "Series" | "Episode" | "Season" => "tvshows",
                "Movie" => "movies",
                "Audio" | "MusicAlbum" | "MusicArtist" => "music",
                _ => "",
            };
            let (lib_idx, lib_id) = match libs.iter().find(|(_, _, ctype)| ctype == target_ctype) {
                Some((idx, id, _)) => (*idx, id.clone()),
                None => {
                    let _ = tx.send(LibEvent::Error(
                        "No matching library for this item type".into(),
                    ));
                    return;
                }
            };

            // Ancestors are ordered nearest→root: [Season, Series, physical_folder, AggregateFolder]
            let ancestors = match client.get_ancestors(&item_id) {
                Ok(a) => a,
                Err(e) => {
                    log::error!(target:"navigate", "get_ancestors failed: {e}");
                    let _ = tx.send(LibEvent::Error(e));
                    return;
                }
            };
            log::debug!(target:"navigate", "ancestors: {:?}", ancestors.iter().map(|a| format!("{}({})", a.name, a.id)).collect::<Vec<_>>());

            // Drop the last two ancestors (physical library folder + AggregateFolder root);
            // everything before those is navigable content inside the library.
            let inside = if ancestors.len() >= 2 {
                &ancestors[..ancestors.len() - 2]
            } else {
                &ancestors[..0]
            };

            // Build nav levels: lib_id first, then inside ancestors from root→item, then item itself.
            // inside is nearest→root order; we need root→item, so iterate reversed.
            let mut parents: Vec<String> = vec![lib_id];
            for a in inside.iter().rev() {
                parents.push(a.id.clone());
            }

            // targets[i] is the item we want the cursor on inside parents[i]
            let mut targets: Vec<String> =
                inside.iter().rev().skip(1).map(|a| a.id.clone()).collect();
            if let Some(a) = inside.first() {
                targets.push(a.id.clone());
            } // last inside level → first inside ancestor
            targets.push(item_id.clone()); // deepest level → the item itself

            let mut nav_stack: Vec<BrowseLevel> = Vec::new();
            for (parent_id, target_id) in parents.into_iter().zip(targets) {
                let (mut items, total_count) = match client.get_items_sorted(
                    &parent_id,
                    None,
                    false,
                    0,
                    500,
                    "SortName",
                    "Ascending",
                ) {
                    Ok(x) => x,
                    Err(e) => {
                        let _ = tx.send(LibEvent::Error(e));
                        return;
                    }
                };
                if items
                    .first()
                    .map(|it| it.item_type == "Episode")
                    .unwrap_or(false)
                {
                    sort_episodes(&mut items);
                }
                let cursor = items.iter().position(|it| it.id == target_id).unwrap_or(0);
                log::debug!(target:"navigate", "level parent={parent_id} target={target_id} cursor={cursor}/{}", items.len());
                nav_stack.push(BrowseLevel {
                    parent_id: parent_id.clone(),
                    title: String::new(),
                    items,
                    total_count,
                    cursor,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    scroll: 0,
                    all_items: None,
                    letter_filter: None,
                    music_grouping: None,
                });
            }
            let _ = tx.send(LibEvent::NavigateTo {
                lib_idx,
                nav_stack,
                switch_tab: true,
            });
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn spawn_browse_page(
        &self,
        lib_idx: usize,
        parent_id: String,
        start_index: usize,
        item_types: Option<String>,
        unplayed_only: bool,
        sort_by: String,
        sort_order: String,
        letter_filter: Option<super::render::LetterFilter>,
    ) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        let (name_ge, name_lt) = letter_filter
            .as_ref()
            .map(|f| (f.name_ge, f.name_lt))
            .unwrap_or((None, None));
        std::thread::spawn(move || {
            match client.get_items_sorted_ranged(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                start_index,
                PAGE_SIZE,
                &sort_by,
                &sort_order,
                name_ge,
                name_lt,
            ) {
                Ok((items, total_count)) => {
                    let _ = tx.send(LibEvent::PageAppended {
                        lib_idx,
                        parent_id,
                        items,
                        total_count,
                    });
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    // Visibility bump: private -> `pub(super)`. Called from
    // `handle_lib_loaded`/`handle_lib_page_appended`, which stay behind in
    // `actions.rs`.
    pub(super) fn spawn_all_items_prefetch(&self, lib_idx: usize) {
        let lib = &self.libs[lib_idx];
        let lvl = match lib.nav_stack.last() {
            Some(l) => l,
            None => return,
        };
        // `lvl.is_fully_loaded()` compares `items.len()` against
        // `lvl.total_count` -- with a letter-range pill active, that count
        // is the FILTERED range's total, not the whole library's, so a
        // fully-loaded small range (e.g. 40 items in `A–C`) would wrongly
        // read as "nothing more to prefetch". `all_items` backs whole-library
        // search (see `input.rs`'s `/` handler and `spawn_search_items_load`
        // below), so it must never be satisfied by just the active range.
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
        let tx = self.lib_tx.clone();
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

    pub(super) fn spawn_search_items_load(&self, lib_idx: usize) {
        let lib = &self.libs[lib_idx];
        let lvl = match lib.nav_stack.last() {
            Some(l) => l,
            None => return,
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
        let tx = self.lib_tx.clone();
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
