use serde::{Deserialize, Serialize};

use crate::api::MediaItem;
use crate::player::{PlayerCommand, PlayerEvent, PlayerStatus};

#[derive(Serialize, Deserialize)]
pub enum CtrlCmd {
    PlayerCmd(PlayerCommand),
    PlayItems {
        item_ids: Vec<String>,
        start_idx: usize,
        start_ticks: i64,
    },
    Stop,
}

#[derive(Serialize, Deserialize)]
pub enum CtrlEvent {
    Player(PlayerEvent),
    State(CtrlState),
    StatusOnly(PlayerStatus),
}

#[derive(Serialize, Deserialize)]
pub struct CtrlState {
    pub status: PlayerStatus,
    pub items: Vec<MediaItem>,
    pub cursor: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_items_command_preserves_start_index() {
        let json = serde_json::to_string(&CtrlCmd::PlayItems {
            item_ids: vec!["a".to_string(), "b".to_string()],
            start_idx: 1,
            start_ticks: 42,
        })
        .unwrap();

        let cmd: CtrlCmd = serde_json::from_str(&json).unwrap();
        match cmd {
            CtrlCmd::PlayItems {
                item_ids,
                start_idx,
                start_ticks,
            } => {
                assert_eq!(item_ids, vec!["a", "b"]);
                assert_eq!(start_idx, 1);
                assert_eq!(start_ticks, 42);
            }
            _ => panic!("expected PlayItems"),
        }
    }
}
