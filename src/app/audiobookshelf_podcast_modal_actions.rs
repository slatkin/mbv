use super::shell::Model;
use super::types_audiobookshelf_browse::{AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter};
use super::types_selection_modal::{
    SelectionModalFilter, SelectionModalItem, SelectionModalListState, SelectionModalRow,
    SelectionModalSource,
};
use crate::app::ui_util::fmt_duration_approx;

impl Model {
    pub(super) fn open_podcast_selection_modal(&mut self) {
        let Some(index) = self.app.tab.audiobookshelf_index() else {
            return;
        };
        // The episode filter is component-owned
        // (split-browse-state-interaction-fields task 3.2); default to `All`
        // when the component is not the active mounted browser.
        let episode_filter = self
            .abs_podcast_component_mut(index)
            .map(|component| component.episode_filter())
            .unwrap_or_default();
        let (title, library_item_id, list_state) = {
            let Some(state) = self.app.audiobookshelf_browse.get(index) else {
                return;
            };
            let Some(show) = state.selected_show() else {
                return;
            };
            (
                show.title.clone(),
                show.library_item_id.clone(),
                podcast_modal_state(state, episode_filter),
            )
        };
        let labels = AudiobookshelfEpisodeFilter::ALL
            .iter()
            .map(|filter| filter.label().to_string())
            .collect();
        let selected = AudiobookshelfEpisodeFilter::ALL
            .iter()
            .position(|filter| *filter == episode_filter)
            .unwrap_or(0);
        self.app.open_selection_modal(
            SelectionModalSource::Podcast { library_item_id },
            title,
            list_state,
            Some(SelectionModalFilter { labels, selected }),
        );
    }

    /// Selects a visible modal filter, rebuilding the modal rows for both
    /// keyboard and mouse selection. The shared `SelectionModalFilterSelected`
    /// shell request dispatches through
    /// `Model::handle_selection_modal_request`, so the modal path must not
    /// depend on the wide-only `episode_selection` state.
    pub(super) fn select_podcast_selection_modal_filter(
        &mut self,
        library_item_id: String,
        selected: usize,
    ) {
        let Some(filter) = AudiobookshelfEpisodeFilter::ALL.get(selected).copied() else {
            return;
        };
        let Some(index) = self.app.audiobookshelf_browse.iter().position(|state| {
            state
                .shows
                .iter()
                .any(|show| show.library_item_id == library_item_id)
        }) else {
            return;
        };
        if let Some(component) = self.abs_podcast_component_mut(index) {
            component.set_episode_filter(filter);
        }
        let modal_state = {
            let Some(state) = self.app.audiobookshelf_browse.get(index) else {
                return;
            };
            // The modal shows one specific show's episodes: prefer that
            // show's cached detail, falling back to the live `episodes` only
            // when it is the selected show (the wide filter-cycle case).
            let episodes = state.detail_cache.get(&library_item_id).or_else(|| {
                (state.selected_id.as_deref() == Some(&library_item_id))
                    .then_some(state.episodes.as_ref())
                    .flatten()
            });
            match episodes {
                Some(episodes) if !state.detail_loading => SelectionModalListState::ready(
                    podcast_episode_modal_rows(state, episodes, filter),
                ),
                _ => SelectionModalListState::Loading,
            }
        };
        self.app.refresh_selection_modal(
            SelectionModalSource::Podcast { library_item_id },
            modal_state,
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
    filter: AudiobookshelfEpisodeFilter,
) -> Vec<SelectionModalRow> {
    state
        .visible_episodes_from(episodes, filter)
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

pub(super) fn podcast_modal_state(
    state: &AudiobookshelfBrowseState,
    filter: AudiobookshelfEpisodeFilter,
) -> SelectionModalListState {
    if state.detail_loading || state.episodes.is_none() {
        SelectionModalListState::Loading
    } else {
        SelectionModalListState::ready(podcast_episode_modal_rows(
            state,
            state.episodes.as_deref().unwrap_or_default(),
            filter,
        ))
    }
}
