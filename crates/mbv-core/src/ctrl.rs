use serde::{Deserialize, Serialize};

use crate::api::MediaItem;
use crate::config::QueueSource;
use crate::playback_queue::{QueueRevision, QueueSlotId};
use crate::player::{PlayerCommand, PlayerEvent, PlayerStatus};

pub const CTRL_PROTOCOL_VERSION: u32 = 8;
pub const CTRL_CAP_QUEUE_STATE: &str = "queue-state";
pub const CTRL_CAP_START_INDEX: &str = "play-items-start-idx";
pub const CTRL_CAP_STATUS_ONLY: &str = "status-only";

pub type PlaybackRequestId = u64;
pub type PlaybackGeneration = u64;

include!("ctrl_compat.rs");
include!("ctrl_wire.rs");
include!("ctrl_event.rs");

#[cfg(test)]
mod tests {
    include!("ctrl_tests.rs");
}
