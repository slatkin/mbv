use super::*;
use crate::app::components::msg::LeafKeyResult;
use mbv_core::audiobookshelf::{AudiobookshelfLibrary, AudiobookshelfProgress, AudiobookshelfShow};
use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::Terminal;
use tuirealm::component::Component;

const DAY: u64 = 24 * 60 * 60;
const NOW: u64 = 30 * DAY;

fn library() -> AudiobookshelfLibrary {
    AudiobookshelfLibrary {
        id: "lib".into(),
        name: "Podcasts".into(),
        media_type: "podcast".into(),
    }
}

fn show(id: &str, title: &str) -> AudiobookshelfShow {
    AudiobookshelfShow {
        library_item_id: id.into(),
        title: title.into(),
        author: None,
        description: None,
        cover_path: None,
    }
}

fn episode(
    show: &str,
    id: &str,
    published_at: Option<u64>,
    duration_seconds: Option<f64>,
) -> AudiobookshelfDownloadedEpisode {
    AudiobookshelfDownloadedEpisode {
        library_item_id: show.into(),
        episode_id: id.into(),
        title: id.into(),
        description: None,
        published_at,
        duration_seconds,
    }
}

/// Two shows: Alpha's cache holds a dated played episode and an undated
/// in-progress one; Beta's cache holds one unplayed episode.
fn fixture_state() -> AudiobookshelfBrowseState {
    let mut state = AudiobookshelfBrowseState::new(library());
    state.append_page(
        0,
        20,
        2,
        vec![show("alpha", "Alpha Show"), show("beta", "Beta Show")],
    );
    state.cache_detail(
        "alpha".into(),
        vec![
            episode("alpha", "dated", Some(NOW - DAY), Some(3600.0)),
            episode("alpha", "undated", None, Some(1800.0)),
        ],
    );
    state.cache_detail("beta".into(), vec![episode("beta", "beta-one", None, None)]);
    state
        .progress
        .insert(("alpha".into(), "dated".into()), finished_progress());
    state.progress.insert(
        ("alpha".into(), "undated".into()),
        AudiobookshelfProgress {
            library_item_id: "alpha".into(),
            episode_id: "undated".into(),
            current_time_seconds: 300.0,
            is_finished: false,
        },
    );
    state
}

fn finished_progress() -> AudiobookshelfProgress {
    AudiobookshelfProgress {
        library_item_id: "alpha".into(),
        episode_id: "dated".into(),
        current_time_seconds: 3600.0,
        is_finished: true,
    }
}

fn owner() -> PodcastContent {
    let mut owner = PodcastContent::new();
    owner.set_now_secs(NOW);
    owner.set_content(&fixture_state(), false);
    owner
}

mod interaction;
mod projection;
