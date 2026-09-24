use super::*;

mod basic;
// Re-export helper functions from basic so all test modules can use them
pub use basic::{
    item, emby_qi, video_feed_qi, connect_client, shared_queue_state, cold_player, recv_event,
    queue_from_items,
};

mod audio_only;
mod ctrl_auth;
mod playback_intent;
mod feed;
mod service_independent;
mod abs_queue;
mod abs_queue_progress;
mod queue_ops;
#[path = "loop.rs"]
mod r#loop;
