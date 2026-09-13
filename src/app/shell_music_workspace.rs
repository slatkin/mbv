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
        let search_active = self
            .music_owner()
            .is_some_and(|owner| owner.inline_search().is_active());
        if let Some(album) = context.selected_album.as_ref() {
            if !self.app.album_tracks_cache.contains_key(&album.id)
                && !self.app.album_tracks_loading.contains(&album.id)
            {
                self.app.fetch_album_tracks(album.id.clone());
            }
        }
        // Pre-warm the display-order neighbours' album art (the `{id}:P` keys
        // the hero projection consumes, same album art chain), so browsing
        // the grouped list shows art instantly instead of waiting on the
        // album art chain on every cursor move (the seam the pre-panel
        // painter used).
        if self.app.images_enabled() && !search_active {
            self.app.prewarm_grouped_music_album_images(
                &context.list.items,
                context.list.cursor(),
                &context.album_order,
            );
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
        self.music_owner_mut().expect("music owner")
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

    /// Restored from the pre-panel painter (49e3fa8c, lost in the 9.x
    /// migration): the grouped Music push pre-warms the display-order
    /// neighbours' `{id}:P` art when navigation is idle, so browsing shows
    /// art instantly instead of re-walking the two-request `AudioChild`
    /// chain on every cursor move. Only the ±window around the cursor
    /// warms; distant albums stay untouched.
    #[test]
    fn grouped_music_push_prewarms_neighbour_album_images() {
        let mut app = make_music_group_app();
        app.image_protocol_enabled = true;
        for number in 2..=7 {
            let mut album = crate::app::tests::make_item(&format!("Album {number}"), "MusicAlbum");
            album.id = format!("album-{number}");
            album.artist = "Alpha".into();
            app.libs[0].nav_stack[1].items.push(album);
        }
        app.libs[0].nav_stack[1].set_resting_cursor(2);
        let mut model = Model::new(app);

        model.sync_music_workspace();

        // The neighbours of the selected album-3 (cursor 2) warm: one behind
        // and three ahead in display order.
        assert!(
            model.app.card_image_loading.contains("album-2:P"),
            "the album behind the cursor must pre-warm"
        );
        assert!(
            model.app.card_image_loading.contains("album-4:P"),
            "the albums ahead of the cursor must pre-warm"
        );
        assert!(
            !model.app.card_image_loading.contains("album-1:P"),
            "albums outside the window must not fetch"
        );
        assert!(
            !model.app.card_image_loading.contains("album-7:P"),
            "albums outside the window must not fetch"
        );
    }

    /// The pre-warm is idle-gated: navigation in the last
    /// `NAV_IMAGE_FETCH_IDLE_DELAY` must not issue neighbour fetches.
    #[test]
    fn grouped_music_prewarm_waits_for_navigation_idle() {
        let mut app = make_music_group_app();
        app.image_protocol_enabled = true;
        let mut album = crate::app::tests::make_item("Album 2", "MusicAlbum");
        album.id = "album-2".into();
        album.artist = "Alpha".into();
        app.libs[0].nav_stack[1].items.push(album);
        app.last_nav_at = std::time::Instant::now();
        let mut model = Model::new(app);

        model.sync_music_workspace();

        assert!(
            !model.app.card_image_loading.contains("album-2:P"),
            "a fresh navigation must suppress the neighbour pre-warm"
        );
    }

    /// End-to-end state round-trip: a *completed* album-image fetch must
    /// reach the owner's projected hero state as `Ready` through the real
    /// sync path (drain → re-project). If this holds, the fetch/projection
    /// pipeline is intact and a runtime placeholder means the fetch itself
    /// resolved empty (the `AudioChild` chain's first-track probe).
    #[test]
    fn completed_album_image_reaches_owner_hero_state_as_ready() {
        let mut app = make_music_group_app();
        app.image_protocol_enabled = true;
        let mut model = Model::new(app);
        // Pre-seed the completed fetch before any sync: the projection's
        // unconditional `fetch_card_image` then dedupes against the existing
        // state instead of spawning a real (failing) fetch thread whose
        // resolved-empty completion would overwrite the seeded image.
        let img = image::DynamicImage::new_rgb8(400, 400);
        model
            .app
            .card_image_tx
            .send(("album-1:P".to_string(), Some(img)))
            .expect("completion");
        model.drain_card_image_completions();
        // One real frame so the root-frame placements exist (the hero
        // projection gates on the library panel's content area).
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).expect("terminal");
        model.sync_mounted_surfaces();
        terminal
            .draw(|frame| model.draw_frame(frame, false, false))
            .expect("draw");
        model.sync_mounted_surfaces();

        let data = model
            .music_owner_mut()
            .expect("music owner")
            .hero_data()
            .expect("hero data");
        assert!(
            matches!(
                data.facts.artwork.image,
                crate::app::components::library_panel::content::HeroImageState::Ready { .. }
            ),
            "a completed fetch must project Ready into the owner, got {:?}",
            data.facts.artwork.image
        );
    }
}

#[cfg(test)]
#[path = "shell_music_workspace_owner_tests.rs"]
mod owner_tests;
