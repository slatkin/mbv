pub(crate) mod ctrl;

include!("context.rs");
include!("core.rs");
include!("run_shutdown.rs");
include!("run.rs");
include!("event_loop.rs");
include!("audiobookshelf.rs");
mod control;
pub use control::*;
include!("ws.rs");
include!("reconciliation.rs");

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
