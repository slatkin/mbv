//! Shell projection for the grouped Music library owner.
//! Music is retained by the LibraryPanel under `LibraryKey::Service(Music)`;
//! it is not a mounted destination component.

use super::components::library_panel::{LibraryKey, LibraryPanel};
use super::components::music_content::MusicContent;
use super::components::{BrowserKey, BrowserKind, ComponentId, InlineSearchHost};
use super::shell::{Model, MusicTrackFocusRequest};
use super::TabSelection;
use mbv_core::config::ServiceKind;

impl Model {
    fn music_owner_key(&self) -> Option<LibraryKey> {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return None;
        };
        let library = self.app.libs.get(index)?;
        (library.library.collection_type == "music"
            && self.app.is_music_group_view(index)
            && self.app.is_viewing_album_folders(index))
        .then(|| {
            LibraryKey::Service(BrowserKey {
                service: ServiceKind::Emby,
                library_id: library.library.id.clone(),
                kind: BrowserKind::Music,
            })
        })
    }

    pub fn music_owner(&self) -> Option<&MusicContent> {
        let key = self.music_owner_key()?;
        self.application
            .get_component(&ComponentId::Library)
            .and_then(|c| c.as_any().downcast_ref::<LibraryPanel>())
            .and_then(|p| p.owner(&key))
            .and_then(|o| o.as_any().downcast_ref::<MusicContent>())
    }

    pub fn music_owner_mut(&mut self) -> Option<&mut MusicContent> {
        let key = self.music_owner_key()?;
        self.application
            .get_component_mut(&ComponentId::Library)
            .and_then(|c| c.as_any_mut().downcast_mut::<LibraryPanel>())
            .and_then(|p| p.owner_mut(&key))
            .and_then(|o| o.as_any_mut().downcast_mut::<MusicContent>())
    }

    fn update_music_owner<R>(&mut self, f: impl FnOnce(&mut MusicContent) -> R) -> Option<R> {
        let key = self.music_owner_key()?;
        if !self.library_panel_has_owner(&key) {
            self.push_library_owner(key.clone(), Box::new(MusicContent::new()));
        }
        self.application
            .get_component_mut(&ComponentId::Library)
            .and_then(|c| c.as_any_mut().downcast_mut::<LibraryPanel>())
            .and_then(|p| p.owner_mut(&key))
            .and_then(|o| o.as_any_mut().downcast_mut::<MusicContent>())
            .map(f)
    }

    pub(super) fn push_music_workspace_content(&mut self) {
        let Some(index) = self.app.tab.emby_library_index() else {
            return;
        };
        // A freshly created owner (first push for this `LibraryKey`) has no
        // prior selection to preserve: adopt the shell's resting cursor once,
        // explicitly, exactly as the old mount-time trigger did. A re-point
        // at an already-installed owner keeps its divergent local cursor.
        if let Some(key) = self.music_owner_key() {
            if !self.library_panel_has_owner(&key) {
                self.music_workspace_reanchor = true;
            }
        }
        let resting = self.app.libs[index]
            .nav_stack
            .last()
            .map(|l| (l.resting().cursor(), l.resting().scroll()));
        let selected_target = (!self.music_workspace_reanchor)
            .then(|| {
                self.music_owner()
                    .and_then(MusicContent::selected_item)
                    .map(|i| i.id)
            })
            .flatten();
        let cursor = self.app.libs[index]
            .nav_stack
            .last()
            .and_then(|level| {
                selected_target
                    .as_deref()
                    .and_then(|t| level.items.iter().position(|i| i.id == t))
            })
            .or_else(|| resting.map(|r| r.0));
        let mut context = self
            .app
            .wide_music_render_ctx(index, cursor.map(|c| (c, 0)));
        if let Some(owner) = self.music_owner() {
            if owner.inline_search().is_active() {
                context.list = context.list.with_search(
                    owner.inline_search().query().to_string(),
                    owner.inline_search().loading(),
                );
            }
        }
        if let Some(album) = context.selected_album.as_ref() {
            if !self.app.album_tracks_cache.contains_key(&album.id)
                && !self.app.album_tracks_loading.contains(&album.id)
            {
                self.app.fetch_album_tracks(album.id.clone());
            }
        }
        let reanchor = std::mem::take(&mut self.music_workspace_reanchor)
            .then(|| resting.unwrap_or((context.list.cursor(), 0)));
        let wide = self.app.is_right_panel_wide();
        let request = self.music_track_focus_request.take();
        let focused = matches!(self.app.effective_panel_focus(), super::PanelFocus::Library);
        self.update_music_owner(|owner| {
            owner.set_content(context);
            if let Some((c, s)) = reanchor {
                owner.re_anchor(c, s);
            }
            owner.set_focused(focused);
            owner.set_inline_track_focus_enabled(wide);
            match request {
                Some(MusicTrackFocusRequest::Clear) => owner.clear_track_focus(),
                Some(MusicTrackFocusRequest::Enter { album_id })
                    if wide && owner.selected_item().is_some_and(|a| a.id == album_id) =>
                {
                    owner.enter_track_focus()
                }
                _ => {}
            }
        });
    }

    pub(super) fn sync_music_workspace(&mut self) {
        self.push_music_workspace_content();
    }

    #[cfg(test)]
    pub(super) fn test_music_owner(&self) -> &MusicContent {
        self.music_owner().expect("music owner")
    }
    #[cfg(test)]
    pub(super) fn test_music_owner_mut(&mut self) -> &mut MusicContent {
        self.music_owner_mut().expect("music owner")
    }

    #[cfg(test)]
    pub(crate) fn render_music_workspace_component(&mut self, frame: &mut ratatui::Frame) {
        self.render_library_panel(frame);
    }
}
