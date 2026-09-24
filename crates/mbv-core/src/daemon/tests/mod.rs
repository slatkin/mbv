use super::*;

// External crate imports needed across test files
use crate::api::{EmbyClient, EmbyItem};
use crate::config::{Config, QueueSource, StayAliveQueueState};
use crate::ctrl::DisconnectReason;
use crate::ctrl::{
    CtrlCmd, CtrlEvent, CtrlHello, PlaybackIntent, PlaybackIntentAction, PlaybackIntentOutcome,
    WireCommand,
};
use crate::mock_http::MockHttp;
use crate::playback_queue::{
    AudiobookshelfBookQueueItem, AudiobookshelfQueueItem, FeedEntry, PlaybackQueue, QueueItem,
};
use crate::player::{
    AudiobookshelfBookProgressUpdate, AudiobookshelfProgressUpdate, Player, PlayerCommand,
    PlayerEvent, PlayerOwnerState, PlayerStatus, SubtitlePrefs,
};
use crate::service_runtime::SetupGeneration;
use crate::stream::SocketStream;
use crate::ws::WsEvent;
use rstest::rstest;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

// Daemon module items used across test files (from the old shared `use super::*;` scope)
use crate::daemon::{
    all_audio, apply_audiobookshelf_book_progress, apply_audiobookshelf_progress,
    apply_queue_enriched, apply_stopped_observation, apply_track_completed_observation,
    audio_only_rejection, broadcast, handle_ctrl_for_role, handle_ws,
    take_authority_for_emby_remote, AuthorityHolder, CtrlClients, CtrlRequest, CtrlTransport,
    DaemonEvent, DaemonLoop, DaemonPlayerOwner, LoopFlow, PendingIdleQueueLoad,
    PlaybackIntentState, SharedQueueState,
};

mod basic;
// Re-export helper functions from basic so all test modules can use them
pub(super) use basic::{
    cold_player, connect_client, emby_qi, item, recv_event, shared_queue_state, video_feed_qi,
};

mod abs_queue;
mod audio_only;
mod ctrl_auth;
mod feed;
mod playback_intent;
mod service_independent;
// Re-export helper from abs_queue
pub use abs_queue::abs_qi;

mod abs_queue_progress;
// Re-export helper from abs_queue_progress
pub use abs_queue_progress::book_qi;
mod queue_ops;
// Re-export helper from queue_ops
pub use queue_ops::owner_with;

#[path = "loop.rs"]
mod r#loop;
