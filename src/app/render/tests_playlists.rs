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

/// The playlists list's row treatments: the selected row paints the canonical
/// Iris bar with Ink text, odd unselected rows take the Slate zebra stripe,
/// and the queue-loaded playlist reads orange on either fill.
#[test]
fn playlists_rows_paint_selected_bar_stripe_and_loaded_marker() {
    let mut app = make_app_stub();
    let mut alpha = make_item("Alpha", "Playlist");
    alpha.id = "alpha".into();
    let mut beta = make_item("Beta", "Playlist");
    beta.id = "loaded-id".into();
    let mut gamma = make_item("Gamma", "Playlist");
    gamma.id = "gamma".into();
    app.playlists = vec![alpha, beta, gamma];
    app.playlists_cursor = 0;
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("loaded-id".into()),
        name: "Beta".into(),
    };
    let (width, height) = (50, 12);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|f| app.render_playlists_panel(f, Some(Rect::new(0, 0, width, height))))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let row_text = |y: u16| -> String {
        (0..width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect()
    };
    let named_row = |name: &str| -> u16 {
        (0..height)
            .find(|&y| row_text(y).contains(name))
            .unwrap_or_else(|| panic!("{name} row painted"))
    };
    let title_cell = |y: u16, name: &str| -> (u16, ratatui::style::Color, ratatui::style::Color) {
        let line = row_text(y);
        let x = line.find(name).unwrap() as u16;
        (x, buffer[(x, y)].fg, buffer[(x, y)].bg)
    };
    // Selected row: Iris bar, Ink title, no gutter mark.
    let selected_y = named_row("Alpha");
    let (sx, sfg, sbg) = title_cell(selected_y, "Alpha");
    assert_eq!(sbg, palette::SELECTED_ROW_BG);
    assert_eq!(sfg, palette::SELECTED_ROW_FG);
    assert_eq!(buffer[(sx - 1, selected_y)].symbol(), " ");
    assert_eq!(buffer[(sx - 1, selected_y)].bg, palette::SELECTED_ROW_BG);
    // Loaded row on the odd stripe: orange text on the Slate fill.
    let loaded_y = named_row("Beta");
    let (_, lfg, lbg) = title_cell(loaded_y, "Beta");
    assert_eq!(lbg, palette::PLAYLIST_STRIPE_BG);
    assert_eq!(lfg, palette::PLAYLIST_LOADED_FG);
    // Plain even row: panel fill, primary text.
    let plain_y = named_row("Gamma");
    let (_, pfg, pbg) = title_cell(plain_y, "Gamma");
    assert_eq!(
        pbg,
        palette::surface_colors(palette::Surface::SidebarBody, false).fill
    );
    assert_eq!(pfg, palette::TEXT_PRIMARY);
}
