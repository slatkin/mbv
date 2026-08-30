use super::components::{
    BrowserComponent, BrowserKey, BrowserKind, ComponentId, ShellRequest, TvWorkspaceComponent,
};
use super::render::TvWideRenderCtx;
use super::shell::Model;
use super::{PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    pub(super) fn handle_tv_request(&mut self, request: ShellRequest) {
        let Some(lib_idx) = self.app.tab.emby_library_index() else {
            return;
        };
        match request {
            ShellRequest::TvEpisodeActivate => {
                let Some((series_id, season_cursor, episode_cursor)) =
                    self.tv_episode_activation_selection()
                else {
                    return;
                };
                self.app
                    .play_tv_episode(&series_id, season_cursor, episode_cursor);
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
                        self.app.activate_selected_series_item(&item);
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
            // Episode and season cursors are component-local until the later
            // episode action slice; retain typed routing without touching App.
            ShellRequest::TvEpisodeMove { .. } | ShellRequest::TvSeasonMove { .. } => {}
            // unreachable: shell_messages.rs routes only the Tv* group
            // (MoveRows/MoveColumn/JumpCursor/Activate/EpisodeActivate/Back/
            // CycleLetterPill/EpisodeMove/SeasonMove) into handle_tv_request;
            // every one has an arm above.
            _ => {}
        }
    }

    fn tv_episode_activation_selection(&self) -> Option<(String, usize, usize)> {
        let id = self.tv_workspace_id.as_ref()?;
        self.application
            .get_component(id)
            .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
            .and_then(TvWorkspaceComponent::episode_activation_selection)
    }

    pub(super) fn tv_workspace_component_id(&self) -> Option<ComponentId> {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return None;
        };
        let library = self.app.libs.get(index)?;
        if library.library.collection_type != "tvshows" || !self.app.layout.main.is_wide_tv_active()
        {
            return None;
        }
        Some(ComponentId::TvWorkspace(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind: BrowserKind::TvShows,
        }))
    }

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
                    self.tv_workspace_id = Some(id);
                    self.push_tv_workspace_content();
                }
                None => {
                    self.tv_workspace_id = None;
                }
            }
        }

        if self.tv_workspace_id.is_none() {
            // No wide TV workspace mounted: a pending breakpoint-hand-off
            // re-anchor cannot be delivered and must not fire later.
            self.tv_workspace_reanchor = false;
        }
    }

    /// Breakpoint hand-off (migrate-narrow-browse task 2.3 / D5): when the
    /// wide TV breakpoint flips while a TV library is active, the
    /// active-destination pointer moves between `TvWorkspaceComponent` and
    /// the narrow `BrowserComponent`, each owning its own selection cursor.
    /// Carry the outgoing component's live cursor into the resting
    /// `BrowseLevel` (the `persist_emby_browser_scroll` write-back shape) and
    /// arm the incoming component's one-shot re-anchor (the
    /// `music_workspace_reanchor` shape) so the selected series survives the
    /// flip. Runs once per tick before `sync_emby_browser` / `sync_tv_workspace`
    /// repoint the pointers.
    pub(super) fn hand_off_tv_breakpoint(&mut self) {
        let TabSelection::EmbyLibrary(lib_idx) = self.app.tab else {
            return;
        };
        let Some(library) = self.app.libs.get(lib_idx) else {
            return;
        };
        if library.library.collection_type != "tvshows" {
            return;
        }
        let wide = self.app.layout.main.is_wide_tv_active();
        match (
            wide,
            self.tv_workspace_id.clone(),
            self.emby_browser_id.clone(),
        ) {
            // wide -> narrow: persist the wide workspace's live cursor/scroll
            // to the resting level so the narrow browser's `set_content`
            // adopts it on the same tick.
            (false, Some(id), _) => {
                let Some((cursor, scroll)) = self
                    .application
                    .get_component(&id)
                    .and_then(|comp| comp.as_any().downcast_ref::<TvWorkspaceComponent>())
                    .map(|tv| (tv.cursor(), tv.scroll()))
                else {
                    return;
                };
                if let Some(level) = self.app.libs[lib_idx].nav_stack.last_mut() {
                    level.set_resting_cursor(cursor);
                    level.set_resting_scroll(scroll);
                }
                self.app.save_default_library_position(lib_idx);
            }
            // narrow -> wide: the narrow browser already writes `level.cursor`
            // on every move; persist its painted scroll too, then arm the
            // kept-mounted wide workspace's one-shot re-anchor to that
            // resting position.
            (true, _, Some(id)) => {
                if let Some(scroll) = self
                    .application
                    .get_component(&id)
                    .and_then(|comp| comp.as_any().downcast_ref::<BrowserComponent>())
                    .map(BrowserComponent::scroll)
                {
                    self.app.persist_library_scroll(lib_idx, scroll);
                }
                self.tv_workspace_reanchor = true;
            }
            _ => {}
        }
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
        if library.library.collection_type != "tvshows" || !self.app.layout.main.is_wide_tv_active()
        {
            return;
        }
        let list = self.app.library_list_render_ctx(
            index,
            false,
            self.app.libs[index]
                .nav_stack
                .last()
                .map_or(0, |l| l.resting().cursor()),
            self.app.libs[index]
                .nav_stack
                .last()
                .map_or(0, |l| l.resting().scroll()),
        );
        // Consume the one-shot breakpoint hand-off re-anchor (task 2.3 / D5):
        // when the active-destination pointer just flipped from the narrow
        // BrowserComponent, adopt the resting cursor/scroll it left behind
        // instead of this kept-mounted component's stale local cursor.
        let reanchor =
            std::mem::take(&mut self.tv_workspace_reanchor).then(|| (list.cursor(), list.scroll()));
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
        let series_detail = selected_series
            .as_ref()
            .and_then(|item| self.app.series_detail_cache.get(&item.id).cloned());
        let context = TvWideRenderCtx::new(
            list,
            selected_series,
            series_detail,
            0,
            None,
            matches!(self.app.effective_panel_focus(), PanelFocus::Library),
            self.app.should_show_letter_pills(index),
        );
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(tv) = comp.as_any_mut().downcast_mut::<TvWorkspaceComponent>() {
                tv.set_content(context);
                if let Some((cursor, scroll)) = reanchor {
                    tv.re_anchor(cursor, scroll);
                }
            }
        }
    }

    pub(super) fn render_tv_workspace_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.tv_workspace_id.as_ref() else {
            return;
        };
        let area = self.app.layout.main.tv_wide_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(id, frame, area);
    }
}

#[cfg(test)]
#[path = "shell_tv_workspace_tests.rs"]
mod tests;
