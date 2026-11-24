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

mod local_daemon;
mod owner_restart;
mod pin;
mod startup;
mod tui;

pub use mbv_client_core::{
    App, AppCommand, CLIENT_IDLE_CADENCE, CardImageDrain, ClientCore, ClientLifecycle,
    ClientTickHost, ClientWaker, Command, FooterPill, HomeContent, HomeEventDrain,
    NavigationIntent, NowPlayingMediaKind, OverlayFocus, PanelIntent, PlaybackIntent,
    PlaybackTargetChoice, PlayerEventDrain, QueueCardArtwork, QueueEndOffsets,
    QueueFooterProjection, QueuePlaybackProjection, QueueSummaryProjection, QueueSurfaceProjection,
    RemoteControlTarget, RouterNow, RouterOutcome, RouterOverlayState, RouterPlaybackState,
    RouterSnapshot, ShellCommand, StartupWorkerDrain, StatusBarProjection, TickOutcome, TrackChip,
    TrackChips, TrackChoice, VisualIntent, WorkspaceRelayout,
};

pub use self::startup::run;
pub use mbv_client_core::config::{Config, load_config};
pub use mbv_client_core::owner_link::{
    OwnerStartOptions, OwnerState, attach_owner, exclusive_owner_refusal_text, find_owner,
    refusal_text, start_owner,
};
pub use mbv_client_core::pin::claim_pin_swap_participant;
pub use mbv_feed::FeedPlatform;
pub use mbv_images::ImageDecode;
pub use mbv_ui_model::toast::{ToastCard, ToastId};
pub use mbv_ui_msg::TrackChipKind;

/// The capabilities a Client builds at attach time and `attach_owner` takes
/// in place of a Client kind (design D6a of `decouple-client-core`). A
/// Client outside the core names them here.
pub use mbv_client_core::state::client_capabilities::{ClientCapabilities, ClientCapability};

/// The parked `App` a Client wrapper moves out of position while it holds the
/// live one in a `GuiCore` (design D4a of `decouple-client-core`). A stub
/// construction with no live Owner, Service, or player event, so the wrapper
/// drops it as soon as the real `App` comes back.
pub use mbv_client_core::handoff_app;

pub use mbv_client_core::LibraryFetchEvent;
/// The connect, session-load, URL-open, and pinned-request overrides a
/// dependent Client's tests install (design D4e): the core reads them, and a
/// Client's test target enables the `test` feature to name them.
#[cfg(any(test, feature = "test"))]
#[doc(hidden)]
pub use mbv_client_core::pin::{PINNED_REQUEST_OVERRIDE, PINNED_REQUEST_TEST_LOCK};
#[cfg(any(test, feature = "test"))]
#[doc(hidden)]
pub use mbv_client_core::test_seams::{
    DAEMON_ROUTE_CONNECT_OVERRIDE, DAEMON_ROUTE_CONNECT_TEST_LOCK, DIRECT_CONNECT_OVERRIDE,
    DIRECT_CONNECT_TEST_LOCK, OPEN_URL_OVERRIDE, OPEN_URL_TEST_LOCK, SESSIONS_LOAD_OVERRIDE,
};
/// The stub fixtures a Client's own test shares with the root package's:
/// the Emby movie library a Service-seam leak would mutate, the card cache
/// keys the shell seeds a cached image under, and the entries the GUI queue
/// row menu opens with.
#[cfg(any(test, feature = "test"))]
#[doc(hidden)]
pub use mbv_client_core::{
    SeededAbsBook, app_with_adopted_named_queues, app_with_connected_peer,
    card_cache_key_for_id_for_test, close_initial_services, current, emby_unified_state,
    inject_local_events, install_test_emby, key_chord_for_test, live_owner_channel, make_app_stub,
    make_audio_items, make_audio_only_remote_app_stub_with_cmd_rx, make_items,
    make_local_daemon_app_stub, make_local_daemon_app_stub_with_cmd_rx,
    make_local_daemon_app_stub_with_cmd_rx_and_compat, make_remote_app_stub,
    make_remote_app_stub_at_index, make_remote_app_stub_with_cmd_rx, named_queue_list, op_answer,
    push_movies_library_for_test, queue_card_cache_key_for_test, queue_target_for_test,
    remote_stub_config, resting, resting_snapshot, seed_queue_row_menu_rows_for_test,
    seed_queue_shuffle_owner_for_test,
};
#[cfg(any(test, feature = "test"))]
pub use mbv_client_core::{
    set_cast_receivers_for_test, set_subtitle_tracks_for_test, stub_app_for_gui_client,
    stub_app_with_cached_emby_client, stub_app_with_cast_target,
    stub_app_with_completed_emby_startup, stub_app_with_completed_emby_startup_and_contents,
    stub_app_with_local_daemon_events, stub_app_with_pending_settings_save,
    stub_app_with_playing_items, stub_app_with_playing_queue, stub_app_with_prefix_and_toast,
    stub_app_with_queue, stub_app_with_queue_and_cmd_rx, stub_app_with_rebound_quit,
    stub_app_with_track_choices, stub_app_with_unsaved_queue, transport_client_capabilities,
};
