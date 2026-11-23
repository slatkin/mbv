mod app_stub;

/// The parked `App` a Client wrapper moves out of position while it holds the
/// live one in a `GuiCore` (design D4a of `decouple-client-core`). It is a stub
/// construction, so it compiles in every build: shipped wrappers park it too.
pub use self::app_stub::handoff_app;

mod client_core;
mod client_drains;
pub mod config;
pub mod dispatch;
pub mod gui_client_state;
mod gui_library;
mod gui_playlists;
mod gui_settings;
#[cfg(any(test, feature = "test"))]
pub use self::app_stub::gui_state_for_test;
#[cfg(feature = "test")]
pub use self::app_stub::seeded_gui_state_for_test;
#[cfg(any(test, feature = "test"))]
pub use self::app_stub::stub_app_with_prefix_and_toast;
#[cfg(feature = "test")]
pub use self::app_stub::{
    drain_lib_event_with_stores_for_test, gui_client_capabilities, set_cast_receivers_for_test,
    set_subtitle_tracks_for_test, stub_app_for_gui_client, stub_app_with_cached_emby_client,
    stub_app_with_cast_target, stub_app_with_completed_emby_startup,
    stub_app_with_completed_emby_startup_and_contents, stub_app_with_failed_emby_setup,
    stub_app_with_local_daemon_events, stub_app_with_open_row_menu,
    stub_app_with_pending_settings_save, stub_app_with_playing_items, stub_app_with_playing_queue,
    stub_app_with_queue, stub_app_with_queue_and_cmd_rx, stub_app_with_rebound_quit,
    stub_app_with_track_choices, stub_app_with_track_popover, stub_app_with_unsaved_queue,
};
mod image_cache;
mod infra;
mod input;
pub mod owner_link;
pub mod pin;
pub mod projection;
mod queue_card_artwork;
mod queue_footer_projection;
pub mod queue_playback_projection;
mod queue_summary_projection;
mod queue_surface;
pub mod state;
mod status_bar_projection;
mod version_rows;
mod waking_relay;

pub use self::client_core::{
    CLIENT_IDLE_CADENCE, ClientCore, ClientTickHost, ClientWaker, TickOutcome, WorkspaceRelayout,
};
pub use self::client_drains::{
    ClientLifecycle, HomeEventDrain, PlayerEventDrain, StartupWorkerDrain,
};
pub use self::dispatch::action::{
    AppCommand, Command, NavigationIntent, PanelIntent, PlaybackIntent, ShellCommand, VisualIntent,
};
pub use self::dispatch::library::stores::drain_lib_event_through;
pub use self::gui_client_state::GuiClientState;
pub use self::gui_client_state::{
    GuiConfirmButtonProjection, GuiConfirmButtonTone, GuiOverlayProjection, PanelTogglesProjection,
    QueueChooserEntryProjection, QueueChooserRenameProjection, QueueHeaderProjection,
    RowMenuEntryProjection, TrackPopoverChoice,
};
/// The Client-only fetch result a dependent Client's test scripts onto the
/// `App`'s channel (`decouple-client-core` design D5): the next tick's
/// `drain_client_events` takes it exactly as it takes a worker's own send.
#[cfg(feature = "test")]
#[doc(hidden)]
pub use self::gui_library::client_events::GuiClientEvent;
pub use self::gui_library::client_events::install_client_event;
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
    GuiSearchProjection, GuiSearchRow, GuiSortTarget, gui_feeds_platform,
};
pub use self::gui_library::{
    GuiLibraryCommand, GuiRowArt, GuiRowArtScope, GuiRowMediaKind, GuiRowSource,
    artist_rows_in_view_id, playing_track_id_for_shell,
};
#[cfg(test)]
pub(in crate::app) use self::gui_library::{gui_active_destination, gui_library_tabs};
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
pub use self::infra::image_fetch::CardImageDrain;
pub use self::infra::open_url::open_url;
pub use self::infra::paging::PAGE_SIZE;
pub(in crate::app) use self::infra::paging::PREFETCH_AHEAD;
pub use self::infra::signals::{QUIT_REQUESTED, install_signal_handlers, start_quit_watchdog};
pub use self::input::gui::GuiInput;
pub use self::input::gui::GuiSurfaceLeaf;
pub use self::input::key_policy::{
    OverlayFocus, RemoteControlTarget, RouterNow, RouterOverlayState, RouterPlaybackState,
    RouterSnapshot,
};
pub use self::input::router::RouterOutcome;
pub use self::queue_card_artwork::QueueCardArtwork;
pub use self::queue_footer_projection::{FooterPill, QueueFooterProjection};
pub use self::queue_playback_projection::{
    NowPlayingMediaKind, PlaybackTargetChoice, QueuePlaybackProjection, TrackChip, TrackChips,
    TrackChoice,
};
pub use self::queue_summary_projection::{QueueEndOffsets, QueueSummaryProjection};
pub use self::queue_surface::QueueSurfaceProjection;
pub(crate) use self::shell::set_mouse_capture;
pub use self::state::app_struct::App;
pub use self::state::playback::HomeContent;
pub use self::status_bar_projection::StatusBarProjection;
pub(in crate::app) use self::waking_relay::spawn_waking_relay;
#[cfg(test)]
mod test_seams;
#[cfg(test)]
pub(in crate::app) use self::test_seams::{
    DAEMON_ROUTE_CONNECT_OVERRIDE, DAEMON_ROUTE_CONNECT_TEST_LOCK, DIRECT_CONNECT_OVERRIDE,
    DIRECT_CONNECT_TEST_LOCK, OPEN_URL_OVERRIDE, OPEN_URL_TEST_LOCK, SESSIONS_LOAD_OVERRIDE,
};
mod shell;
use self::dispatch::notify::ToastSeverity;
pub use self::shell::Model;
/// The TUI's image pickers, built before the `TuiRealm` listener starts
/// (#654); the `Model` constructor takes the store it returns.
pub use self::shell::init_image_protocols;
use self::state::app_init::AppInit;
use self::state::bootstrap::bootstrap_unified_queue;
use self::state::playback_target::LocalPlaybackTarget;
use crate::app::state::events::SessionEvent;
pub(in crate::app) use crate::app::state::events::{AudiobookshelfEvent, PlaylistEvent};
pub use crate::app::state::events::{
    BrowseEvent, LibEvent, ModelContentEvent, MusicEvent, SeriesEvent,
};
use crate::app::state::playback::{
    DestinationLatestSource, PendingQueueAction, QueueLinkResolution, QueueTarget, RemoteSlotState,
    ReplacementExecutor, RoutedReplacementPrep, SuspendedLocalSession, UndoEntry,
};
use crate::app::state::queue_deferrals::QueueDeferrals;
pub use crate::app::state::queue_view::QueueView;
#[cfg(test)]
use mbv_ctrl::player::PlayerEvent;
#[cfg(test)]
use mbv_emby_model::EmbyItem;
#[cfg(test)]
use mbv_ui_model::browse::restore_library_position;
use mbv_ui_model::browse::{
    AlbumIndex, AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel, SeriesDetail,
    restore_library_position_with_fetched_rows_for_kind,
};
use mbv_ui_model::confirm::{ConfirmAction, ConfirmModal};
#[cfg(test)]
use mbv_ui_model::context_menu::MultiSelectKind;
use mbv_ui_model::context_menu::{ContextAction, ContextMenuAnchor, ContextMenuEntry};
use mbv_ui_model::daemon_lost::DaemonLostModal;
use mbv_ui_model::feed::{
    FeedHomeVideoSelection, FeedHomeVideoState, SavePlaylistDialog, SavePlaylistStage,
};
use mbv_ui_model::library_tab::LibraryTab;
pub use mbv_ui_model::overlay::SidebarId;
use mbv_ui_model::playback::QueueLink;
#[cfg(test)]
use mbv_ui_model::settings::SettingKey;
use mbv_ui_model::settings::{PanelFocus, PanelMode};
use mbv_ui_model::tab_selection::TabSelection;
#[cfg(test)]
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
pub(crate) mod tests;
