use super::playlists::{PlaylistsComponent, PlaylistsContent};
use crate::app::components::msg::{Msg, ShellRequest};
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

#[test]
fn playlists_component_hit_test_uses_wrapped_row_geometry() {
    let mut component = PlaylistsComponent::new();
    component.set_content(PlaylistsContent {
        playlists: vec![],
        cursor: 0,
        scroll: 0,
        loading: false,
        open: Some(make_item("Playlist", "Playlist")),
        open_items: vec![
            make_item("A deliberately long playlist item that wraps", "Video"),
            make_item("Second item", "Video"),
        ],
        open_cursor: 1,
        open_scroll: 0,
        open_loading: false,
        loaded_id: None,
    });
    component.set_panel_area(Some(Rect::new(0, 0, 24, 8)));
    let mut terminal = Terminal::new(TestBackend::new(24, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();

    let message = component.on(&Event::Mouse(MouseEvent {
        column: 2,
        row: 3,
        kind: MouseEventKind::Down(MouseButton::Left),
        modifiers: KeyModifiers::NONE,
    }));
    assert!(message.is_none());
    assert_eq!(component.open_cursor(), 0);
}

#[test]
fn playlists_component_double_click_activates_an_open_playlist_item() {
    let mut component = PlaylistsComponent::new();
    component.set_content(PlaylistsContent {
        playlists: vec![],
        cursor: 0,
        scroll: 0,
        loading: false,
        open: Some(make_item("Playlist", "Playlist")),
        open_items: vec![
            make_item("First item", "Video"),
            make_item("Second item", "Video"),
        ],
        open_cursor: 1,
        open_scroll: 0,
        open_loading: false,
        loaded_id: None,
    });
    component.set_panel_area(Some(Rect::new(0, 0, 24, 8)));
    let mut terminal = Terminal::new(TestBackend::new(24, 8)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();

    let down = MouseEvent {
        column: 2,
        row: 3,
        kind: MouseEventKind::Down(MouseButton::Left),
        modifiers: KeyModifiers::NONE,
    };
    // First click selects the row (no message)…
    assert!(component.on(&Event::Mouse(down)).is_none());
    assert_eq!(component.open_cursor(), 0);
    // …the second click — a double click — activates it (Enter equivalent).
    assert_eq!(
        component.on(&Event::Mouse(down)),
        Some(Msg::Shell(ShellRequest::PlaylistsActivate {
            open: true,
            index: 0
        }))
    );
}

#[test]
fn playlists_component_right_click_on_an_open_playlist_goes_back() {
    let mut component = PlaylistsComponent::new();
    component.set_content(PlaylistsContent {
        playlists: vec![],
        cursor: 0,
        scroll: 0,
        loading: false,
        open: Some(make_item("Playlist", "Playlist")),
        open_items: vec![make_item("First item", "Video")],
        open_cursor: 0,
        open_scroll: 0,
        open_loading: false,
        loaded_id: None,
    });
    component.set_panel_area(Some(Rect::new(0, 0, 24, 8)));

    assert_eq!(
        component.on(&Event::Mouse(MouseEvent {
            column: 2,
            row: 3,
            kind: MouseEventKind::Down(MouseButton::Right),
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Shell(ShellRequest::PlaylistsBack))
    );
}
