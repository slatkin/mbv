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
            // effects); the computed content travels into the stored
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

    pub fn spawn_destination_latest_snapshot(&self, lib_idx: usize) {
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

    pub(crate) fn spawn_emby_latest_snapshot(&self, library_id: String) {
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
    pub fn maybe_spawn_tv_upcoming_snapshot(&mut self, lib_idx: usize) {
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
        self.spawn_tv_upcoming_snapshot_fetch(library_id);
    }

    /// Refetch one TV library's Upcoming snapshot (gui-list-header-buttons
    /// design D9): with no snapshot the call is the first load's, which skips
    /// while a fetch runs or one is already under way; with a snapshot the
    /// refetch keeps it in place and starts the fetch, so a second press while
    /// that fetch runs starts nothing. The failure arm
    /// [`Self::tv_upcoming_snapshot_failed`] already keeps the stored snapshot,
    /// so the Shows list keeps its rows and names the failure. A library index
    /// that names no TV library, and a missing Emby client, start nothing at
    /// all.
    pub fn refetch_tv_upcoming_snapshot(&mut self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        if lib.library.collection_type != "tvshows" {
            return;
        }
        let library_id = lib.library.id.clone();
        if !self.tv_upcoming_snapshots.contains_key(&library_id) {
            self.maybe_spawn_tv_upcoming_snapshot(lib_idx);
            return;
        }
        if TvUpcomingFetchState::blocks_request(self.tv_upcoming_fetches.get(&library_id)) {
            return;
        }
        self.spawn_tv_upcoming_snapshot_fetch(library_id);
    }

    /// Start one TV library's Upcoming snapshot fetch (design D9): the spawn
    /// body the first load and the refetch share, so both mark the fetch
    /// `InFlight` and send the same completion events. A missing Emby client
    /// starts nothing and marks nothing, so a later request retries.
    fn spawn_tv_upcoming_snapshot_fetch(&mut self, library_id: String) {
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

    /// The App-side half of an `EmbyLatestSnapshotFetched` completion
    /// (add-gui-library-panel row 7.1): the TV library's Latest root level
    /// fills from the snapshot — the one write every Client's drain reaches
    /// through `handle_model_content_event`, so a Client
    /// that keeps no shell state (the GUI) fills the level too through its
    /// own drain arm. A library whose Latest root is not the level in wait
    /// (no TV library, no root level, another mode) keeps it unchanged.
    pub fn apply_tv_latest_snapshot(&mut self, library_id: &str, items: Vec<EmbyItem>) {
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
        if level.tv_content_mode != Some(mbv_queue::TvContentMode::Latest) {
            return;
        }
        level.items = items;
        level.rows = ServerRows::complete(level.items.len());
        level.loading = false;
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
mod tests;
