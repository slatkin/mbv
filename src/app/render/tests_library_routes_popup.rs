use super::test_helpers::buffer_to_string;
use super::{render_library_routes_content, LibraryRoutesRenderModel};
use crate::app::{LibraryRoutePopup, LibraryRouteStage};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_routes(width: u16, height: u16, cursor: usize) -> String {
    let popup = LibraryRoutePopup {
        stage: LibraryRouteStage::PickLibrary {
            items: vec![
                ("movies".into(), "Movies".into(), None),
                ("music".into(), "Music".into(), None),
            ],
        },
        cursor,
    };
    let mut dim_backdrop_active = false;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            render_library_routes_content(
                f,
                &mut dim_backdrop_active,
                LibraryRoutesRenderModel {
                    stage: &popup.stage,
                    cursor: popup.cursor,
                },
            );
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn library_routes_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, cursor) in [(60, 20, 0), (60, 20, 1), (20, 10, 1), (32, 12, 0)] {
        let output = render_routes(width, height, cursor);
        assert!(
            output.contains("Movies"),
            "library-routes popup missing: {output:?}"
        );
    }
}
