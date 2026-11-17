//! The GUI Library's plain Emby-library data path (add-gui-library-panel
//! design D12, row 8.1): the per-library selection the core owns (the picked
//! row, and a feed-view library's checked folder set — gui-collection-dropdown
//! design D2, D3), and the open, select, collection-toggle, All, and date-order
//! actions the core's commands apply. A feed-view library's checked set reads and
//! fills the shared `feed_home_video` cache without writing its `selection`,
//! so the TUI's own pill selection stays the TUI's. The root-list store
//! follows the Music collection store's precedent (design D5): the TUI's root
//! path cannot serve a full flat list — a library over `LIBRARY_PILL_THRESHOLD`
//! is bucketed by letter range and a smaller one pages as its cursor moves —
//! so the store fetches the whole root list with `get_items_sorted`, paged to
//! completion, outside `nav_stack`. Every action runs the TUI's existing
//! loaders, so the two Clients cannot drift. The TUI never reads any of it.

use mbv_emby_model::EmbyItem;

use super::collection::{GuiCollectionSet, GuiDateOrder};
use crate::app::state::events::LibEvent;
use crate::app::{App, FeedHomeVideoSelection};

/// One plain Emby library's root list in the store (row 8.1): a library's
/// first open starts its fetch, a loaded list lives for the session, and a
/// failed list stays failed until the library is opened again.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum GuiEmbyRootItems {
    /// The root-list fetch is in flight.
    Loading,
    /// The whole root list, paged to completion, in the server's `SortName`
    /// order.
    Loaded(Vec<EmbyItem>),
    /// The root-list fetch failed.
    Failed,
}

/// The completion of one plain Emby library's root-list fetch, carried on
/// the shared library event channel. Only the GUI Client's root-list store
/// starts one, so the TUI never receives it.
#[derive(Debug)]
pub(in crate::app) struct GuiEmbyRootItemsEvent {
    pub(in crate::app) lib_idx: usize,
    pub(in crate::app) result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
}

/// The GUI Library's plain Emby-library selection (row 8.1): per library,
/// the row the Hero shows, by stable Service id. A feed-view library's
/// checked folder set is the GUI's own store beside it
/// ([`App::gui_emby_feed_effective_set`](crate::app::App)), read over the
/// library tab's shared `feed_home_video` cache without writing its
/// `selection` — the TUI's selection stays the TUI's. A stored row the active
/// list no longer carries falls back to the list's first row in the
/// projection, as every selection's stale arm does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiEmbyLibrarySelection {
    /// The row the Hero shows. `None` before a select command names one.
    pub picked_row: Option<String>,
}

impl App {
    /// Whether `lib_idx` names a plain Emby library destination (row 8.1):
    /// every GUI Emby-library method's destination guard — an Emby library
    /// that is neither a Grouped Music library nor a TV (Shows) library.
    pub(in crate::app::gui_library) fn gui_emby_library(&self, lib_idx: usize) -> bool {
        self.libs.get(lib_idx).is_some_and(|lib| {
            !self.is_grouped_music_library(lib_idx) && lib.library.collection_type != "tvshows"
        })
    }

    /// Open the plain Emby library `lib_idx` (row 8.1): the root list loads
    /// through the TUI's own activation — the tab settlement's
    /// `activate_library_position`, which starts the root listing or the
    /// saved-position restore — and the root-list store's fetch starts beside
    /// it, because the TUI path cannot serve the full flat list the letter
    /// rail reads (the module doc). A feed-view library's root is the folder
    /// listing the feed group view already owns; its checked folders' items
    /// load through [`App::ensure_gui_emby_selection`], the frame path's
    /// shown-pass trigger. A `lib_idx` that names no plain Emby library is an
    /// ignored command.
    pub fn gui_emby_open(&mut self, lib_idx: usize) {
        if !self.gui_emby_library(lib_idx) {
            Self::log_ignored_library_command(
                "embyOpen",
                format_args!("the destination is no plain Emby library"),
            );
            return;
        }
        if self.libs[lib_idx].nav_stack.is_empty() {
            self.activate_library_position(lib_idx);
        }
        if !self.is_feed_home_video_library(lib_idx) {
            // A failed list retries on an open (the failure "stays failed
            // until the library is opened again"), so the open clears the
            // failed mark the frame path's fetch would otherwise honour.
            if matches!(
                self.gui_emby_roots.get(&lib_idx),
                Some(GuiEmbyRootItems::Failed)
            ) {
                self.gui_emby_roots.remove(&lib_idx);
            }
            self.fetch_gui_emby_root_items(lib_idx);
        }
    }

    /// Select the row the Hero shows (row 8.1): the selection the core owns,
    /// so the frame carries it back. A plain library's row must be one the
    /// root-list store holds — or one whose list is still landing, which the
    /// list is what would name stale — and a feed-view library's row must be
    /// one the merged checked-folder list holds. A stale frame's id changes
    /// nothing, as every item-named command's stale arm does.
    pub fn gui_emby_select(&mut self, lib_idx: usize, item_id: &str) {
        if !self.gui_emby_library(lib_idx) {
            Self::log_ignored_library_command(
                "embySelect",
                format_args!("the destination is no plain Emby library"),
            );
            return;
        }
        if !self.gui_emby_row_known(lib_idx, item_id) {
            Self::log_stale_library_item("embySelect", item_id);
            return;
        }
        let unchanged = self
            .gui_emby_libs
            .get(&lib_idx)
            .and_then(|selection| selection.picked_row.as_deref())
            == Some(item_id);
        if unchanged {
            return;
        }
        self.gui_emby_libs.entry(lib_idx).or_default().picked_row = Some(item_id.to_string());
        // A pick move is library navigation: it closes the image warm gate, so
        // fast arrow traffic warms no neighbour art (the TUI's own rule).
        self.stamp_navigation();
    }

    /// Whether the active list can still name `item_id` (row 8.1): a
    /// feed-view library's merged checked-folder list, else the root-list
    /// store's loaded list. A loading or not-yet-started list admits the
    /// pick, because the list is what would name it stale; a failed list
    /// names nothing.
    fn gui_emby_row_known(&self, lib_idx: usize, item_id: &str) -> bool {
        if self.is_feed_home_video_library(lib_idx) {
            let (items, loading) = self.gui_emby_feed_items(lib_idx);
            return loading || items.iter().any(|item| item.id == item_id);
        }
        match self.gui_emby_roots.get(&lib_idx) {
            Some(GuiEmbyRootItems::Loaded(items)) => items.iter().any(|item| item.id == item_id),
            Some(GuiEmbyRootItems::Failed) => false,
            Some(GuiEmbyRootItems::Loading) | None => true,
        }
    }

    /// Check or uncheck one folder of the feed-view library `lib_idx` (row
    /// 8.1; gui-collection-dropdown design D2, D3, D5). The selection is the
    /// GUI's own, so the shared `FeedHomeVideoState::selection` stays what the
    /// TUI set; the checked set's fetches start through the frame path's
    /// ensure, one at a time through [`App::spawn_feed_home_video_fetch`]. A
    /// folder the root listing does not hold is a stale frame's key and
    /// changes nothing, and a listing still landing admits it, because the
    /// listing is what would name the folder gone. A `lib_idx` that names no
    /// feed-view library is an ignored command.
    pub fn toggle_gui_emby_feed_folder(&mut self, lib_idx: usize, folder_id: &str) {
        if let Some(reason) = self.gui_emby_feed_reject_reason(lib_idx) {
            Self::log_ignored_library_command(
                "collectionToggle",
                format_args!("library {lib_idx}: {reason}"),
            );
            return;
        }
        if !self.gui_emby_folder_known(lib_idx, folder_id) {
            Self::log_stale_library_item("collectionToggle", folder_id);
            return;
        }
        let (_, order) = self.gui_emby_feed_set(lib_idx);
        let next = self.gui_emby_feed_effective_set(lib_idx).toggle(folder_id);
        self.install_gui_emby_feed_set(lib_idx, next, order);
    }

    /// Select All for the feed-view library `lib_idx` (row 8.1;
    /// gui-collection-dropdown design D5): the All row's press and the
    /// dropdown's Clear. A `lib_idx` that names no feed-view library is an
    /// ignored command.
    pub fn select_all_gui_emby_feed_folders(&mut self, lib_idx: usize) {
        if let Some(reason) = self.gui_emby_feed_reject_reason(lib_idx) {
            Self::log_ignored_library_command(
                "collectionAll",
                format_args!("library {lib_idx}: {reason}"),
            );
            return;
        }
        let (_, order) = self.gui_emby_feed_set(lib_idx);
        self.install_gui_emby_feed_set(lib_idx, GuiCollectionSet::all(), order);
    }

    /// Set the feed-view library's list order (row 8.1; gui-collection-dropdown
    /// design D4, D5): the dropdown's order toggle. Newest is the launch
    /// default. A `lib_idx` that names no feed-view library is an ignored
    /// command.
    pub fn set_gui_emby_feed_date_order(&mut self, lib_idx: usize, order: GuiDateOrder) {
        if let Some(reason) = self.gui_emby_feed_reject_reason(lib_idx) {
            Self::log_ignored_library_command(
                "dateOrder",
                format_args!("library {lib_idx}: {reason}"),
            );
            return;
        }
        let (set, _) = self.gui_emby_feed_set(lib_idx);
        self.install_gui_emby_feed_set(lib_idx, set, order);
    }

    /// Why a collection command cannot apply to `lib_idx` at all (row 8.1;
    /// gui-collection-dropdown design D5): the index names no plain Emby
    /// library, or that library presents no feed-view folders.
    fn gui_emby_feed_reject_reason(&self, lib_idx: usize) -> Option<&'static str> {
        if !self.gui_emby_library(lib_idx) {
            return Some("the destination is no plain Emby library");
        }
        if !self.is_feed_home_video_library(lib_idx) {
            return Some("the destination is no feed-view library");
        }
        None
    }

    /// Store the feed-view library's set and order and start the fetches the
    /// set is missing (row 8.1; gui-collection-dropdown design D5): the one
    /// write the toggle, All, and order presses share.
    fn install_gui_emby_feed_set(
        &mut self,
        lib_idx: usize,
        set: GuiCollectionSet,
        order: GuiDateOrder,
    ) {
        self.gui_emby_feed_sets.insert(lib_idx, (set, order));
        self.ensure_gui_emby_feed_fetch(lib_idx);
    }

    /// The shown-pass trigger the GUI frame path runs for the active plain
    /// Emby library destination (row 8.1): a feed-view library's checked set
    /// fetches the first target it is missing (design D3), and a plain
    /// library's root list fetches when the store holds no list. A `lib_idx`
    /// that names no plain Emby library is a no-op, so the frame path can
    /// call it unguarded for its destination.
    pub fn ensure_gui_emby_selection(&mut self, lib_idx: usize) {
        if !self.gui_emby_library(lib_idx) {
            return;
        }
        if self.is_feed_home_video_library(lib_idx) {
            self.ensure_gui_emby_feed_fetch(lib_idx);
        } else {
            self.fetch_gui_emby_root_items(lib_idx);
        }
    }

    /// Start the first fetch the GUI's checked set is missing (design D3):
    /// the first target — All, or the first checked folder — with no cache
    /// entry, while no fetch is outstanding. `fetching` names the one fetch
    /// that runs, so the next target's fetch starts on a later frame once it
    /// clears: one fetch at a time, as the TUI's own selection fetch. The
    /// shared `FeedHomeVideoState::selection` is never written here, so the
    /// TUI keeps its own selection.
    fn ensure_gui_emby_feed_fetch(&mut self, lib_idx: usize) {
        // The GUI owns this library's set from the first frame it shows: the
        // entry marks that library's results as the GUI's own, which the
        // result arm's second acceptance rule reads. A Client that never
        // shows the destination keeps no entry, so the TUI's own stale-drop
        // rule alone governs its results.
        self.gui_emby_feed_sets
            .entry(lib_idx)
            .or_insert_with(|| (GuiCollectionSet::all(), GuiDateOrder::default()));
        let Some(state) = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
        else {
            return;
        };
        if state.fetching.is_some() {
            return;
        }
        let Some(target) = self
            .gui_emby_feed_targets(lib_idx)
            .into_iter()
            .find(|target| !state.items.contains_key(target))
        else {
            return;
        };
        self.spawn_feed_home_video_fetch(lib_idx, target);
    }

    /// The library's stored feed-group selection (design D2): the checked
    /// folders or All, and the list's date order. A library the GUI has not
    /// selected on reads All on Newest, the launch default.
    fn gui_emby_feed_set(&self, lib_idx: usize) -> (GuiCollectionSet, GuiDateOrder) {
        self.gui_emby_feed_sets
            .get(&lib_idx)
            .cloned()
            .unwrap_or((GuiCollectionSet::all(), GuiDateOrder::default()))
    }

    /// The folder catalog the checked set resolves against (design D1, D3):
    /// the root listing's folders' stable ids, or `None` while the listing has
    /// not landed — the set is then kept as it is, because the listing is what
    /// would name a folder gone.
    fn gui_emby_folder_catalog(&self, lib_idx: usize) -> Option<Vec<String>> {
        let root = self.libs.get(lib_idx)?.nav_stack.first()?;
        if root.loading {
            return None;
        }
        Some(
            root.items
                .iter()
                .filter(|item| item.is_folder)
                .map(|item| item.id.clone())
                .collect(),
        )
    }

    /// Whether the GUI may bind a press to `folder_id` (row 8.1): the root
    /// listing holds the folder, or the listing has not landed yet — it is
    /// what would name the folder gone.
    fn gui_emby_folder_known(&self, lib_idx: usize, folder_id: &str) -> bool {
        self.gui_emby_folder_catalog(lib_idx)
            .is_none_or(|present| present.iter().any(|id| id == folder_id))
    }

    /// The effective feed-group selection the pills, the list, and the fetches
    /// read (design D1): the stored set with every checked folder the root
    /// listing no longer holds dropped, so a vanished folder's check falls back
    /// to All when none survive. All stays All.
    pub(in crate::app) fn gui_emby_feed_effective_set(&self, lib_idx: usize) -> GuiCollectionSet {
        let (set, _) = self.gui_emby_feed_set(lib_idx);
        match self.gui_emby_folder_catalog(lib_idx) {
            Some(present) => set.retain_present(&present),
            None => set,
        }
    }

    /// The date order the GUI's feed-group list sorts by (design D4).
    pub(in crate::app) fn gui_emby_feed_order(&self, lib_idx: usize) -> GuiDateOrder {
        self.gui_emby_feed_set(lib_idx).1
    }

    /// The targets the effective selection reads (design D3): All is the one
    /// All list, and a checked set is one target per checked folder.
    pub(in crate::app) fn gui_emby_feed_targets(
        &self,
        lib_idx: usize,
    ) -> Vec<FeedHomeVideoSelection> {
        match self.gui_emby_feed_effective_set(lib_idx) {
            GuiCollectionSet::All => vec![FeedHomeVideoSelection::All],
            GuiCollectionSet::Checked(keys) => keys
                .into_iter()
                .map(FeedHomeVideoSelection::Folder)
                .collect(),
        }
    }

    /// Whether the GUI's checked set still wants `target`'s list (design D3):
    /// the result arm's second acceptance rule. A library with no GUI
    /// selection entry wants nothing, so the TUI's own stale-drop rule alone
    /// applies to its results.
    pub(in crate::app) fn gui_emby_feed_wants_target(
        &self,
        lib_idx: usize,
        target: &FeedHomeVideoSelection,
    ) -> bool {
        if !self.gui_emby_feed_sets.contains_key(&lib_idx) {
            return false;
        }
        self.gui_emby_feed_targets(lib_idx)
            .iter()
            .any(|wanted| wanted == target)
    }

    /// Start `lib_idx`'s root-list fetch when the store holds no list (row
    /// 8.1): the Client core thread never fetches, and the completion lands
    /// in the store outside `nav_stack`. A failed fetch is not retried here:
    /// it stays failed until the library is opened again, so a paint never
    /// re-issues it.
    pub(in crate::app) fn fetch_gui_emby_root_items(&mut self, lib_idx: usize) {
        if matches!(
            self.gui_emby_roots.get(&lib_idx),
            Some(
                GuiEmbyRootItems::Loading | GuiEmbyRootItems::Loaded(_) | GuiEmbyRootItems::Failed
            )
        ) {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            self.gui_emby_roots
                .insert(lib_idx, GuiEmbyRootItems::Failed);
            return;
        };
        let (item_types, _unplayed_only, sort_by, sort_order) =
            self.default_library_level_fields(lib_idx);
        debug_assert_eq!(
            sort_by.as_str(),
            "SortName",
            "the root-list store reads the name sort"
        );
        debug_assert_eq!(sort_order.as_str(), "Ascending");
        self.gui_emby_roots
            .insert(lib_idx, GuiEmbyRootItems::Loading);
        let parent_id = self.libs[lib_idx].library.id.clone();
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            let result = super::store::fetch_all_pages(&client, &parent_id, item_types.as_deref());
            let _ = tx.send(LibEvent::GuiEmbyRootItems(GuiEmbyRootItemsEvent {
                lib_idx,
                result,
            }));
        });
    }

    /// Land one plain Emby library's root-list fetch (row 8.1): a loaded list
    /// replaces the loading mark, and a failure marks only that library.
    pub(in crate::app) fn handle_gui_emby_root_items(&mut self, event: GuiEmbyRootItemsEvent) {
        let GuiEmbyRootItemsEvent { lib_idx, result } = event;
        let state = match result {
            Ok(items) => GuiEmbyRootItems::Loaded(items),
            Err(error) => {
                tracing::info!(
                    name: "gui_library.emby_root.failed",
                    target: "gui_library",
                    library_index = lib_idx,
                    error = %error,
                    "the library's root list failed to load"
                );
                GuiEmbyRootItems::Failed
            }
        };
        self.gui_emby_roots.insert(lib_idx, state);
    }

    /// Play the plain Emby-library row a command names (design D12, row
    /// 8.1): the shared library play path, whose folder-play rule expands a
    /// folder or a boxset into its playable content and whose admission and
    /// route checks are the Owner's queue's own. A stale id plays nothing.
    pub(in crate::app::gui_library) fn play_gui_emby_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        match self.gui_emby_item(lib_idx, item_id) {
            Some(item) => self.play_or_activate_lib_item(lib_idx, item),
            None => Self::log_stale_library_item("embyPlay", item_id),
        }
    }

    /// Play the plain Emby-library row next (design D12, row 8.1): the
    /// shared play-next admission, so the row enters directly after the
    /// active slot.
    pub(in crate::app::gui_library) fn play_next_gui_emby_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        match self.gui_emby_item(lib_idx, item_id) {
            Some(item) => self.play_lib_item_next(lib_idx, item),
            None => Self::log_stale_library_item("embyPlayNext", item_id),
        }
    }

    /// Add the plain Emby-library row to the queue (design D12, row 8.1):
    /// the shared add path.
    pub(in crate::app::gui_library) fn enqueue_gui_emby_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        match self.gui_emby_item(lib_idx, item_id) {
            Some(item) => self.enqueue_lib_item(lib_idx, item),
            None => Self::log_stale_library_item("embyQueue", item_id),
        }
    }

    /// The root items the GUI store loaded for `lib_idx` (row 8.1): the list
    /// the letter rail and the projection read. Empty while the list is
    /// loading or its fetch failed.
    pub(in crate::app) fn gui_stored_emby_root_items(
        &self,
        lib_idx: usize,
    ) -> impl Iterator<Item = &EmbyItem> {
        match self.gui_emby_roots.get(&lib_idx) {
            Some(GuiEmbyRootItems::Loaded(items)) => items.iter(),
            Some(GuiEmbyRootItems::Loading | GuiEmbyRootItems::Failed) | None => [].iter(),
        }
    }
}
