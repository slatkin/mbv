use super::*;
use crate::app::tests::{make_app_stub, make_audio_only_remote_app_stub_with_cmd_rx, make_items};
use crate::app::{ConfirmAction, ContextAction, PanelFocus};
use mbv_ctrl::CtrlCmd;
use mbv_emby::test_support::make_session;
use mbv_emby_model::test_support::make_item;
use rstest::rstest;

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
