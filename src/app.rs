#[cfg(any(test, feature = "test"))]
mod app_stub;
mod client_core;
mod client_drains;
mod dispatch;
#[cfg(feature = "test")]
pub use self::app_stub::{
    stub_app_with_pending_settings_save, stub_app_with_playing_queue,
    stub_app_with_prefix_and_toast, stub_app_with_queue, stub_app_with_unsaved_queue,
};
mod infra;
mod input;
mod queue_card_artwork;
mod queue_footer_projection;
mod queue_playback_projection;
mod queue_surface;
pub mod state;
mod status_bar_projection;
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
pub use self::queue_card_artwork::QueueCardArtwork;
pub use self::queue_footer_projection::{FooterPill, QueueFooterProjection};
pub use self::queue_playback_projection::QueuePlaybackProjection;
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
    DIRECT_CONNECT_TEST_LOCK, SESSIONS_LOAD_OVERRIDE,
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
    FeedHomeVideoSelection, FeedHomeVideoState, IdleFeed, SavePlaylistDialog, SavePlaylistStage,
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
