mod app_stub;

/// The parked `App` a Client wrapper moves out of position while it holds the
/// live one in a `GuiCore` (design D4a of `decouple-client-core`). It is a stub
/// construction, so it compiles in every build: shipped wrappers park it too.
pub use self::app_stub::handoff_app;

mod client_core;
mod client_drains;
pub mod config;
pub mod dispatch;
#[cfg(any(test, feature = "test"))]
pub use self::app_stub::stub_app_with_prefix_and_toast;
#[cfg(test)]
pub(crate) use self::app_stub::{HOME_POSITION, PEER_POSITION};
/// The whole-`App` fixtures two Clients' tests share (`2-split-tui-gui-clients`
/// design D4b rule 5): the stub Owner constructions, the queue shapes a menu or
/// transport test reads, and the Named-queue fixtures whose queue list and op
/// answers both Clients' tests drive.
#[cfg(any(test, feature = "test"))]
#[doc(hidden)]
pub use self::app_stub::{
    SeededAbsBook, close_initial_services, current, emby_unified_state, gui_app_with_named_queues,
    gui_app_with_peer, inject_local_events, install_test_emby, live_owner_channel, make_app_stub,
    make_audio_only_remote_app_stub_with_cmd_rx, make_local_daemon_app_stub,
    make_local_daemon_app_stub_with_cmd_rx, make_local_daemon_app_stub_with_cmd_rx_and_compat,
    make_remote_app_stub, make_remote_app_stub_at_index, make_remote_app_stub_with_cmd_rx,
    named_queue_list, op_answer, remote_stub_config, resting, resting_snapshot,
    seed_queue_row_menu_rows_for_test, seed_queue_shuffle_owner_for_test,
};
/// The stub fixtures the crate root re-exports for a Client's own test: the
/// Emby movie library a Service-seam leak would mutate, the card cache keys
/// the shell seeds a cached image under, and the entries the GUI queue row
/// menu opens with.
#[cfg(any(test, feature = "test"))]
#[doc(hidden)]
pub use self::app_stub::{
    card_cache_key_for_id_for_test, gui_client_capabilities, key_chord_for_test, make_audio_items,
    make_items, push_movies_library_for_test, queue_card_cache_key_for_test, queue_target_for_test,
};
#[cfg(any(test, feature = "test"))]
pub use self::app_stub::{
    set_cast_receivers_for_test, set_subtitle_tracks_for_test, stub_app_for_gui_client,
    stub_app_with_cached_emby_client, stub_app_with_cast_target,
    stub_app_with_completed_emby_startup, stub_app_with_completed_emby_startup_and_contents,
    stub_app_with_failed_emby_setup_form, stub_app_with_local_daemon_events,
    stub_app_with_pending_settings_save, stub_app_with_playing_items, stub_app_with_playing_queue,
    stub_app_with_queue, stub_app_with_queue_and_cmd_rx, stub_app_with_rebound_quit,
    stub_app_with_track_choices, stub_app_with_unsaved_queue,
};
mod image_cache;
mod infra;
pub mod input;
pub mod owner_link;
pub mod pin;
pub mod queue_card_artwork;
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
pub use self::infra::image_fetch::CardImageDrain;
pub use self::infra::open_url::open_url;
pub use self::infra::paging::PAGE_SIZE;
pub(in crate::app) use self::infra::paging::PREFETCH_AHEAD;
pub use self::infra::signals::{QUIT_REQUESTED, install_signal_handlers, start_quit_watchdog};
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
pub use self::state::app_struct::App;
pub use self::state::gui_client_events::GuiClientEvent;
pub use self::state::playback::HomeContent;
pub use self::status_bar_projection::StatusBarProjection;
pub(in crate::app) use self::waking_relay::spawn_waking_relay;
#[cfg(any(test, feature = "test"))]
pub mod test_seams;
#[cfg(test)]
use self::dispatch::notify::ToastSeverity;
use self::state::app_init::AppInit;
use self::state::bootstrap::bootstrap_unified_queue;
pub use self::state::playback_target::LocalPlaybackTarget;
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
use mbv_ui_model::settings::{PanelFocus, PanelMode};
use mbv_ui_model::tab_selection::TabSelection;
#[cfg(test)]
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
pub(crate) mod tests;
