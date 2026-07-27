use super::{
    all_audio, audio_only_rejection, broadcast, handle_ctrl, handle_ws,
    take_authority_for_emby_remote, AuthorityHolder, CtrlClients, CtrlEvent, CtrlOutbound,
    CtrlRequest, SharedQueueState,
};
use crate::api::MediaItem;
use crate::config::{Config, QueueSource};
use crate::ctrl::DisconnectReason;
use crate::ctrl::{CtrlCmd, WireCommand};
use crate::player::{Player, PlayerCommand, PlayerEvent, PlayerStatus, SubtitlePrefs};
use crate::ws::WsEvent;
use std::sync::{mpsc, Arc, Mutex};

fn item(name: &str, media_type: &str, item_type: &str) -> MediaItem {
    MediaItem {
        id: name.into(),
        name: name.into(),
        item_type: item_type.into(),
        is_folder: false,
        media_type: media_type.into(),
        collection_type: String::new(),
        runtime_ticks: 0,
        played: false,
        playback_position_ticks: 0,
        series_id: String::new(),
        series_name: String::new(),
        album_id: String::new(),
        album: String::new(),
        index_number: 0,
        parent_index_number: 0,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        director: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genre: String::new(),
        playlist_item_id: String::new(),
    }
}

/// Connects a client the same way the accept thread does. Under the
/// exclusive-connection model (ADR 0003 / #119) connecting *is* becoming
/// the driver — there is no separate "pending" step.
fn connect_client(clients: &mut CtrlClients) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (tx, rx) = mpsc::channel();
    let id = clients.connect(tx);
    (id, rx)
}

fn shared_queue_state() -> SharedQueueState {
    SharedQueueState {
        items: Arc::new(Mutex::new(Vec::new())),
        cursor: Arc::new(Mutex::new(0)),
        source: Arc::new(Mutex::new(QueueSource::Unknown)),
    }
}

fn cold_player() -> Player {
    let (event_tx, _event_rx) = mpsc::channel::<PlayerEvent>();
    Player::new(
        String::new(),
        String::new(),
        false,
        false,
        true,
        false,
        false,
        SubtitlePrefs::default(),
        event_tx,
        None,
    )
}

fn recv_event(rx: &mpsc::Receiver<CtrlOutbound>) -> CtrlEvent {
    match rx.recv().unwrap() {
        CtrlOutbound::Event(json) => serde_json::from_str(&json).unwrap(),
        CtrlOutbound::Close => panic!("expected event, got close"),
    }
}

fn assert_close(rx: &mpsc::Receiver<CtrlOutbound>) {
    match rx.recv().unwrap() {
        CtrlOutbound::Close => {}
        CtrlOutbound::Event(json) => panic!("expected close, got {json}"),
    }
}
