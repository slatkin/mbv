//! Shell wiring for the TV embedded content owner (`TvContent`, tasks
//! 8.1–8.4, design D2/D12). Mirrors `shell_emby_library_content.rs`: the owner
//! lives inside the mounted `LibraryPanel`, addressed by
//! `LibraryKey::Service(LibraryKey{TvShows})`, and the shell projects
//! Model-owned browse snapshots into it. Task 8.4 deleted the last mounted TV
//! component, so there is no TV-specific `ComponentId`, no mount/unmount pass
//! and no shell render step — the panel paints the owner and resolves its
//! pointer geometry.
//!
//! Typed effects need no new dispatch: the owner emits the same
//! `ShellRequest::Tv*`/`EmbyLibrary*` messages the mounted component did, and
//! the shell's dispatch is keyed by the active tab, not by which owner sent
//! it.

use super::components::library_panel::{LibraryKey, LibraryPanel};
use super::components::tv_content::TvContent;
use super::components::ComponentId;
use super::components::{LibraryKind, ShellRequest};
use super::render::TvWideRenderCtx;
use super::shell::{Model, PendingEpisodeSelection};
use super::TabSelection;
use mbv_core::api::EmbyItem;
use mbv_core::config::ServiceKind;

impl Model {
    pub(super) fn handle_tv_request(&mut self, request: ShellRequest) {
        let Some(lib_idx) = self.app.tab.emby_library_index() else {
            return;
        };
        match request {
            ShellRequest::TvTreeExpand { target } => {
                let source = self
                    .tv_owner()
                    .and_then(|owner| owner.tree_expansion_source(&target));
                if let Some((series_id, season_id)) = source {
                    if let Some(season_id) = season_id {
                        self.app.fetch_series_season_episodes(series_id, season_id);
                    } else {
                        self.app.fetch_series_detail(series_id);
                    }
                    self.push_tv_workspace_content();
                }
            }
            // The owner resolved the episode from its own season detail and
            // carried the stable item (design.md D4); the shell plays it
            // directly without reading any owner cursor. An id-less
            // `/Shows/Upcoming` placeholder carries a `series_id` instead of
            // a playable episode id: navigate to that series' Workspace
            // rather than submitting an empty-id item to playback.
            ShellRequest::TvEpisodeActivate { episode } => {
                if !self
                    .app
                    .open_series_for_unplayable_episode(lib_idx, &episode)
                {
                    self.app.play_item(episode);
                }
            }
            // Activation, back, and letter-pill effects resolve the
            // owner's selection directly (item-targeted) or from the App
            // nav stack; no cursor mirror remains.
            ShellRequest::TvMoveRows { .. }
            | ShellRequest::TvJumpCursor { .. }
            | ShellRequest::TvActivate { .. }
            | ShellRequest::TvBack
            | ShellRequest::TvCycleLetterPill { .. } => {
                match request {
                    ShellRequest::TvActivate { item } => {
                        // Flat Latest/Upcoming rows are leaf episodes. Keep a
                        // defensive item-kind gate here so a stale or legacy
                        // request can never enter the Series workspace -- an
                        // id-less Upcoming placeholder is the one exception
                        // and routes to its series' Workspace.
                        if item.item_type == "Episode" {
                            if !self.app.open_series_for_unplayable_episode(lib_idx, &item) {
                                self.app.play_item(item);
                            }
                        } else {
                            let owner_has_target = self
                                .tv_owner()
                                .and_then(TvContent::selected_tree_show)
                                .is_some_and(|selected| {
                                    selected.id == item.id && selected.name == item.name
                                });
                            if self.app.wide_tv_library_area(lib_idx).is_some() {
                                self.app.activate_selected_series_item(lib_idx, &item);
                            } else if owner_has_target {
                                self.open_library_hero_overlay();
                            }
                        }
                    }
                    ShellRequest::TvBack => self.app.go_back(lib_idx),
                    ShellRequest::TvCycleLetterPill { delta } => {
                        self.app.cycle_letter_pill(lib_idx, delta);
                        self.acknowledge_active_tv_latest();
                    }
                    // closed set: the outer arm's guard already restricts this to
                    // TvMoveRows/TvJumpCursor/TvActivate/TvBack/
                    // TvCycleLetterPill; the pure cursor moves need no App effect.
                    _ => {}
                }
                self.push_tv_workspace_content();
            }
            ShellRequest::TvEpisodeMove { .. } => {}
            ShellRequest::TvSeasonMove { .. } => {
                // The owner moves its season cursor first; use that
                // authoritative selection to lazily fetch uncached episodes.
                let selected_season = self.tv_owner().and_then(|owner| owner.selected_season());
                if let Some((series_id, season_id)) = selected_season {
                    self.app.fetch_series_season_episodes(series_id, season_id);
                    self.push_tv_workspace_content();
                }
            }
            // unreachable: shell_messages.rs routes only the Tv* group
            // (TreeExpand/MoveRows/JumpCursor/Activate/EpisodeActivate/Back/
            // CycleLetterPill/EpisodeMove/SeasonMove) into handle_tv_request;
            // every one has an arm above.
            _ => {}
        }
    }

    /// One-shot shell-driven selection re-anchor: point the active TV
    /// owner's series selection at a stable target (a navigation the shell
    /// performed, e.g. an Inline Search activation); the next content push
    /// preserves it.
    pub(super) fn reanchor_tv_owner_selection(&mut self, target: &str) {
        self.update_tv_owner(|owner| {
            owner.select_series_target(target);
        });
    }

    /// Make the TV owner's workspace active: episode selection holds the
    /// local focus (the same state the ordinary second Enter enters).
    pub(super) fn focus_tv_owner_episodes(&mut self) {
        self.update_tv_owner(TvContent::enter_episode_selection);
    }

    /// The series detail hand-off shared by Inline Search's series activation
    /// and a completed `NavigateLanding::Series` (task 3.1, design D3): the
    /// owner re-anchors onto the series, the workspace content is pushed, the
    /// Wide workspace / Narrow Library Hero overlay opens, and the content is
    /// re-pushed so the opened presentation reads the re-anchored selection.
    pub(super) fn open_series_workspace_handoff(&mut self, lib_idx: usize, item: &EmbyItem) {
        // Re-anchor the owner's selection onto the navigated-to series before
        // the presentation push reads it (the owner otherwise preserves its
        // prior stable target).
        self.reanchor_tv_owner_selection(&item.id);
        self.push_tv_workspace_content();
        if self.app.wide_tv_library_area(lib_idx).is_some() {
            self.app.activate_selected_series_item(lib_idx, item);
            // The workspace is active: episode selection takes the local
            // focus, like music's track-selection mode.
            self.focus_tv_owner_episodes();
        } else {
            self.open_library_hero_overlay();
        }
        self.push_tv_workspace_content();
    }

    /// Consume a landing that completed since the last drain (task 3.1): the
    /// App arms the hand-off at the actual landing completion -- the immediate
    /// `NavigateTo` arm or the deferred pending-landing retry -- and this
    /// runs the same presentation sequence Inline Search's series activation
    /// runs. A no-op when no landing completed. A carried episode id (task
    /// 6.1, design D6) arms the deep-selection pending after the presentation
    /// opens; the selection itself resolves against the series detail.
    pub(in crate::app) fn drain_series_navigation_handoff(&mut self) {
        let Some(handoff) = self.app.pending_series_handoff.take() else {
            return;
        };
        self.open_series_workspace_handoff(handoff.lib_idx, &handoff.reveal);
        if let Some(episode_id) = handoff.episode_id {
            self.pending_episode_selection = Some(PendingEpisodeSelection {
                lib_idx: handoff.lib_idx,
                series_id: handoff.reveal.id.clone(),
                episode_id,
            });
            self.drain_pending_episode_selection();
        }
    }

    /// Deep-selection retry (task 6.1, design D6): resolve the navigated
    /// episode against the cached series detail -- fetching a season's
    /// episodes when uncached -- and select it with episode focus. Runs at
    /// the sync pass and after every lib-event drain, so the detail and
    /// season-episodes fetches it arms re-drive it. Absence is not failure:
    /// once every season's episodes are in hand without a match, the pending
    /// clears and the landed show keeps its default selection (no error --
    /// the navigation target was reached). A manual tab change discards the
    /// pending silently, like the hand-off it extends.
    pub(super) fn drain_pending_episode_selection(&mut self) {
        enum Step {
            /// The series detail (or an in-flight season fetch) has not
            /// landed yet; stay armed.
            Wait,
            /// The season's episodes are uncached: arm the fetch and retry
            /// on its drain (`fetch_series_season_episodes` deduplicates).
            Fetch(String),
            /// The episode is in the season's cached episodes.
            Select(usize),
            /// Every season's episodes are in hand; the episode is absent.
            Absent,
        }
        let Some(sel) = self.pending_episode_selection.clone() else {
            return;
        };
        if self.app.tab != TabSelection::EmbyLibrary(sel.lib_idx) {
            self.pending_episode_selection = None;
            return;
        }
        let step = match self.app.series_detail_cache.get(&sel.series_id) {
            None => Step::Wait,
            Some(detail) => detail
                .seasons
                .iter()
                .enumerate()
                .find_map(
                    |(season_index, season)| match detail.episodes.get(&season.id) {
                        None => Some(Step::Fetch(season.id.clone())),
                        Some(episodes)
                            if episodes.iter().any(|episode| episode.id == sel.episode_id) =>
                        {
                            Some(Step::Select(season_index))
                        }
                        Some(_) => None,
                    },
                )
                .unwrap_or(Step::Absent),
        };
        match step {
            Step::Wait => {}
            Step::Fetch(season_id) => {
                self.app
                    .fetch_series_season_episodes(sel.series_id.clone(), season_id);
            }
            Step::Select(season_index) => {
                let selected = self
                    .update_tv_owner(|owner| {
                        owner.select_episode_in_season(season_index, &sel.episode_id)
                    })
                    .unwrap_or(false);
                self.pending_episode_selection = None;
                if selected {
                    // Materialize the moved season/episode rows in the opened
                    // presentation.
                    self.push_tv_workspace_content();
                }
            }
            Step::Absent => self.pending_episode_selection = None,
        }
    }

    pub(super) fn acknowledge_active_tv_latest(&mut self) {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let Some(library) = self.app.libs.get(index) else {
            return;
        };
        if library.library.collection_type == "tvshows"
            && library.tv_content_mode == Some(mbv_core::config::TvContentMode::Latest)
        {
            self.acknowledge_home_latest(super::types_playback::DestinationLatestSource::Emby(
                library.library.id.clone(),
            ));
        }
    }

    /// The active TV library's owner key (design D2's
    /// `LibraryKey::Service(LibraryKey)`), or `None` for every other tab.
    fn tv_owner_key(&self) -> Option<LibraryKey> {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return None;
        };
        let library = self.app.libs.get(index)?;
        if library.library.collection_type != "tvshows" {
            return None;
        }
        Some(LibraryKey::Service {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind: LibraryKind::TvShows,
        })
    }

    /// The TV owner installed for the active library, whether or not its tab
    /// is active (the shell's typed reads reach a retained owner through
    /// `as_any`, exactly as the former mounted-owner downcast did).
    fn tv_owner(&self) -> Option<&TvContent> {
        let key = self.tv_owner_key()?;
        self.library_owner(&key)
    }

    /// Mutate the TV owner inside the mounted `LibraryPanel` (design D2: the
    /// shell pushes content addressed by `LibraryKey`), creating it on first
    /// push.
    fn update_tv_owner<R>(&mut self, f: impl FnOnce(&mut TvContent) -> R) -> Option<R> {
        let key = self.tv_owner_key()?;
        self.update_library_owner(key, || Box::new(TvContent::new()), f)
    }

    /// Test-only: the active TV owner's key, for shell tests' owner-map
    /// membership checks (mirrors [`Model::test_tv_owner`]).
    #[cfg(test)]
    pub(super) fn test_tv_owner_key(&self) -> LibraryKey {
        self.tv_owner_key().expect("TV library active")
    }

    /// Test-only: the owner key for library `index` whether or not it is the
    /// active tab (the Movies-tab assertions check the absence of a TV owner
    /// for a non-TV library).
    #[cfg(test)]
    pub(super) fn test_tv_owner_key_at(&self, index: usize) -> LibraryKey {
        LibraryKey::Service {
            service: ServiceKind::Emby,
            library_id: self.app.libs[index].library.id.clone(),
            kind: LibraryKind::TvShows,
        }
    }

    /// Test-only: the mounted panel's last painted role rects, for the
    /// characterization tests that used to read the deleted component's
    /// geometry (task 8.4).
    #[cfg(test)]
    pub(in crate::app) fn test_painted_library_layout(
        &self,
    ) -> crate::app::layout::PaintedRowGeometry {
        self.application
            .get_component(&ComponentId::Library)
            .expect("library panel mounted")
            .as_any()
            .downcast_ref::<LibraryPanel>()
            .expect("LibraryPanel")
            .test_painted_layout()
    }

    /// Test-only: the panel-hosted TV owner (task 8.4 deleted
    /// TV-specific component id, so tests address the owner through the panel's
    /// `LibraryKey` map exactly as production does).
    #[cfg(test)]
    pub(super) fn test_tv_owner(&self) -> &TvContent {
        let key = self.tv_owner_key().expect("TV library active");
        self.library_owner(&key).expect("tv owner installed")
    }

    /// Test-only: mutable twin of [`Model::test_tv_owner`].
    #[cfg(test)]
    pub(super) fn test_tv_owner_mut(&mut self) -> &mut TvContent {
        let key = self.tv_owner_key().expect("TV library active");
        self.library_owner_mut(&key).expect("tv owner installed")
    }

    /// Test-only: paint the mounted `LibraryPanel` into `area` and take the
    /// hero image paint it retained (task 8.4: the panel, not the owner, is
    /// the paint path — the shell's own draw does the same through
    /// `render_library_panel` + `take_image_paint`).
    #[cfg(test)]
    pub(super) fn test_paint_library_panel(
        &mut self,
        area: ratatui::layout::Rect,
    ) -> Option<crate::app::components::library_panel::PanelHeroImagePaint> {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(area.width, area.height))
                .expect("test terminal");
        {
            let panel = self
                .application
                .get_component_mut(&ComponentId::Library)
                .expect("library panel mounted")
                .as_any_mut()
                .downcast_mut::<LibraryPanel>()
                .expect("LibraryPanel");
            terminal
                .draw(|frame| tuirealm::component::Component::view(panel, frame, area))
                .expect("paint library panel");
        }
        self.application
            .get_component_mut(&ComponentId::Library)
            .expect("library panel mounted")
            .as_any_mut()
            .downcast_mut::<LibraryPanel>()
            .expect("LibraryPanel")
            .take_image_paint()
    }

    /// Project this frame's TV snapshot into the owner (task 8.4; the
    /// retired `sync_tv_workspace` body). The owner is retained across
    /// pushes, so its cursor/scroll/season survive a refresh and a hidden
    /// tab leaves the retained owner untouched. Runs in the sync pass so the
    /// owner exists before the panel's focus/mouse pass reads the migrated
    /// owner map.
    pub(super) fn sync_tv_content(&mut self) {
        self.push_tv_workspace_content();
    }

    /// The compact mini-view is the one presentation where a selected flat
    /// TV episode gets the existing Library Hero overlay. It is synchronized
    /// after the panel points at the active owner, so opening it never targets
    /// the previous tab's owner.
    pub(super) fn sync_tv_mini_view_hero(&mut self) {
        let mini_view = self.app.terminal_width < crate::app::MINI_VIEW_THRESHOLD
            && matches!(self.app.effective_panel_focus(), super::PanelFocus::Library);
        if let Some(panel) = self
            .application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        {
            panel.sync_mini_view_hero_overlay(mini_view);
        }
    }

    pub(super) fn push_tv_workspace_content(&mut self) {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let Some(library) = self.app.libs.get(index) else {
            return;
        };
        let library_id = library.library.id.clone();
        if library.library.collection_type != "tvshows" {
            return;
        }
        let lib_area = self.app.wide_tv_library_area(index);
        let is_wide = lib_area.is_some();
        let list = self.app.library_list_render_ctx(
            index,
            self.app.libs[index]
                .nav_stack
                .last()
                .map_or(0, |l| l.resting().cursor()),
        );
        let tv_content_mode = self.app.libs[index]
            .nav_stack
            .last()
            .and_then(|level| level.tv_content_mode.clone())
            .or_else(|| self.app.libs[index].tv_content_mode.clone());
        // The TV owner owns the selection cursor. Derive the pushed Series
        // snapshot from the owner's authoritative selection (its own cursor
        // over its cached list), not the App browse cursor. Only on first
        // mount, when the owner has no prior content, fall back to the
        // App-derived item.
        let selected_series = self
            .tv_owner()
            .and_then(TvContent::selected_tree_show)
            .filter(|selected| {
                list.items
                    .iter()
                    .any(|item| item.id == selected.id && item.name == selected.name)
            })
            .or_else(|| self.tv_owner().and_then(TvContent::selected_item))
            .filter(|item| item.item_type == "Series")
            .or_else(|| {
                list.selected_item()
                    .cloned()
                    .filter(|item| item.item_type == "Series")
            });
        let selected_series = if matches!(
            tv_content_mode,
            Some(
                mbv_core::config::TvContentMode::Latest | mbv_core::config::TvContentMode::Upcoming
            )
        ) {
            None
        } else {
            selected_series
        };
        if let Some(item) = selected_series.as_ref() {
            // Detail loading is an App-owned effect; schedule it at the shell
            // hand-off rather than from the render context or painter.
            self.app.fetch_series_detail(item.id.clone());
        }
        let series_detail = selected_series
            .as_ref()
            .and_then(|item| self.app.series_detail_cache.get(&item.id).cloned());
        // Loaded details for every listed show: the tree projects children
        // per show so an expanded show keeps them while another is selected.
        let series_details = list
            .items
            .iter()
            .filter_map(|item| {
                self.app
                    .series_detail_cache
                    .get(&item.id)
                    .map(|detail| (item.id.clone(), detail.clone()))
            })
            .collect();
        // The hero image is NOT projected here: task 5.10's central projection
        // (`sync_library_hero_images`, design D9) is the one projector for
        // every migrated owner, and TV is one since task 8.4. Pushing a second
        // projection here would race the sync-pass one (double fetch, and the
        // sync-pass `Loading` state overwriting the push's `Ready`).
        let mut context = TvWideRenderCtx::new(
            list,
            selected_series,
            series_detail,
            0,
            None,
            self.app.should_show_letter_pills(index),
        );
        context.set_tv_content_mode(tv_content_mode.clone());
        context.set_series_details(series_details);
        let latest_source =
            super::types_playback::DestinationLatestSource::Emby(library_id.clone());
        if tv_content_mode == Some(mbv_core::config::TvContentMode::Latest)
            && !self
                .acknowledged_home_latest_sources
                .contains(&latest_source)
        {
            // Selection is an acknowledgement even when Latest was already
            // selected before its asynchronous snapshot arrived.
            self.record_home_latest_acknowledgement(latest_source.clone());
        }
        let latest_has_new_content = self
            .tv_latest_snapshots
            .get(&library_id)
            .is_some_and(|snapshot| snapshot.has_new_content);
        let latest_acknowledged = self
            .acknowledged_home_latest_sources
            .contains(&latest_source);
        let list_pane_width = self.app.list_pane_width;
        // Panel focus is the library area's focus bit; the owner paints its
        // focused pane and claims local chords from it (task 8.4).
        let library_focused =
            matches!(self.app.effective_panel_focus(), super::PanelFocus::Library);
        self.update_tv_owner(|owner| {
            owner.set_is_wide(is_wide);
            owner.set_list_pane_width(list_pane_width);
            owner.set_latest_marker(latest_has_new_content, latest_acknowledged);
            owner.set_content(context);
            owner.set_focused(library_focused);
        });
    }
}

#[cfg(test)]
#[path = "shell_tv_workspace_tests.rs"]
mod tests;
