use super::music_workspace::MusicWorkspaceComponent;
use crate::app::components::{LegacyTerminalEvent, Msg};
use crate::app::render::{LibraryListRenderCtx, MusicWideRenderCtx};
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

fn context(track_cursor: Option<usize>) -> MusicWideRenderCtx {
    let album = make_item("First Album", "MusicAlbum");
    let mut track = make_item("Track One", "Audio");
    track.index_number = 1;
    let mut second_track = make_item("Track Two", "Audio");
    second_track.index_number = 2;
    MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![album.clone()], 0, 0),
        Some(album),
        "Artist".into(),
        vec![make_item("Artist", "MusicArtist")],
        0,
        vec![("Artist".into(), "2024".into(), "First Album".into())],
        vec![0],
        true,
        true,
        Some(vec![track, second_track]),
        false,
        track_cursor,
    )
}

#[test]
fn music_workspace_keeps_track_cursor_local_between_syncs() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context(Some(0)));
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    component.set_content(context(Some(0)));
    assert_eq!(component.track_cursor(), Some(1));
}

#[test]
fn music_workspace_vertical_move_follows_album_display_order() {
    let albums = vec![
        make_item("Album 0", "MusicAlbum"),
        make_item("Album 1", "MusicAlbum"),
        make_item("Album 2", "MusicAlbum"),
        make_item("Album 3", "MusicAlbum"),
    ];
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(albums.clone(), 2, 0),
        Some(albums[2].clone()),
        "Artist".into(),
        vec![make_item("Artist", "MusicArtist")],
        0,
        vec![
            ("Artist".into(), "2024".into(), "Album 0".into()),
            ("Artist".into(), "2023".into(), "Album 1".into()),
            ("Artist".into(), "2022".into(), "Album 2".into()),
            ("Artist".into(), "2021".into(), "Album 3".into()),
        ],
        vec![2, 0, 3, 1],
        true,
        true,
        None,
        false,
        None,
    ));
    component.set_album_columns(2);
    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.album_cursor(), 3);
    assert!(matches!(
        message,
        Some(Msg::Legacy(LegacyTerminalEvent::Key(_)))
    ));
}

#[test]
fn music_workspace_renders_without_app() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context(None));
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert!(terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .any(|cell| cell.symbol() == "F"));
}
