use super::test_helpers::*;
use crate::app::palette;

#[test]
fn short_window_keeps_queue_in_left_column() {
    let mut app = make_movie_app();
    app.queue_column_width = 40;

    let layout = render_view(&mut app, 100, 12);

    assert!(
        layout.queue_area.x < app.queue_column_width,
        "expected short-height queue to stay in the left column, got {:?}",
        layout.queue_area
    );
    assert!(
        layout.left_area.x >= app.queue_column_width,
        "expected library area to remain in the right column, got {:?}",
        layout.left_area
    );
}

#[test]
fn short_queue_panel_drops_padding_before_rows() {
    let mut app = make_queue_app(20);

    let (_term, layout) = render_view_to_terminal(&mut app, 100, 12);

    assert!(
        layout.queue_area.height >= 1,
        "expected at least one usable queue row on a short terminal, got {:?}",
        layout.queue_area
    );
}

#[test]
fn queue_only_layout_spans_full_width() {
    let mut app = make_queue_app(20);
    app.panel_mode = crate::app::PanelMode::QueueOnly;

    let (term, layout) = render_view_to_terminal(&mut app, 80, 20);

    assert_eq!(
        layout.queue_area.width, 76,
        "queue must span the full width minus inner padding"
    );
    assert_eq!(
        layout.panel_area.width, 80,
        "left panel must span full width in QueueOnly"
    );
    let text = buffer_to_string(&term);
    assert!(
        text.contains("Queue Item"),
        "queue items should render in QueueOnly"
    );
}

#[test]
fn queue_only_renders_queue_focused_when_queue_holds_focus() {
    // Queue-only with the queue holding panel focus must render with focused
    // styling — no longer the old forced-unfocused look — at any width. Below
    // MINI_VIEW_THRESHOLD, queue-only is driven by mini_view_focus rather
    // than the wide-mode panel_mode/panel_focus.
    for width in [79, 80, 100] {
        let mut app = make_queue_app(20);
        if width < crate::app::MINI_VIEW_THRESHOLD {
            app.mini_view_focus = crate::app::PanelFocus::Queue;
        } else {
            app.panel_mode = crate::app::PanelMode::QueueOnly;
            assert_eq!(app.panel_focus, crate::app::PanelFocus::Queue);
        }

        let (term, layout) = render_view_to_terminal(&mut app, width, 20);
        let buf = term.backend().buffer();
        let cell = &buf[(layout.queue_area.x + 1, layout.queue_area.y + 1)];
        assert_eq!(
            cell.style().bg,
            Some(palette::BG_GREEN_SOFT),
            "queue-only with queue focus at width {width} must use the queue's focused frame background, got {:?}",
            cell.style().bg
        );
    }
}

#[test]
fn both_mode_focused_queue_keeps_focused_styling() {
    let mut app = make_queue_app(20);

    let (term, layout) = render_view_to_terminal(&mut app, 80, 20);
    let buf = term.backend().buffer();
    let cell = &buf[(layout.queue_area.x + 1, layout.queue_area.y + 1)];
    assert_eq!(
        cell.style().bg,
        Some(palette::BG_GREEN_SOFT),
        "focused queue in both mode must keep the queue's focused frame background, got {:?}",
        cell.style().bg
    );
}

#[test]
fn mini_view_starts_at_library_only_by_default() {
    // A fresh app on a narrow terminal, with no prior interaction, must show
    // library-only (the default mini_view_focus), not both and not queue-only.
    let mut app = make_movie_app();
    let width = crate::app::MINI_VIEW_THRESHOLD - 1;

    let layout = render_view(&mut app, width, 20);

    assert_eq!(
        layout.panel_area.width, 0,
        "mini view must start library-only: no left panel rendered, got {:?}",
        layout.panel_area
    );
    assert_eq!(
        app.effective_panel_mode(),
        crate::app::PanelMode::LibraryOnly,
        "fresh narrow app defaults to library-only mini view"
    );
}
