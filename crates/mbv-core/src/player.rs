use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use crate::api::{EmbyClient, EmbyItem, TICKS_PER_SECOND};
use crate::id_types::{EmbySessionId, ItemId, MediaSourceId};
#[cfg(test)]
use crate::playback_queue::QueueMutationResult;
use crate::playback_queue::{PlaybackQueue, QueueItem, QueueSlotId};
use libmpv2::{
    events::{Event, PropertyData},
    mpv_end_file_reason, EndFileReason, Format, Mpv,
};

fn mpv_err_str(e: &libmpv2::Error) -> String {
    if let libmpv2::Error::Raw(code) = e {
        format!("Raw({}) [{}]", code, libmpv2_sys::mpv_error_str(*code))
    } else {
        format!("{e:?}")
    }
}

fn mpv_title_opt(title: &str) -> String {
    // Use mpv's %N% length-prefix format so the value is passed verbatim —
    // no escaping needed, handles commas, backslashes, and any other character.
    format!("force-media-title=%{}%{}", title.len(), title)
}

fn send_ep_info(mpv: &Mpv, item: &crate::api::EmbyItem) {
    let val =
        if item.item_type == "Episode" && item.parent_index_number > 0 && item.index_number > 0 {
            format!(
                "Season {}  Episode {}",
                item.parent_index_number, item.index_number
            )
        } else {
            String::new()
        };
    let _ = mpv.set_property("user-data/mbv/ep-tag", val.as_str());
}

include!("player_types.rs");
include!("player_runtime.rs");
include!("player_run_state.rs");
include!("player_run_types.rs");
include!("player_run_queue.rs");
include!("player_run_commands.rs");
include!("player_run_events.rs");
include!("player_run_run.rs");
include!("player_runtime_controller.rs");
include!("player_proxy.rs");

#[cfg(test)]
mod tests {
    include!("player_tests_basic.rs");
    include!("player_tests_session.rs");
    include!("player_tests_session_feed.rs");
    include!("player_tests_status.rs");
    include!("player_tests_submit.rs");
}
