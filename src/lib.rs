//! The `mbv` library: the shell `App`, the Client core tick that every Client
//! drives, the central keyboard router, and the Client-side Owner link
//! (design D2 and D7 of `extract-client-core`).
//!
//! The `mbv` binary is a thin `main` over [`run`], and a GUI Client
//! program shares the rest of this surface: the core tick, the router
//! entry, and the Owner find, start, and attach steps. The library still
//! carries the TUI Client's dependencies — `ratatui`, `crossterm`, and
//! `TuiRealm` — so any Client that links it links them; it never carries a
//! window stack.

mod app;
mod config;
mod launch;
mod local_daemon;
mod owner_link;
mod owner_restart;
mod pin;
mod single_instance;
mod startup;

pub use self::app::{
    App, AppCommand, CLIENT_IDLE_CADENCE, CardImageDrain, ClientCore, ClientLifecycle,
    ClientTickHost, ClientWaker, Command, FooterPill, GuiConfirmButtonProjection,
    GuiConfirmButtonTone, GuiInput, GuiLibraryArtwork, GuiLibraryCommand, GuiLibraryMenuItem,
    GuiLibraryRowMenu, GuiLibraryTab, GuiLibraryTabs, GuiMusicAlbum, GuiMusicArtist, GuiMusicHero,
    GuiMusicPill, GuiMusicProjection, GuiMusicTrack, GuiMusicTrackTotal, GuiOverlayProjection,
    GuiSurfaceLeaf, HomeContent, HomeEventDrain, NavigationIntent, OverlayFocus, PanelIntent,
    PanelTogglesProjection, PlaybackChapter, PlaybackDecorations, PlaybackIntent,
    PlaybackTargetChoice, PlayerEventDrain, QueueCardArtwork, QueueFooterProjection,
    QueuePlaybackProjection, QueueRightColumn, QueueSummaryProjection, QueueSurfaceProjection,
    RemoteControlTarget, RouterOutcome, RouterOverlayState, RouterPlaybackState, RouterSnapshot,
    RowMenuEntryProjection, SegmentMark, ShellCommand, StartupWorkerDrain, StatusBarProjection,
    TickOutcome, TrackChip, TrackChips, TrackChoice, TrackPopoverChoice, TrickplayGrid,
    TrickplayPreviewTile, TrickplaySheetKey, TrickplayTile, VisualIntent, WorkspaceRelayout,
    resolve_router_outcome_with_focused,
};
pub use self::config::{Config, load_config};
pub use self::owner_link::{
    OwnerStartOptions, OwnerState, attach_owner, declare_gui_client, exclusive_owner_refusal_text,
    find_owner, refusal_text, start_owner,
};
pub use self::startup::run;
pub use mbv_ui_msg::TrackChipKind;

/// The `App` stub construction a Client's own tests build their `App` from
/// (`extract-client-core` design D11). Compiled only under the root package's
/// `test` feature, which no shipped build enables.
#[cfg(feature = "test")]
pub use self::app::{
    set_cast_receivers_for_test, set_subtitle_tracks_for_test, stub_app_with_cached_emby_client,
    stub_app_with_cast_target, stub_app_with_completed_emby_startup,
    stub_app_with_local_daemon_events, stub_app_with_open_row_menu,
    stub_app_with_pending_settings_save, stub_app_with_playing_items, stub_app_with_playing_queue,
    stub_app_with_prefix_and_toast, stub_app_with_queue, stub_app_with_queue_and_cmd_rx,
    stub_app_with_rebound_quit, stub_app_with_track_choices, stub_app_with_track_popover,
    stub_app_with_unsaved_queue,
};
