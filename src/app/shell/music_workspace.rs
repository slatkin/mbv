//! Shell projection for the grouped Music library owner.
//! Music is retained by the LibraryPanel under `LibraryKey::Service(Music)`;
//! it is not a mounted destination component.

use super::components::library_panel::LibraryKey;
use super::components::msg::MusicArtistTarget;
use super::components::music_content::MusicContent;
use super::components::LibraryKind;
use super::BrowseLevel;
use super::TabSelection;
use super::{Model, MusicTrackFocusRequest, MusicTrackSelection};
use mbv_core::config::ServiceKind;

impl Model {
    pub(in crate::app) fn music_owner_key(&self) -> Option<LibraryKey> {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return None;
        };
        let library = self.app.libs.get(index)?;
        (library.library.collection_type == "music" && self.app.is_music_group_view(index)).then(
            || LibraryKey::Service {
                service: ServiceKind::Emby,
                library_id: library.library.id.clone(),
                kind: LibraryKind::Music,
            },
        )
    }

    pub fn music_owner(&self) -> Option<&MusicContent> {
        let key = self.music_owner_key()?;
        self.library_owner(&key)
    }

    pub(in crate::app) fn update_music_owner<R>(
        &mut self,
        f: impl FnOnce(&mut MusicContent) -> R,
    ) -> Option<R> {
        let key = self.music_owner_key()?;
        self.update_library_owner(&key, || Box::new(MusicContent::new()), f)
    }

    /// Starts (or reuses) both halves of the focused artist's detail fetch
    /// (design D7, tasks 6.1/6.2): the typed track request arms the verified
    /// `ArtistIds` query — or, for a fallback root, the per-album fetches —
    /// and the typed artwork request goes through the existing image/cache
    /// boundary by the artist's stable ID. Both dedupe on their caches, so
    /// repeat pushes with the same identity are free. Used by the projection
    /// push; the component's typed request arms dispatch the two concerns
    /// through their own shell arms.
    pub(in crate::app) fn request_music_artist_detail(&mut self, target: &MusicArtistTarget) {
        self.request_music_artist_tracks(target);
        self.request_music_artist_artwork(target);
    }

    /// The typed track request's handler (task 6.1): the component-resolved
    /// identity arms the `ArtistIds` Audio query — or, for a fallback root
    /// (`artist_id == None`), the explicit per-album aggregation fetches —
    /// with no invented provider ID.
    pub(in crate::app) fn request_music_artist_tracks(&mut self, target: &MusicArtistTarget) {
        let Some(destination) = self.music_owner_key() else {
            return;
        };
        self.app.request_artist_tracks(destination, target);
    }

    /// The typed artwork request's handler (task 6.2): the stable artist ID
    /// walks the existing image/cache boundary; a fallback artist is the
    /// explicit no-artwork arm inside `request_artist_artwork`.
    pub(in crate::app) fn request_music_artist_artwork(&mut self, target: &MusicArtistTarget) {
        let Some(destination) = self.music_owner_key() else {
            return;
        };
        self.app.request_artist_artwork(&destination, target);
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
        // D7: validate the target library and the prepared stack BEFORE any
        // commit. A rejected apply reports through the existing library-error
        // path (which also drops the deferred tab switch) and leaves the
        // active tab, nav stack, saved Library position, and retained-owner
        // selection untouched -- never a silent no-op or a partial landing.
        if let Err(error) = self
            .app
            .validate_grouped_music_landing(&library_id, &nav_stack)
        {
            self.app.pending_track_selection = None;
            self.app.handle_lib_event(super::LibEvent::Error(error));
            return;
        }
        let library_id_lookup = library_id.clone();
        self.app
            .handle_lib_event(super::LibEvent::RecursiveAlbumActivated {
                library_id,
                nav_stack,
            });
        // The activated album: the resting cursor of the replaced nav stack.
        let activated_album_id = self
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
            });
        // Bind the enter request to the activated album so it can retry once
        // the album's tracks arrive without ever firing on an album the user
        // moved to meanwhile.
        self.music_track_focus_request = activated_album_id
            .clone()
            .map(|album_id| MusicTrackFocusRequest::Enter { album_id });
        // Deep selection (task 6.2, design D6): a navigated track rides the
        // album activation. Adopt the App's pending selection only when this
        // activation is the navigation's library, and bind it to the
        // activated album so the workspace push selects the track once its
        // rows arrive.
        self.pending_music_track_selection = self
            .app
            .pending_track_selection
            .take()
            .filter(|(lib_idx, _)| {
                self.app
                    .libs
                    .get(*lib_idx)
                    .is_some_and(|lib| lib.library.id == library_id_lookup)
            })
            .and_then(|(_, track_id)| {
                activated_album_id.map(|album_id| MusicTrackSelection { album_id, track_id })
            });
        // Nav stack was replaced wholesale; its resting cursor now points at
        // the activated album. Re-anchor the component explicitly, regardless
        // of prior local moves.
        self.music_workspace_reanchor = true;
    }

    pub(in crate::app) fn push_music_workspace_content(&mut self) {
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
        let (resting, cursor) = self.music_workspace_cursor(&key, index);
        let context = self.project_music_workspace_context(&key, index, cursor);
        self.fetch_music_workspace_album_tracks(&context);
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
        if let Some(rearm) =
            self.push_music_workspace_owner(context, reanchor, wide, focused, request)
        {
            self.music_track_focus_request = Some(rearm);
        }
        self.apply_pending_music_track_selection();
    }

    fn music_workspace_cursor(
        &mut self,
        key: &LibraryKey,
        index: usize,
    ) -> (Option<(usize, usize)>, Option<usize>) {
        // A fresh owner adopts the shell's resting cursor once; an existing
        // owner retains its divergent local cursor when the projection repoints.
        if !self.library_panel_has_owner(key) {
            self.music_workspace_reanchor = true;
        }
        let resting = self.app.libs[index]
            .nav_stack
            .last()
            .map(|level| (level.resting().cursor(), level.resting().scroll()));
        let selected_target = (!self.music_workspace_reanchor)
            .then(|| {
                self.music_owner()
                    .and_then(MusicContent::selected_item)
                    .map(|item| item.id)
            })
            .flatten();
        let cursor = self.app.libs[index]
            .nav_stack
            .last()
            .and_then(|level| {
                selected_target
                    .as_deref()
                    .and_then(|target| level.items.iter().position(|item| item.id == target))
            })
            .or_else(|| resting.map(|position| position.0));
        (resting, cursor)
    }

    fn project_music_workspace_context(
        &mut self,
        key: &LibraryKey,
        index: usize,
        cursor: Option<usize>,
    ) -> crate::app::render::MusicWideRenderCtx {
        let base_context = self.app.wide_music_render_ctx(index, cursor);
        // The artist detail projection (design D7, tasks 6.1–6.3): read the
        // owner's component-resolved artist target, re-bind it to this push's
        // settled source revision, request the missing tracks/artwork (both
        // dedupe on their caches), and project the cached summary and grouped
        // tracks. The shell never re-resolves a tree cursor.
        let artist_target = self
            .music_owner()
            .and_then(MusicContent::artist_detail_target)
            .map(|mut target| {
                target.revision = base_context.catalog_revision;
                target
            });
        if let Some(target) = artist_target.clone() {
            self.request_music_artist_detail(&target);
        }
        match artist_target {
            Some(target) => self
                .app
                .project_music_artist_detail(key, base_context, &target),
            None => base_context,
        }
    }

    fn fetch_music_workspace_album_tracks(
        &mut self,
        context: &crate::app::render::MusicWideRenderCtx,
    ) {
        // Album fetch follows the tree owner's resolved album; an artist root
        // has no album and never starts an album-track fetch.
        let owner_selection_is_artist = self
            .music_owner()
            .is_some_and(MusicContent::selected_is_artist);
        if !owner_selection_is_artist {
            if let Some(album) = context.selected_album.as_ref() {
                if !self.app.album_tracks_cache.contains_key(&album.id)
                    && !self.app.album_tracks_loading.contains(&album.id)
                {
                    self.app.fetch_album_tracks(album.id.clone());
                }
            }
        }
    }

    fn push_music_workspace_owner(
        &mut self,
        context: crate::app::render::MusicWideRenderCtx,
        reanchor: Option<(usize, usize)>,
        wide: bool,
        focused: bool,
        request: Option<MusicTrackFocusRequest>,
    ) -> Option<MusicTrackFocusRequest> {
        self.update_music_owner(|owner| {
            owner.set_content(context);
            if let Some((cursor, scroll)) = reanchor {
                owner.re_anchor(cursor, scroll);
            }
            owner.set_focused(focused);
            owner.set_inline_track_focus_enabled(wide);
            match request {
                Some(MusicTrackFocusRequest::Clear) => {
                    owner.clear_track_focus();
                    None
                }
                Some(MusicTrackFocusRequest::Enter { album_id })
                    if wide
                        && owner
                            .selected_item()
                            .is_some_and(|album| album.id == album_id) =>
                {
                    owner.enter_track_focus();
                    (wide && !owner.track_focused())
                        .then_some(MusicTrackFocusRequest::Enter { album_id })
                }
                _ => None,
            }
        })
        .flatten()
    }

    fn apply_pending_music_track_selection(&mut self) {
        // Deep selection (task 6.2, design D6): select the navigated track
        // once its album's track rows arrive. A superseded album or an
        // absent track drops the pending silently (no error -- the
        // navigation target was reached).
        if let Some(selection) = self.pending_music_track_selection.clone() {
            let resolved = self
                .update_music_owner(|owner| {
                    if owner
                        .selected_item()
                        .is_none_or(|album| album.id != selection.album_id)
                    {
                        // Superseded: the owner moved to another album.
                        return true;
                    }
                    if owner.track_list.rows().is_empty() {
                        // Rows not here yet; stay armed for the re-push.
                        return false;
                    }
                    owner.track_list.select_target(&selection.track_id);
                    true
                })
                .unwrap_or(false);
            if resolved {
                self.pending_music_track_selection = None;
            }
        }
    }

    #[cfg(test)]
    pub(in crate::app) fn test_music_owner(&self) -> &MusicContent {
        self.music_owner().expect("music owner")
    }
    #[cfg(test)]
    pub(in crate::app) fn test_music_owner_mut(&mut self) -> &mut MusicContent {
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
    /// Positive control for the guard above: a genuine Music album-folder
    /// selection still fetches the selected album's tracks.
    /// A settled one-album catalog whose only album carries a Service
    /// (`ArtistItems`) identity, with images enabled and a configured
    /// (unroutable) client: the loading reservations the artist-detail
    /// requests make are observable synchronously.
    fn settled_service_artist_app() -> crate::app::App {
        let mut app = make_music_group_app();
        app.image_protocol_enabled = true;
        app.emby_runtime = ready_emby_runtime();
        {
            let level = app.libs[0].nav_stack.last_mut().unwrap();
            for item in &mut level.items {
                item.artist_items = vec![mbv_core::api::EmbyArtistRef {
                    name: "Alpha".into(),
                    id: "artist-alpha".into(),
                }];
            }
            let mut catalog = crate::app::state::music_grouping::build_grouped_album_catalog(
                &level.items,
                &std::collections::HashMap::default(),
            );
            catalog.revision = 7;
            catalog.parent_id = level.parent_id.clone();
            level.music_grouping = Some(crate::app::state::music_grouping::MusicGroupingState {
                revision: 7,
                candidate: None,
                settled: Some(catalog),
            });
        };
        app
    }

    fn music_destination() -> crate::app::components::library_panel::LibraryKey {
        crate::app::components::library_panel::LibraryKey::Service {
            service: mbv_core::config::ServiceKind::Emby,
            library_id: "lib-music".into(),
            kind: crate::app::components::LibraryKind::Music,
        }
    }

    fn artist_focused_model() -> (Model, mbv_core::service_runtime::SetupGeneration) {
        let mut model = Model::new(settled_service_artist_app());
        model.app.panel_focus = crate::app::PanelFocus::Library;
        model.sync_mounted_surfaces();
        model
            .test_music_owner_mut()
            .browser
            .apply(crate::app::components::list::tree_browser::TreeOperation::First);
        assert!(model.test_music_owner().selected_is_artist());
        model.push_music_workspace_content();
        let generation = model.app.emby_runtime.generation();
        (model, generation)
    }

    /// Tasks 6.1/6.2 (design D7): an artist-root focus dispatches the typed
    /// track request with the full source identity and the typed artwork
    /// request through the shared image boundary by stable ID; a cached
    /// result never re-arms either fetch.
    #[test]
    fn artist_focus_dispatches_the_typed_requests_and_reuses_the_cache() {
        let destination = music_destination();
        let (mut model, generation) = artist_focused_model();
        let key = crate::app::state::music_artist_detail::ArtistDetailKey {
            destination: destination.clone(),
            generation: generation.value(),
            artist_id: "artist-alpha".into(),
            revision: 7,
        };
        assert!(
            model.app.artist_detail_loading.contains(&key),
            "the push dispatches the typed track request with the full identity"
        );
        assert!(
            model.app.card_image_loading.contains(
                &crate::app::state::music_artist_detail::artist_artwork_cache_key(
                    &destination,
                    generation.value(),
                    "artist-alpha",
                )
            ),
            "the typed artwork request walks the shared image boundary by stable ID"
        );

        // The typed request arm dispatches through the shell with the
        // component-resolved identity; repeat dispatches reuse the caches.
        let target = model
            .test_music_owner()
            .artist_detail_target()
            .expect("artist target");
        let (mut music_resize, mut tv_resize) = (false, false);
        model.handle_terminal_message(
            crate::app::components::Msg::Shell(Box::new(
                crate::app::components::ShellRequest::MusicArtistTracks { target },
            )),
            &mut music_resize,
            &mut tv_resize,
        );
        assert!(model.app.artist_detail_loading.contains(&key));
        model.app.artist_detail_loading.remove(&key);
        model.app.artist_detail_cache.insert(
            key,
            crate::app::state::music_artist_detail::ArtistDetailCacheEntry::default(),
        );
        let target = model
            .test_music_owner()
            .artist_detail_target()
            .expect("artist target");
        model.handle_terminal_message(
            crate::app::components::Msg::Shell(Box::new(
                crate::app::components::ShellRequest::MusicArtistTracks { target },
            )),
            &mut music_resize,
            &mut tv_resize,
        );
        assert!(
            model.app.artist_detail_loading.is_empty(),
            "a cached artist result never re-arms the fetch"
        );
    }

    /// Task 6.1/6.3 (design D7): a fallback root arms the existing per-album
    /// fetches and explicitly starts no artist-ID query and no artist-ID
    /// artwork request.
    /// Task 6.1/6.3 (design D7): only a completion matching the pushed
    /// destination, generation, revision, and focused artist reaches the
    /// visible Workspace; a stale revision paints nothing, and the matching
    /// result projects its grouped rows on the next push.
    #[test]
    fn only_a_completion_matching_the_pushed_identity_reaches_the_workspace() {
        use crate::app::components::media_list::MediaListRow;

        let destination = music_destination();
        let (mut model, generation) = artist_focused_model();

        model
            .app
            .handle_lib_event(crate::app::LibEvent::ArtistTracksFetched {
                destination: destination.clone(),
                generation,
                artist_id: "artist-alpha".into(),
                revision: 6,
                result: Ok(vec![crate::app::tests::make_item("Stale", "Audio")]),
            });
        model.push_music_workspace_content();
        assert!(
            model.test_music_owner().track_list.rows().is_empty(),
            "a replaced snapshot's completion paints nothing"
        );

        let mut track = crate::app::tests::make_item("Song", "Audio");
        track.id = "track-1".into();
        track.album_id = "album-1".into();
        model
            .app
            .handle_lib_event(crate::app::LibEvent::ArtistTracksFetched {
                destination: destination.clone(),
                generation,
                artist_id: "artist-alpha".into(),
                revision: 7,
                result: Ok(vec![track]),
            });
        model.push_music_workspace_content();
        let rows = model.test_music_owner().track_list.rows();
        assert!(
            matches!(rows.first(), Some(MediaListRow::Heading { .. })),
            "the matching completion projects its canonical heading row"
        );
        assert_eq!(
            rows.iter()
                .filter_map(MediaListRow::selectable_target)
                .count(),
            1,
        );
    }
}

#[cfg(test)]
mod owner_tests;
