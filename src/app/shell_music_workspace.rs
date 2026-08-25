use super::components::{BrowserKey, BrowserKind, ComponentId, MusicWorkspaceComponent};
use super::render::MusicWideRenderCtx;
use super::shell::Model;
use super::TabSelection;
use mbv_core::config::ServiceKind;

impl Model {
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
            }
        }

        let Some(id) = self.music_workspace_id.as_ref() else {
            return;
        };
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return;
        };
        let context: MusicWideRenderCtx = self.app.wide_music_render_ctx(index);
        let columns = self.app.current_library_columns(index);
        let wide = self.app.layout.main.is_wide_music_active();
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(music) = comp.as_any_mut().downcast_mut::<MusicWorkspaceComponent>() {
                music.set_content(context);
                music.set_album_columns(columns);
                music.set_page_rows(self.app.layout.main.left_area.height as usize);
                music.set_inline_track_focus_enabled(wide);
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
    fn grouped_music_cursor_routing_matches_legacy_after_each_key() {
        let prepare = |app: &mut crate::app::App| {
            let mut albums = app.libs[0].nav_stack[1].items.clone();
            for (name, artist) in [
                ("Beta Album", "Beta"),
                ("Alpha Album", "Alpha"),
                ("Gamma Album", "Gamma"),
            ] {
                let mut album = crate::app::tests::make_item(name, "MusicAlbum");
                album.artist = artist.into();
                albums.push(album);
            }
            app.libs[0].nav_stack[1].items = albums;
            let backend = ratatui::backend::TestBackend::new(200, 30);
            let mut terminal = ratatui::Terminal::new(backend).unwrap();
            terminal.draw(|frame| app.render(frame)).unwrap();
            app.layout.main.left_area.width = 82;
            app.layout.main.left_area.height = 10;
            assert!(app.layout.main.left_sorted_indices.len() >= 4);
        };

        let mut legacy_app = make_music_group_app();
        let mut routed_model = Model::new(make_music_group_app());
        prepare(&mut legacy_app);
        prepare(&mut routed_model.app);
        routed_model.sync_music_workspace();
        assert!(
            routed_model.app.current_library_columns(0) > 1,
            "expected multi-column layout, got {}",
            routed_model.app.current_library_columns(0)
        );
        let id = routed_model.music_workspace_id.clone().unwrap();
        let keys = [
            (Key::Down, crossterm::event::KeyCode::Down),
            (Key::Down, crossterm::event::KeyCode::Down),
            (Key::Char('l'), crossterm::event::KeyCode::Char('l')),
            (Key::Char('h'), crossterm::event::KeyCode::Char('h')),
            (Key::End, crossterm::event::KeyCode::End),
            (Key::Home, crossterm::event::KeyCode::Home),
            (Key::PageDown, crossterm::event::KeyCode::PageDown),
            (Key::PageUp, crossterm::event::KeyCode::PageUp),
        ];
        let mut legacy_cursors = Vec::new();
        let mut routed_cursors = Vec::new();

        for (component_key, legacy_key) in keys {
            legacy_app.handle_key(crossterm::event::KeyEvent::new(
                legacy_key,
                crossterm::event::KeyModifiers::NONE,
            ));
            legacy_cursors.push(legacy_app.libs[0].nav_stack[1].cursor);

            let message =
                routed_model
                    .application
                    .get_component_mut(&id)
                    .unwrap()
                    .on(&Event::Keyboard(KeyEvent {
                        code: component_key,
                        modifiers: KeyModifiers::NONE,
                    }));
            let Some(Msg::Shell(ShellRequest::MusicAlbumCursor { target, kind })) = message else {
                panic!("grouped Music key must emit an album cursor intent: {component_key:?}");
            };
            match kind {
                AlbumCursorKind::Move => {
                    assert!(routed_model.app.move_music_group_display_cursor(0, target));
                }
                AlbumCursorKind::Jump => {
                    assert!(routed_model.app.jump_music_group_display_cursor(0, target));
                }
                AlbumCursorKind::Page => {
                    assert!(routed_model.app.page_grouped_album_cursor(0, target));
                }
            }
            routed_model.sync_music_workspace();
            routed_cursors.push(routed_model.app.libs[0].nav_stack[1].cursor);
            assert_eq!(legacy_cursors.last(), routed_cursors.last());
        }
        assert_eq!(legacy_cursors, routed_cursors);
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
}
