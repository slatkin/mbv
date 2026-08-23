use super::types_audiobookshelf_browse::{AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter};
use super::types_selection_modal::{
    SelectionModalFilter, SelectionModalItem, SelectionModalListState, SelectionModalRow,
    SelectionModalSource,
};
use super::App;
use crate::app::ui_util::fmt_duration_approx;

impl App {
    /// Opens the podcast constituent-list modal (design.md decisions 3/4):
    /// a flat scrollable list of the selected show's `Item` rows, no headers
    /// (episodes aren't hierarchical), with the played/unplayed filter shown
    /// as pills at the modal's top. Mirrors `open_series_selection_modal`'s
    /// shape; unlike Series/Album, whose ids are globally unique, an episode
    /// id is only unique within its show (`(library_item_id, episode_id)`,
    /// see `types_audiobookshelf_browse`'s progress-map key), so activation
    /// resolves through the currently selected show's own filtered episode
    /// list (`activate_selection_modal_item`) rather than a global scan.
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

    /// Cycles the played/unplayed filter shown as pills at the top of the
    /// open podcast selection modal (design.md decision 4), rebuilding
    /// modal state from the newly filtered episode list. Resets the cursor
    /// to the first episode row, mirroring `AudiobookshelfEpisodeFilter`'s
    /// existing reset-to-0 behavior for the (wide-only) in-hero episode
    /// table (see `AudiobookshelfBrowseState::set_episode_filter`).
    pub(super) fn cycle_podcast_selection_modal_filter(&mut self, delta: i64) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get(index) else {
            return;
        };
        let current = AudiobookshelfEpisodeFilter::ALL
            .iter()
            .position(|filter| *filter == state.episode_filter)
            .unwrap_or(0);
        let next = (current as i64 + delta)
            .rem_euclid(AudiobookshelfEpisodeFilter::ALL.len() as i64) as usize;
        self.select_podcast_selection_modal_filter(next);
    }

    /// Selects a visible modal filter, rebuilding the modal rows for both
    /// keyboard and mouse selection. The shared pill hit-test dispatch calls
    /// `select_audiobookshelf_filter`, so the modal path must not depend on
    /// the wide-only `episode_selection` state.
    pub(super) fn select_podcast_selection_modal_filter(&mut self, selected: usize) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(filter) = AudiobookshelfEpisodeFilter::ALL.get(selected).copied() else {
            return;
        };
        let (rows, loading) = {
            let Some(state) = self.audiobookshelf_browse.get_mut(index) else {
                return;
            };
            state.episode_filter = filter;
            let loading = state.detail_loading || state.episodes.is_none();
            (
                podcast_episode_modal_rows(state, state.episodes.as_deref().unwrap_or_default()),
                loading,
            )
        };

        let Some(modal) = self.selection_modal.as_mut() else {
            return;
        };
        if !matches!(modal.source, SelectionModalSource::Podcast { .. }) {
            return;
        }
        modal.cursor = rows
            .iter()
            .position(|row| matches!(row, SelectionModalRow::Item(_)))
            .unwrap_or(0);
        modal.state = if loading {
            SelectionModalListState::Loading
        } else {
            SelectionModalListState::ready(rows)
        };
        if let Some(filter) = modal.filter.as_mut() {
            filter.selected = selected;
        }
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
