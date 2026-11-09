//! The GUI Library's Podcasts data path (add-gui-library-panel design D12,
//! row 10.1): the per-podcast-library selection the core owns — the selected
//! pill and the picked episode — and the open, set-pill, select, and ensure
//! actions the later core surface's commands apply. The show catalog and the
//! per-show episodes already land App-side: the shared startup's show-list
//! chain pages into the browse state, and the bounded fan-out caches each
//! show's episodes. So this module names the pill and triggers the loads the
//! TUI's own loaders run. The TUI never reads any of it.

use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind;
use mbv_ui_msg::PodcastEpisodeTarget;

use crate::app::App;

/// The Podcasts destination's pills (row 10.1): the library's merged Latest
/// view, or one show's episodes by that show's `library_item_id`. The pill
/// row is single-select, as the TUI's one-valued `PillSelection` is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GuiPodcastPill {
    /// Every show's episodes, merged newest first with undated rows last:
    /// the TUI's default pill and the GUI's too.
    #[default]
    Latest,
    /// The merged view scoped to one show's episodes.
    Show(String),
}

/// The GUI Library's Podcasts selection (row 10.1): per podcast library, the
/// selected pill and the episode the Hero shows. The episode is named by the
/// provider's stable `(library_item_id, episode_id)` identity — the identity
/// the progress map and the queue-item resolver use — because an episode id
/// alone names no row across shows. A pick the selected pill's list no longer
/// holds falls back to that list's first row in the projection, as every
/// selection's stale arm does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiPodcastsSelection {
    /// The selected pill. Latest is the default.
    pub pill: GuiPodcastPill,
    /// The episode the Hero shows. `None` before a select command names one.
    pub picked_episode: Option<PodcastEpisodeTarget>,
}

impl App {
    /// Whether `lib_idx` names a Podcasts destination (row 10.1): an
    /// Audiobookshelf library whose `media_type` resolves to the podcast
    /// browse kind. Every GUI Podcasts method's destination guard, and the
    /// mirror of the Audiobooks guard: a book library stays Audiobooks.
    pub(in crate::app::gui_library) fn gui_podcast_library(&self, lib_idx: usize) -> bool {
        self.audiobookshelf_libraries
            .get(lib_idx)
            .is_some_and(|library| {
                AudiobookshelfBrowseKind::from_media_type(&library.media_type)
                    == AudiobookshelfBrowseKind::Podcast
            })
    }

    /// Open the Podcasts destination `lib_idx` (row 10.1): an unloaded show
    /// catalog starts the TUI's own show-list load — the refresh path the F5
    /// press runs, whose replacement batch is single-flight and whose
    /// completion publishes the whole catalog — and the selected pill's loads
    /// start through the frame path's ensure below. A catalog already
    /// loading, loaded, or owned by the startup's own fetch chain loads
    /// nothing here, so no second chain walks beside the first. A `lib_idx`
    /// that names no podcast library is an ignored command.
    pub fn gui_podcast_open(&mut self, lib_idx: usize) {
        if !self.gui_podcast_library(lib_idx) {
            Self::log_ignored_library_command(
                "podcastOpen",
                format_args!("the destination is no Podcasts library"),
            );
            return;
        }
        if self.tab.audiobookshelf_index() != Some(lib_idx) {
            return;
        }
        let unloaded = self
            .audiobookshelf_browse
            .get(lib_idx)
            .is_some_and(|state| {
                state.shows.is_empty() && state.total == 0 && state.replacement.is_none()
            });
        if unloaded {
            // The show list only: the Latest shelf and the per-show episodes
            // are the pill's loads, which the ensure starts through the same
            // shared guards the shell's projection pass uses.
            self.audiobookshelf_refresh(false);
        }
        self.ensure_gui_podcast_selection(lib_idx);
    }

    /// Select the destination's pill (row 10.1): the selection the core
    /// owns, so the frame carries it back, and the loads that pill needs
    /// start — every show's episodes for Latest through the TUI's bounded
    /// fan-out, or that one show's episodes. A show the published list does
    /// not carry is a stale frame's pill and changes nothing; a list still
    /// landing admits it, because the list is what would name it stale.
    pub fn gui_podcast_set_pill(&mut self, lib_idx: usize, pill: GuiPodcastPill) {
        if !self.gui_podcast_library(lib_idx) {
            Self::log_ignored_library_command(
                "podcastSetPill",
                format_args!("the destination is no Podcasts library"),
            );
            return;
        }
        if let GuiPodcastPill::Show(show_id) = &pill
            && !self.gui_podcast_show_known(lib_idx, show_id)
        {
            Self::log_stale_library_item("podcastSetPill", show_id);
            return;
        }
        self.gui_podcast_libs.entry(lib_idx).or_default().pill = pill;
        self.ensure_gui_podcast_selection(lib_idx);
    }

    /// Select the episode the Hero shows (row 10.1): the selection the core
    /// owns, so the frame carries it back. The episode's own fields and its
    /// show's facts are already cached, so no load starts. An episode the
    /// published per-show cache does not carry is a stale frame's pick and
    /// changes nothing, as every item-named command's stale arm does; a cache
    /// still landing admits the pick, because the list is what would name it
    /// stale.
    pub fn gui_podcast_select(&mut self, lib_idx: usize, target: &PodcastEpisodeTarget) {
        if !self.gui_podcast_library(lib_idx) {
            Self::log_ignored_library_command(
                "podcastSelect",
                format_args!("the destination is no Podcasts library"),
            );
            return;
        }
        if !self.gui_podcast_episode_known(lib_idx, target) {
            Self::log_stale_library_item("podcastSelect", target.episode_id());
            return;
        }
        self.gui_podcast_libs
            .entry(lib_idx)
            .or_default()
            .picked_episode = Some(target.clone());
    }

    /// The frame-path trigger the GUI's frame runs for the active Podcasts
    /// destination (row 10.1), the Audiobooks frame path's counterpart: the
    /// selected pill's show scope is committed to the browse state the
    /// fan-out reads, and the loads that pill needs start — a Latest pill's
    /// one-shot shelf fetch and every listed show's episodes, or the one
    /// show's episodes. Every guard here is idempotent (the shelf's request
    /// set, the fan-out's cached and in-flight skips, the scope's
    /// compare-before-write), so the frame path runs it on every frame. A
    /// `lib_idx` that names no podcast library, or a destination that is not
    /// the active one, is a no-op, so the frame path can call it unguarded.
    pub fn ensure_gui_podcast_selection(&mut self, lib_idx: usize) {
        if !self.gui_podcast_library(lib_idx) || self.tab.audiobookshelf_index() != Some(lib_idx) {
            return;
        }
        let pill = self.gui_podcast_pill(lib_idx);
        self.commit_gui_podcast_show_scope(lib_idx, &pill);
        if matches!(pill, GuiPodcastPill::Latest)
            && let Some(library_id) = self
                .audiobookshelf_libraries
                .get(lib_idx)
                .map(|library| library.id.clone())
        {
            self.ensure_audiobookshelf_latest_shelf(&library_id);
        }
        self.start_audiobookshelf_podcast_fan_out(lib_idx);
    }

    /// The pill the frame paints (row 10.1): the stored pill, or Latest for a
    /// library the session never selected one for — the default.
    #[must_use]
    pub fn gui_podcast_pill(&self, lib_idx: usize) -> GuiPodcastPill {
        self.gui_podcast_libs
            .get(&lib_idx)
            .map_or(GuiPodcastPill::Latest, |selection| selection.pill.clone())
    }

    /// The episode the Hero shows (row 10.1): the stored pick. The projection
    /// falls back to the selected pill's first row when the pick left the
    /// list, so the Hero's default is never blank.
    #[must_use]
    pub fn gui_podcast_picked_episode(&self, lib_idx: usize) -> Option<&PodcastEpisodeTarget> {
        self.gui_podcast_libs.get(&lib_idx)?.picked_episode.as_ref()
    }

    /// Commit the selected pill's show scope to the browse state the bounded
    /// fan-out reads (row 10.1): a Show pill scopes the fan-out to that show,
    /// and a Latest pill needs every listed show. The write happens only when
    /// the stored scope differs, so the frame path's ensure does not rewrite
    /// the browse state on every frame. The scope is the one shared write both
    /// Clients make; the browse state's own selected episode and show stay the
    /// TUI's.
    fn commit_gui_podcast_show_scope(&mut self, lib_idx: usize, pill: &GuiPodcastPill) {
        let scope = match pill {
            GuiPodcastPill::Latest => None,
            GuiPodcastPill::Show(show_id) => Some(show_id.clone()),
        };
        if let Some(state) = self.audiobookshelf_browse.get_mut(lib_idx)
            && state.committed_show_pill != scope
        {
            state.committed_show_pill = scope;
        }
    }

    /// Whether the published show list can still name `show_id` (row 10.1): a
    /// list that holds no show admits the pill — it is still landing, and the
    /// list is what would name the pill stale — and a loaded list admits only
    /// a show it carries.
    fn gui_podcast_show_known(&self, lib_idx: usize, show_id: &str) -> bool {
        let Some(state) = self.audiobookshelf_browse.get(lib_idx) else {
            return false;
        };
        state.shows.is_empty()
            || state
                .shows
                .iter()
                .any(|show| show.library_item_id == show_id)
    }

    /// Whether the published per-show cache can still name `target` (row
    /// 10.1): a cache that holds no episode admits the pick, and a landed
    /// cache admits only an episode it carries.
    fn gui_podcast_episode_known(&self, lib_idx: usize, target: &PodcastEpisodeTarget) -> bool {
        let Some(state) = self.audiobookshelf_browse.get(lib_idx) else {
            return false;
        };
        state.detail_cache.is_empty()
            || state
                .episode_by_identity(target.library_item_id(), target.episode_id())
                .is_some()
    }
}
