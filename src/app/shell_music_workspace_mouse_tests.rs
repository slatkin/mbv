use super::tests::{make_item, make_music_group_app};
use super::*;
use crate::app::components::msg::ShellRequest;
use crate::app::components::Msg;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

#[test]
fn narrow_short_wide_grouped_music_moves_one_album_per_down_and_page() {
    let mut model = Model::new(make_music_group_app());
    model.app.layout.main.left_area = ratatui::layout::Rect::new(0, 0, 100, 6);
    model.app.layout.main.wide_music_area = ratatui::layout::Rect::default();
    for index in 2..8 {
        let name = format!("Album {index}");
        let mut album = make_item(&name, "MusicAlbum");
        album.id = format!("album-{index}");
        model.app.libs[0].nav_stack[1].items.push(album);
    }
    model.sync_music_workspace();
    let id = model.music_workspace_id.clone().unwrap();
    let mut terminal = Terminal::new(TestBackend::new(100, 6)).unwrap();
    terminal
        .draw(|frame| model.render_music_workspace_component(frame))
        .unwrap();
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
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { target: 1, .. }))
    ));
    let message = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Keyboard(KeyEvent {
            code: Key::PageDown,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor { target: 6, .. }))
    ));
}

#[test]
fn music_mouse_track_click_stays_component_local() {
    let mut model = Model::new(make_music_group_app());
    let mut track = make_item("Track One", "Audio");
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
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| model.render_music_workspace_component(frame))
        .unwrap();
    let (column, row) = {
        let component = model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<MusicWorkspaceComponent>()
            .unwrap();
        assert_eq!(component.track_cursor(), None);
        let (rect, _) = component
            .layout()
            .wide_music_track_hitmap
            .first()
            .copied()
            .expect("painted track hitmap");
        (rect.x + 1, rect.y)
    };
    let message = model
        .application
        .get_component_mut(&id)
        .unwrap()
        .on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }));
    assert_eq!(message, None);
    let component = model
        .application
        .get_component(&id)
        .unwrap()
        .as_any()
        .downcast_ref::<MusicWorkspaceComponent>()
        .unwrap();
    assert_eq!(component.track_cursor(), Some(0));
}
