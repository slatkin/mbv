use super::test_helpers::buffer_to_string;
use super::{render_multiselect_content, MultiSelectRenderModel};
use crate::app::types_context_menu::{MultiSelectKind, MultiSelectPopup};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_multiselect(width: u16, height: u16, cursor: usize, kind: MultiSelectKind) -> String {
    let popup = MultiSelectPopup {
        kind,
        items: vec![
            ("movies".into(), "Movies".into(), false),
            ("shows".into(), "Shows".into(), true),
        ],
        cursor,
    };
    let mut dim_backdrop_active = false;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            render_multiselect_content(
                f,
                &mut dim_backdrop_active,
                MultiSelectRenderModel {
                    kind: popup.kind,
                    items: &popup.items,
                    cursor: popup.cursor,
                },
            );
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn multiselect_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, cursor, kind) in [
        (70, 16, 0, MultiSelectKind::HiddenLibraries),
        (70, 16, 1, MultiSelectKind::HiddenLibraries),
        (24, 10, 1, MultiSelectKind::HiddenLatest),
        (40, 12, 0, MultiSelectKind::FeedViewLibraries),
    ] {
        let output = render_multiselect(width, height, cursor, kind);
        assert!(
            output.contains("Movies"),
            "multiselect item missing: {output:?}"
        );
        assert!(
            output.contains("["),
            "multiselect check missing: {output:?}"
        );
    }
}
