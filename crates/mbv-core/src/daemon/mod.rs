pub(crate) mod ctrl;

mod context;
pub use context::*;
include!("core.rs");
mod run_shutdown;
pub use run_shutdown::*;
include!("run.rs");
mod event_loop;
pub use event_loop::*;
mod audiobookshelf;
pub use audiobookshelf::*;
mod control;
pub use control::*;
mod ws;
pub use ws::*;
mod reconciliation;
pub use reconciliation::*;

#[cfg(test)]
mod tests {
    include!("tests/basic.rs");
    include!("tests/audio_only.rs");
    include!("tests/ctrl_auth.rs");
    include!("tests/playback_intent.rs");
    include!("tests/feed.rs");
    include!("tests/service_independent.rs");
    include!("tests/abs_queue.rs");
    include!("tests/abs_queue_progress.rs");
    include!("tests/queue_ops.rs");
    include!("tests/loop.rs");
}
