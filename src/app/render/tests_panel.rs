use super::test_helpers::*;
use super::*;
use crate::app::layout::{AppLayout, LayoutPlayback, LibraryRowTarget};
use crate::app::tests::{make_app_stub, make_item};
use crate::app::{BrowseLevel, LibSearch, LibraryTab, QueueScope, RemoteSlotState};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn expanded_power_view_tab_panel_has_two_column_side_gutters() {
    let mut app = make_app_stub();
    app.queue_column_width = 40;

    let layout = render_power_view(&mut app, 80, 24);

    assert_eq!(layout.left_area.x, 40 + POWER_TAB_LEFT_PAD);
    assert_eq!(layout.left_area.width, 40 - 2 * POWER_TAB_LEFT_PAD);
}

#[test]
fn expanded_power_panel_bounds_follow_sidebar_resize() {
    let mut app = make_app_stub();
    app.queue_column_width = 31;
    let first = render_power_view(&mut app, 80, 24);
    assert_eq!(first.panel_area, Rect::new(0, 0, 31, 24));
    assert_eq!(first.panel_content_area, Rect::new(2, 3, 27, 19));

    app.queue_column_width = 47;
    let second = render_power_view(&mut app, 80, 24);
    assert_eq!(second.panel_area, Rect::new(0, 0, 47, 24));
    assert_eq!(second.panel_content_area, Rect::new(2, 3, 43, 19));
}

#[test]
fn settings_content_has_two_column_and_one_row_insets() {
    let mut app = make_app_stub();
    let backend = TestBackend::new(20, 10);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = AppLayout::default();
    term.draw(|f| {
        app.render_settings_panel(f, &mut layout, None);
    })
    .unwrap();

    assert_eq!(layout.settings_content_area, Rect::new(2, 4, 15, 3));
}

#[test]
fn collapsed_power_right_panel_keeps_one_column_after_scrollbar() {
    let right_panel = Rect::new(0, 0, 80, 24);
    let content = power_right_panel_content_area(right_panel, true);
    assert_eq!(content.x + content.width, right_panel.right() - 1);
}
