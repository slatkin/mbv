use super::test_helpers::buffer_to_string;
use crate::app::palette;
use crate::app::tests::{make_app_stub, make_session};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_sessions(width: u16, height: u16, selected: bool, loading: bool) -> String {
    let mut app = make_app_stub();
    app.sessions_loading = loading;
    if !loading {
        app.sessions = vec![make_session("Living Room", "Emby")];
    }
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let targets = crate::app::panel_targets::build_panel_targets(&app.sessions, &[]);
            let mut cursor = usize::from(selected);
            let mut scroll = 0;
            crate::app::render::render_sessions_overlay_content(
                f,
                Some(Rect::new(0, 0, width, height)),
                &targets,
                app.sessions_loading,
                &mut cursor,
                &mut scroll,
                None,
                None,
                false,
            );
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn sessions_none_fallback_paints_the_fullscreen_shell() {
    let mut app = make_app_stub();
    app.sessions = vec![make_session("Living Room", "Emby")];
    let width = 40;
    let height = 12;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut panel_area = None;
    terminal
        .draw(|f| {
            let targets = crate::app::panel_targets::build_panel_targets(&app.sessions, &[]);
            let mut cursor = 0;
            let mut scroll = 0;
            panel_area = Some(
                crate::app::render::render_sessions_overlay_content(
                    f,
                    None,
                    &targets,
                    false,
                    &mut cursor,
                    &mut scroll,
                    None,
                    None,
                    false,
                )
                .0,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(panel_area, Some(Rect::new(0, 0, width, height)));
    assert_eq!(
        buffer[(0, 0)].bg,
        palette::surface_colors(palette::Surface::SidebarBody, false).fill
    );
    assert_eq!(
        buffer[(2, 1)].bg,
        palette::surface_colors(palette::Surface::SidebarBand, false).fill
    );
    assert_eq!(buffer[(3, 1)].symbol(), "R");
    assert_eq!(buffer[(width - 1, 2)].symbol(), " ");
}

#[test]
fn sessions_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, selected, loading) in [
        (50, 12, false, false),
        (50, 12, true, false),
        (18, 8, true, false),
        (30, 8, false, true),
    ] {
        let output = render_sessions(width, height, selected, loading);
        assert!(
            output.contains("REMOTE"),
            "sessions shell missing: {output:?}"
        );
    }
}
