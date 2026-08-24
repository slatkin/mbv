use super::types_audiobookshelf_browse::{AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter};
use super::types_selection_modal::{
    SelectionModalFilter, SelectionModalItem, SelectionModalListState, SelectionModalRow,
    SelectionModalSource,
};
use super::App;
use crate::app::ui_util::fmt_duration_approx;

impl App {
    pub(super) fn open_podcast_selection_modal(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get(index) else {
            return;
        };
        let Some(show) = state.selected_show() else {
            return;
        };
        let title = show.title.clone();
        let list_state = podcast_modal_state(state);
        let labels = AudiobookshelfEpisodeFilter::ALL
            .iter()
            .map(|filter| filter.label().to_string())
            .collect();
        let selected = AudiobookshelfEpisodeFilter::ALL
            .iter()
            .position(|filter| *filter == state.episode_filter)
            .unwrap_or(0);
        self.open_selection_modal(
            SelectionModalSource::Podcast {
                library_item_id: show.library_item_id.clone(),
            },
            title,
            list_state,
            Some(SelectionModalFilter { labels, selected }),
        );
    }

    /// Selects a visible modal filter, rebuilding the modal rows for both
    /// keyboard and mouse selection. The shared pill hit-test dispatch calls
    /// `select_audiobookshelf_filter`, so the modal path must not depend on
    /// the wide-only `episode_selection` state.
    pub(super) fn select_podcast_selection_modal_filter(
        &mut self,
        library_item_id: String,
        selected: usize,
    ) {
        let Some(filter) = AudiobookshelfEpisodeFilter::ALL.get(selected).copied() else {
            return;
        };
        let (rows, loading) = {
            let Some((_, state)) =
                self.audiobookshelf_browse
                    .iter_mut()
                    .enumerate()
                    .find(|(_, state)| {
                        state
                            .shows
                            .iter()
                            .any(|show| show.library_item_id == library_item_id)
                    })
            else {
                return;
            };
            state.episode_filter = filter;
            let loading = state.detail_loading || state.episodes.is_none();
            (
                podcast_episode_modal_rows(state, state.episodes.as_deref().unwrap_or_default()),
                loading,
            )
        };

        let state = if loading {
            SelectionModalListState::Loading
        } else {
            SelectionModalListState::ready(rows)
        };
        self.refresh_selection_modal(
            SelectionModalSource::Podcast { library_item_id },
            state,
            None,
        );
    }
}

/// Builds one `Item` row per entry in `state.visible_episodes()` (no
/// `Header` rows -- episodes aren't hierarchical, unlike Series'
/// season/episode nesting). `id` is the episode's `episode_id`, which
/// `activate_selection_modal_item` resolves against the currently selected
/// show's own filtered episode list (see `open_podcast_selection_modal`'s
/// doc comment for why that id is only unique per-show).
fn podcast_episode_modal_rows(
    state: &AudiobookshelfBrowseState,
    episodes: &[mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode],
) -> Vec<SelectionModalRow> {
    state
        .visible_episodes_from(episodes)
        .iter()
        .map(|episode| {
            let mut meta_parts = Vec::new();
            if let Some(seconds) = episode.duration_seconds.filter(|seconds| *seconds > 0.0) {
                meta_parts.push(fmt_duration_approx(seconds as i64));
            }
            let played = state
                .progress
                .get(&(episode.library_item_id.clone(), episode.episode_id.clone()))
                .is_some_and(|progress| progress.is_finished);
            if played {
                meta_parts.push("Played".to_string());
            }
            SelectionModalRow::Item(SelectionModalItem {
                name: episode.title.clone(),
                meta: meta_parts.join(" \u{b7} "),
                id: episode.episode_id.clone(),
            })
        })
        .collect()
}

pub(super) fn podcast_modal_state(state: &AudiobookshelfBrowseState) -> SelectionModalListState {
    if state.detail_loading || state.episodes.is_none() {
        SelectionModalListState::Loading
    } else {
        SelectionModalListState::ready(podcast_episode_modal_rows(
            state,
            state.episodes.as_deref().unwrap_or_default(),
        ))
    }
}

pub(super) fn podcast_modal_state_for_detail(
    state: &AudiobookshelfBrowseState,
    library_item_id: &str,
) -> SelectionModalListState {
    let Some(episodes) = state.detail_cache.get(library_item_id) else {
        return SelectionModalListState::Loading;
    };
    SelectionModalListState::ready(podcast_episode_modal_rows(state, episodes))
}
