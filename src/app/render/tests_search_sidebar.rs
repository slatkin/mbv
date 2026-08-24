use super::test_helpers::buffer_to_string;
use crate::app::search_sidebar::SearchSidebar;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn render_sidebar(width: u16, height: u16, selected: bool) -> String {
    let mut sidebar = SearchSidebar::new();
    sidebar.query = "clip".into();
    sidebar.results = vec![
        make_item("Birthday Clip", "Movie"),
        make_item("Other Clip", "Series"),
    ];
    sidebar.cursor = usize::from(selected);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            crate::app::render::render_search_sidebar(
                f,
                Some(Rect::new(0, 0, width, height)),
                &mut sidebar,
            );
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn search_sidebar_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, selected) in [
        (40, 12, false),
        (40, 12, true),
        (12, 6, true),
        (24, 8, true),
    ] {
        let output = render_sidebar(width, height, selected);
        assert!(
            output.contains("SEARCH"),
            "search shell missing: {output:?}"
        );
        assert!(output.contains("sel"), "search hint missing: {output:?}");
    }
}
