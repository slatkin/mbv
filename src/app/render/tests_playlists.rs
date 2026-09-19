use super::test_helpers::buffer_to_string;
use crate::app::palette;
use crate::app::tests::{make_app_stub, make_item};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_playlists(width: u16, height: u16, selected: bool, open: bool) -> String {
    let mut app = make_app_stub();
    let playlist = make_item("Road Trip", "Playlist");
    app.playlists = vec![playlist.clone(), make_item("Favorites", "Playlist")];
    app.playlists_cursor = usize::from(selected);
    if open {
        app.playlists_open = Some(playlist);
        app.playlists_open_items = vec![make_item("Birthday Clip", "Video")];
    }

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            app.render_playlists_panel(f, Some(Rect::new(0, 0, width, height)));
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn playlists_none_fallback_paints_the_fullscreen_shell() {
    let mut app = make_app_stub();
    app.playlists = vec![make_item("Road Trip", "Playlist")];
    let width = 40;
    let height = 12;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|f| app.render_playlists_panel(f, None))
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(0, 0)].bg,
        palette::surface_colors(palette::Surface::SidebarBody, false).fill
    );
    assert_eq!(
        buffer[(2, 1)].bg,
        palette::surface_colors(palette::Surface::SidebarBand, false).fill
    );
    assert_eq!(buffer[(3, 1)].symbol(), "P");
    assert_eq!(buffer[(width - 1, 2)].symbol(), " ");
}

#[test]
fn playlists_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, selected, open) in [
        (50, 12, false, false),
        (50, 12, true, false),
        (18, 8, true, false),
        (30, 8, true, true),
    ] {
        let output = render_playlists(width, height, selected, open);
        assert!(output.contains("ROAD TRIP") || output.contains("PLAYLISTS"));
    }
}
