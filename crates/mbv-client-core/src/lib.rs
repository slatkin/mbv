//! The mbv Client core.
//!
//! One core answers for both mbv Clients: the terminal Client (`mbv`) and the
//! GUI Client (`mbv-gui`). It holds the Client-side Owner link, the Service
//! setup and queue authority, playback dispatch, and the keyboard routing that
//! both Clients share. It names no terminal or window toolkit, so neither
//! Client's dependencies reach the other's.
//!
//! The root package re-exports this surface under its own names until the
//! Clients are separate crates (`2-split-tui-gui-clients` design D1 and D4c).

mod app;

pub use self::app::{
    App, AppCommand, CLIENT_IDLE_CADENCE, CardImageDrain, ClientCore, ClientLifecycle,
    ClientTickHost, ClientWaker, Command, FooterPill, HomeContent, HomeEventDrain,
    NavigationIntent, NowPlayingMediaKind, OverlayFocus, PanelIntent, PlaybackIntent,
    PlaybackTargetChoice, PlayerEventDrain, QueueCardArtwork, QueueEndOffsets,
    QueueFooterProjection, QueuePlaybackProjection, QueueSummaryProjection, QueueSurfaceProjection,
    RemoteControlTarget, RouterNow, RouterOutcome, RouterOverlayState, RouterPlaybackState,
    RouterSnapshot, ShellCommand, StartupWorkerDrain, StatusBarProjection, TickOutcome, TrackChip,
    TrackChips, TrackChoice, VisualIntent, WorkspaceRelayout,
};

/// The rest of the core's own names a Client uses: the events both Clients
/// drain, the paging and signal facts, and the helpers the root modules call.
pub use self::app::{
    BrowseEvent, LibEvent, LocalPlaybackTarget, ModelContentEvent, MusicEvent, PAGE_SIZE,
    QUIT_REQUESTED, SeriesEvent, SidebarId, drain_lib_event_through, install_signal_handlers,
    open_url, start_quit_watchdog,
};

pub use self::app::config::{Config, load_config};
pub use self::app::owner_link::{
    OwnerStartOptions, OwnerState, attach_owner, exclusive_owner_refusal_text, find_owner,
    refusal_text, start_owner,
};
pub use self::app::pin::claim_pin_swap_participant;

/// The capabilities a Client builds at attach time and `attach_owner` takes
/// in place of a Client kind (design D6a of `decouple-client-core`). A
/// Client outside the core names them here.
pub use self::app::state::client_capabilities::{ClientCapabilities, ClientCapability};

/// The parked `App` a Client wrapper moves out of position while it holds the
/// live one in its own core (design D4a of `decouple-client-core`). A stub
/// construction with no live Owner, Service, or player event, so the wrapper
/// drops it as soon as the real `App` comes back.
pub use self::app::handoff_app;

pub use self::app::LibraryFetchEvent;

/// The core modules a Client reaches by their own path. The crate root
/// re-exports them so that a Client names one crate for the whole core, and the
/// move changes only paths (design D4c).
pub use self::app::{config, dispatch, input, owner_link, pin};
pub use self::app::{queue_playback_projection, state};

/// The artist-artwork cache key a Client's test names when it asserts the shared
/// image boundary's key for a typed artist artwork request.
#[cfg(any(test, feature = "test"))]
#[doc(hidden)]
pub use self::app::artist_artwork_cache_key;
/// The connect, session-load, URL-open, and pinned-request overrides a
/// dependent Client's tests install (design D4e): the core reads them, and a
/// Client's test target enables the `test` feature to name them.
#[cfg(any(test, feature = "test"))]
#[doc(hidden)]
pub use self::app::pin::{PINNED_REQUEST_OVERRIDE, PINNED_REQUEST_TEST_LOCK, PINNED_REQUESTS};
#[cfg(any(test, feature = "test"))]
pub use self::app::test_seams;
#[cfg(any(test, feature = "test"))]
#[doc(hidden)]
pub use self::app::test_seams::{
    DAEMON_ROUTE_CONNECT_OVERRIDE, DAEMON_ROUTE_CONNECT_TEST_LOCK, DIRECT_CONNECT_OVERRIDE,
    DIRECT_CONNECT_TEST_LOCK, OPEN_URL_OVERRIDE, OPEN_URL_TEST_LOCK, SESSIONS_LOAD_OVERRIDE,
};
/// The core's own fixtures and test-trait prelude (`2-split-tui-gui-clients`
/// design D4e): a Client's tests name `mbv_client_core::tests` exactly as the
/// one-crate tests named `crate::app::tests`. The `test` feature alone makes it
/// reachable, so a shipped build has no such path.
#[cfg(any(test, feature = "test"))]
#[doc(hidden)]
pub use self::app::tests;
/// The whole-`App` fixtures two Clients' tests share (`2-split-tui-gui-clients`
/// design D4b rule 5): the stub Owner constructions, the queue shapes a menu or
/// transport test reads, and the Named-queue fixtures whose queue list and op
/// answers both Clients' tests drive.
#[cfg(any(test, feature = "test"))]
#[doc(hidden)]
pub use self::app::{
    SeededAbsBook, app_with_adopted_named_queues, app_with_connected_peer,
    card_cache_key_for_id_for_test, close_initial_services, current, emby_unified_state,
    inject_local_events, install_test_emby, key_chord_for_test, live_owner_channel, make_app_stub,
    make_audio_items, make_audio_only_remote_app_stub_with_cmd_rx, make_items,
    make_local_daemon_app_stub, make_local_daemon_app_stub_with_cmd_rx,
    make_local_daemon_app_stub_with_cmd_rx_and_compat, make_remote_app_stub,
    make_remote_app_stub_at_index, make_remote_app_stub_with_cmd_rx, named_queue_list, op_answer,
    push_movies_library_for_test, queue_card_cache_key_for_test, queue_target_for_test,
    remote_stub_config, resting, resting_snapshot, seed_queue_row_menu_rows_for_test,
    seed_queue_shuffle_owner_for_test, test_source,
};
/// The stub fixtures the root package's and the GUI's tests share: the Emby
/// movie library a Service-seam leak would mutate, the card cache keys the
/// shell seeds a cached image under, and the entries the GUI queue row menu
/// opens with.
#[cfg(any(test, feature = "test"))]
#[doc(hidden)]
pub use self::app::{
    set_cast_receivers_for_test, set_subtitle_tracks_for_test, stub_app_for_gui_client,
    stub_app_with_cached_emby_client, stub_app_with_cast_target,
    stub_app_with_completed_emby_startup, stub_app_with_completed_emby_startup_and_contents,
    stub_app_with_failed_emby_setup_form, stub_app_with_local_daemon_events,
    stub_app_with_pending_settings_save, stub_app_with_playing_items, stub_app_with_playing_queue,
    stub_app_with_prefix_and_toast, stub_app_with_queue, stub_app_with_queue_and_cmd_rx,
    stub_app_with_rebound_quit, stub_app_with_track_choices, stub_app_with_unsaved_queue,
    transport_client_capabilities,
};
