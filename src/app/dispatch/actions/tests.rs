use super::*;
use crate::app::components::list::tree_browser::TreeOperation;
use crate::app::components::msg::{Msg, MusicArtistTarget, ShellRequest};
use crate::app::dispatch::library::browse::{
    build_album_index_with, recursive_album_search_eligible,
};
use crate::app::render::make_music_group_app_with_second_album;
use crate::app::shell::Model;
use crate::app::tests::{confirm_replace_queue, install_test_emby, make_app_stub, make_item};
use crate::app::{
    AlbumIndexState, AlbumPathPart, BrowseLevel, LibEvent, LibraryTab, PanelFocus, TabSelection,
};
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
fn remote_playback_app() -> App {
    let config = crate::config::Config::default();
    let (remote, player_rx, _cmd_rx) =
        mbv_core::remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    App::new_remote_with_config(
        mbv_core::api::EmbyClient::new(config.clone()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    )
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
/// Populated/empty-queue replacement-gate tests for album-track and folder plays.
#[cfg(test)]
mod replacement_gate;
