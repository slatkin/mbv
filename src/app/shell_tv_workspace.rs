use super::components::{BrowserKey, BrowserKind, ComponentId, ShellRequest, TvWorkspaceComponent};
use super::images::series_image_cache_key;
use super::render::components::hero_model::SERIES_LANDSCAPE_IMAGE_TYPES;
use super::render::TvWideRenderCtx;
use super::shell::Model;
use super::TabSelection;
use mbv_core::config::ServiceKind;

impl Model {
    pub(super) fn handle_tv_request(&mut self, request: ShellRequest) {
        let Some(lib_idx) = self.app.tab.emby_library_index() else {
            return;
        };
        match request {
            // The component resolved the episode from its own season detail and
            // carried the stable item (design.md D4); the shell plays it
            // directly without reading any component cursor.
            ShellRequest::TvEpisodeActivate { episode } => {
                self.app.play_item(episode);
            }
            // Activation, back, and letter-pill effects resolve the
            // component's selection directly (item-targeted) or from the App
            // nav stack; no cursor mirror remains.
            ShellRequest::TvMoveRows { .. }
            | ShellRequest::TvMoveColumn { .. }
            | ShellRequest::TvJumpCursor { .. }
            | ShellRequest::TvActivate { .. }
            | ShellRequest::TvBack
            | ShellRequest::TvCycleLetterPill { .. } => {
                match request {
                    ShellRequest::TvActivate { item } => {
                        self.app.activate_selected_series_item(lib_idx, &item);
                    }
                    ShellRequest::TvBack => self.app.go_back(lib_idx),
                    ShellRequest::TvCycleLetterPill { delta } => {
                        self.app.cycle_letter_pill(lib_idx, delta)
                    }
                    // closed set: the outer arm's guard already restricts this to
                    // TvMoveRows/TvMoveColumn/TvJumpCursor/TvActivate/TvBack/
                    // TvCycleLetterPill; the pure cursor moves need no App effect.
                    _ => {}
                }
                self.push_tv_workspace_content();
            }
            ShellRequest::TvEpisodeMove { .. } => {}
            ShellRequest::TvSeasonMove { .. } => {
                // The component moves its season cursor first; use that
                // authoritative selection to lazily fetch uncached episodes.
                let selected_season = self
                    .tv_workspace_id
                    .as_ref()
                    .and_then(|id| self.application.get_component(id))
                    .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
                    .and_then(TvWorkspaceComponent::selected_season);
                if let Some((series_id, season_id)) = selected_season {
                    self.app.fetch_series_season_episodes(series_id, season_id);
                    self.push_tv_workspace_content();
                }
            }
            // unreachable: shell_messages.rs routes only the Tv* group
            // (MoveRows/MoveColumn/JumpCursor/Activate/EpisodeActivate/Back/
            // CycleLetterPill/EpisodeMove/SeasonMove) into handle_tv_request;
            // every one has an arm above.
            _ => {}
        }
    }

    /// The one merged TV owner's `ComponentId` (design.md D12, task 8.1):
    /// mounted for a `tvshows` library at every breakpoint, so the id no
    /// longer depends on `App::wide_tv_library_area`.
    pub(super) fn tv_workspace_component_id(&self) -> Option<ComponentId> {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return None;
        };
        let library = self.app.libs.get(index)?;
        if library.library.collection_type != "tvshows" {
            return None;
        }
        Some(ComponentId::TvWorkspace(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind: BrowserKind::TvShows,
        }))
    }

    /// Populate `tv_wide_*` layout geometry *before* the mount/focus gates in
    /// `sync_mounted_surfaces` read it. Consumers gated on the paint-free
    /// `App::wide_tv_library_area` no longer need this to avoid a
    /// narrow→wide flash, but other readers of `tv_wide_right_area`/
    /// `tv_wide_left_area` (e.g. context-menu anchors) still rely on this
    /// priming this same frame. This recomputes exactly what
    /// `render_library` publishes, but paint-free from the current terminal
    /// size (mirrors Music's `render_music_workspace_component` priming its
    /// own `publish_geometry`).
    pub(super) fn prime_wide_tv_geometry(&mut self) {
        use ratatui::layout::Rect;
        match self
            .app
            .tab
            .emby_library_index()
            .and_then(|idx| self.app.wide_tv_library_area(idx).map(|area| (idx, area)))
        {
            Some((idx, lib_area)) => {
                let ctx = self.app.wide_tv_render_ctx(idx, None);
                ctx.publish_geometry(lib_area, &mut self.app.layout.main);
            }
            None => {
                // Clear any stale wide rect so the mount gate does not stay
                // wide after leaving the workspace or narrowing the terminal.
                self.app.layout.main.tv_wide_area = Rect::default();
                self.app.layout.main.tv_wide_left_area = Rect::default();
                self.app.layout.main.tv_wide_right_area = Rect::default();
            }
        }
    }

    /// Mount/retire the one merged TV owner (design.md D12): unlike before
    /// the merge, a Wide<->Narrow breakpoint flip never changes which
    /// component is mounted, so there is no hand-off to perform here.
    pub(super) fn sync_tv_workspace(&mut self) {
        let next_id = self.tv_workspace_component_id();
        if self.tv_workspace_id != next_id {
            match next_id {
                Some(id) => {
                    if !self.application.mounted(&id) {
                        self.application
                            .mount(id.clone(), Box::new(TvWorkspaceComponent::new()), vec![])
                            .expect("mount TV workspace");
                        self.register_destination(&id);
                    }
                    self.tv_workspace_id = Some(id.clone());
                }
                None => {
                    self.tv_workspace_id = None;
                }
            }
        }
        self.push_tv_workspace_content();
    }

    pub(super) fn push_tv_workspace_content(&mut self) {
        let Some(id) = self.tv_workspace_id.as_ref() else {
            return;
        };
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let Some(library) = self.app.libs.get(index) else {
            return;
        };
        if library.library.collection_type != "tvshows" {
            return;
        }
        let is_wide = self.app.wide_tv_library_area(index).is_some();
        let list = self.app.library_list_render_ctx(
            index,
            self.app.libs[index]
                .nav_stack
                .last()
                .map_or(0, |l| l.resting().cursor()),
            self.app.libs[index]
                .nav_stack
                .last()
                .map_or(0, |l| l.resting().scroll()),
        );
        // The TV component owns the selection cursor. Derive the pushed Series
        // snapshot from the component's authoritative selection (its own cursor
        // over its cached list), not the App browse cursor (which the removed
        // mirror used to keep in sync). Only on first mount, when the component
        // has no prior content, fall back to the App-derived item.
        let selected_series = self
            .application
            .get_component(id)
            .and_then(|comp| comp.as_any().downcast_ref::<TvWorkspaceComponent>())
            .and_then(TvWorkspaceComponent::selected_item)
            .filter(|item| item.item_type == "Series")
            .or_else(|| {
                list.selected_item()
                    .cloned()
                    .filter(|item| item.item_type == "Series")
            });
        if let Some(item) = selected_series.as_ref() {
            // Detail loading is an App-owned effect; schedule it at the shell
            // hand-off rather than from the render context or painter.
            self.app.fetch_series_detail(item.id.clone());
        }
        let series_detail = selected_series
            .as_ref()
            .and_then(|item| self.app.series_detail_cache.get(&item.id).cloned());
        // The Wide hero's landscape-chain fetch runs from this push
        // (design D9's TV precedent); Narrow's own Primary-chain fetch stays
        // a paint-time `HomeImagePaint` return (unchanged by this merge --
        // task 5.10 generalizes both to the shell projection later).
        let mut image_loading = false;
        if is_wide {
            if let Some(item) = selected_series
                .as_ref()
                .filter(|_| self.app.images_enabled())
            {
                self.app.fetch_card_image(
                    series_image_cache_key(&item.id, SERIES_LANDSCAPE_IMAGE_TYPES),
                    item.id.clone(),
                    String::new(),
                    SERIES_LANDSCAPE_IMAGE_TYPES,
                );
            }
            image_loading = selected_series.as_ref().is_some_and(|item| {
                self.app.images_enabled()
                    && !self
                        .app
                        .card_image_states
                        .contains_key(&series_image_cache_key(
                            &item.id,
                            SERIES_LANDSCAPE_IMAGE_TYPES,
                        ))
            });
        }
        let context = TvWideRenderCtx::new(
            list,
            selected_series,
            series_detail,
            0,
            None,
            self.app.should_show_letter_pills(index),
        )
        .with_image_state(self.app.images_enabled(), image_loading);
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(tv) = comp.as_any_mut().downcast_mut::<TvWorkspaceComponent>() {
                tv.set_is_wide(is_wide);
                tv.set_content(context);
            }
        }
    }

    pub(super) fn render_tv_workspace_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.tv_workspace_id.as_ref() else {
            return;
        };
        if !self.library_panel_visible() {
            return;
        }
        let is_wide = self
            .app
            .tab
            .emby_library_index()
            .is_some_and(|idx| self.app.wide_tv_library_area(idx).is_some());
        let area = if is_wide {
            self.app.layout.main.tv_wide_area
        } else {
            self.app.layout.main.left_area
        };
        if area.width == 0 || area.height == 0 {
            return;
        }
        if is_wide {
            if let Some(comp) = self
                .application
                .get_component_mut(id)
                .and_then(|comp| comp.as_any_mut().downcast_mut::<TvWorkspaceComponent>())
            {
                comp.set_list_pane_width(self.app.list_pane_width);
            }
        } else {
            let browse_cursor = self
                .application
                .get_component(id)
                .and_then(|comp| comp.as_any().downcast_ref::<TvWorkspaceComponent>())
                .map_or(0, TvWorkspaceComponent::browse_cursor);
            let extras = self
                .app
                .tab
                .emby_library_index()
                .map(|lib_idx| self.app.narrow_browse_extras(lib_idx, browse_cursor));
            if let Some((comp, extras)) = self
                .application
                .get_component_mut(id)
                .and_then(|comp| comp.as_any_mut().downcast_mut::<TvWorkspaceComponent>())
                .zip(extras)
            {
                comp.set_narrow_extras(extras);
            }
        }
        self.application.view(id, frame, area);
        let image_paint = self
            .application
            .get_component_mut(id)
            .and_then(|comp| comp.as_any_mut().downcast_mut::<TvWorkspaceComponent>())
            .and_then(TvWorkspaceComponent::take_image_paint);
        self.app.paint_home_image(frame, image_paint);
    }
}

#[cfg(test)]
#[path = "shell_tv_workspace_tests.rs"]
mod tests;
