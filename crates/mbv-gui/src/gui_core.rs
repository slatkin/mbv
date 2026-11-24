//! The GUI Client's core state (design D4 of `decouple-client-core`): the
//! shared `App` the GUI owns, wrapped by composition.
//!
//! `GuiCore`, the GUI modules, and the GUI routing entry live here, outside
//! `mbv-client-core`, so the boundary rehearsal takes their module shape now
//! (`2-split-tui-gui-clients` design D4c). The GUI's `CoreThread` holds one
//! `GuiCore` and keeps its `ClientTickHost` implementation; no `App` field
//! becomes `pub` for it (this change's D4), so the owned `App` stays behind
//! this field, visible only inside this module. The GUI state
//! (`GuiClientState`) sits beside it, so a GUI `impl GuiCore` block reads the
//! core as `self.app.<...>` and its Client state as `self.gui.<...>`.

mod gui_client_state;
mod gui_library;
mod gui_playlists;
mod gui_settings;
mod input;

#[cfg(any(test, feature = "test"))]
pub mod test_support;
#[cfg(test)]
mod tests;

pub use self::gui_client_state::GuiClientState;
pub use self::gui_client_state::{
    GuiConfirmButtonProjection, GuiConfirmButtonTone, GuiOverlayProjection, PanelTogglesProjection,
    QueueChooserEntryProjection, QueueChooserRenameProjection, QueueHeaderProjection,
    RowMenuEntryProjection, TrackPopoverChoice,
};
pub use self::gui_library::client_events::{ClientEventLanding, install_client_event};
pub use self::gui_library::{
    GuiArtShape, GuiCollectionEntry, GuiCollectionNoun, GuiCollectionSelection, GuiCollectionSet,
    GuiCollectionTarget, GuiCollections, GuiDateOrder, GuiEntryListProjection, GuiEntryRow,
    GuiHomeHero, GuiHomeItemRow, GuiHomeProjection, GuiHomeRow, GuiHomeRowId, GuiHomeRowKind,
    compare_gui_date_order,
};
pub use self::gui_library::{
    GuiAudiobooksChapterRow, GuiAudiobooksHero, GuiAudiobooksProjection, GuiAudiobooksRow,
    GuiAudiobooksWorkspace,
};
pub use self::gui_library::{
    GuiDestinationKind, GuiEmbyHero, GuiEmbyHeroCredit, GuiEmbyProjection, GuiEmbyRow,
    GuiEmbyRowKind, GuiShowsEpisodeRow, GuiShowsHero, GuiShowsProjection, GuiShowsRow,
    GuiShowsSeasonNode, GuiShowsSeasonPill, GuiShowsSeries, GuiShowsView, GuiShowsWorkspace,
};
pub use self::gui_library::{
    GuiFeedEntryId, GuiFeedsHero, GuiFeedsProjection, GuiFeedsRow, GuiFeedsSelection,
    GuiLaunchOpen, GuiLibraryArtwork, GuiLibraryMenuItem, GuiLibraryMenuRow, GuiLibraryRowMenu,
    GuiLibraryTab, GuiLibraryTabs, GuiListSort, GuiListSortFrame, GuiMusicAlbum, GuiMusicArtist,
    GuiMusicHero, GuiMusicProjection, GuiMusicTrack, GuiMusicTrackTotal, GuiPlayheadMarks,
    GuiPodcastEpisodeId, GuiPodcastsHero, GuiPodcastsProjection, GuiPodcastsRow,
    GuiSearchProjection, GuiSearchRow, GuiSortTarget,
};
pub use self::gui_library::{
    GuiLibraryCommand, GuiRowArt, GuiRowArtScope, GuiRowMediaKind, GuiRowSource,
    artist_rows_in_view_id, playing_track_id_for_shell,
};
#[cfg(test)]
pub(crate) use self::gui_library::{gui_active_destination, gui_library_tabs};
pub use self::gui_playlists::{
    GuiOpenPlaylistProjection, GuiPlaylistItemProjection, GuiPlaylistRowProjection,
    GuiPlaylistsCommand, GuiPlaylistsProjection,
};
pub use self::gui_settings::{
    GuiSettingsChoiceProjection, GuiSettingsCommand, GuiSettingsFeedProjection,
    GuiSettingsFeedsProjection, GuiSettingsGroup, GuiSettingsKeyRefusalProjection,
    GuiSettingsKeyRowProjection, GuiSettingsKeysProjection, GuiSettingsLibraryProjection,
    GuiSettingsRouteDeviceProjection, GuiSettingsRouteProjection, GuiSettingsRowKind,
    GuiSettingsRowProjection, GuiSettingsSection, GuiSettingsServiceProjection,
    GuiSettingsServicesProjection, GuiSettingsSetupProjection, GuiSettingsState, GuiSetupService,
    SetupSecret,
};
pub use self::input::{GuiInput, GuiSurfaceLeaf};

use mbv_client_core::App;
use mbv_client_core::dispatch::notify::ToastSeverity;

/// The GUI Client's core state: the `App` it owns and its own Client state.
/// Every GUI `impl` block becomes `impl GuiCore` and reads its core state as
/// `self.app.<...>` (design D4).
pub struct GuiCore {
    /// The owned core. Its `impl GuiCore` blocks live in this module and its
    /// children, so the field is visible there; every `App` field stays as
    /// private as it was.
    pub(crate) app: App,
    /// The GUI Client's own state (design D4): every field only GUI code
    /// reads or writes. It is its own `ClientLibraryStores` implementor, so
    /// the core's library seam reads it as this Client's stores.
    pub(crate) gui: GuiClientState,
}

impl GuiCore {
    /// The GUI Client's core over an attached `App`.
    #[must_use]
    pub fn new(app: App) -> Self {
        let player_auto_hide =
            self::gui_client_state::player_auto_hide_from_prefs(&App::load_prefs());
        Self {
            app,
            gui: GuiClientState::new(player_auto_hide),
        }
    }

    /// The shared `App` the GUI Client owns: the tick host's drains, the
    /// router, and the projections read and mutate it.
    #[must_use]
    pub fn app(&self) -> &App {
        &self.app
    }

    /// The shared `App` the GUI Client owns, for mutation.
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// The GUI Client's own state, for the Client's test seeds (design D4):
    /// the seeds write the stores a landed fetch would, so a Client's own test
    /// drives the production `GuiCore` and never a second copy of its state.
    #[cfg(any(test, feature = "test"))]
    #[doc(hidden)]
    #[must_use]
    pub fn gui_state(&mut self) -> &mut GuiClientState {
        &mut self.gui
    }

    /// The GUI Client's own state, for the core seams that keep stores. Only
    /// the root package's own GUI test helpers swap a state in and out of it;
    /// every other caller reaches the state through this type's methods.
    pub(crate) fn gui_state_mut(&mut self) -> &mut GuiClientState {
        &mut self.gui
    }

    /// Take one pending library event and apply it through the GUI Client's
    /// own stores (`decouple-client-core` design D4a (c), row 8.1): the
    /// event's App-owned half runs in the core, and the store work it
    /// collects applies to this Client's `GuiClientState` before the next
    /// event. The GUI's tick host calls this once per event.
    pub fn drain_lib_event(&mut self) -> bool {
        mbv_client_core::drain_lib_event_through(&mut self.app, &mut self.gui)
    }

    /// Take one Client-only fetch result and install it in this Client's own
    /// stores (design D5, D8): the GUI's tick host calls this once per queued
    /// result, looping, after the library drain, for the waker to have woken
    /// it for. A failed refresh kept the store's loaded value, so this drain
    /// reports it as the error toast the refresh path promises. The channel
    /// lives on the owned `App`, so a result a worker must send outlives any
    /// one `GuiCore`.
    pub fn drain_client_event(&mut self) -> bool {
        let Some(event) = self.app.take_one_library_fetch_event() else {
            return false;
        };
        if install_client_event(&mut self.gui, event) == ClientEventLanding::RefreshFailed {
            self.app
                .flash("Refresh failed".to_string(), ToastSeverity::Error);
        }
        true
    }

    /// Take the GUI Client's own state a dependent Client's test seeds beside
    /// its `App` (`decouple-client-core` row 8.1), and publish this core's
    /// back for that test's reads.
    ///
    /// The tick host makes both calls at one step's edges: `take` at the
    /// start, so the step reads the state a test seeded between steps, and
    /// [`Self::publish_seeded_gui_state_for_test`] at the end, so the test's
    /// own `*_for_test` reads see what the step left. Only a dependent
    /// Client's `test` build keeps the sidecar: this crate's own tests hold
    /// their state in `test_support`, and a build without the `test`
    /// feature holds none, so there the pair is empty and the calls compile
    /// out. The tick host's step makes them in every build.
    pub fn take_seeded_gui_state_for_test(&mut self) {
        #[cfg(all(not(test), feature = "test"))]
        {
            let seeded =
                crate::gui_core::test_support::seeded_gui_state_for_test(std::option::Option::take);
            if let Some(seeded) = seeded {
                self.gui = seeded;
            }
        }
    }

    /// Publish this core's own state back to the sidecar a dependent Client's
    /// test reads beside its `App` (`decouple-client-core` row 8.1), so the
    /// state a step left is the one the test's `*_for_test` reads answer with.
    /// Pairs with [`Self::take_seeded_gui_state_for_test`], and compiles out
    /// with it everywhere else.
    pub fn publish_seeded_gui_state_for_test(&self) {
        #[cfg(all(not(test), feature = "test"))]
        crate::gui_core::test_support::seeded_gui_state_for_test(|slot| {
            *slot = Some(self.gui.clone());
        });
    }

    /// The shared `App` back out of the wrapper, for a caller that wraps the
    /// `App` it owns for one call and takes it back afterwards.
    #[must_use]
    pub fn into_app(self) -> App {
        self.app
    }

    /// Take the `App` back into the caller's slot, keeping this Client's own
    /// state beside it (the root package's test helpers, task 8.1): the state
    /// a call left installs in the slot's sidecar.
    #[cfg(test)]
    pub(crate) fn into_app_keeping_gui(self, app: &mut App) {
        let Self { app: owned, gui } = self;
        *app = owned;
        crate::gui_core::test_support::stub_gui_state(|slot| *slot = gui);
    }
}

impl mbv_client_core::RouterNow for GuiCore {
    /// The armed bit the router reads for this Client. It lives beside the
    /// core the GUI owns, so the routing entry reads it through the router's
    /// neutral access instead of the router reading the routing site's
    /// fields (design D6 of `decouple-client-core`, row 8.4).
    fn prefix_armed(&self) -> bool {
        self.app.prefix_armed()
    }
}

impl std::fmt::Debug for GuiCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiCore")
            .field("app", &self.app)
            .field("gui", &self.gui)
            .finish()
    }
}
