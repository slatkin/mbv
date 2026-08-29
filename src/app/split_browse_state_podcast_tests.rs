//! Characterization tests for `split-browse-state-interaction-fields` task 3.1.
//!
//! They pin two behaviours the `AudiobookshelfBrowseState` content/interaction
//! split (tasks 3.2–3.4) must not regress:
//!   1. the selected show is restored on tab re-entry from the saved position;
//!   2. a component's episode filter and episode-mode selection survive a
//!      content refresh that keeps the selected show.
//!
//! The reset-on-vanish direction is already covered by
//! `abs_podcast_component_drops_stale_episode_state_when_selection_vanishes`.

use super::super::types_audiobookshelf_browse::{AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter};
use super::super::types_tab_selection::TabSelection;
use crate::app::components::AudiobookshelfPodcastComponent;
use crate::app::tests::make_app_stub;
use mbv_core::config::AudiobookshelfSetup;

fn library() -> mbv_core::audiobookshelf::AudiobookshelfLibrary {
    mbv_core::audiobookshelf::AudiobookshelfLibrary {
        id: "lib".into(),
        name: "Shows".into(),
        media_type: "podcast".into(),
    }
}

fn shows(ids: &[&str]) -> Vec<mbv_core::audiobookshelf::AudiobookshelfShow> {
    ids.iter()
        .map(|id| mbv_core::audiobookshelf::AudiobookshelfShow {
            library_item_id: format!("show-{id}"),
            title: format!("Show {id}"),
            author: None,
            description: None,
            cover_path: None,
        })
        .collect()
}

#[test]
fn show_position_restores_selected_id_after_tab_switch_away_and_back() {
    let mut app = make_app_stub();
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(AudiobookshelfSetup::new("https://shows.example"));
    app.tab = TabSelection::AudiobookshelfLibrary(0);
    app.audiobookshelf_libraries.push(library());
    let mut state = AudiobookshelfBrowseState::new(library());
    state.shows = shows(&["a", "b", "c", "d"]);
    state.total = 4;
    app.audiobookshelf_browse.push(state);

    app.select_audiobookshelf_show(2);
    assert_eq!(
        app.audiobookshelf_browse[0].selected_id.as_deref(),
        Some("show-c"),
    );
    app.save_audiobookshelf_position(0);

    // Tab away and back: re-entry has a fresh browse state with no selection,
    // then the saved position is re-applied.
    app.audiobookshelf_browse[0].selected_id = None;
    app.activate_audiobookshelf_position(0);

    assert_eq!(
        app.audiobookshelf_browse[0].selected_id.as_deref(),
        Some("show-c"),
        "the saved position must restore the selected show on re-entry",
    );
}

#[test]
fn component_episode_filter_and_selection_survive_a_show_refresh() {
    let mut state = AudiobookshelfBrowseState::new(library());
    state.shows = shows(&["a", "b", "c"]);
    state.total = 3;
    state.select(1);

    let mut component = AudiobookshelfPodcastComponent::new();
    component.set_content(&state, true, false);
    component.set_episode_filter(AudiobookshelfEpisodeFilter::Unplayed);
    component.set_episode_selection(Some(2));

    // Refresh that keeps the selected show (show-b): the component's own
    // interaction state must ride through the re-projection unchanged.
    component.set_content(&state, true, false);

    assert_eq!(
        component.episode_filter(),
        AudiobookshelfEpisodeFilter::Unplayed,
        "the episode filter must survive a refresh that keeps the selected show",
    );
    assert_eq!(
        component.episode_selection(),
        Some(2),
        "the episode-mode selection must survive a refresh that keeps the selected show",
    );
}
