use super::test_helpers::buffer_to_string;
use super::{render_feeds_manage_content, FeedsManageRenderModel};
use crate::app::types_feeds_manage::{FeedsManagePopup, FeedsManageStage};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_feeds_manage(width: u16, height: u16) -> String {
    let popup = FeedsManagePopup::new();
    let feeds = Vec::new();
    let stage = FeedsManageStage::List;
    let mut dim_backdrop_active = false;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            render_feeds_manage_content(
                f,
                &mut dim_backdrop_active,
                FeedsManageRenderModel {
                    feeds: &feeds,
                    stage: &stage,
                    cursor: 0,
                    pending_add: popup.pending_add,
                },
            );
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn feeds_manage_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height) in [(60, 20), (60, 20), (20, 10), (32, 12)] {
        let output = render_feeds_manage(width, height);
        assert!(
            output.contains("Manage"),
            "feed-management shell missing: {output:?}"
        );
    }
}
