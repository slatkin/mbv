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
    pub(super) fn focused_music_track(&self, lib_idx: usize) -> Option<(String, EmbyItem)> {
        let id = self.music_workspace_id.as_ref()?;
        let cursor = self
            .application
            .get_component(id)?
            .as_any()
            .downcast_ref::<MusicWorkspaceComponent>()?
            .track_cursor()?;
        let album = self.app.selected_album_item(lib_idx)?;
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
            if let Some(id) = self.music_workspace_id.take() {
                let _ = self.application.umount(&id);
            }
            if let Some(id) = next_id.clone() {
                self.application
                    .mount(id.clone(), Box::new(MusicWorkspaceComponent::new()), vec![])
                    .expect("mount Music workspace");
                self.application
                    .active(&id)
                    .expect("activate Music workspace");
                self.music_workspace_id = Some(id);
                self.push_music_workspace_content();
            }
        }

        if self.music_workspace_id.is_none() {
            // No Music workspace is mounted right now: a pending focus
            // request (recursive album activation / position restore that
            // landed on a non-mountable state) cannot be delivered, and must
            // not fire later on an unrelated album.
            self.music_track_focus_request = None;
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
        let selected_album = self.app.selected_album_item(index);
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
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(music) = comp.as_any_mut().downcast_mut::<MusicWorkspaceComponent>() {
                music.set_content(context);
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
        let area = self.app.layout.main.wide_music_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        if let Some(lib_idx) = self.app.tab.emby_library_index() {
            let context = self.app.wide_music_render_ctx(lib_idx);
            context.publish_geometry(area, &mut self.app.layout.main);
        }
        self.application.view(id, frame, area);
        let image_paint = self
            .application
            .get_component_mut(id)
            .and_then(|comp| comp.as_any_mut().downcast_mut::<MusicWorkspaceComponent>())
            .and_then(MusicWorkspaceComponent::take_image_paint);
        self.app.paint_music_image(frame, image_paint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::msg::{AlbumCursorKind, ShellRequest};
    use crate::app::components::Msg;
    use crate::app::render::make_music_group_app;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn shell_mounts_and_syncs_music_workspace() {
        let mut model = Model::new(make_music_group_app());
        model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
        model.sync_music_workspace();
        let id = model
            .music_workspace_id
            .clone()
            .expect("Music workspace mounted");
        let message = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        assert!(matches!(
            message,
            Some(Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))
        ));
    }

    #[test]
    fn push_music_workspace_fetches_selected_album_tracks() {
        let mut model = Model::new(make_music_group_app());
        model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);

        let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
        client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
            server_url: "http://127.0.0.1:1".into(),
            user_id: "user-id".into(),
            token: "token".into(),
        });
        model.app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(
            std::sync::Arc::new(std::sync::Mutex::new(client)),
        );
        model.sync_music_workspace();

        assert!(model.app.album_tracks_loading.contains("album-1"));
        let component = model
            .application
            .get_component(&model.music_workspace_id.clone().unwrap())
            .unwrap()
            .as_any()
            .downcast_ref::<MusicWorkspaceComponent>()
            .unwrap();
        assert!(
            component.album_tracks_loading(),
            "first mounted content push must project album track loading"
        );
    }

    #[test]
    fn grouped_music_cursor_no_fallthrough_when_left_sorted_indices_empty() {
        let mut model = Model::new(make_music_group_app());
        // Add sibling albums so the display order (sorted by name) differs
        // from raw insertion order: raw [0 "First Album", 1 "Zebra Album",
        // 2 "Mango Album"] sorts to display order [0, 2, 1].
        let mut zebra = crate::app::tests::make_item("Zebra Album", "MusicAlbum");
        zebra.artist = "Charlie".into();
        let mut mango = crate::app::tests::make_item("Mango Album", "MusicAlbum");
        mango.artist = "Bravo".into();
        model.app.libs[0].nav_stack[1].items.extend([zebra, mango]);
        // Force a single column so the display-order move is deterministic.
        model.app.layout.main.left_area.width = 40;

        // No library-list render has run, so the render-output order the
        // legacy fallback would have read is empty.
        assert!(model.app.layout.main.left_sorted_indices.is_empty());

        model.sync_music_workspace();
        let id = model.music_workspace_id.clone().expect("mounted");

        let order = model.app.wide_music_render_ctx(0).album_order.clone();
        assert_eq!(order, vec![0, 2, 1], "display order must differ from raw");

        let message = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        let target = match message {
            Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
                target,
                kind: AlbumCursorKind::Move,
            })) => target,
            other => panic!("Down must emit an album cursor intent, got {other:?}"),
        };
        // The target is the display-order successor of raw index 0 (== order[1]),
        // never the raw successor (1) the legacy empty-left_sorted_indices path used.
        assert_eq!(target, order[1]);
        assert_ne!(target, 1, "must not fall through to raw-index navigation");

        // The shell arm applies the target via the display-order cursor setter,
        // which must not fall through to raw-index navigation.
        assert!(model.app.move_music_group_display_cursor(0, target));
        assert_eq!(model.app.libs[0].nav_stack[1].cursor, order[1]);
    }

    #[test]
    fn shell_mounts_music_workspace_in_narrow_mode() {
        let mut model = Model::new(make_music_group_app());
        assert!(model.app.is_music_group_view(0));
        assert!(model.app.is_viewing_album_folders(0));
        assert!(!model.app.layout.main.is_wide_music_active());

        let wide_area = model.app.layout.main.wide_music_area;
        assert_eq!(wide_area.width, 0);
        assert_eq!(wide_area.height, 0);
        model.sync_music_workspace();
        let id = model
            .music_workspace_id
            .clone()
            .expect("narrow Music workspace mounted");
        assert!(model.application.mounted(&id));
        assert_eq!(model.app.layout.main.wide_music_area, wide_area);
    }

    #[test]
    fn music_resize_push_uses_current_frame_geometry() {
        let mut model = Model::new(make_music_group_app());
        let mut track = crate::app::tests::make_item("Track 1", "Audio");
        track.id = "track-1".into();
        model
            .app
            .album_tracks_cache
            .insert("album-1".into(), vec![track]);
        model.sync_music_workspace();

        let mut wide_terminal = Terminal::new(TestBackend::new(160, 30)).unwrap();
        wide_terminal.draw(|frame| model.app.render(frame)).unwrap();
        model.push_music_workspace_content();
        let id = model.music_workspace_id.clone().unwrap();
        {
            let wide = model
                .application
                .get_component_mut(&id)
                .unwrap()
                .as_any_mut()
                .downcast_mut::<MusicWorkspaceComponent>()
                .unwrap();
            wide.enter_track_focus();
            assert!(model.app.layout.main.is_wide_music_active());
            assert_eq!(wide.track_cursor(), Some(0));
        }
        let hitmap_before = model.app.layout.main.wide_music_track_hitmap.len();
        assert!(hitmap_before > 0);
        wide_terminal
            .draw(|frame| model.render_music_workspace_component(frame))
            .unwrap();
        assert_eq!(
            model.app.layout.main.wide_music_track_hitmap.len(),
            hitmap_before
        );

        let mut narrow_terminal = Terminal::new(TestBackend::new(60, 30)).unwrap();
        narrow_terminal
            .draw(|frame| model.app.render(frame))
            .unwrap();
        model.push_music_workspace_content();
        let narrow = model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<MusicWorkspaceComponent>()
            .unwrap();
        assert!(!model.app.layout.main.is_wide_music_active());
        assert_eq!(narrow.track_cursor(), None);
    }

    #[test]
    fn narrow_music_workspace_ignores_enter_for_inline_track_focus() {
        let mut model = Model::new(make_music_group_app());
        assert!(!model.app.layout.main.is_wide_music_active());
        model.sync_music_workspace();
        let id = model
            .music_workspace_id
            .clone()
            .expect("narrow Music workspace mounted");
        model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Enter,
                modifiers: KeyModifiers::NONE,
            }));
        let component = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<MusicWorkspaceComponent>()
            .unwrap();
        assert_eq!(component.track_cursor(), None);
    }

    #[test]
    fn wide_music_workspace_allows_enter_for_inline_track_focus() {
        let mut model = Model::new(make_music_group_app());
        let mut track = crate::app::tests::make_item("Track One", "Audio");
        track.id = "track-1".into();
        model
            .app
            .album_tracks_cache
            .insert("album-1".into(), vec![track]);
        model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
        assert!(model.app.layout.main.is_wide_music_active());
        model.sync_music_workspace();
        let id = model
            .music_workspace_id
            .clone()
            .expect("wide Music workspace mounted");
        model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Enter,
                modifiers: KeyModifiers::NONE,
            }));
        let component = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<MusicWorkspaceComponent>()
            .unwrap();
        assert_eq!(component.track_cursor(), Some(0));
    }

    #[test]
    fn recursive_album_activation_enters_track_focus_only_in_wide() {
        // Recursive album activation used to write
        // `Some(0)` on the deleted inline track-focus field; the shell now delivers a
        // one-shot enter request consumed at the next content push -- wide only, so
        // narrow stays explicitly unfocused.
        let mut model = Model::new(make_music_group_app());
        let mut track = crate::app::tests::make_item("Track One", "Audio");
        track.id = "track-1".into();
        model
            .app
            .album_tracks_cache
            .insert("album-1".into(), vec![track]);
        model.sync_music_workspace();
        assert!(!model.app.layout.main.is_wide_music_active());
        let id = model
            .music_workspace_id
            .clone()
            .expect("narrow Music workspace mounted");

        model.music_track_focus_request = Some(true);
        model.push_music_workspace_content();
        let component = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<MusicWorkspaceComponent>()
            .unwrap();
        assert_eq!(
            component.track_cursor(),
            None,
            "narrow keeps inline track focus explicitly off"
        );

        model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
        model.music_track_focus_request = Some(true);
        model.push_music_workspace_content();
        let component = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<MusicWorkspaceComponent>()
            .unwrap();
        assert_eq!(
            component.track_cursor(),
            Some(0),
            "wide recursive activation enters track focus"
        );
    }

    #[test]
    fn position_restore_request_clears_track_focus_at_next_sync() {
        let mut model = Model::new(make_music_group_app());
        let mut track = crate::app::tests::make_item("Track One", "Audio");
        track.id = "track-1".into();
        model
            .app
            .album_tracks_cache
            .insert("album-1".into(), vec![track]);
        model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);
        model.sync_music_workspace();
        let id = model
            .music_workspace_id
            .clone()
            .expect("wide Music workspace mounted");
        model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Enter,
                modifiers: KeyModifiers::NONE,
            }));

        // The deleted track-focus-clear rehome: a position-restore request
        // clears the component's inline track focus at the next content push.
        model.music_track_focus_request = Some(false);
        model.sync_music_workspace();
        assert_eq!(model.music_track_focus_request, Some(false));
        model.push_music_workspace_content();
        let component = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<MusicWorkspaceComponent>()
            .unwrap();
        assert_eq!(component.track_cursor(), None);
    }
}
