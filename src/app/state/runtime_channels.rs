use crate::app::playback_metadata::PlaybackDecorations;
use crate::app::playback_trickplay::{TrickplaySheetFetch, TrickplaySheetKey};
use crate::app::state::events::{LibEvent, SessionEvent};
use crate::app::state::types::cast::CastEvent;
use mbv_emby_model::EmbyItem;
use mbv_queue::QueueItemContentId;
use std::sync::mpsc;

pub(in crate::app) struct RuntimeChannels {
    pub(in crate::app) lib_tx: mpsc::Sender<LibEvent>,
    pub(in crate::app) lib_rx: mpsc::Receiver<LibEvent>,
    pub(in crate::app) search_tx:
        mpsc::Sender<(String, Result<Vec<EmbyItem>, mbv_emby::EmbyError>)>,
    pub(in crate::app) search_rx:
        mpsc::Receiver<(String, Result<Vec<EmbyItem>, mbv_emby::EmbyError>)>,
    pub(in crate::app) sessions_tx: mpsc::Sender<SessionEvent>,
    pub(in crate::app) sessions_rx: mpsc::Receiver<SessionEvent>,
    pub(in crate::app) cast_tx: mpsc::Sender<CastEvent>,
    pub(in crate::app) cast_rx: mpsc::Receiver<CastEvent>,
    pub(in crate::app) notif_action_tx: mpsc::Sender<String>,
    pub(in crate::app) notif_action_rx: mpsc::Receiver<String>,
    /// Outcomes of panel requests that run on a worker thread, drained by
    /// the shell and applied through the pinned view/settings handlers.
    pub(in crate::app) pinned_tx: mpsc::Sender<Result<(), String>>,
    pub(in crate::app) pinned_rx: mpsc::Receiver<Result<(), String>>,
    /// Landed item-metadata fetches (add-tauri-gui-client design D4a): the
    /// item identity and its decorations, one send per spawned worker.
    pub(in crate::app) metadata_tx: mpsc::Sender<(QueueItemContentId, PlaybackDecorations)>,
    pub(in crate::app) metadata_rx: mpsc::Receiver<(QueueItemContentId, PlaybackDecorations)>,
    /// Landed trickplay tile sheets (add-tauri-gui-client task 6.7): the
    /// sheet's key and what its fetch produced — the bytes, the server's
    /// definitive absence, or a transient failure the cache does not record.
    /// One send per spawned worker.
    pub(in crate::app) trickplay_tx: mpsc::Sender<(TrickplaySheetKey, TrickplaySheetFetch)>,
    pub(in crate::app) trickplay_rx: mpsc::Receiver<(TrickplaySheetKey, TrickplaySheetFetch)>,
}

impl RuntimeChannels {
    pub(in crate::app) fn new() -> Self {
        let (lib_tx, lib_rx) = mpsc::channel();
        let (search_tx, search_rx) = mpsc::channel();
        let (sessions_tx, sessions_rx) = mpsc::channel();
        let (cast_tx, cast_rx) = mpsc::channel();
        let (notif_action_tx, notif_action_rx) = mpsc::channel();
        let (pinned_tx, pinned_rx) = mpsc::channel();
        let (metadata_tx, metadata_rx) = mpsc::channel();
        let (trickplay_tx, trickplay_rx) = mpsc::channel();
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
            pinned_tx,
            pinned_rx,
            metadata_tx,
            metadata_rx,
            trickplay_tx,
            trickplay_rx,
        }
    }
}
