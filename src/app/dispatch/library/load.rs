use crate::app::state::playback::HomeContent;
use crate::app::{
    App, BrowseLevel, FeedHomeVideoState, LibEvent, TabSelection, dispatch::notify::ToastSeverity,
};
use crate::app::{ModelContentEvent, PlaylistEvent};
use mbv_emby_model::EmbyItem;
use std::collections::HashMap;

/// Binds one podcast library's `Newest Episodes` shelf catalogs to queue
/// items. The shelf fetch path builds no progress-resumed state, so each
/// episode starts not-started; the browse state and the Player owner own
/// live progress.
fn home_shelf_items(
    catalogs: Vec<mbv_queue::AudiobookshelfEpisodeCatalog>,
) -> Vec<mbv_queue::QueueItem> {
    catalogs
        .into_iter()
        .map(|catalog| {
            mbv_queue::QueueItem::Audiobookshelf(mbv_queue::AudiobookshelfItem::Episode(
                mbv_queue::AudiobookshelfQueueItem::from_catalog(
                    catalog,
                    mbv_queue::EpisodeResume::NOT_STARTED,
                ),
            ))
        })
        .collect()
}

/// Whether one Emby library's Latest items belong in the Home `Recently added`
/// root (design D3 of `home-recently-added-tree`): the destination's own
/// Latest rules minus Music and Playlists, with TV included (it fetches
/// through `get_latest_episodes`). Only `tvshows` uses the episode query; the
/// `tvshows | playlists | music` guard in `spawn_destination_latest_snapshot`
/// is the separate library-snapshot guard and stays unchanged.
fn home_emby_latest_eligible(collection_type: &str) -> bool {
    !matches!(collection_type, "music" | "playlists")
}

impl App {
    pub(in crate::app) fn refresh_lib(&mut self, lib_idx: usize) {
        // Defensive bounds check: the dispatch front door normalizes a stale
        // destination first, but async Service removal can invalidate the
        // matched index between normalization and this call. No-op (never
        // substitute library zero) on a miss.
        if lib_idx >= self.libs.len() {
            return;
        }
        self.start_album_index(lib_idx, true);
        if self.is_feed_home_video_group_view(lib_idx)
            && let Some(state) = self.libs[lib_idx].feed_home_video.as_mut()
        {
            state.loading = true;
        }
        self.log_feed_home_video_state(lib_idx, "refresh_lib_before_spawn");
        if self.libs[lib_idx].library.collection_type != "tvshows"
            && self.libs[lib_idx].library.collection_type != "playlists"
        {
            self.spawn_emby_latest_snapshot(
                self.libs[lib_idx].library.id.clone(),
                self.libs[lib_idx].library.name.clone(),
            );
        }
        if self.libs[lib_idx].library.collection_type == "tvshows"
            && self.libs[lib_idx].nav_stack.len() == 1
            && (self.libs[lib_idx].tv_content_mode == Some(mbv_queue::TvContentMode::Latest)
                || self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .and_then(|level| level.tv_content_mode.as_ref())
                    == Some(&mbv_queue::TvContentMode::Latest))
        {
            let parent_id = self.libs[lib_idx]
                .nav_stack
                .last()
                .map(|level| level.parent_id.clone());
            if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
                level.loading = true;
            }
            if let Some(parent_id) = parent_id {
                let title = self.libs[lib_idx].library.name.clone();
                self.spawn_tv_latest(lib_idx, parent_id, title);
            }
            return;
        }
        if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            lvl.loading = true;
            let key = mbv_ui_model::browse::LevelFetchKey::from_level(lvl);
            let loaded_count = lvl.items.len();
            self.spawn_refresh(lib_idx, loaded_count, key);
        }
    }

    /// Refresh the selected browse destination (Home, an Emby library, an
    /// Audiobookshelf library, or Feeds) regardless of Panel focus or whether
    /// the Library panel is painted. A stale selected library index normalizes
    /// to Home and stops without a fetch. This is a data refresh only: it never
    /// resets presentation state, writes preferences, or clears a saved
    /// library position (#745). `podcast_latest_selected` is the active
    /// podcast owner's pill answer: the destination Latest shelf is fetched
    /// only while that pill is selected (design D4).
    pub(in crate::app) fn refresh_current_view(&mut self, podcast_latest_selected: bool) {
        self.force_clear = true;
        if self.normalize_stale_browse_destination() {
            return;
        }
        match self.tab {
            TabSelection::Home => {
                // A Home refresh refetches both Services' `Recently added`
                // data in the background (design D3 of
                // `home-recently-added-tree`). A failing Service contributes
                // nothing; Continue Watching still refreshes below.
                self.spawn_home_recently_added(mbv_queue::ServiceKind::Emby);
                self.spawn_home_recently_added(mbv_queue::ServiceKind::Audiobookshelf);
                match self.fetch_home() {
                    Ok(content) => {
                        // The fetch runs synchronously (its App-side
                        // side effects are order-sensitive); the
                        // computed content travels to Model-owned
                        // `home_content` via lib_tx (task 5.3d).
                        let _ = self.channels.lib_tx.send(LibEvent::ModelContent(
                            ModelContentEvent::HomeContentRefreshed(Box::new(content)),
                        ));
                    }
                    Err(e) => {
                        self.flash(format!("Refresh error: {e}"), ToastSeverity::Error);
                    }
                }
            }
            TabSelection::EmbyLibrary(lib_idx) => self.refresh_lib(lib_idx),
            TabSelection::AudiobookshelfLibrary(index) => {
                match self.audiobookshelf_kind_at(index) {
                    Some(mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Book) => {
                        self.audiobookshelf_book_refresh();
                    }
                    _ => self.audiobookshelf_refresh(podcast_latest_selected),
                }
            }
            TabSelection::Feeds => self.refresh_feeds(),
        }
    }

    pub(in crate::app) fn spawn_load_playlists(&mut self) {
        if self.playlists_loading {
            return;
        }
        self.playlists_loading = true;
        let Some(client) = self.emby_snapshot() else {
            self.playlists_loading = false;
            return;
        };
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || match client.get_playlists() {
            Ok(items) => {
                let _ = tx.send(LibEvent::Playlist(PlaylistEvent::ListLoaded(items)));
            }
            Err(e) => {
                let _ = tx.send(LibEvent::Playlist(PlaylistEvent::ListLoadError(format!(
                    "Playlist list failed: {e}"
                ))));
            }
        });
    }

    pub(in crate::app) fn spawn_rename_playlist(&mut self, playlist_id: String, new_name: String) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = client.rename_playlist(&playlist_id, &new_name) {
                let _ = tx.send(LibEvent::Error(format!("Rename failed: {e}")));
            } else {
                let _ = tx.send(LibEvent::Playlist(PlaylistEvent::Renamed { new_name }));
            }
            match client.get_playlists() {
                Ok(items) => {
                    let _ = tx.send(LibEvent::Playlist(PlaylistEvent::ListLoaded(items)));
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e.to_string()));
                }
            }
        });
    }

    pub(in crate::app) fn spawn_delete_playlist(&mut self, playlist_id: String, name: String) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = client.delete_playlist(&playlist_id) {
                let _ = tx.send(LibEvent::Error(format!("Delete failed: {e}")));
            } else {
                let _ = tx.send(LibEvent::Playlist(PlaylistEvent::Deleted { name }));
            }
            match client.get_playlists() {
                Ok(items) => {
                    let _ = tx.send(LibEvent::Playlist(PlaylistEvent::ListLoaded(items)));
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e.to_string()));
                }
            }
        });
    }

    pub(in crate::app) fn spawn_open_playlist(&mut self, playlist: &EmbyItem) {
        if self.playlists_open_loading {
            return;
        }
        self.playlists_open_loading = true;
        self.playlists_open = Some(playlist.clone());
        self.playlists_open_items = Vec::new();
        let Some(client) = self.emby_snapshot() else {
            self.playlists_open_loading = false;
            return;
        };
        let tx = self.channels.lib_tx.clone();
        let playlist_id = playlist.id.clone();
        std::thread::spawn(move || match client.get_playlist_items(&playlist_id) {
            Ok(items) => {
                let _ = tx.send(LibEvent::Playlist(PlaylistEvent::ItemsLoaded {
                    playlist_id,
                    items,
                }));
            }
            Err(e) => {
                let _ = tx.send(LibEvent::Playlist(PlaylistEvent::ItemsLoadError {
                    playlist_id,
                    error: format!("Playlist load failed: {e}"),
                }));
            }
        });
    }

    /// Fetches a playlist's items synchronously and returns the playable
    /// (non-folder) ones. Flashes and returns `None` when Emby is unavailable,
    /// the fetch fails, the playlist is empty, or nothing in it is playable.
    pub(in crate::app) fn playlist_playable_items(
        &mut self,
        playlist_id: &str,
    ) -> Option<Vec<EmbyItem>> {
        let Some(client) = self.emby_snapshot() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return None;
        };
        let items = match client.get_playlist_items(playlist_id) {
            Ok(r) => r,
            Err(e) => {
                self.flash(format!("Playlist load failed: {e}"), ToastSeverity::Error);
                return None;
            }
        };
        if items.is_empty() {
            self.flash("Playlist is empty".into(), ToastSeverity::Error);
            return None;
        }
        let playable: Vec<EmbyItem> = items.into_iter().filter(|i| !i.is_folder).collect();
        if playable.is_empty() {
            self.flash("No playable items in playlist".into(), ToastSeverity::Error);
            return None;
        }
        Some(playable)
    }

    pub(in crate::app) fn rebuild_library_tabs_from_views(&mut self, all_views: &[EmbyItem]) {
        // Drain existing libs, preserving nav stacks and scroll pos so that a
        // UserDataChanged websocket refresh (fired when playback starts)
        // doesn't silently reset list scroll position.
        struct SavedLibState {
            nav_stack: Vec<BrowseLevel>,
            feed_home_video: Option<FeedHomeVideoState>,
            library_total: Option<usize>,
            tv_content_mode: Option<mbv_queue::TvContentMode>,
        }
        let old_libs: HashMap<String, SavedLibState> = self
            .libs
            .drain(..)
            .map(|mut l| {
                (
                    l.library.id.clone(),
                    SavedLibState {
                        nav_stack: std::mem::take(&mut l.nav_stack),
                        feed_home_video: l.feed_home_video,
                        library_total: l.library_total,
                        tv_content_mode: l.tv_content_mode,
                    },
                )
            })
            .collect();

        for view in all_views.iter().filter(|v| {
            v.collection_type != "playlists"
                && !self.hidden_libraries.contains(&v.name.to_lowercase())
        }) {
            let saved = old_libs.get(&view.id);
            let stack = saved
                .map(|s| {
                    s.nav_stack
                        .iter()
                        .map(|lvl| BrowseLevel {
                            parent_id: lvl.parent_id.clone(),
                            title: lvl.title.clone(),
                            items: lvl.items.clone(),
                            rows: lvl.rows,
                            item_types: lvl.item_types.clone(),
                            unplayed_only: lvl.unplayed_only,
                            sort_by: lvl.sort_by.clone(),
                            sort_order: lvl.sort_order.clone(),
                            loading: false,
                            resting: lvl.resting(),
                            all_items: lvl.all_items.clone(),
                            letter_filter: lvl.letter_filter.clone(),
                            tv_content_mode: lvl.tv_content_mode.clone(),
                            music_grouping: lvl.music_grouping.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let feed_home_video = saved.and_then(|s| s.feed_home_video.clone());
            let library_total = saved.and_then(|s| s.library_total);
            let tv_content_mode = (view.collection_type == "tvshows").then(|| {
                mbv_ui_model::sort_filter::resolve_tv_content_mode(
                    library_total.unwrap_or_default(),
                    saved.and_then(|state| state.tv_content_mode.as_ref()),
                )
            });
            self.libs.push(crate::app::LibraryTab {
                nav_stack: stack,
                feed_home_video,
                library_total,
                tv_content_mode,
                ..crate::app::LibraryTab::new(view.clone())
            });
        }

        self.resolve_launch_service_tab(mbv_queue::ServiceKind::Emby);
    }

    /// Compute the Home Continue Watching snapshot. Continue Watching and
    /// the library catalog are fetched only when Emby is configured and connected.
    /// Without one it is skipped -- not an error -- so Home still populates
    /// from whatever local Sources exist (#543 Part 1). Returns the
    /// computed `HomeContent` instead of writing deleted `App.home`; the
    /// shell assigns it to `Model.home_content` (directly for shell-side
    /// callers, via `LibEvent::ModelContent(ModelContentEvent::HomeContentRefreshed)` for App-internal ones)
    /// and preserves the Continue Watching column cursor at the assignment.
    pub(in crate::app) fn fetch_home(&mut self) -> Result<HomeContent, mbv_emby::EmbyFailure> {
        let mut emby_fetched = false;
        let (continue_items, all_views) = match self.emby_client() {
            Some(client) => {
                emby_fetched = true;
                let client = client.lock().unwrap();
                let views = match client.get_views_classified() {
                    Ok(views) => views,
                    Err(error) => {
                        drop(client);
                        self.handle_emby_runtime_failure(error.clone());
                        return Err(error);
                    }
                };
                (client.get_continue_watching(10).unwrap_or_default(), views)
            }
            _ => (Vec::new(), Vec::new()),
        };

        // Library tabs are Emby-modeled state; only rebuild them from the
        // freshly fetched views when Emby was actually reachable, so a broken
        // or absent Emby does not clear existing library tabs.
        if emby_fetched {
            self.rebuild_library_tabs_from_views(&all_views);
            for lib_idx in 0..self.libs.len() {
                self.start_album_index(lib_idx, false);
            }
        }

        Ok(HomeContent {
            continue_items,
            loading: false,
        })
    }

    /// The `Newest Episodes` shelf's catalog entries, or an empty list when
    /// the shelf is absent (only that shelf feeds Home).
    pub(in crate::app) fn newest_episodes_items(
        shelves: Vec<mbv_audiobookshelf::AudiobookshelfShelf>,
    ) -> Vec<mbv_queue::AudiobookshelfEpisodeCatalog> {
        shelves
            .into_iter()
            .find(|shelf| shelf.label.eq_ignore_ascii_case("Newest episodes"))
            .map(|shelf| {
                shelf
                    .entries
                    .into_iter()
                    .filter_map(|entry| match entry {
                        mbv_audiobookshelf::AudiobookshelfShelfEntry::Episode(item) => Some(item),
                        mbv_audiobookshelf::AudiobookshelfShelfEntry::Show(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Dispatches one Service's Home Recently added fetch (design D3 of
    /// `home-recently-added-tree`). Triggered when a Service becomes ready,
    /// in the Emby bootstrap, in `start_audiobookshelf_catalog_fetches`, and
    /// on every Home refresh.
    pub(in crate::app) fn spawn_home_recently_added(&self, service: mbv_queue::ServiceKind) {
        match service {
            mbv_queue::ServiceKind::Emby => self.spawn_home_emby_recently_added(),
            mbv_queue::ServiceKind::Audiobookshelf => {
                self.spawn_home_audiobookshelf_recently_added();
            }
        }
    }

    /// Emby's Home fetch: one background thread per Service, one event per
    /// eligible library — TV through `get_latest_episodes`, every other
    /// eligible library through `get_latest`. A failing library logs and
    /// contributes nothing; the other libraries still land.
    fn spawn_home_emby_recently_added(&self) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let generation = self.emby_runtime.generation();
        let libraries: Vec<(String, bool)> = self
            .libs
            .iter()
            .filter(|library| home_emby_latest_eligible(&library.library.collection_type))
            .map(|library| {
                (
                    library.library.id.clone(),
                    library.library.collection_type == "tvshows",
                )
            })
            .collect();
        if libraries.is_empty() {
            return;
        }
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            for (library_id, tvshows) in libraries {
                let latest = if tvshows {
                    client.get_latest_episodes(&library_id, 30)
                } else {
                    client.get_latest(&library_id, 30)
                };
                match latest {
                    Ok(items) => {
                        let _ = tx.send(LibEvent::ModelContent(
                            ModelContentEvent::HomeRecentlyAddedFetched {
                                source: crate::app::DestinationLatestSource::Emby(
                                    library_id.clone(),
                                ),
                                generation,
                                items: items
                                    .into_iter()
                                    .map(|item| mbv_queue::QueueItem::Emby(Box::new(item)))
                                    .collect(),
                            },
                        ));
                    }
                    Err(error) => {
                        tracing::warn!(
                            name: "home.emby.recently_added_fetch_failed",
                            target: "home",
                            library_id = %library_id,
                            error = %error,
                            "Home Recently added fetch failed"
                        );
                    }
                }
            }
        });
    }

    /// Audiobookshelf's Home fetch: every podcast library's `Newest Episodes`
    /// shelf, bound to queue items. The shelf path builds no progress-resumed
    /// state, so each episode starts not-started; the browse state and the
    /// Player owner own live progress.
    fn spawn_home_audiobookshelf_recently_added(&self) {
        let generation = self.audiobookshelf_runtime.generation();
        let config = self.config.lock().unwrap().clone();
        let Ok((client, key)) =
            crate::app::dispatch::session::service_startup::audiobookshelf_client(&config)
        else {
            return;
        };
        let libraries: Vec<String> = self
            .audiobookshelf_libraries
            .iter()
            .filter(|library| {
                mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::from_media_type(
                    &library.media_type,
                ) == mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Podcast
            })
            .map(|library| library.id.clone())
            .collect();
        if libraries.is_empty() {
            return;
        }
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            for library_id in libraries {
                let shelves = client
                    .shelves_bounded(
                        &key,
                        &library_id,
                        mbv_audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
                    )
                    .map(App::newest_episodes_items)
                    .map(home_shelf_items);
                match shelves {
                    Ok(items) => {
                        let _ = tx.send(LibEvent::ModelContent(
                            ModelContentEvent::HomeRecentlyAddedFetched {
                                source: crate::app::DestinationLatestSource::Audiobookshelf(
                                    library_id,
                                ),
                                generation,
                                items,
                            },
                        ));
                    }
                    Err(error) => {
                        tracing::warn!(
                            name: "home.audiobookshelf.recently_added_fetch_failed",
                            target: "home",
                            library_id = %library_id,
                            error = %error,
                            "Home Recently added fetch failed"
                        );
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod home_recently_added_tests {
    use super::*;
    use crate::app::tests::make_app_stub;
    use crate::app::{DestinationLatestSource, LibraryTab};
    use mbv_net::mock_http::MockHttp;

    fn library_of(id: &str, name: &str, collection_type: &str) -> LibraryTab {
        let mut item = mbv_emby_model::test_support::make_item(name, "CollectionFolder");
        item.id = id.into();
        item.collection_type = collection_type.into();
        LibraryTab::new(item)
    }

    fn ready_emby_app(http: &MockHttp) -> crate::app::App {
        let mut app = crate::app::tests::make_app_stub();
        let config = crate::config::Config {
            server_url: "http://127.0.0.1:1".into(),
            ..crate::config::Config::default()
        };
        let client = mbv_emby::EmbyClient::new(config).with_test_agent(http.agent());
        app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(
            std::sync::Arc::new(std::sync::Mutex::new(client)),
        );
        app
    }

    fn model_content_events(app: &crate::app::App, expected: usize) -> Vec<ModelContentEvent> {
        (0..expected)
            .map(|_| match app.channels.lib_rx.recv().expect("fetch event") {
                LibEvent::ModelContent(event) => match event {
                    ModelContentEvent::HomeRecentlyAddedFetched { .. } => event,
                    other => panic!("unexpected ModelContentEvent: {other:?}"),
                },
                other => panic!("unexpected LibEvent: {other:?}"),
            })
            .collect()
    }

    /// Design D3 (`home-recently-added-tree`): Home's Emby fetch keeps each
    /// destination's own Latest rule — a TV library sends the episode query,
    /// every other eligible library sends `get_latest` — and an ineligible
    /// Music or Playlists library contributes no request at all.
    #[test]
    fn home_emby_fetch_uses_destination_latest_rules_per_eligible_library() {
        let http = MockHttp::new();
        let mut app = ready_emby_app(&http);
        app.libs = vec![
            library_of("lib-tv", "Shows", "tvshows"),
            library_of("lib-movies", "Movies", "movies"),
            library_of("lib-music", "Music", "music"),
            library_of("lib-playlists", "Playlists", "playlists"),
        ];
        // The eligible libraries' responses in send order: TV first, then
        // Movies; music and playlists get none.
        http.respond(
            200,
            r#"{"Items":[{"Id":"ep-1","Name":"Episode","Type":"Episode"}],"TotalRecordCount":1}"#,
        );
        http.respond(200, r#"[{"Id":"movie-1","Name":"Movie","Type":"Movie"}]"#);

        app.spawn_home_recently_added(mbv_queue::ServiceKind::Emby);
        let events = model_content_events(&app, 2);

        let ids_of = |event: &ModelContentEvent| match event {
            ModelContentEvent::HomeRecentlyAddedFetched { source, items, .. } => (
                source.clone(),
                items
                    .iter()
                    .filter_map(|item| item.as_emby())
                    .map(|item| item.id.clone())
                    .collect::<Vec<_>>(),
            ),
            _ => unreachable!(),
        };
        assert_eq!(
            ids_of(&events[0]),
            (
                DestinationLatestSource::Emby("lib-tv".into()),
                vec!["ep-1".into()]
            )
        );
        assert_eq!(
            ids_of(&events[1]),
            (
                DestinationLatestSource::Emby("lib-movies".into()),
                vec!["movie-1".into()]
            )
        );
        let requests = http.requests();
        assert_eq!(requests.len(), 2);
        assert!(
            requests[0].contains("IncludeItemTypes=Episode"),
            "TV must use the episode query: {}",
            requests[0]
        );
        assert!(
            requests[1].contains("GroupItems=true"),
            "non-TV must use get_latest: {}",
            requests[1]
        );
    }

    /// Spec ("Recently added loads at launch independent of destinations",
    /// "One Service is down"): an unconfigured Audiobookshelf contributes
    /// nothing, without a network call and without blocking the others.
    #[test]
    fn home_audiobookshelf_fetch_without_setup_contributes_nothing() {
        let app = make_app_stub();

        app.spawn_home_recently_added(mbv_queue::ServiceKind::Audiobookshelf);

        app.channels.lib_rx.try_recv().unwrap_err();
    }
}
