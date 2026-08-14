include!("daemon_context.rs");
include!("daemon_core.rs");
include!("daemon_run.rs");
include!("daemon_audiobookshelf.rs");
include!("daemon_control.rs");
include!("daemon_ws.rs");
include!("daemon_reconciliation.rs");

#[cfg(test)]
mod tests {
    include!("daemon_tests.rs");
    include!("daemon_tests_audio_only.rs");
    include!("daemon_tests_ctrl_auth.rs");
    include!("daemon_tests_playback_intent.rs");
    include!("daemon_tests_feed.rs");
    include!("daemon_tests_service_independent.rs");
    include!("daemon_tests_abs_queue.rs");
}
