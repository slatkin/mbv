use super::*;

// External crate imports needed across test files
use crate::api::{EmbyClient, EmbyItem};
use crate::mock_http::MockHttp;
use crate::config::{Config, QueueSource};
use crate::ctrl::DisconnectReason;
use crate::ctrl::{
    CtrlCmd, CtrlEvent, PlaybackIntent, PlaybackIntentAction, PlaybackIntentOutcome, WireCommand,
};
use crate::playback_queue::{FeedEntry, PlaybackQueue, QueueItem};
use crate::player::{Player, PlayerCommand, PlayerEvent, PlayerStatus, PlayerOwnerState, SubtitlePrefs};
use crate::ws::WsEvent;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use rstest::rstest;

// Daemon module items used across test files (from the old shared `use super::*;` scope)
use crate::daemon::{
    all_audio, apply_queue_enriched, apply_stopped_observation,
    apply_track_completed_observation, audio_only_rejection, broadcast,
    handle_ctrl_for_role, handle_ws,
    take_authority_for_emby_remote, AuthorityHolder, CtrlClients, CtrlRequest, CtrlTransport,
    DaemonEvent, DaemonPlayerOwner, PlaybackIntentState, SharedQueueState,
};

mod basic;
// Re-export helper functions from basic so all test modules can use them
pub(super) use basic::{
    item, emby_qi, video_feed_qi, connect_client, shared_queue_state, cold_player, recv_event,
    queue_from_items,
};

mod audio_only;
mod ctrl_auth;
mod playback_intent;
mod feed;
mod service_independent;
mod abs_queue;
// Re-export helper from abs_queue
pub use abs_queue::abs_qi;

mod abs_queue_progress;
mod queue_ops;
// Re-export helper from queue_ops
pub use queue_ops::owner_with;

#[path = "loop.rs"]
mod r#loop;
