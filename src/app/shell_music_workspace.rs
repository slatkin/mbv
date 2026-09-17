//! Shell projection for the grouped Music library owner.
//! Music is retained by the LibraryPanel under `LibraryKey::Service(Music)`;
//! it is not a mounted destination component.

use super::components::library_panel::LibraryKey;
use super::components::music_content::MusicContent;
use super::components::LibraryKind;
use super::shell::{Model, MusicTrackFocusRequest};
use super::BrowseLevel;
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
        .then(|| LibraryKey::Service {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind: LibraryKind::Music,
        })
    }

    pub fn music_owner(&self) -> Option<&MusicContent> {
        let key = self.music_owner_key()?;
        self.library_owner(&key)
    }

    fn update_music_owner<R>(&mut self, f: impl FnOnce(&mut MusicContent) -> R) -> Option<R> {
        let key = self.music_owner_key()?;
        self.update_library_owner(key, || Box::new(MusicContent::new()), f)
    }

    /// The shell's reaction to a completed recursive album activation
    /// (`LibEvent::RecursiveAlbumActivated`): install the landed path through
    /// App, bind the one-shot inline track-focus request to the activated
    /// album, and re-anchor the workspace regardless of any prior local move.
    /// Sole owner of the return to the standard Music presentation, shared by
    /// Inline Search activation and a navigated `NavigateLanding::Album`
    /// (task 3.2: the arm already covers the navigated case).
    pub(in crate::app) fn on_recursive_album_activated(
        &mut self,
        library_id: String,
        nav_stack: Vec<BrowseLevel>,
    ) {
        let library_id_lookup = library_id.clone();
        self.app
            .handle_lib_event(super::LibEvent::RecursiveAlbumActivated {
                library_id,
                nav_stack,
            });
        // Bind the enter request to the activated album (the resting cursor of
        // the replaced nav stack) so it can retry once the album's tracks
        // arrive without ever firing on an album the user moved to meanwhile.
        self.music_track_focus_request = self
            .app
            .libs
            .iter()
            .find(|lib| lib.library.id == library_id_lookup)
            .and_then(|lib| {
                let level = lib.nav_stack.last()?;
                level
                    .items
                    .get(level.resting().cursor())
                    .map(|item| item.id.clone())
            })
            .map(|album_id| MusicTrackFocusRequest::Enter { album_id });
        // Nav stack was replaced wholesale; its resting cursor now points at
        // the activated album. Re-anchor the component explicitly, regardless
        // of prior local moves.
        self.music_workspace_reanchor = true;
    }

    pub(super) fn push_music_workspace_content(&mut self) {
        // Music projects only in the album-folder grouped view (the same
        // gate `music_owner_key` applies): outside it there is no owner to
        // push into, and the body below must not run its side effects
        // (`wide_music_render_ctx`, `fetch_album_tracks`) for an unrelated
        // Emby library/item.
        let Some(key) = self.music_owner_key() else {
            return;
        };
        // Keep grouped album content loading in the shell projection seam;
        // rendering must remain effect-free.
        if let TabSelection::EmbyLibrary(index) = self.app.tab {
            self.app.ensure_music_group_album_level(index);
        }
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        // A freshly created owner (first push for this `LibraryKey`) has no
        // prior selection to preserve: adopt the shell's resting cursor once,
        // explicitly, exactly as the old mount-time trigger did. A re-point
        // at an already-installed owner keeps its divergent local cursor.
        if !self.library_panel_has_owner(&key) {
            self.music_workspace_reanchor = true;
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
        let context = self.app.wide_music_render_ctx(index, cursor);
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
        // Activation can outrun the album's track fetch (its tracks are not
        // yet cached when the one-shot Enter request is consumed): the
        // closure only reaches the owner, so it reports back whether the
        // request needs to stay armed (bound to this album) for the tracks
        // re-push to retry, rather than re-arming `self` from inside.
        let rearm = self
            .update_music_owner(|owner| {
                owner.set_content(context);
                if let Some((c, s)) = reanchor {
                    owner.re_anchor(c, s);
                }
                owner.set_focused(focused);
                owner.set_inline_track_focus_enabled(wide);
                match request {
                    Some(MusicTrackFocusRequest::Clear) => {
                        owner.clear_track_focus();
                        None
                    }
                    Some(MusicTrackFocusRequest::Enter { album_id })
                        if wide && owner.selected_item().is_some_and(|a| a.id == album_id) =>
                    {
                        owner.enter_track_focus();
                        (wide && !owner.track_focused())
                            .then_some(MusicTrackFocusRequest::Enter { album_id })
                    }
                    _ => None,
                }
            })
            .flatten();
        if let Some(rearm) = rearm {
            self.music_track_focus_request = Some(rearm);
        }
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
        self.music_owner_key()
            .and_then(|key| self.library_owner_mut::<MusicContent>(&key))
            .expect("music owner")
    }
}

#[cfg(test)]
mod tests {
    use crate::app::render::{make_movie_app, make_music_group_app};
    use crate::app::shell::Model;
    use mbv_core::api::{EmbyClient, EmbyCredentialExchange};
    use mbv_core::service_runtime::EmbyRuntime;
    use std::sync::{Arc, Mutex};

    /// A configured-but-unroutable `EmbyClient` (design mirrors the retired
    /// `push_music_workspace_fetches_selected_album_tracks` test): the
    /// `album_tracks_loading` insert `fetch_album_tracks` performs happens
    /// synchronously before the network attempt, so it is observable
    /// immediately after the sync call without waiting on (or requiring)
    /// the doomed connection to resolve.
    fn ready_emby_runtime() -> EmbyRuntime {
        let mut client = EmbyClient::new(crate::config::Config::default());
        client.apply_credential_exchange(&EmbyCredentialExchange {
            server_url: "http://127.0.0.1:1".into(),
            user_id: "user-id".into(),
            token: "token".into(),
        });
        EmbyRuntime::ready(Arc::new(Mutex::new(client)))
    }

    /// Regression: `push_music_workspace_content` must not run its
    /// Music-only side effects (`wide_music_render_ctx`, `fetch_album_tracks`)
    /// for a selection on a non-Music Emby library. Before the fix the guard
    /// only checked `tab.emby_library_index()`, so every tick spawned a real
    /// track fetch for whatever item was selected on ANY Emby library tab.
    #[test]
    fn push_music_workspace_content_is_a_no_op_outside_music_album_folder_view() {
        let mut app = make_movie_app();
        app.emby_runtime = ready_emby_runtime();
        let mut model = Model::new(app);

        model.sync_mounted_surfaces();

        assert!(
            model.music_owner().is_none(),
            "a non-Music library must never install a Music owner"
        );
        assert!(
            !model.app.album_tracks_loading.contains("movie-focused"),
            "the selected movie's tracks must never be fetched"
        );
        assert!(
            !model.app.album_tracks_cache.contains_key("movie-focused"),
            "the selected movie's tracks must never be cached as an album"
        );
    }

    /// Positive control for the guard above: a genuine Music album-folder
    /// selection still fetches the selected album's tracks.
    #[test]
    fn push_music_workspace_content_fetches_the_selected_albums_tracks() {
        let mut app = make_music_group_app();
        app.emby_runtime = ready_emby_runtime();
        let mut model = Model::new(app);

        model.sync_mounted_surfaces();

        assert!(
            model.music_owner().is_some(),
            "the Music album-folder view must install its owner"
        );
        assert!(
            model.app.album_tracks_loading.contains("album-1"),
            "the selected album's tracks must be fetched"
        );
    }
}

#[cfg(test)]
#[path = "shell_music_workspace_owner_tests.rs"]
mod owner_tests;
