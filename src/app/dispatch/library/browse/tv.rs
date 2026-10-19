use crate::app::state::app_struct::TvUpcomingFetchState;
use crate::app::{App, BrowseEvent, BrowseLevel, LibEvent, ModelContentEvent};
use mbv_emby::EmbyClient;
use mbv_emby_model::{EmbyItem, UpcomingPremiere};
use mbv_ui_model::browse::{BrowseResting, ServerRows};

type BrowseRefresh = (
    usize,
    mbv_ui_model::browse::LevelFetchKey,
    usize,
    Option<mbv_queue::TvContentMode>,
);
trait TvLatestSource {
    fn get_latest_episodes(
        &self,
        view_id: &str,
        limit: usize,
    ) -> Result<Vec<EmbyItem>, mbv_emby::EmbyError>;
}

impl TvLatestSource for EmbyClient {
    fn get_latest_episodes(
        &self,
        view_id: &str,
        limit: usize,
    ) -> Result<Vec<EmbyItem>, mbv_emby::EmbyError> {
        EmbyClient::get_latest_episodes(self, view_id, limit)
    }
}

trait TvUpcomingSource {
    fn get_upcoming(
        &self,
        parent_id: &str,
        limit: usize,
    ) -> Result<Vec<UpcomingPremiere>, mbv_emby::EmbyError>;
}

impl TvUpcomingSource for EmbyClient {
    fn get_upcoming(
        &self,
        parent_id: &str,
        limit: usize,
    ) -> Result<Vec<UpcomingPremiere>, mbv_emby::EmbyError> {
        EmbyClient::get_upcoming(self, parent_id, limit)
    }
}

/// Spacing between the Upcoming fetch's retry attempts. The Emby Upcoming
/// query blocks for the whole 30 s agent budget while the server's metadata
/// refresh (TheTVDB/fanart downloads) runs — the query itself needs ~1 s
/// once that work settles (2026-10-15 server log: 30063 ms against a
/// normally-1.24 s endpoint). Delays are injected so tests pay no sleeps.
const UPCOMING_RETRY_DELAYS: [std::time::Duration; 2] = [
    std::time::Duration::from_secs(30),
    std::time::Duration::from_secs(60),
];

/// `get_upcoming` with spaced retries: one attempt per `delays` entry (each
/// failed attempt sleeps its delay before the next), then a final attempt,
/// returning only the final error. An empty slice is a single attempt.
fn fetch_upcoming_with_retries<S: TvUpcomingSource>(
    source: &S,
    parent_id: &str,
    delays: &[std::time::Duration],
) -> Result<Vec<UpcomingPremiere>, mbv_emby::EmbyError> {
    for delay in delays {
        if let Ok(items) = source.get_upcoming(parent_id, 30) {
            return Ok(items);
        }
        std::thread::sleep(*delay);
    }
    source.get_upcoming(parent_id, 30)
}

/// The canonical fetch fields each TV content mode's root level must carry
/// while that mode owns the level. `drop_stale_root_completion` compares a
/// root `Loaded`'s fetch key against the root level's, so the level armed for
/// a pill fetch must be stamped with the same fields the pill's response
/// carries, or the correct response is dropped and replaced by a generic
/// alphabetical all-episodes refetch (seen 2026-10-06: Latest/Upcoming pill
/// content clobbered by `SortName` episode listings).
pub(in crate::app) fn tv_mode_fetch_fields(
    mode: &mbv_queue::TvContentMode,
) -> (Option<&'static str>, &'static str, &'static str) {
    match mode {
        mbv_queue::TvContentMode::Latest => (Some("Episode"), "DateCreated", "Descending"),
        mbv_queue::TvContentMode::Upcoming => (Some("Episode"), "PremiereDate", "Ascending"),
        mbv_queue::TvContentMode::All | mbv_queue::TvContentMode::Range(_) => {
            (Some("Series"), "SortName", "Ascending")
        }
    }
}

/// Stamps a root level with the fetch fields its TV content mode's fetch
/// carries, so an arriving pill response passes the stale-root key check.
pub(in crate::app) fn align_level_to_tv_mode(
    level: &mut crate::app::BrowseLevel,
    mode: &mbv_queue::TvContentMode,
) {
    let (item_types, sort_by, sort_order) = tv_mode_fetch_fields(mode);
    level.item_types = item_types.map(str::to_string);
    level.sort_by = sort_by.to_string();
    level.sort_order = sort_order.to_string();
}

fn build_tv_latest_level<S: TvLatestSource>(
    source: &S,
    parent_id: String,
    title: String,
) -> Result<BrowseLevel, mbv_emby::EmbyError> {
    let items = source.get_latest_episodes(&parent_id, 30)?;
    let total_count = items.len();
    let (item_types, sort_by, sort_order) = tv_mode_fetch_fields(&mbv_queue::TvContentMode::Latest);
    Ok(BrowseLevel {
        parent_id,
        title,
        items,
        rows: ServerRows::complete(total_count),
        resting: BrowseResting::new(0, 0),
        item_types: item_types.map(str::to_string),
        unplayed_only: false,
        sort_by: sort_by.to_string(),
        sort_order: sort_order.to_string(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    })
}

/// The Upcoming level holds no items: premieres are not media, so they travel
/// beside the level and live in `App::tv_upcoming_snapshots`.
fn build_tv_upcoming_level<S: TvUpcomingSource>(
    source: &S,
    parent_id: String,
    title: String,
) -> Result<(BrowseLevel, Vec<UpcomingPremiere>), mbv_emby::EmbyError> {
    let premieres = fetch_upcoming_with_retries(source, &parent_id, &UPCOMING_RETRY_DELAYS)?;
    let total_count = premieres.len();
    let (item_types, sort_by, sort_order) =
        tv_mode_fetch_fields(&mbv_queue::TvContentMode::Upcoming);
    let level = BrowseLevel {
        parent_id,
        title,
        items: Vec::new(),
        rows: ServerRows::complete(total_count),
        resting: BrowseResting::new(0, 0),
        item_types: item_types.map(str::to_string),
        unplayed_only: false,
        sort_by: sort_by.to_string(),
        sort_order: sort_order.to_string(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    };
    Ok((level, premieres))
}

impl App {
    pub(in crate::app) fn refresh_after_stop(&mut self) {
        if let Ok(content) = self.fetch_home() {
            // The fetch runs synchronously (order-sensitive side
            // effects); the computed content travels to Model-owned
            // `home_content` via lib_tx (task 5.3d).
            let _ = self.channels.lib_tx.send(LibEvent::ModelContent(
                ModelContentEvent::HomeContentRefreshed(Box::new(content)),
            ));
        }
        // The feed-view cache stripping that ran when the completed item was
        // a feed-view video is gone (task 4.1, design D4): a playback
        // completion leaves the group view's loaded rows in place.
        let fetches: Vec<BrowseRefresh> = self
            .libs
            .iter()
            .enumerate()
            .filter_map(|(i, lib)| {
                lib.nav_stack.last().map(|lvl| {
                    (
                        i,
                        mbv_ui_model::browse::LevelFetchKey::from_level(lvl),
                        lvl.items.len(),
                        (lib.library.collection_type == "tvshows" && lib.nav_stack.len() == 1)
                            .then(|| {
                                lvl.tv_content_mode
                                    .clone()
                                    .or_else(|| lib.tv_content_mode.clone())
                            })
                            .flatten(),
                    )
                })
            })
            .collect();
        for (lib_idx, key, loaded_count, tv_content_mode) in fetches {
            match tv_content_mode {
                Some(mbv_queue::TvContentMode::Latest) => {
                    self.spawn_tv_latest(
                        lib_idx,
                        key.parent_id.clone(),
                        self.libs[lib_idx].library.name.clone(),
                    );
                }
                Some(mbv_queue::TvContentMode::Upcoming) => {
                    self.spawn_tv_upcoming(
                        lib_idx,
                        key.parent_id.clone(),
                        self.libs[lib_idx].library.name.clone(),
                    );
                }
                Some(mbv_queue::TvContentMode::All | mbv_queue::TvContentMode::Range(_)) | None => {
                    self.spawn_refresh(lib_idx, loaded_count, key);
                }
            }
        }
    }

    /// Build one TV mode root level off-thread and send it as `Loaded`. A build
    /// that also fetched the Upcoming snapshot returns it: it lands before the
    /// level so the projection that follows the level's `Loaded` already has
    /// its rows. An Upcoming build's failure is a snapshot failure (it marks
    /// the fetch state and unsets the waiting level); a Latest build's
    /// failure stays a plain error event.
    fn spawn_tv_content<F>(
        &self,
        lib_idx: usize,
        parent_id: String,
        title: String,
        on_error: fn(String, mbv_emby::EmbyError) -> LibEvent,
        build: F,
    ) where
        F: FnOnce(
                &EmbyClient,
                String,
                String,
            )
                -> Result<(BrowseLevel, Option<Vec<UpcomingPremiere>>), mbv_emby::EmbyError>
            + Send
            + 'static,
    {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || match build(&client, parent_id.clone(), title) {
            Ok((level, premieres)) => {
                if let Some(items) = premieres {
                    let _ = tx.send(LibEvent::ModelContent(
                        ModelContentEvent::TvUpcomingSnapshotFetched {
                            library_id: parent_id.clone(),
                            items,
                        },
                    ));
                }
                let _ = tx.send(LibEvent::Browse(BrowseEvent::Loaded {
                    lib_idx,
                    parent_id,
                    level: Box::new(level),
                }));
            }
            Err(e) => {
                let _ = tx.send(on_error(parent_id, e));
            }
        });
    }

    pub(in crate::app) fn spawn_tv_latest(
        &mut self,
        lib_idx: usize,
        parent_id: String,
        title: String,
    ) {
        if let Some(level) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
        {
            align_level_to_tv_mode(level, &mbv_queue::TvContentMode::Latest);
        }
        self.spawn_tv_content(
            lib_idx,
            parent_id,
            title,
            |_, e| LibEvent::Error(e.to_string()),
            |client, parent_id, title| {
                build_tv_latest_level(client, parent_id, title).map(|level| (level, None))
            },
        );
    }

    pub(in crate::app) fn spawn_destination_latest_snapshot(&self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        if matches!(
            lib.library.collection_type.as_str(),
            "tvshows" | "playlists" | "music"
        ) {
            return;
        }
        self.spawn_emby_latest_snapshot(lib.library.id.clone());
    }

    pub(in crate::app) fn spawn_emby_latest_snapshot(&self, library_id: String) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            let items = client.get_latest(&library_id, 30);
            match items {
                Ok(items) => {
                    let _ = tx.send(LibEvent::ModelContent(
                        ModelContentEvent::EmbyLatestSnapshotFetched { library_id, items },
                    ));
                }
                Err(error) => {
                    let _ = tx.send(LibEvent::Error(error.to_string()));
                }
            }
        });
    }

    /// Request the per-session TV Upcoming snapshot for one TV library
    /// (user rule, 2026-10-11): the eager background fetch runs once when the
    /// TV tab loads, pill selection materializes the root level from it
    /// instead of refetching, and only the focused manual refresh replaces
    /// it. A no-op when the snapshot already exists, the request is in
    /// flight, or an Upcoming top level is still `loading`: a saved position
    /// restoring straight into Upcoming installs that level synchronously and
    /// fetches behind it, and its completion fills the snapshot. A `Failed`
    /// state with no fetch under way re-requests — the current==saved
    /// shortcut suppressed the restore's fetch on that tab load, so this
    /// request is the spec's tab-load retry. Without a Service client this
    /// stays unrequested, so a later tab load retries.
    pub(in crate::app) fn maybe_spawn_tv_upcoming_snapshot(&mut self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        if lib.library.collection_type != "tvshows" {
            return;
        }
        let library_id = lib.library.id.clone();
        if self.tv_upcoming_snapshots.contains_key(&library_id)
            || TvUpcomingFetchState::blocks_request(self.tv_upcoming_fetches.get(&library_id))
        {
            return;
        }
        // A fetch is already under way for an Upcoming top level: a saved
        // position restoring straight into Upcoming installs its `loading`
        // level synchronously and fetches behind it, so a second eager
        // request here would double the server's slowest query. With no
        // fetch under way — the current==saved shortcut suppressed the
        // restore's fetch — a `Failed` state still falls through: this
        // request is the spec's tab-load retry.
        if lib.nav_stack.last().is_some_and(|level| {
            level.loading
                && level.tv_content_mode.as_ref() == Some(&mbv_queue::TvContentMode::Upcoming)
        }) {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        self.tv_upcoming_fetches
            .insert(library_id.clone(), TvUpcomingFetchState::InFlight);
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            match fetch_upcoming_with_retries(&client, &library_id, &UPCOMING_RETRY_DELAYS) {
                Ok(items) => {
                    let _ = tx.send(LibEvent::ModelContent(
                        ModelContentEvent::TvUpcomingSnapshotFetched { library_id, items },
                    ));
                }
                Err(_) => {
                    let _ = tx.send(LibEvent::ModelContent(
                        ModelContentEvent::TvUpcomingSnapshotFailed { library_id },
                    ));
                }
            }
        });
    }

    /// Accept one eager TV Upcoming completion: store the per-session
    /// snapshot and fill an active Upcoming root level that is waiting for
    /// it (spec-aligned empty state when the source has no rows).
    pub(in crate::app) fn accept_tv_upcoming_snapshot(
        &mut self,
        library_id: &str,
        premieres: Vec<UpcomingPremiere>,
    ) {
        let total_count = premieres.len();
        self.tv_upcoming_snapshots
            .insert(library_id.to_string(), premieres);
        self.tv_upcoming_fetches.remove(library_id);
        let Some(lib) = self
            .libs
            .iter_mut()
            .find(|lib| lib.library.id == library_id && lib.library.collection_type == "tvshows")
        else {
            return;
        };
        let Some(level) = lib.nav_stack.last_mut() else {
            return;
        };
        if level.tv_content_mode.as_ref() != Some(&mbv_queue::TvContentMode::Upcoming) {
            return;
        }
        align_level_to_tv_mode(level, &mbv_queue::TvContentMode::Upcoming);
        level.rows = ServerRows::complete(total_count);
        level.loading = false;
    }

    /// The eager TV Upcoming fetch failed after all its retries: mark the
    /// library `Failed` — the panel names the failure and a pill reselect or
    /// later tab load retries — and unset the loading flag of an active
    /// Upcoming root level that was waiting for the fetch.
    pub(in crate::app) fn tv_upcoming_snapshot_failed(&mut self, library_id: &str) {
        self.tv_upcoming_fetches
            .insert(library_id.to_string(), TvUpcomingFetchState::Failed);
        let Some(lib) = self
            .libs
            .iter_mut()
            .find(|lib| lib.library.id == library_id && lib.library.collection_type == "tvshows")
        else {
            return;
        };
        let Some(level) = lib.nav_stack.last_mut() else {
            return;
        };
        if level.tv_content_mode.as_ref() == Some(&mbv_queue::TvContentMode::Upcoming)
            && level.loading
        {
            level.loading = false;
        }
    }

    pub(in crate::app) fn spawn_tv_upcoming(
        &mut self,
        lib_idx: usize,
        parent_id: String,
        title: String,
    ) {
        if let Some(level) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
        {
            align_level_to_tv_mode(level, &mbv_queue::TvContentMode::Upcoming);
        }
        // A fetch is starting: repeated pill selections must not stack a
        // second one behind the first. `spawn_tv_content`'s no-client
        // no-op above this point means the mark only lands on real spawns.
        if self.emby_client().is_some() {
            self.tv_upcoming_fetches
                .insert(parent_id.clone(), TvUpcomingFetchState::InFlight);
        }
        self.spawn_tv_content(
            lib_idx,
            parent_id,
            title,
            |library_id, _| {
                LibEvent::ModelContent(ModelContentEvent::TvUpcomingSnapshotFailed { library_id })
            },
            |client, parent_id, title| {
                build_tv_upcoming_level(client, parent_id, title)
                    .map(|(level, premieres)| (level, Some(premieres)))
            },
        );
    }
}

#[cfg(test)]
mod tv_latest_tests {
    use super::*;
    use crate::app::LibraryTab;
    use rstest::rstest;
    use std::cell::RefCell;

    pub(crate) struct FakeLatestSource {
        pub(crate) request: RefCell<Option<(String, usize)>>,
        pub(crate) items: Vec<EmbyItem>,
    }

    impl TvLatestSource for FakeLatestSource {
        fn get_latest_episodes(
            &self,
            view_id: &str,
            limit: usize,
        ) -> Result<Vec<EmbyItem>, mbv_emby::EmbyError> {
            *self.request.borrow_mut() = Some((view_id.into(), limit));
            Ok(self.items.clone())
        }
    }

    #[test]
    fn latest_library_fetch_uses_home_feed_request_without_home_state() {
        let mut episode = mbv_emby_model::test_support::make_item("Feed episode", "Episode");
        episode.id = "feed-episode".into();
        let source = FakeLatestSource {
            request: RefCell::new(None),
            items: vec![episode.clone()],
        };

        let level = build_tv_latest_level(&source, "view-id".into(), "TV".into())
            .expect("fake Latest request succeeds");

        assert_eq!(
            source.request.borrow().as_ref(),
            Some(&("view-id".into(), 30))
        );
        assert_eq!(level.items, vec![episode]);
        assert_eq!(level.item_types.as_deref(), Some("Episode"));
    }

    #[rstest]
    #[case::latest(mbv_queue::TvContentMode::Latest)]
    fn refresh_after_stop_reloads_selected_tv_mode(#[case] mode: mbv_queue::TvContentMode) {
        let mut app = crate::app::tests::make_app_stub();
        let config = crate::config::Config {
            server_url: "http://127.0.0.1:1".into(),
            ..crate::config::Config::default()
        };
        let http = mbv_net::mock_http::MockHttp::new();
        let client = mbv_emby::EmbyClient::new(config).with_test_agent(http.agent());
        app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(
            std::sync::Arc::new(std::sync::Mutex::new(client)),
        );

        let mut library = mbv_emby_model::test_support::make_item("Shows", "CollectionFolder");
        library.id = "tv-library".into();
        library.collection_type = "tvshows".into();
        let mut level_item = mbv_emby_model::test_support::make_item("Old episode", "Episode");
        level_item.id = "old-episode".into();
        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "tv-library".into(),
                title: "Shows".into(),
                items: vec![level_item],
                rows: ServerRows::complete(1),
                resting: BrowseResting::new(0, 0),
                item_types: Some("Episode".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
                tv_content_mode: Some(mode.clone()),
                music_grouping: None,
            }],
            tv_content_mode: Some(mode),
            ..LibraryTab::new(mbv_emby_model::test_support::make_item(
                "unused",
                "CollectionFolder",
            ))
        });

        // fetch_home makes three requests; the fourth response is the selected
        // TV mode's request.
        http.respond(
            200,
            r#"[{"ItemId":"tv-library","Name":"Shows","CollectionType":"tvshows"}]"#,
        );
        http.respond(200, r#"{"Items":[]}"#);
        http.respond(200, r#"{"Items":[]}"#);
        http.respond(
            200,
            r#"{"Items":[{"Id":"new-episode","Name":"New episode","Type":"Episode"}],"TotalRecordCount":1}"#,
        );

        app.refresh_after_stop();
        let event = loop {
            let event = app
                .channels
                .lib_rx
                .recv()
                .expect("selected TV refresh must complete");
            if let event @ LibEvent::Browse(
                BrowseEvent::Loaded { .. } | BrowseEvent::Refreshed { .. },
            ) = event
            {
                break event;
            }
        };
        match event {
            LibEvent::Browse(BrowseEvent::Loaded { level, .. }) => {
                assert_eq!(level.item_types.as_deref(), Some("Episode"));
                assert_eq!(level.items[0].id, "new-episode");
            }
            LibEvent::Browse(BrowseEvent::Refreshed { .. }) => {
                panic!("Latest/Upcoming stop refresh must not use the generic ranged fetch")
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tv_upcoming_retry_tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::VecDeque;
    use std::time::Duration;

    struct FakeUpcomingSource {
        results: std::cell::RefCell<VecDeque<Result<Vec<UpcomingPremiere>, mbv_emby::EmbyError>>>,
        attempts: Cell<usize>,
    }

    impl TvUpcomingSource for FakeUpcomingSource {
        fn get_upcoming(
            &self,
            _parent_id: &str,
            _limit: usize,
        ) -> Result<Vec<UpcomingPremiere>, mbv_emby::EmbyError> {
            self.attempts.set(self.attempts.get() + 1);
            self.results
                .borrow_mut()
                .pop_front()
                .expect("every retry test scripts one result per attempt")
        }
    }

    fn one_premiere() -> Vec<UpcomingPremiere> {
        vec![UpcomingPremiere {
            series_id: mbv_ids::ItemId::new("series-1"),
            series_name: "Series 1".into(),
            season: Some(1),
            episode: Some(1),
            title: "Premiere".into(),
            premiere_date: "2026-10-12T00:00:00.0000000Z".into(),
        }]
    }

    /// The eager Upcoming fetch retries on failure so a server-side metadata
    /// refresh blocking one attempt (2026-10-15: 30063 ms server log) does
    /// not leave the session snapshot empty; zero-length delays keep the
    /// test sleep-free.
    #[test]
    fn upcoming_fetch_retries_a_failed_attempt() {
        let source = FakeUpcomingSource {
            results: std::cell::RefCell::new(VecDeque::from([Err(error()), Ok(one_premiere())])),
            attempts: Cell::new(0),
        };

        let items = fetch_upcoming_with_retries(&source, "lib-1", &[Duration::ZERO])
            .expect("the second attempt succeeds");

        assert_eq!(items.len(), 1);
        assert_eq!(source.attempts.get(), 2);
    }

    #[test]
    fn upcoming_fetch_stops_after_the_final_attempt() {
        let source = FakeUpcomingSource {
            results: std::cell::RefCell::new(VecDeque::from([Err(error())])),
            attempts: Cell::new(0),
        };

        let Err(error) = fetch_upcoming_with_retries(&source, "lib-1", &[]) else {
            panic!("an exhausted retry must return the final error");
        };
        assert_eq!(
            source.attempts.get(),
            1,
            "an empty delay slice is one attempt"
        );
        assert_eq!(error.kind_name(), "emby.resolve");
    }

    fn error() -> mbv_emby::EmbyError {
        mbv_emby::EmbyError::resolve("test failure")
    }
}

#[cfg(test)]
mod tv_pill_stale_key_tests {
    use super::tv_latest_tests::FakeLatestSource;
    use super::*;
    use crate::app::LibraryTab;
    use std::cell::RefCell;

    /// Regression (2026-10-06): a TV root armed for a pill fetch kept its
    /// legacy `SortName` fetch key, so `drop_stale_root_completion` discarded
    /// the pill's own `DateCreated`/`PremiereDate` response and re-issued a
    /// generic alphabetical all-episodes fetch that replaced the Latest and
    /// Upcoming pill content with old episodes.
    #[test]
    fn pill_response_arriving_after_pill_spawn_is_not_dropped_as_stale() {
        let mut app = crate::app::tests::make_app_stub();
        let config = crate::config::Config {
            server_url: "http://127.0.0.1:1".into(),
            ..crate::config::Config::default()
        };
        let http = mbv_net::mock_http::MockHttp::new();
        let client = mbv_emby::EmbyClient::new(config).with_test_agent(http.agent());
        app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(
            std::sync::Arc::new(std::sync::Mutex::new(client)),
        );

        let mut library = mbv_emby_model::test_support::make_item("Shows", "CollectionFolder");
        library.id = "tv-library".into();
        library.collection_type = "tvshows".into();
        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "tv-library".into(),
                title: "Shows".into(),
                items: vec![],
                rows: ServerRows::new(0),
                resting: BrowseResting::new(0, 0),
                item_types: Some("Episode".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: true,
                all_items: None,
                letter_filter: None,
                tv_content_mode: Some(mbv_queue::TvContentMode::Latest),
                music_grouping: None,
            }],
            tv_content_mode: Some(mbv_queue::TvContentMode::Latest),
            // A captured total keeps the first-capture default-pill refetch
            // out of this stale-key contract.
            library_total: Some(1),
            ..LibraryTab::new(mbv_emby_model::test_support::make_item(
                "unused",
                "CollectionFolder",
            ))
        });

        // Arming the pill fetch aligns the root level's fetch fields with the
        // fields the pill's response carries.
        http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#);
        app.spawn_tv_latest(0, "tv-library".into(), "TV".into());
        let level = &app.libs[0].nav_stack[0];
        assert_eq!(level.sort_by, "DateCreated");
        assert_eq!(level.sort_order, "Descending");

        // A pill response — here a duplicate of the spawn's own completion,
        // as happens when two pill fetches race — must apply, not be dropped
        // and replaced by a generic refetch.
        let mut episode = mbv_emby_model::test_support::make_item("New episode", "Episode");
        episode.id = "new-episode".into();
        let source = FakeLatestSource {
            request: RefCell::new(None),
            items: vec![episode.clone()],
        };
        let level = build_tv_latest_level(&source, "tv-library".into(), "TV".into())
            .expect("fake Latest request succeeds");
        app.handle_lib_event(LibEvent::Browse(BrowseEvent::Loaded {
            lib_idx: 0,
            parent_id: "tv-library".into(),
            level: Box::new(level),
        }));

        let level = &app.libs[0].nav_stack[0];
        assert_eq!(level.items, vec![episode]);
        assert!(!level.loading);
    }
}
