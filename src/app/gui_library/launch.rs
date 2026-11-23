//! The GUI's launch location (add-gui-library-panel design D9): the
//! destination and the selected collections of each destination, saved at
//! orderly exit and restored after the catalog lands.
//!
//! The GUI keeps its own file ([`mbv_config::save_gui_launch_state`]): the TUI
//! launch state never moves this Client's tab, and this file never holds a TUI
//! launch location. Identities are stable Service ids, so a resolution that
//! finds no match opens nothing instead of a stale position or another
//! destination the saved location did not name.

use mbv_config::{
    GUI_LAUNCH_STATE_VERSION, GuiDestinationCollections, GuiLaunchCollectionTarget,
    GuiLaunchDateOrder, GuiLaunchListSort, GuiLaunchShowsView, GuiLaunchSort, GuiLaunchState,
    TabIdentity,
};
use mbv_queue::ServiceKind;
use mbv_ui_model::browse::BrowseLevel;
use mbv_ui_model::tab_selection::TabSelection;

use super::collection::{GuiCollectionSet, GuiCollectionTarget};
use super::collection_command::{CollectionOp, GuiCollectionSite};
use super::list_sort::GuiListSort;
use super::shows_view::GuiShowsView;
use super::tabs::GuiDestinationKind;
use super::{
    GuiEmbyFeedCollectionSite, GuiFeedsCollectionSite, GuiMusicCollectionSite,
    GuiPodcastsCollectionSite,
};
use crate::app::App;

impl App {
    /// The GUI's launch location (design D9, D10): the active destination, the
    /// selected collections of each destination, and each destination's list
    /// sort. Read at orderly exit only; nothing writes it while the window is
    /// open.
    #[must_use]
    pub fn gui_launch_state(&self) -> GuiLaunchState {
        GuiLaunchState {
            version: GUI_LAUNCH_STATE_VERSION,
            destination: self.gui_launch_destination(),
            collections: self.gui_launch_collections(),
            shows_view: self.gui_launch_shows_view(),
            sorts: self.gui_launch_sorts(),
        }
    }

    /// Finish the launch location's first phase: Home stays the destination
    /// every start opens on, whatever destination the saved location holds,
    /// so this opens nothing. It waits only for the catalog: once a
    /// destination is presented, the per-library lists the saved sets and the
    /// saved Shows view apply to exist. [`GuiLaunchOpen::Opened`] carries the
    /// active destination's identity, as the saved file carries it, so a tab
    /// switch before the sets land abandons them. [`GuiLaunchOpen::Retry`]
    /// means no catalog has landed, so the caller retries on the drain that
    /// delivers one.
    pub fn open_gui_launch_destination(&mut self, state: &GuiLaunchState) -> GuiLaunchOpen {
        // The saved sorts key by destination identity, so they install before
        // any catalog lands, and a retry cannot clobber them either.
        self.restore_gui_launch_sorts(state);
        if self.presented_destinations().is_empty() {
            return GuiLaunchOpen::Retry;
        }
        // The catalog has landed, so the Shows destination's two per-library
        // lists are requested now, once per launch (gui-collection-dropdown
        // design D9). Both requests are idempotent, so a later catalog landing
        // re-requests nothing.
        self.spawn_gui_shows_sections();
        self.restore_gui_shows_view(state.shows_view);
        GuiLaunchOpen::Opened(self.gui_launch_destination())
    }

    /// The saved sorts of the lists the session sorted (design D10), in the
    /// store's order: one entry per destination whose sort the session
    /// changed, so a destination the session never touched opens on its own
    /// default.
    fn gui_launch_sorts(&self) -> Vec<GuiLaunchListSort> {
        self.gui
            .list_sorts
            .iter()
            .map(|(destination, sort)| GuiLaunchListSort {
                destination: destination.clone(),
                sort: launch_list_sort(*sort),
            })
            .collect()
    }

    /// Apply the saved list sorts (design D10): every saved sort installs by
    /// its stable identity unless the session already set that destination's
    /// sort, so a restore never clobbers a choice the session made, and a
    /// saved sort for a library absent this session stays in the store for
    /// the next save. Then each collection entry's saved date order installs
    /// as Newest or Oldest where the file holds no sort for that destination,
    /// so a launch file written before the list sort still opens on its own
    /// order.
    pub fn restore_gui_launch_sorts(&mut self, state: &GuiLaunchState) {
        for saved in &state.sorts {
            self.install_saved_sort(&saved.destination, list_sort(saved.sort));
        }
        for entry in &state.collections {
            let Some(order) = entry.order else {
                continue;
            };
            // A saved sort wins over the entry's older date order: the loop
            // above installed it, so `install_saved_sort` holds the slot and
            // this install changes nothing.
            self.install_saved_sort(&entry.destination, date_sort(order));
        }
    }

    /// Install one saved sort unless the session already set that
    /// destination's sort (design D10).
    fn install_saved_sort(&mut self, destination: &TabIdentity, sort: GuiListSort) {
        if self.gui_list_sort_stored(destination).is_some() {
            return;
        }
        self.install_gui_list_sort(destination.clone(), sort);
    }

    /// Apply the saved Shows view to every presented Shows destination
    /// (design D14): the file's field is separate from the collection
    /// entries, and Home is the opened destination, so no Shows destination
    /// is active to take it. A view the session already chose is not
    /// overwritten.
    fn restore_gui_shows_view(&mut self, view: GuiLaunchShowsView) {
        for kind in self.presented_destinations() {
            let GuiDestinationKind::Shows { lib_idx } = kind else {
                continue;
            };
            if !self.gui.shows_views.contains_key(&lib_idx) {
                self.set_gui_shows_view(lib_idx, shows_view(view));
            }
        }
    }

    /// The saved Shows view (design D14): the active Shows destination's
    /// choice when there is one, else the first presented Shows library's, so
    /// a session that changed a resting destination still saves its view.
    fn gui_launch_shows_view(&self) -> GuiLaunchShowsView {
        let active = match self.gui_active_destination() {
            Some(GuiDestinationKind::Shows { lib_idx }) => Some(lib_idx),
            _ => None,
        };
        let shows = active.or_else(|| {
            self.presented_destinations().into_iter().find_map(|kind| {
                let GuiDestinationKind::Shows { lib_idx } = kind else {
                    return None;
                };
                Some(lib_idx)
            })
        });
        shows.map_or(GuiLaunchShowsView::All, |lib_idx| {
            launch_shows_view(self.gui_shows_view(lib_idx))
        })
    }

    /// Apply the saved collection choices of every destination the file
    /// carries (design D9, D10; gui-collection-dropdown design D11, D13):
    /// each entry restores through the destination and collection target it
    /// names, so a destination that was not the one the first phase opened
    /// still gets its saved set. The site's catalog retention drops saved ids
    /// the library no longer holds; an emptied set and a saved All fall back
    /// to All, as does a destination with no saved entry. The active
    /// destination must still be the one phase 1 recorded: a mid-restore tab
    /// switch abandons the restore, so no other destination's saved set lands
    /// under the user. Returns `false` while a saved entry's destination
    /// catalog can still land, so the caller retries.
    pub fn restore_gui_launch_collections(
        &mut self,
        state: &GuiLaunchState,
        destination: &TabIdentity,
    ) -> bool {
        if self.gui_launch_destination() != *destination {
            // A different destination is active: the restore gives up rather
            // than applying a saved set the user did not ask for.
            return true;
        }
        let mut pending = false;
        for entry in &state.collections {
            pending |= !self.restore_gui_launch_entry(entry);
        }
        !pending
    }

    /// Apply one saved entry through the destination it names (design D13):
    /// the presented destination whose identity the entry holds, and whose
    /// collection dropdown the entry's target names. A destination the
    /// settled catalog no longer holds, and a destination that paints no
    /// dropdown or another dropdown, install nothing, so saved ids never land
    /// in a destination the entry did not name. Returns `false` while the
    /// entry's destination catalog can still land, so the caller retries.
    fn restore_gui_launch_entry(&mut self, entry: &GuiDestinationCollections) -> bool {
        let kind = match &entry.destination {
            // Home paints no collection dropdown, so no saved entry names it.
            TabIdentity::Home => return true,
            destination => match self.launch_destination(destination) {
                LaunchDestination::Open(kind) => kind,
                LaunchDestination::Pending => return false,
                LaunchDestination::Unmatched => return true,
            },
        };
        let Some(target) = self.gui_collection_target(kind) else {
            // The destination paints no collection dropdown.
            return true;
        };
        if launch_collection_target(&target) != entry.target {
            // The destination's dropdown is not the one the entry names.
            return true;
        }
        self.restore_collection_for_target(&target, entry);
        true
    }

    /// Apply one saved entry through `target`'s own site (design D13).
    fn restore_collection_for_target(
        &mut self,
        target: &GuiCollectionTarget,
        entry: &GuiDestinationCollections,
    ) {
        match target {
            GuiCollectionTarget::Music(lib_idx) => {
                self.restore_collection_at(&GuiMusicCollectionSite(*lib_idx), entry);
            }
            GuiCollectionTarget::Podcasts(lib_idx) => {
                self.restore_collection_at(&GuiPodcastsCollectionSite(*lib_idx), entry);
            }
            GuiCollectionTarget::Feeds => {
                self.restore_collection_at(&GuiFeedsCollectionSite, entry);
            }
            GuiCollectionTarget::EmbyFeed(lib_idx) => {
                self.restore_collection_at(&GuiEmbyFeedCollectionSite(*lib_idx), entry);
            }
        }
    }

    /// The identity of the active destination (design D9), as the saved file
    /// carries it: a stable Service library id, never a tab position.
    fn gui_launch_destination(&self) -> TabIdentity {
        match self.tab {
            TabSelection::Home => TabIdentity::Home,
            TabSelection::Feeds => TabIdentity::Feeds,
            TabSelection::EmbyLibrary(lib_idx) => {
                self.libs.get(lib_idx).map_or(TabIdentity::Home, |tab| {
                    library_identity(ServiceKind::Emby, &tab.library.id)
                })
            }
            TabSelection::AudiobookshelfLibrary(index) => self
                .audiobookshelf_libraries
                .get(index)
                .map_or(TabIdentity::Home, |library| {
                    library_identity(ServiceKind::Audiobookshelf, &library.id)
                }),
        }
    }

    /// The selected collections of the catalog's collection dropdowns
    /// (design D9, D10), in catalog order: one entry per destination whose
    /// choice the session holds, so a destination the session never touched
    /// saves nothing and opens on All. Every dropdown reads its own site's
    /// stored set and picks, so the four destinations share one save path. The file keeps the list shape, and each entry holds the
    /// checked ids (an empty list is All), so the saved file is stable.
    fn gui_launch_collections(&self) -> Vec<GuiDestinationCollections> {
        self.presented_destinations()
            .into_iter()
            .filter_map(|kind| {
                let target = self.gui_collection_target(kind)?;
                let destination = self.gui_destination_identity(kind)?;
                let launch_target = launch_collection_target(&target);
                match target {
                    GuiCollectionTarget::Music(lib_idx) => self.gui_launch_collection_entry(
                        &GuiMusicCollectionSite(lib_idx),
                        launch_target,
                        destination,
                    ),
                    GuiCollectionTarget::Podcasts(lib_idx) => self.gui_launch_collection_entry(
                        &GuiPodcastsCollectionSite(lib_idx),
                        launch_target,
                        destination,
                    ),
                    GuiCollectionTarget::Feeds => self.gui_launch_collection_entry(
                        &GuiFeedsCollectionSite,
                        launch_target,
                        destination,
                    ),
                    GuiCollectionTarget::EmbyFeed(lib_idx) => self.gui_launch_collection_entry(
                        &GuiEmbyFeedCollectionSite(lib_idx),
                        launch_target,
                        destination,
                    ),
                }
            })
            .collect()
    }

    /// One destination's saved entry, read from its own site: a destination
    /// the session never touched writes nothing, and every dropdown writes
    /// the same checked ids. The entry's `order` stays readable for an older
    /// file, and the list's sort saves through the launch state's own sorts.
    fn gui_launch_collection_entry(
        &self,
        site: &impl GuiCollectionSite,
        target: GuiLaunchCollectionTarget,
        destination: TabIdentity,
    ) -> Option<GuiDestinationCollections> {
        if !site.has_stored_selection(self) {
            return None;
        }
        Some(GuiDestinationCollections {
            destination,
            target,
            group_ids: site.stored_set(self).checked_keys().to_vec(),
            order: None,
            picks: site.saved_picks(self),
        })
    }

    /// The collection dropdown a presented destination carries (design D5):
    /// Music, Podcasts, and Feeds, and only a feed-view Emby library's
    /// folder dropdown. A Shows, Audiobooks, or plain Emby destination has
    /// no collection dropdown, so it saves and restores nothing.
    fn gui_collection_target(&self, kind: GuiDestinationKind) -> Option<GuiCollectionTarget> {
        match kind {
            GuiDestinationKind::Music { lib_idx } => Some(GuiCollectionTarget::Music(lib_idx)),
            GuiDestinationKind::Podcasts { lib_idx } => {
                Some(GuiCollectionTarget::Podcasts(lib_idx))
            }
            GuiDestinationKind::Feeds => Some(GuiCollectionTarget::Feeds),
            GuiDestinationKind::Library { lib_idx }
                if lib_idx < self.libs.len() && self.is_feed_home_video_library(lib_idx) =>
            {
                Some(GuiCollectionTarget::EmbyFeed(lib_idx))
            }
            GuiDestinationKind::Home
            | GuiDestinationKind::Shows { .. }
            | GuiDestinationKind::Library { .. }
            | GuiDestinationKind::Audiobooks { .. } => None,
        }
    }

    /// The launch identity a presented destination carries: a stable Service
    /// library id, or Feeds for the one synthetic tab.
    pub(in crate::app::gui_library) fn gui_destination_identity(
        &self,
        kind: GuiDestinationKind,
    ) -> Option<TabIdentity> {
        match kind {
            GuiDestinationKind::Home => Some(TabIdentity::Home),
            GuiDestinationKind::Feeds => Some(TabIdentity::Feeds),
            GuiDestinationKind::Music { lib_idx }
            | GuiDestinationKind::Shows { lib_idx }
            | GuiDestinationKind::Library { lib_idx } => self
                .libs
                .get(lib_idx)
                .map(|lib| library_identity(ServiceKind::Emby, &lib.library.id)),
            GuiDestinationKind::Audiobooks { lib_idx }
            | GuiDestinationKind::Podcasts { lib_idx } => self
                .audiobookshelf_libraries
                .get(lib_idx)
                .map(|lib| library_identity(ServiceKind::Audiobookshelf, &lib.id)),
        }
    }

    /// Apply one saved entry through its destination's site (design D13): the
    /// saved set installs against the site's landed catalog — the retention
    /// drops the ids the library no longer holds and filters the picks — and
    /// against the saved ids alone while the catalog has not landed, because a
    /// resting destination never loads one. The per-frame effective read then
    /// drops a vanished key when the catalog lands, so gating the install on a
    /// catalog the destination has not loaded would strand the entry instead.
    /// The shared apply path installs what survives unless the session already
    /// changed this destination's selection.
    fn restore_collection_at(
        &mut self,
        site: &impl GuiCollectionSite,
        entry: &GuiDestinationCollections,
    ) {
        let present = site.present(self);
        let picks = match &present {
            Some(present) => entry
                .picks
                .iter()
                .filter(|pick| present(&pick.group_id))
                .cloned()
                .collect::<Vec<_>>(),
            // A catalog that has not landed names no pick stale, so the saved
            // picks install as they are; a projection that reads the landed
            // catalog drops a pick it does not hold.
            None => entry.picks.clone(),
        };
        let set = GuiCollectionSet::Checked(entry.group_ids.clone())
            .effective(present)
            .into_owned();
        if site.has_stored_selection(self) {
            // The user already selected in the pre-restore window: the
            // restore never clobbers a choice the session made.
            return;
        }
        site.install_picks(self, &picks);
        self.apply_collection_at(site, CollectionOp::Restore { set });
    }

    /// The destinations the GUI presents, in catalog order (design D9, D2):
    /// the Grouped Music libraries, the Emby TV (Shows) libraries, every
    /// other Emby library (row 8.1's plain Emby-library destination), the
    /// Audiobookshelf libraries — the book libraries' Audiobooks destination
    /// (row 9.1) and the podcast libraries' Podcasts destination (row 10.1) —
    /// and the Feeds tab when subscriptions are configured (row 11.1), as the
    /// destination-kind enum names them. The launch restore's resolution and
    /// the frame's destination tabs (`gui_library_tabs`) read this one list,
    /// so they cannot disagree about the presented set.
    pub(in crate::app::gui_library) fn presented_destinations(&self) -> Vec<GuiDestinationKind> {
        let mut presented: Vec<GuiDestinationKind> = (0..self.libs.len())
            .map(|lib_idx| {
                if self.is_grouped_music_library(lib_idx) {
                    GuiDestinationKind::Music { lib_idx }
                } else if self.libs[lib_idx].library.collection_type == "tvshows" {
                    GuiDestinationKind::Shows { lib_idx }
                } else {
                    GuiDestinationKind::Library { lib_idx }
                }
            })
            .collect();
        // The Audiobookshelf libraries' tab positions follow the Emby ones,
        // so appending them keeps the catalog order. A library's browse kind
        // picks its destination: a book library presents Audiobooks and a
        // podcast library presents Podcasts.
        presented.extend(
            (0..self.audiobookshelf_libraries.len()).filter_map(|lib_idx| {
                match self.audiobookshelf_kind_at(lib_idx) {
                    Some(mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Book) => {
                        Some(GuiDestinationKind::Audiobooks { lib_idx })
                    }
                    Some(
                        mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Podcast,
                    ) => Some(GuiDestinationKind::Podcasts { lib_idx }),
                    None => None,
                }
            }),
        );
        // The Feeds tab follows every Service library, and it presents while
        // subscriptions are configured (row 11.1): the TUI's own tab-strip
        // rule, so a subscription set the session emptied presents no tab.
        presented.extend(
            self.has_feeds_subscriptions()
                .then_some(GuiDestinationKind::Feeds),
        );
        presented
    }

    /// Whether an open-destination command's `position` names a destination
    /// the GUI presents (design D2): Home, or one of the presented
    /// destinations' tab positions. The command arm reads this one
    /// definition, so a stale frame's position cannot activate a destination
    /// the top bar does not paint — an unpresented destination opens
    /// nothing, as the absent digit key does.
    pub(in crate::app::gui_library) fn presents_open_position(&self, position: usize) -> bool {
        position == 0
            || self
                .presented_destinations()
                .iter()
                .any(|kind| self.gui_destination_position(*kind) == Some(position))
    }

    /// The presented destination a saved collection entry names (design D9),
    /// or why none resolves: the presented destination whose identity matches,
    /// never a substitute for one the presented set does not hold, because
    /// every destination the GUI serves is presented now.
    ///
    /// The restore waits only while a catalog can still land: the saved
    /// identity's own Service still connecting, or no destination presented at
    /// all. Once a destination is presented and the saved identity's Service
    /// has settled, an identity the presented set does not hold is gone, and
    /// the caller finishes the restore rather than waiting for it.
    fn launch_destination(&self, destination: &TabIdentity) -> LaunchDestination {
        let saved = match destination {
            TabIdentity::ServiceLibrary { kind, library_id } => Some((*kind, library_id.as_str())),
            TabIdentity::Home | TabIdentity::Feeds => None,
        };
        // A saved Service library whose Service can still publish its catalog
        // waits, so a startup that has not finished is never read as a library
        // the catalog lost.
        if saved.is_some_and(|(kind, _)| self.launch_catalog_pending(kind)) {
            return LaunchDestination::Pending;
        }
        let presented = self.presented_destinations();
        // Nothing is presented yet: no catalog has landed, so the restore
        // waits for one, and the window stays interactive meanwhile.
        if presented.is_empty() {
            return LaunchDestination::Pending;
        }
        // The Feeds destination names no library: the saved identity matches
        // the presented kind or nothing, and the subscriptions land with the
        // config, so its Service is never pending.
        if *destination == TabIdentity::Feeds {
            return presented
                .iter()
                .copied()
                .find(|kind| matches!(kind, GuiDestinationKind::Feeds))
                .map_or(LaunchDestination::Unmatched, LaunchDestination::Open);
        }
        let matched = saved.and_then(|(saved_kind, library_id)| {
            presented.iter().copied().find(|kind| {
                kind.library_index().is_some_and(|lib_idx| {
                    self.gui_destination_library_id(saved_kind, lib_idx)
                        .is_some_and(|id| id == library_id)
                })
            })
        });
        matched.map_or(LaunchDestination::Unmatched, LaunchDestination::Open)
    }

    /// Whether the Service `kind`'s catalog can still land (design D9): the
    /// restore then retries instead of reading the saved identity as gone.
    /// Emby publishes its libraries when its startup attempt completes, and
    /// Audiobookshelf publishes them when its catalog fetch answers, so each
    /// Service's own pending mark is the signal.
    fn launch_catalog_pending(&self, kind: ServiceKind) -> bool {
        match kind {
            ServiceKind::Emby => {
                self.emby_runtime.state == mbv_core::service_runtime::ServiceState::Connecting
            }
            ServiceKind::Audiobookshelf => self.gui_abs_catalog_pending(),
        }
    }

    /// The stable Service library id the presented destination `kind` names
    /// at `lib_idx` (design D9): the identity the launch file saves and the
    /// restore resolves against. An Emby kind's index names `App::libs`; the
    /// Audiobooks and Podcasts kinds' index names the Audiobookshelf
    /// libraries.
    fn gui_destination_library_id(&self, kind: ServiceKind, lib_idx: usize) -> Option<String> {
        match kind {
            ServiceKind::Audiobookshelf => {
                Some(self.audiobookshelf_libraries.get(lib_idx)?.id.clone())
            }
            ServiceKind::Emby => Some(self.libs.get(lib_idx)?.library.id.clone()),
        }
    }

    /// Whether the Audiobookshelf catalog can still land (row 9.1): the
    /// startup is still running, or its catalog fetch is still in flight —
    /// either way the libraries list is not final, and a saved
    /// Audiobookshelf destination's resolution must wait for it.
    fn gui_abs_catalog_pending(&self) -> bool {
        self.audiobookshelf_runtime.state == mbv_core::service_runtime::ServiceState::Connecting
            || self.setup.audiobookshelf_catalog_rx.is_some()
    }

    /// The active Grouped Music library's group level: the library's root in
    /// `nav_stack`, whose items are the collection groups the pills paint.
    pub(in crate::app::gui_library) fn gui_music_group_level(
        &self,
        lib_idx: usize,
    ) -> Option<&BrowseLevel> {
        let lib = self.libs.get(lib_idx)?;
        lib.nav_stack
            .iter()
            .find(|level| level.parent_id == lib.library.id)
    }
}

/// What the launch location's first phase found (add-gui-library-panel
/// design D9): the start opened on Home, or no catalog has landed yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuiLaunchOpen {
    /// No destination is presented yet: the caller retries on the drain that
    /// delivers a destination.
    Retry,
    /// The start is open on the active destination (Home), and the saved
    /// collection sets apply while it stays active. The identity is the
    /// saved file's.
    Opened(TabIdentity),
}

/// A saved destination's resolution inside this module (design D9).
enum LaunchDestination {
    /// The presented destination to open.
    Open(GuiDestinationKind),
    /// A catalog can still land: the caller retries.
    Pending,
    /// The settled presented set holds no such destination.
    Unmatched,
}

/// The launch file's dropdown kind for one core target. The target's index
/// is the catalog's, and the entry's destination names the library, so only
/// the kind is written.
fn launch_collection_target(target: &GuiCollectionTarget) -> GuiLaunchCollectionTarget {
    match target {
        GuiCollectionTarget::Music(_) => GuiLaunchCollectionTarget::Music,
        GuiCollectionTarget::Podcasts(_) => GuiLaunchCollectionTarget::Podcasts,
        GuiCollectionTarget::Feeds => GuiLaunchCollectionTarget::Feeds,
        GuiCollectionTarget::EmbyFeed(_) => GuiLaunchCollectionTarget::EmbyFeed,
    }
}

/// The on-disk form of one list sort (design D10).
fn launch_list_sort(sort: GuiListSort) -> GuiLaunchSort {
    match sort {
        GuiListSort::Title => GuiLaunchSort::Title,
        GuiListSort::Author => GuiLaunchSort::Author,
        GuiListSort::Newest => GuiLaunchSort::Newest,
        GuiListSort::Oldest => GuiLaunchSort::Oldest,
    }
}

/// The core form of one saved list sort.
fn list_sort(saved: GuiLaunchSort) -> GuiListSort {
    match saved {
        GuiLaunchSort::Title => GuiListSort::Title,
        GuiLaunchSort::Author => GuiListSort::Author,
        GuiLaunchSort::Newest => GuiListSort::Newest,
        GuiLaunchSort::Oldest => GuiListSort::Oldest,
    }
}

/// The sort a collection entry's saved date order migrates to (design D10).
fn date_sort(order: GuiLaunchDateOrder) -> GuiListSort {
    match order {
        GuiLaunchDateOrder::Newest => GuiListSort::Newest,
        GuiLaunchDateOrder::Oldest => GuiListSort::Oldest,
    }
}

/// The on-disk form of the Shows view (design D14).
fn launch_shows_view(view: GuiShowsView) -> GuiLaunchShowsView {
    match view {
        GuiShowsView::All => GuiLaunchShowsView::All,
        GuiShowsView::New => GuiLaunchShowsView::New,
        GuiShowsView::Upcoming => GuiLaunchShowsView::Upcoming,
    }
}

/// The core form of the saved Shows view.
fn shows_view(view: GuiLaunchShowsView) -> GuiShowsView {
    match view {
        GuiLaunchShowsView::All => GuiShowsView::All,
        GuiLaunchShowsView::New => GuiShowsView::New,
        GuiLaunchShowsView::Upcoming => GuiShowsView::Upcoming,
    }
}

/// The saved identity of one Service library.
fn library_identity(kind: ServiceKind, library_id: &str) -> TabIdentity {
    TabIdentity::ServiceLibrary {
        kind,
        library_id: library_id.to_string(),
    }
}
