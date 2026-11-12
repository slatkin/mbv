#[cfg(any(test, feature = "test"))]
mod app_stub;
mod client_core;
mod client_drains;
mod dispatch;
mod gui_client_state;
mod gui_library;
mod gui_settings;
#[cfg(feature = "test")]
pub use self::app_stub::{
    set_cast_receivers_for_test, set_subtitle_tracks_for_test, stub_app_with_cached_emby_client,
    stub_app_with_cast_target, stub_app_with_completed_emby_startup,
    stub_app_with_completed_emby_startup_and_contents, stub_app_with_failed_emby_setup,
    stub_app_with_local_daemon_events, stub_app_with_open_row_menu,
    stub_app_with_pending_settings_save, stub_app_with_playing_items, stub_app_with_playing_queue,
    stub_app_with_prefix_and_toast, stub_app_with_queue, stub_app_with_queue_and_cmd_rx,
    stub_app_with_rebound_quit, stub_app_with_track_choices, stub_app_with_track_popover,
    stub_app_with_unsaved_queue,
};
mod infra;
mod input;
mod playback_metadata;
mod playback_trickplay;
mod queue_card_artwork;
mod queue_footer_projection;
mod queue_playback_projection;
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
pub use self::gui_client_state::{
    GuiConfirmButtonProjection, GuiConfirmButtonTone, GuiOverlayProjection, PanelTogglesProjection,
    RowMenuEntryProjection, TrackPopoverChoice,
};
pub use self::gui_library::GuiLibraryCommand;
pub use self::gui_library::{
    GuiArtShape, GuiEntryListProjection, GuiEntryRow, GuiHomeHero, GuiHomeItemRow,
    GuiHomeProjection, GuiHomeRow, GuiHomeRowId, GuiHomeRowKind, GuiHomeRowState, GuiPillOption,
};
pub use self::gui_library::{
    GuiAudiobooksChapterRow, GuiAudiobooksHero, GuiAudiobooksProjection, GuiAudiobooksRow,
    GuiAudiobooksWorkspace,
};
pub use self::gui_library::{
    GuiDestinationKind, GuiEmbyFeedGroup, GuiEmbyHero, GuiEmbyHeroCredit, GuiEmbyPill,
    GuiEmbyProjection, GuiEmbyRow, GuiEmbyRowKind, GuiShowsEpisodeRow, GuiShowsHero, GuiShowsMode,
    GuiShowsPill, GuiShowsProjection, GuiShowsRow, GuiShowsSeasonNode, GuiShowsSeasonPill,
    GuiShowsSeries, GuiShowsWorkspace,
};
pub use self::gui_library::{
    GuiFeedEntryId, GuiFeedPill, GuiFeedsHero, GuiFeedsPillOption, GuiFeedsProjection, GuiFeedsRow,
    GuiFeedsSelection, GuiLaunchOpen, GuiLibraryArtwork, GuiLibraryMenuItem, GuiLibraryRowMenu,
    GuiLibraryTab, GuiLibraryTabs, GuiMusicAlbum, GuiMusicArtist, GuiMusicHero, GuiMusicPill,
    GuiMusicProjection, GuiMusicTrack, GuiMusicTrackTotal, GuiPlayheadMarks, GuiPodcastEpisodeId,
    GuiPodcastPill, GuiPodcastsHero, GuiPodcastsPillOption, GuiPodcastsProjection, GuiPodcastsRow,
    GuiSearchProjection, GuiSearchRow,
};
pub use self::gui_settings::{
    GuiSettingsChoiceProjection, GuiSettingsCommand, GuiSettingsFeedProjection,
    GuiSettingsFeedsProjection, GuiSettingsGroup, GuiSettingsKeyRefusalProjection,
    GuiSettingsKeyRowProjection, GuiSettingsKeysProjection, GuiSettingsLibraryProjection,
    GuiSettingsOpenPlaylistProjection, GuiSettingsPlaylistItemProjection,
    GuiSettingsPlaylistRowProjection, GuiSettingsPlaylistsProjection,
    GuiSettingsRouteDeviceProjection, GuiSettingsRouteProjection, GuiSettingsRowKind,
    GuiSettingsRowProjection, GuiSettingsSection, GuiSettingsServiceProjection,
    GuiSettingsServicesProjection, GuiSettingsSetupProjection, GuiSettingsState, GuiSetupService,
    SetupSecret,
};
pub use self::infra::image_fetch::CardImageDrain;
pub(in crate::app) use self::infra::paging::{PAGE_SIZE, PREFETCH_AHEAD};
pub(in crate::app) use self::infra::signals::{
    QUIT_REQUESTED, install_signal_handlers, start_quit_watchdog,
};
pub(crate) use self::infra::terminal::set_mouse_capture;
pub(in crate::app) use self::infra::terminal::{init_terminal, open_url, restore_terminal};
pub use self::input::gui::GuiInput;
pub use self::input::gui::GuiSurfaceLeaf;
pub use self::input::key_policy::{
    OverlayFocus, RemoteControlTarget, RouterOverlayState, RouterPlaybackState, RouterSnapshot,
};
pub use self::input::router::{RouterOutcome, resolve_router_outcome_with_focused};
pub use self::playback_metadata::PlaybackDecorations;
pub use self::playback_trickplay::{TrickplayPreviewTile, TrickplaySheetKey};
pub use self::queue_card_artwork::QueueCardArtwork;
pub use self::queue_footer_projection::{FooterPill, QueueFooterProjection};
pub use self::queue_playback_projection::{
    PlaybackChapter, PlaybackTargetChoice, QueuePlaybackProjection, SegmentMark, TrackChip,
    TrackChips, TrackChoice, TrickplayGrid, TrickplayTile,
};
pub use self::queue_summary_projection::{QueueRightColumn, QueueSummaryProjection};
pub use self::queue_surface::QueueSurfaceProjection;
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
use self::state::app_init::AppInit;
use self::state::bootstrap::bootstrap_unified_queue;
use self::state::playback_target::LocalPlaybackTarget;
use crate::app::state::events::SessionEvent;
pub(in crate::app) use crate::app::state::events::{
    AudiobookshelfEvent, BrowseEvent, LibEvent, ModelContentEvent, MusicEvent, PlaylistEvent,
    SeriesEvent,
};
use crate::app::state::playback::{
    DestinationLatestSource, PendingQueueAction, QueueLinkResolution, RemoteSlotState,
    ReplacementExecutor, RoutedReplacementPrep, SuspendedLocalSession, UndoEntry,
};
use crate::app::state::queue_deferrals::QueueDeferrals;
pub(crate) use crate::app::state::queue_view::QueueView;
#[cfg(test)]
use mbv_ctrl::player::PlayerEvent;
#[cfg(test)]
use mbv_emby_model::EmbyItem;
pub(crate) use mbv_images::resize::spawn_resize_worker;
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
pub(in crate::app) use mbv_ui_model::overlay::SidebarId;
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
