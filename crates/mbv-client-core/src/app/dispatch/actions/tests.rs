use super::*;
use crate::app::LibEvent;
use crate::app::dispatch::library::browse::{
    build_album_index_with, recursive_album_search_eligible,
};
use crate::app::tests::make_app_stub;
use mbv_emby_model::test_support::make_item;
use mbv_ui_model::browse::{AlbumIndexState, AlbumPathPart, BrowseLevel};
use mbv_ui_model::library_tab::LibraryTab;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_model::tab_selection::TabSelection;
use std::collections::HashMap;

fn folder(id: &str, name: &str) -> EmbyItem {
    let mut item = make_item(name, "Folder");
    item.id = id.into();
    item.is_folder = true;
    item
}

/// A started remote-backed App (mirrors `album_playback_routes_with_album_queue_source`):
/// `play_album_track`'s Emby-availability gate passes and the resulting queue
/// is observable.
fn remote_playback_app() -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let config = crate::app::config::Config::default();
    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    (
        App::new_remote_with_config(
            mbv_emby::EmbyClient::new(config.clone()),
            remote,
            player_rx,
            &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
            config,
        ),
        cmd_rx,
    )
}

/// The item ids and `start_idx` of the most recent `UnifiedQueueReplace` sent
/// to the owner (row 5.3: a replacement is an answered owner op, so the
/// resolved queue is observed on the wire, not in an optimistic Client
/// write). Drains the command channel; call once per assertion point.
fn last_replace(
    cmd_rx: &std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) -> (Vec<String>, Option<usize>) {
    let mut ids = Vec::new();
    let mut last_start = None;
    for command in cmd_rx.try_iter() {
        if let mbv_ctrl::CtrlCmd::UnifiedQueueReplace {
            items, start_idx, ..
        } = command
        {
            ids = items.iter().map(|item| item.id().to_string()).collect();
            last_start = start_idx;
        }
    }
    (ids, last_start)
}

fn queued_track_ids(app: &App) -> Vec<String> {
    let mut ids: Vec<String> = app
        .playback_queue()
        .emby_items()
        .iter()
        .map(|item| item.id.clone())
        .collect();
    ids.sort();
    ids
}

/// Album, artist and grouped-track playback dispatch tests.
#[cfg(test)]
mod album_artist_playback;
/// Recursive album-index build, traversal and activation-path tests.
#[cfg(test)]
mod album_index;
