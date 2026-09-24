#![allow(dead_code, unused_imports)]

use super::*;
use crate::app::dispatch::library::browse::{
    build_album_index_with, full_library_fetch_limit, recursive_album_search_eligible,
};
use crate::app::tests::{
    install_test_emby, make_app_stub, make_audio_only_remote_app_stub_with_cmd_rx, make_item,
    make_items, make_session,
};
use crate::app::{
    AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel, ConfirmAction, ContextAction,
    FeedHomeVideoState, LibEvent, LibraryTab, PanelFocus, QueueScope, TabSelection,
};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::ctrl::CtrlCmd;
use mbv_core::mock_http::MockHttp;
use mbv_core::player::PlayerEvent;
use rstest::rstest;
use std::collections::HashMap;
use std::sync::mpsc;

fn selection(media_types: &[&str]) -> Vec<EmbyItem> {
    media_types
        .iter()
        .enumerate()
        .map(|(index, media_type)| {
            let mut item = make_item(&format!("item-{index}"), "Movie");
            item.id = format!("item-{index}");
            item.media_type = (*media_type).into();
            item
        })
        .collect()
}

/// Session-aware audio ownership reads and album-track fetch guards.
mod audio_session;
/// Library-route enqueue conflicts, direct-remote play submission, and
/// library autoplay gating.
mod library_routing;
/// Playback eligibility classification and deferred local-play fall-through
/// when the queue owner cannot play the selection.
mod playback;
