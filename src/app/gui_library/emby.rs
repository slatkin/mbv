//! The GUI Library's plain Emby-library data path (add-gui-library-panel
//! design D12, row 8.1): the per-library selection the core owns (the picked
//! row, and the feed-view library's selected group through the TUI's own
//! feed state), and the open, select, and set-feed-group actions the later
//! core surface's commands apply. The root-list store follows the Music
//! collection store's precedent (design D5): the TUI's root path cannot
//! serve a full flat list — a library over `LIBRARY_PILL_THRESHOLD` is
//! bucketed by letter range and a smaller one pages as its cursor moves —
//! so the store fetches the whole root list with `get_items_sorted`, paged
//! to completion, outside `nav_stack`. Every action runs the TUI's existing
//! loaders, so the two Clients cannot drift. The TUI never reads any of it.

use mbv_emby_model::EmbyItem;

use super::emby_projection::GuiEmbyFeedGroup;
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

/// One Emby library's destination-Latest snapshot in the store (row 8.1):
/// the list the New pill paints, the GUI's own counterpart of the TUI
/// shell's `tv_latest_snapshots`. A New-pill selection starts its fetch, a
/// landed list lives for the session, and a failed list stays failed until
/// the pill is selected again — a re-press is the retry, as the TUI's
/// refresh is.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum GuiEmbyLatestItems {
    /// The Latest fetch is in flight.
    Loading,
    /// The snapshot's items, in the server's newest-first order.
    Loaded(Vec<EmbyItem>),
    /// The Latest fetch failed.
    Failed,
}

/// The completion of one Emby library's Latest fetch, carried on the shared
/// library event channel. Only the GUI Client's Latest store starts one, so
/// the TUI never receives it.
#[derive(Debug)]
pub(in crate::app) struct GuiEmbyLatestItemsEvent {
    pub(in crate::app) lib_idx: usize,
    pub(in crate::app) result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
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
/// the row the Hero shows, by stable Service id, and whether the New pill is
/// the selected one. The feed-view library's selected All-or-folder group is
/// not mirrored here: [`mbv_ui_model::feed::FeedHomeVideoState`]'s
/// `selection` on the library tab is the one store of that fact, and the
/// set-feed-group action writes it through the TUI's own path. The New pill
/// is the GUI's own view over the destination-Latest store, whose shared
/// fetch the TUI runs in its shell; a stored row the active list no longer
/// carries falls back to the list's first row in the projection, as every
/// selection's stale arm does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiEmbyLibrarySelection {
    /// The row the Hero shows. `None` before a select command names one.
    pub picked_row: Option<String>,
    /// Whether the New pill is the selected one (row 8.1). `All` and a
    /// folder pill clear it; selecting it again re-reads the Latest store.
    pub feed_latest: bool,
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
    /// listing the feed group view already owns; its selected group's items
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
    /// one the selected group's items hold. A stale frame's id changes
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
        self.gui_emby_libs.entry(lib_idx).or_default().picked_row = Some(item_id.to_string());
    }

    /// Whether the active list can still name `item_id` (row 8.1): the
    /// feed-view library's New pill's Latest items, else the selected group's
    /// items, else the root-list store's loaded list. A loading or
    /// not-yet-started list admits the pick, because the list is what would
    /// name it stale; a failed list names nothing.
    fn gui_emby_row_known(&self, lib_idx: usize, item_id: &str) -> bool {
        if self.is_feed_home_video_library(lib_idx) {
            if self
                .gui_emby_libs
                .get(&lib_idx)
                .is_some_and(|sel| sel.feed_latest)
            {
                return match self.gui_emby_latest.get(&lib_idx) {
                    Some(GuiEmbyLatestItems::Loaded(items)) => {
                        items.iter().any(|item| item.id == item_id)
                    }
                    Some(GuiEmbyLatestItems::Failed) => false,
                    Some(GuiEmbyLatestItems::Loading) | None => true,
                };
            }
            return self
                .feed_home_video_selected_items(lib_idx)
                .iter()
                .any(|item| item.id == item_id);
        }
        match self.gui_emby_roots.get(&lib_idx) {
            Some(GuiEmbyRootItems::Loaded(items)) => items.iter().any(|item| item.id == item_id),
            Some(GuiEmbyRootItems::Failed) => false,
            Some(GuiEmbyRootItems::Loading) | None => true,
        }
    }

    /// Set the feed-view library's selected pill (row 8.1): the New pill
    /// selects the GUI's own Latest view and triggers its lazy fetch, and
    /// the All and folder pills clear it and run the TUI's own selection
    /// path, so the session cache, the stale-drop, and the lazy fetch behave
    /// as the TUI pill's press does. A `Folder` selection whose folder the
    /// root listing does not hold is a stale frame's pick and changes
    /// nothing; `All` and `New` always apply. A `lib_idx` that names no
    /// feed-view library is an ignored command.
    pub fn gui_emby_set_feed_group(&mut self, lib_idx: usize, group: &GuiEmbyFeedGroup) {
        if !self.gui_emby_library(lib_idx) {
            Self::log_ignored_library_command(
                "embySetFeedGroup",
                format_args!("the destination is no plain Emby library"),
            );
            return;
        }
        if !self.is_feed_home_video_library(lib_idx) {
            Self::log_ignored_library_command(
                "embySetFeedGroup",
                format_args!("the destination is no feed-view library"),
            );
            return;
        }
        let entry = self.gui_emby_libs.entry(lib_idx).or_default();
        entry.feed_latest = matches!(group, GuiEmbyFeedGroup::Latest);
        match group {
            GuiEmbyFeedGroup::Latest => self.ensure_gui_emby_latest_items(lib_idx),
            GuiEmbyFeedGroup::All => {
                self.apply_feed_home_video_group_selection(lib_idx, &FeedHomeVideoSelection::All);
            }
            GuiEmbyFeedGroup::Folder(folder_id) => {
                let in_root = self.libs[lib_idx]
                    .nav_stack
                    .first()
                    .is_some_and(|root| root.items.iter().any(|item| &item.id == folder_id));
                if !in_root {
                    Self::log_stale_library_item("embySetFeedGroup", folder_id);
                    entry.feed_latest = false;
                    return;
                }
                self.apply_feed_home_video_group_selection(
                    lib_idx,
                    &FeedHomeVideoSelection::Folder(folder_id.clone()),
                );
            }
        }
    }

    /// The shown-pass trigger the GUI frame path runs for the active plain
    /// Emby library destination (row 8.1), the App-side counterpart of the
    /// TUI projection pass's lazy-fetch arm: a feed-view library's shown
    /// selection fetches when it is uncached and nothing is fetching it, its
    /// New pill's Latest list fetches when the store holds none, and a plain
    /// library's root list fetches when the store holds no list. A `lib_idx`
    /// that names no plain Emby library is a no-op, so the frame path can
    /// call it unguarded for its destination.
    pub fn ensure_gui_emby_selection(&mut self, lib_idx: usize) {
        if !self.gui_emby_library(lib_idx) {
            return;
        }
        if self.is_feed_home_video_library(lib_idx) {
            if self
                .gui_emby_libs
                .get(&lib_idx)
                .is_some_and(|sel| sel.feed_latest)
            {
                self.ensure_gui_emby_latest_items(lib_idx);
            } else {
                self.ensure_feed_home_video_selection_loaded(lib_idx, false);
            }
        } else {
            self.fetch_gui_emby_root_items(lib_idx);
        }
    }

    /// Start `lib_idx`'s Latest fetch when the store holds no list (row
    /// 8.1): the New pill's lazy load, the TUI's own destination-Latest
    /// trigger's shape (`spawn_destination_latest_snapshot`, which the TUI
    /// runs from its shell). The fetch is the TUI's `get_latest` call with
    /// the TUI's 30-item limit, and the completion lands in the store
    /// outside `nav_stack`. A failed fetch is not retried here: it stays
    /// failed until the pill is selected again, so a paint never re-issues
    /// it.
    fn ensure_gui_emby_latest_items(&mut self, lib_idx: usize) {
        if self.gui_emby_latest.contains_key(&lib_idx) {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            // No Service client yet: leave the store empty; a later selection
            // or shown pass retries.
            return;
        };
        let library_id = self.libs[lib_idx].library.id.clone();
        self.gui_emby_latest
            .insert(lib_idx, GuiEmbyLatestItems::Loading);
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            let result = client.get_latest(&library_id, 30);
            let _ = tx.send(LibEvent::GuiEmbyLatestItems(GuiEmbyLatestItemsEvent {
                lib_idx,
                result,
            }));
        });
    }

    /// Land one Emby library's Latest fetch (row 8.1): a loaded snapshot
    /// replaces the loading mark, and a failure marks only that library.
    pub(in crate::app) fn handle_gui_emby_latest_items(&mut self, event: GuiEmbyLatestItemsEvent) {
        let GuiEmbyLatestItemsEvent { lib_idx, result } = event;
        let state = match result {
            Ok(items) => GuiEmbyLatestItems::Loaded(items),
            Err(error) => {
                tracing::info!(
                    name: "gui_library.emby_latest.failed",
                    target: "gui_library",
                    library_index = lib_idx,
                    error = %error,
                    "the library's Latest snapshot failed to load"
                );
                GuiEmbyLatestItems::Failed
            }
        };
        self.gui_emby_latest.insert(lib_idx, state);
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

    /// The Latest items the GUI store loaded for `lib_idx` (row 8.1): the
    /// list the New pill paints. Empty while the snapshot is loading or its
    /// fetch failed.
    pub(in crate::app) fn gui_stored_emby_latest_items(
        &self,
        lib_idx: usize,
    ) -> impl Iterator<Item = &EmbyItem> {
        match self.gui_emby_latest.get(&lib_idx) {
            Some(GuiEmbyLatestItems::Loaded(items)) => items.iter(),
            Some(GuiEmbyLatestItems::Loading | GuiEmbyLatestItems::Failed) | None => [].iter(),
        }
    }
}
