//! Player-owner runtime shared by `mbvd` and the TUI in-process.
//!
//! The daemon event loop, ctrl server, owner queue handling, queue persistence, and
//! Service reconciliation live here. Playback runs in `mbv-player` and desktop surfaces
//! in `mbv-desktop`. `mbvd` itself is a headless server daemon with no tray, pinned
//! panel, or Client functionality; the tray and MPRIS live in the Local process.

pub(crate) mod ctrl;
#[cfg(test)]
pub(crate) use ctrl::CtrlOutbound;
#[doc(inline)]
pub use ctrl::CtrlTransport;
pub(crate) use ctrl::{
    AuthorityHolder, ClientRegistry, CtrlClientId, CtrlClients, CtrlSender, send_to,
    serialize_ctrl_event, take_authority_for_emby_remote,
};

mod context;
mod error;
mod owner_settings;
#[doc(inline)]
pub use context::{AudiobookshelfOwnerContext, DaemonRole, DaemonStartupContext, EmbyOwnerContext};
#[doc(inline)]
pub use error::DaemonLibError;
#[cfg(test)]
pub(crate) use owner_settings::OwnerSettings;
pub(crate) use owner_settings::OwnerSettingsReader;
mod core;
#[cfg(test)]
pub(crate) use core::PlaybackIntentState;
pub(crate) use core::{
    DaemonEvent, DaemonOwnerContext, DaemonPlayerOwner, PendingIdleQueueLoad,
    QueuePersistenceRequest, SharedQueueState, audio_only_rejection, broadcast, dispatch_slot_jump,
    expire_and_redispatch, reset_slot_jumps, settle_and_redispatch, submit_queue_slots_cold_start,
};
#[doc(inline)]
pub use core::{DaemonPlayerHandle, DaemonRuntimeHooks, SwapDirection, pid_file};
mod core_ctrl_spawn;
pub(crate) use core_ctrl_spawn::spawn_ctrl_client;
mod run_shutdown;
pub(crate) use run_shutdown::setup_shutdown_signal;
mod run;
#[cfg(test)]
pub(crate) use run::playback_run_identity_is_current;
#[doc(inline)]
pub use run::run_with_options;
pub(crate) use run::{
    ConsumePolicy, apply_queue_enriched, apply_stopped_observation,
    apply_track_completed_observation, broadcast_player_event_if_not_replaced,
};
mod event_loop;
pub(crate) use event_loop::{DaemonLoop, LoopFlow, TrayState};
mod audiobookshelf;
pub(crate) use audiobookshelf::{
    apply_audiobookshelf_book_progress, apply_audiobookshelf_progress,
    apply_audiobookshelf_progress_refresh, install_daemon_audiobookshelf_context,
};
mod control;
pub(crate) use control::{
    CtrlContext, cancel_pending_idle_queue_load, cancel_pending_idle_queue_load_if_run_changed,
    complete_pending_idle_queue_load, expire_pending_idle_queue_load, handle_ctrl_for_role,
    owner_admin_transport_allowed, play_resolved_items, start_queue_enrichment,
};
mod control_queue;
pub(crate) use control_queue::{
    broadcast_queue_state, persist_stay_alive_owner_queue, project_queue_state,
    stay_alive_owner_queue_snapshot,
};
#[cfg(test)]
pub(crate) use control_queue::{daemon_admits, unified_queue_state_for_peer};
mod ws;
pub(crate) use ws::handle_ws;
mod reconciliation;
#[doc(inline)]
pub use reconciliation::{
    ABS_REPLACEMENT_FINALIZE_HARD_BOUND, EMBY_REPLACEMENT_FINALIZE_HARD_BOUND,
};
pub(crate) use reconciliation::{reconcile_packaged_audiobookshelf, reconcile_packaged_emby};

#[cfg(test)]
mod tests;
