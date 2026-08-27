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
        let (title, library_item_id, list_state, app_episode_filter) = {
            let Some(state) = self.app.audiobookshelf_browse.get(index) else {
                return;
            };
            let Some(show) = state.selected_show() else {
                return;
            };
            (
                show.title.clone(),
                show.library_item_id.clone(),
                podcast_modal_state(state),
                state.episode_filter,
            )
        };
        // Read the episode filter through the mounted component (task 5.3d.11
        // U3), falling back to the App browse-state mirror when the component
        // is not the active mounted browser.
        let episode_filter = self
            .abs_podcast_component_mut(index)
            .map(|component| component.episode_filter())
            .unwrap_or(app_episode_filter);
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
        // D14 stage-1 mirror: keep the App browse-state filter in sync so
        // downstream rows rebuild from the mirror faithfully.
        self.app.audiobookshelf_browse[index].episode_filter = filter;
        let (rows, loading) = {
            let Some(state) = self.app.audiobookshelf_browse.get(index) else {
                return;
            };
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
        self.app.refresh_selection_modal(
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
