use serde::{Deserialize, Serialize};

use crate::api::MediaItem;
use crate::player::{PlayerCommand, PlayerEvent, PlayerStatus};

#[derive(Serialize, Deserialize)]
pub enum CtrlCmd {
    PlayerCmd(PlayerCommand),
    PlayItems { item_ids: Vec<String>, start_ticks: i64 },
    Stop,
}

#[derive(Serialize, Deserialize)]
pub enum CtrlEvent {
    Player(PlayerEvent),
    State(CtrlState),
}

#[derive(Serialize, Deserialize)]
pub struct CtrlState {
    pub status: PlayerStatus,
    pub items: Vec<MediaItem>,
    pub cursor: usize,
}
