use super::music_workspace::MusicWorkspaceComponent;
use crate::app::components::msg::{AlbumCursorKind, ShellRequest};
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

fn grouped_context(
    cursor: usize,
    order: Vec<usize>,
    focused: bool,
    track_cursor: Option<usize>,
) -> MusicWideRenderCtx {
    let albums: Vec<_> = (0..4)
        .map(|index| make_item(&format!("Album {index}"), "MusicAlbum"))
        .collect();
    MusicWideRenderCtx::new(
        LibraryListRenderCtx::from_items(albums.clone(), cursor, 0),
        Some(albums[cursor].clone()),
        "Artist".into(),
        vec![make_item("Artist", "MusicArtist")],
        0,
        (0..4)
            .map(|index| ("Artist".into(), "2024".into(), format!("Album {index}")))
            .collect(),
        order,
        focused,
        true,
        None,
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
        Some(Msg::Shell(ShellRequest::MusicAlbumCursor {
            target: 3,
            kind: AlbumCursorKind::Move,
        }))
    ));
}

#[test]
fn music_workspace_enter_ignored_when_inline_track_focus_disabled() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context(None));
    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.track_cursor(), None);
    assert!(matches!(
        message,
        Some(Msg::Legacy(LegacyTerminalEvent::Key(_)))
    ));
}

#[test]
fn music_workspace_enter_sets_track_cursor_when_inline_track_focus_enabled() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(context(None));
    component.set_inline_track_focus_enabled(true);
    component.on(&Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(component.track_cursor(), Some(0));
    component.set_inline_track_focus_enabled(false);
    assert_eq!(component.track_cursor(), None);
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

#[test]
fn music_workspace_horizontal_move_is_ignored_at_one_column() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(grouped_context(1, vec![0, 1, 2, 3], true, None));
    component.set_album_columns(1);

    for key in [Key::Char('h'), Key::Char('l')] {
        let message = component.on(&Event::Keyboard(KeyEvent {
            code: key,
            modifiers: KeyModifiers::NONE,
        }));
        assert!(matches!(
            message,
            Some(Msg::Legacy(LegacyTerminalEvent::Key(_)))
        ));
        assert_eq!(component.album_cursor(), 1);
    }
}

#[test]
fn music_workspace_page_moves_saturate_at_both_ends() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(grouped_context(0, vec![0, 1, 2, 3], true, None));
    component.set_album_columns(2);
    component.set_page_rows(2);

    for key in [Key::PageUp, Key::PageDown, Key::PageDown, Key::PageUp] {
        component.on(&Event::Keyboard(KeyEvent {
            code: key,
            modifiers: KeyModifiers::NONE,
        }));
    }
    assert_eq!(component.album_cursor(), 0);
}

#[test]
fn music_workspace_does_not_emit_album_intent_while_tracks_are_focused() {
    let mut component = MusicWorkspaceComponent::new();
    component.set_content(grouped_context(1, vec![0, 1, 2, 3], true, Some(0)));
    component.set_album_columns(2);

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        message,
        Some(Msg::Legacy(LegacyTerminalEvent::Key(_)))
    ));
    assert_eq!(component.album_cursor(), 1);
}
