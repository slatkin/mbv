use super::test_helpers::buffer_to_string;
use crate::app::components::SessionsComponent;
use crate::app::palette;
use crate::app::tests::{make_app_stub, make_session};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use rstest::rstest;
use tuirealm::component::Component;

fn render_sessions(width: u16, height: u16, loading: bool, has_target: bool) -> String {
    let mut app = make_app_stub();
    app.sessions_loading = loading;
    if has_target {
        app.sessions = vec![make_session("Living Room", "Emby")];
    }
    let targets = crate::app::panel_targets::build_panel_targets(&app.sessions, &[]);
    let mut component = SessionsComponent::new();
    component.set_content(
        &targets,
        loading,
        None,
        None,
        false,
        Some(Rect::new(0, 0, width, height)),
    );
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| component.view(f, f.area())).unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn sessions_none_fallback_paints_the_fullscreen_shell() {
    let mut app = make_app_stub();
    app.sessions = vec![make_session("Living Room", "Emby")];
    let targets = crate::app::panel_targets::build_panel_targets(&app.sessions, &[]);
    let width = 40;
    let height = 12;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut component = SessionsComponent::new();
    component.set_content(&targets, false, None, None, false, None);
    terminal.draw(|f| component.view(f, f.area())).unwrap();
    let buffer = terminal.backend().buffer();
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

#[rstest]
#[case::ordinary(50, 12)]
#[case::narrow(18, 8)]
fn sessions_buffer_characterization_covers_populated_layout(
    #[case] width: u16,
    #[case] height: u16,
) {
    let output = render_sessions(width, height, false, true);
    assert!(
        output.contains("REMOTE"),
        "sessions shell missing: {output:?}"
    );
}

#[test]
fn sessions_buffer_preserves_loading_and_empty_states() {
    let loading = render_sessions(30, 8, true, false);
    assert!(
        loading.contains("Loading"),
        "loading state missing: {loading:?}"
    );
    let empty = render_sessions(30, 8, false, false);
    assert!(
        empty.contains("No sessions"),
        "empty state missing: {empty:?}"
    );
}
