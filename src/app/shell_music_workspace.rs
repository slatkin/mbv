use super::components::{BrowserKey, BrowserKind, ComponentId, MusicWorkspaceComponent};
use super::render::MusicWideRenderCtx;
use super::shell::Model;
use super::TabSelection;
use mbv_core::api::EmbyItem;
use mbv_core::config::ServiceKind;

impl Model {
    /// Resolve the focused track of the active Music workspace: the
    /// component owns the cursor index, the shell owns the target resolution
    /// (album + cached track list). Returns `None` when no track is focused,
    /// the cache has no entry, or the cursor is out of bounds.
    pub(super) fn focused_music_track(&self, _lib_idx: usize) -> Option<(String, EmbyItem)> {
        let id = self.music_workspace_id.as_ref()?;
        let comp = self
            .application
            .get_component(id)?
            .as_any()
            .downcast_ref::<MusicWorkspaceComponent>()?;
        let cursor = comp.track_cursor()?;
        let album = comp.selected_item()?;
        let track = self
            .app
            .album_tracks_cache
            .get(&album.id)?
            .get(cursor)?
            .clone();
        Some((album.id.clone(), track))
    }

    fn music_workspace_component_id(&self) -> Option<ComponentId> {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return None;
        };
        let library = self.app.libs.get(index)?;
        if library.library.collection_type != "music"
            || !self.app.is_music_group_view(index)
            || !self.app.is_viewing_album_folders(index)
        {
            return None;
        }
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind: BrowserKind::Music,
        }))
    }

    pub(super) fn sync_music_workspace(&mut self) {
        let next_id = self.music_workspace_component_id();
        if self.music_workspace_id != next_id {
            match next_id {
                Some(id) => {
                    if !self.application.mounted(&id) {
                        self.application
                            .mount(id.clone(), Box::new(MusicWorkspaceComponent::new()), vec![])
                            .expect("mount Music workspace");
                        self.register_destination(&id);
                        // First projection into a freshly mounted workspace:
                        // adopt the shell's resting cursor once, explicitly
                        // (a saved position restored before mount lands here).
                        // A re-point at an already-mounted component keeps its
                        // divergent local cursor.
                        self.music_workspace_reanchor = true;
                    }
                    self.music_workspace_id = Some(id);
                    self.push_music_workspace_content();
                }
                None => {
                    self.music_workspace_id = None;
                }
            }
        }

        if self.music_workspace_id.is_none() {
            // No Music workspace is mounted right now: a pending focus
            // request (recursive album activation / position restore that
            // landed on a non-mountable state) cannot be delivered, and must
            // not fire later on an unrelated album.
            self.music_track_focus_request = None;
            self.music_workspace_reanchor = false;
        }
    }

    /// Event-scoped projection replacing the per-frame content mirror:
    /// mirrors the active Music browse snapshot and panel geometry into the
    /// mounted component, preserving its local cursor and track-focus state.
    pub(super) fn push_music_workspace_content(&mut self) {
        let Some(id) = self.music_workspace_id.as_ref() else {
            return;
        };
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let Some(library) = self.app.libs.get(index) else {
            return;
        };
        if library.library.collection_type != "music"
            || !self.app.is_music_group_view(index)
            || !self.app.is_viewing_album_folders(index)
        {
            return;
        }
        // The Music component owns the selection cursor. Derive the selected
        // album from the component's authoritative selection (its own cursor),
        // not the App browse cursor. Only on first mount fall back to the
        // App-derived item.
        let list = self.app.library_list_render_ctx(index, true);
        let selected_album = self
            .application
            .get_component(id)
            .and_then(|comp| comp.as_any().downcast_ref::<MusicWorkspaceComponent>())
            .and_then(MusicWorkspaceComponent::selected_item)
            .or_else(|| list.selected_item().cloned());
        if let Some(album) = selected_album.as_ref() {
            if !self.app.album_tracks_cache.contains_key(&album.id)
                && !self.app.album_tracks_loading.contains(&album.id)
            {
                self.app.fetch_album_tracks(album.id.clone());
            }
        }
        let context: MusicWideRenderCtx = self.app.wide_music_render_ctx(index);
        let columns = self.app.current_library_columns(index);
        let wide = self.app.layout.main.is_wide_music_active();
        // Consume the one-shot re-anchor trigger: a genuine navigation event
        // (mount, group switch, recursive activation, saved-position restore)
        // adopts the shell's resting cursor/scroll below, unconditionally.
        let reanchor = std::mem::take(&mut self.music_workspace_reanchor)
            .then(|| (context.list.cursor(), context.list.scroll()));
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(music) = comp.as_any_mut().downcast_mut::<MusicWorkspaceComponent>() {
                music.set_content(context);
                if let Some((cursor, scroll)) = reanchor {
                    music.re_anchor(cursor, scroll);
                }
                music.set_album_columns(columns);
                music.set_page_rows(self.app.layout.main.left_area.height as usize);
                music.set_inline_track_focus_enabled(wide);
                // Consume the one-shot track-focus request after the content
                // push, so it cannot be clobbered by `set_content`'s album
                // identity reset on the same tick.
                if let Some(request) = self.music_track_focus_request.take() {
                    if request {
                        music.enter_track_focus();
                    } else {
                        music.clear_track_focus();
                    }
                }
            }
        }
    }

    pub(super) fn render_music_workspace_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.music_workspace_id.as_ref() else {
            return;
        };
        // Wide Music paints into `wide_music_area`; narrow Music has no wide
        // area, so fall back to the narrow main content area (`left_area`) so
        // the component's `view` is still reached. It paints nothing at narrow
        // until task 3.6 gives it a narrow branch.
        let mut area = self.app.layout.main.wide_music_area;
        if area.width == 0 || area.height == 0 {
            area = self.app.layout.main.left_area;
        }
        if area.width == 0 || area.height == 0 {
            return;
        }
        if let Some(lib_idx) = self.app.tab.emby_library_index() {
            let context = self.app.wide_music_render_ctx(lib_idx);
            context.publish_geometry(area, &mut self.app.layout.main);
        }
        self.application.view(id, frame, area);
        let projection = self
            .application
            .get_component_mut(id)
            .and_then(|comp| comp.as_any_mut().downcast_mut::<MusicWorkspaceComponent>())
            .map(|music| {
                let image_paint = music.take_image_paint();
                let layout = music.layout();
                (
                    image_paint,
                    layout.wide_music_track_hitmap.clone(),
                    layout.selected_item_rect,
                )
            });
        if let Some((image_paint, track_hitmap, selected_item_rect)) = projection {
            self.app.paint_music_image(frame, image_paint);
            self.app.layout.main.wide_music_track_hitmap = track_hitmap;
            self.app.layout.main.selected_item_rect = selected_item_rect;
        }
    }
}

#[cfg(test)]
#[path = "shell_music_workspace_tests.rs"]
mod tests;
