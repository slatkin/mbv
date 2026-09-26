use crate::app::state::types::cast::CastEvent;
use crate::app::state::types::events::{LibEvent, SessionEvent};
use mbv_core::api::EmbyItem;
use std::sync::mpsc;

pub(in crate::app) struct RuntimeChannels {
    pub(in crate::app) lib_tx: mpsc::Sender<LibEvent>,
    pub(in crate::app) lib_rx: mpsc::Receiver<LibEvent>,
    pub(in crate::app) search_tx: mpsc::Sender<(String, Result<Vec<EmbyItem>, String>)>,
    pub(in crate::app) search_rx: mpsc::Receiver<(String, Result<Vec<EmbyItem>, String>)>,
    pub(in crate::app) sessions_tx: mpsc::Sender<SessionEvent>,
    pub(in crate::app) sessions_rx: mpsc::Receiver<SessionEvent>,
    pub(in crate::app) cast_tx: mpsc::Sender<CastEvent>,
    pub(in crate::app) cast_rx: mpsc::Receiver<CastEvent>,
    pub(in crate::app) notif_action_tx: mpsc::Sender<String>,
    pub(in crate::app) notif_action_rx: mpsc::Receiver<String>,
}

impl RuntimeChannels {
    pub(in crate::app) fn new() -> Self {
        let (lib_tx, lib_rx) = mpsc::channel();
        let (search_tx, search_rx) = mpsc::channel();
        let (sessions_tx, sessions_rx) = mpsc::channel();
        let (cast_tx, cast_rx) = mpsc::channel();
        let (notif_action_tx, notif_action_rx) = mpsc::channel();
        Self {
            lib_tx,
            lib_rx,
            search_tx,
            search_rx,
            sessions_tx,
            sessions_rx,
            cast_tx,
            cast_rx,
            notif_action_tx,
            notif_action_rx,
        }
    }
}
