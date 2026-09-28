pub(crate) mod ctrl;
pub use ctrl::*;

mod context;
mod error;
pub use context::*;
pub use error::DaemonLibError;
mod core;
pub use core::*;
mod core_ctrl_spawn;
pub(crate) use core_ctrl_spawn::spawn_ctrl_client;
mod run_shutdown;
pub(crate) use run_shutdown::setup_shutdown_signal;
mod run;
pub use run::*;
mod event_loop;
pub(crate) use event_loop::{DaemonLoop, LoopFlow};
mod audiobookshelf;
pub(crate) use audiobookshelf::{
    apply_audiobookshelf_book_progress, apply_audiobookshelf_progress,
    install_daemon_audiobookshelf_context,
};
mod control;
pub(crate) use control::{
    CtrlContext, cancel_pending_idle_queue_load, cancel_pending_idle_queue_load_if_run_changed,
    complete_pending_idle_queue_load, expire_pending_idle_queue_load, handle_ctrl_for_role,
    owner_admin_transport_allowed, play_resolved_items,
};
mod control_queue;
pub(crate) use control_queue::{
    broadcast_queue_state, persist_stay_alive_owner_queue, project_queue_state,
};
#[cfg(test)]
pub(crate) use control_queue::{daemon_admits, unified_queue_state_for_peer};
mod ws;
pub(crate) use ws::handle_ws;
mod reconciliation;
pub use reconciliation::*;

#[cfg(test)]
mod tests;
