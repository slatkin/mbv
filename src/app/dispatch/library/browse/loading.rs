use crate::app::state::types::browse::BrowseResting;
use crate::app::{App, BrowseLevel, LibEvent, PAGE_SIZE};
use mbv_core::api::EmbyItem;

fn restored_tv_content_mode(
    is_tv_library: bool,
    library_total: Option<usize>,
    saved_mode: Option<&mbv_core::config::TvContentMode>,
) -> Option<mbv_core::config::TvContentMode> {
    is_tv_library.then(|| {
        crate::app::render::resolve_tv_content_mode(library_total.unwrap_or_default(), saved_mode)
    })
}

fn restored_letter_filter(
    index: Option<usize>,
    kind: crate::app::render::LetterFilterKind,
) -> Option<crate::app::render::LetterFilter> {
    let index = index?;
    if kind == crate::app::render::LetterFilterKind::Tv
        && index >= crate::app::render::LetterFilter::count_for_kind(kind)
    {
        return None;
    }
    crate::app::render::LetterFilter::for_index_for_kind(index, kind)
}

pub(in crate::app) fn retain_grouped_music_items(items: &mut Vec<EmbyItem>, grouped_music: bool) {
    if grouped_music {
        items.retain(|item| !(item.is_folder && item.child_count == Some(0)));
    }
}

pub(in crate::app) fn retain_grouped_music_level_items(
    level: &mut BrowseLevel,
    grouped_music: bool,
) {
    let fetched_rows = level.items.len();
    retain_grouped_music_items(&mut level.items, grouped_music);
    level.fetched_rows = fetched_rows;
    level.resting = BrowseResting::new(
        level
            .resting()
            .cursor()
            .min(level.items.len().saturating_sub(1)),
        level.resting().scroll(),
    );
}

impl App {
    pub(in crate::app) fn ensure_lib_loaded_for(&mut self, idx: usize) {
        if idx >= self.libs.len() {
            return;
        }
        if self.tab.emby_library_index() == Some(idx) && self.is_feed_home_video_library(idx) {
            self.ensure_feed_home_video_root_loaded(idx);
            return;
        }
        if self.libs[idx].nav_stack.is_empty() {
            if let Some(saved) = self.saved_library_position(idx) {
                if let Some(root) = saved.levels.first() {
                    let filter_kind = crate::app::render::LetterFilterKind::from_collection_type(
                        self.libs[idx].library.collection_type.as_str(),
                    );
                    self.libs[idx].library_total = root.library_total;
                    self.libs[idx].tv_content_mode = restored_tv_content_mode(
                        self.libs[idx].library.collection_type == "tvshows",
                        root.library_total,
                        root.tv_content_mode.as_ref(),
                    );
                    let letter_filter =
                        restored_letter_filter(root.letter_filter_index, filter_kind);
                    self.libs[idx].nav_stack.push(BrowseLevel {
                        parent_id: root.parent_id.clone(),
                        title: root.title.clone(),
                        items: Vec::new(),
                        fetched_rows: 0,
                        total_count: 0,
                        resting: BrowseResting::new(0, 0),
                        item_types: root.item_types.clone(),
                        unplayed_only: root.unplayed_only,
                        sort_by: root.sort_by.clone(),
                        sort_order: root.sort_order.clone(),
                        loading: true,

                        all_items: None,
                        letter_filter,
                        tv_content_mode: root.tv_content_mode.clone(),
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
                fetched_rows: 0,
                total_count: 0,
                resting: BrowseResting::new(0, 0),
                item_types: item_types.clone(),
                unplayed_only,
                sort_by: sort_by.into(),
                sort_order: sort_order.into(),
                loading: true,

                all_items: None,
                letter_filter: None,
                tv_content_mode: None,
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

    pub(in crate::app) fn spawn_restore_library_position(
        &self,
        lib_idx: usize,
        saved: crate::config::LibraryPosition,
    ) {
        let visible_rows = self.lib_page_size();
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        let filter_kind = crate::app::render::LetterFilterKind::from_collection_type(
            self.libs[lib_idx].library.collection_type.as_str(),
        );
        std::thread::spawn(move || {
            let restored = crate::app::restore_library_position_with_fetched_rows_for_kind(
                &saved,
                visible_rows,
                filter_kind,
                |saved_level| {
                    let tv_mode =
                        (filter_kind == crate::app::render::LetterFilterKind::Tv).then(|| {
                            crate::app::render::resolve_tv_content_mode(
                                saved_level.library_total.unwrap_or_default(),
                                saved_level.tv_content_mode.as_ref(),
                            )
                        });
                    if matches!(tv_mode, Some(mbv_core::config::TvContentMode::Latest)) {
                        let items = client.get_latest_episodes(&saved_level.parent_id, 30)?;
                        let total_count = items.len();
                        return Ok((items, total_count, total_count));
                    }
                    if matches!(tv_mode, Some(mbv_core::config::TvContentMode::Upcoming)) {
                        let items = client.get_upcoming(&saved_level.parent_id, 30)?;
                        let total_count = items.len();
                        return Ok((items, total_count, total_count));
                    }
                    let letter_filter = match tv_mode {
                        Some(mbv_core::config::TvContentMode::Range(index)) => {
                            crate::app::render::LetterFilter::for_index_for_kind(index, filter_kind)
                        }
                        _ => saved_level.letter_filter_index.and_then(|index| {
                            crate::app::render::LetterFilter::for_index_for_kind(index, filter_kind)
                        }),
                    };
                    let (name_ge, name_lt) = letter_filter
                        .as_ref()
                        .map_or((None, None), |f| (f.name_ge, f.name_lt));
                    let (items, total_count) =
                        client.get_items_sorted_ranged(&mbv_core::api::SortedItemsParams {
                            parent_id: &saved_level.parent_id,
                            item_types: saved_level.item_types.as_deref(),
                            unplayed_only: saved_level.unplayed_only,
                            start_index: 0,
                            limit: PAGE_SIZE,
                            sort_by: &saved_level.sort_by,
                            sort_order: &saved_level.sort_order,
                            name_ge,
                            name_lt,
                        })?;
                    let fetched_rows = items.len();
                    if total_count > fetched_rows {
                        let (items, total_count) =
                            client.get_items_sorted_ranged(&mbv_core::api::SortedItemsParams {
                                parent_id: &saved_level.parent_id,
                                item_types: saved_level.item_types.as_deref(),
                                unplayed_only: saved_level.unplayed_only,
                                start_index: 0,
                                limit: total_count,
                                sort_by: &saved_level.sort_by,
                                sort_order: &saved_level.sort_order,
                                name_ge,
                                name_lt,
                            })?;
                        let fetched_rows = items.len();
                        Ok((items, total_count, fetched_rows))
                    } else {
                        Ok((items, total_count, fetched_rows))
                    }
                },
            );
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

    pub(in crate::app) fn spawn_browse(
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
                    let fetched_rows = items.len();
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
                            fetched_rows,
                            total_count,
                            resting: BrowseResting::new(0, 0),
                            item_types,
                            unplayed_only,
                            sort_by,
                            sort_order,
                            loading: false,

                            all_items: None,
                            letter_filter: None,
                            tv_content_mode: None,
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

    pub(in crate::app) fn spawn_browse_page_sized(
        &self,
        lib_idx: usize,
        start_index: usize,
        limit: usize,
        key: crate::app::state::types::browse::LevelFetchKey,
    ) {
        let crate::app::state::types::browse::LevelFetchKey {
            parent_id,
            item_types,
            unplayed_only,
            sort_by,
            sort_order,
            letter_filter,
        } = key;
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        let (name_ge, name_lt) = letter_filter.map_or((None, None), |f| (f.name_ge, f.name_lt));
        std::thread::spawn(move || {
            match client.get_items_sorted_ranged(&mbv_core::api::SortedItemsParams {
                parent_id: &parent_id,
                item_types: item_types.as_deref(),
                unplayed_only,
                start_index,
                limit,
                sort_by: &sort_by,
                sort_order: &sort_order,
                name_ge,
                name_lt,
            }) {
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
}
