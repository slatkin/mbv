use serde::{Deserialize, Serialize};

use crate::api::MediaItem;
use crate::player::{PlayerCommand, PlayerEvent, PlayerStatus};

pub const CTRL_PROTOCOL_VERSION: u32 = 1;
pub const CTRL_CAP_QUEUE_STATE: &str = "queue-state";
pub const CTRL_CAP_START_INDEX: &str = "play-items-start-idx";
pub const CTRL_CAP_STATUS_ONLY: &str = "status-only";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CtrlHello {
    pub protocol_version: u32,
    pub app_version: String,
    pub capabilities: Vec<String>,
}

impl CtrlHello {
    pub fn current() -> Self {
        Self {
            protocol_version: CTRL_PROTOCOL_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: vec![
                CTRL_CAP_QUEUE_STATE.to_string(),
                CTRL_CAP_START_INDEX.to_string(),
                CTRL_CAP_STATUS_ONLY.to_string(),
            ],
        }
    }

    pub fn validate_peer(&self) -> Result<(), String> {
        if self.protocol_version != CTRL_PROTOCOL_VERSION {
            return Err(format!(
                "incompatible daemon protocol version: peer={} local={}",
                self.protocol_version, CTRL_PROTOCOL_VERSION
            ));
        }
        for required in [
            CTRL_CAP_QUEUE_STATE,
            CTRL_CAP_START_INDEX,
            CTRL_CAP_STATUS_ONLY,
        ] {
            if !self.capabilities.iter().any(|cap| cap == required) {
                return Err(format!(
                    "peer missing daemon protocol capability: {required}"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub enum CtrlCmd {
    Hello(CtrlHello),
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
    Hello(CtrlHello),
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

    #[test]
    fn current_hello_validates() {
        CtrlHello::current().validate_peer().unwrap();
    }

    #[test]
    fn hello_rejects_incompatible_protocol_version() {
        let mut hello = CtrlHello::current();
        hello.protocol_version += 1;
        assert!(hello.validate_peer().is_err());
    }

    #[test]
    fn hello_rejects_missing_capability() {
        let mut hello = CtrlHello::current();
        hello.capabilities.retain(|cap| cap != CTRL_CAP_START_INDEX);
        assert!(hello.validate_peer().is_err());
    }
}
