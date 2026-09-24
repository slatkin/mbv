use super::*;

/// Task 4.1 (D10): the right column reserves the playback strip's
/// `PLAYER_BOX_HEIGHT` rows only in library-only. `both` reserves none — the
/// library starts at the tab bar's bottom edge and the right column paints
/// no transport while playback is active — `queue-only` reserves none, and
/// library-only's strip band is exactly `PLAYER_BOX_HEIGHT` tall, sits
/// between the tab bar and the library, and paints the frame's transport
/// (the seekbar track glyph is its signature).
#[test]
fn strip_rows_are_reserved_only_in_library_only() {
    fn active_app() -> App {
        let app = make_queue_app(5);
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.queue_len = 5;
        status.current_idx = 0;
        status.position_ticks = 45 * mbv_core::api::TICKS_PER_SECOND;
        status.runtime_ticks = 90 * mbv_core::api::TICKS_PER_SECOND;
        drop(status);
        app
    }

    // The seekbar's filled track is the transport's signature: a `▔` cell in
    // the ACCENT foreground (the hero borders' `▔` frames use the track
    // colour instead), collected together with the frame's panel placements.
    fn transport_cells(buf: &ratatui::buffer::Buffer) -> Vec<(u16, u16)> {
        let mut cells = Vec::new();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                let cell = &buf[(x, y)];
                if cell.symbol() == "\u{2594}" && cell.style().fg == Some(palette::ACCENT) {
                    cells.push((x, y));
                }
            }
        }
        cells
    }

    // `both`: no strip rows are reserved — the library starts directly below
    // the tab bar, and no transport cell paints in the right column (the
    // frame's one transport is the queue column's).
    let mut app = active_app();
    app.panel_mode = crate::app::PanelMode::Both;
    let (term, _) = render_queue_view_to_terminal(&mut app, 100, 40);
    let chrome = app.compute_chrome_geometry(Rect::new(0, 0, 100, 40));
    assert!(chrome.root.library_playback.is_none());
    assert_eq!(
        chrome.root.library.expect("library placed").y,
        chrome.root.tab.expect("tab placed").bottom(),
        "both: the library starts at the tab bar's bottom edge — no strip band reserved"
    );
    let library_x = chrome.root.library.unwrap().x;
    let cells = transport_cells(term.backend().buffer());
    assert!(!cells.is_empty(), "both: the queue-column transport paints");
    assert!(
        cells.iter().all(|&(x, _)| x < library_x),
        "both: no transport cell paints in the right column: {cells:?}"
    );

    // `queue-only`: no strip at all.
    let mut app = active_app();
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    let (_term, _) = render_queue_view_to_terminal(&mut app, 80, 40);
    let chrome = app.compute_chrome_geometry(Rect::new(0, 0, 80, 40));
    assert!(chrome.root.library_playback.is_none());
    assert!(chrome.root.library.is_none());
    assert!(chrome.root.tab.is_none());

    // `library-only`: the strip band is exactly `PLAYER_BOX_HEIGHT`, sits
    // between the tab bar and the library, and paints the frame's one
    // transport.
    let mut app = active_app();
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.panel_focus = crate::app::PanelFocus::Library;
    app.mini_view_focus = crate::app::PanelFocus::Library;
    let (term, _) = render_queue_view_to_terminal(&mut app, 100, 40);
    let chrome = app.compute_chrome_geometry(Rect::new(0, 0, 100, 40));
    let strip = chrome
        .root
        .library_playback
        .expect("library-only places the strip");
    // (The strip band's placement — exactly `PLAYER_BOX_HEIGHT` tall,
    // between the tab bar and the library — is asserted once in
    // `chrome.rs::root_frame_tests`; this test keeps the paint proof.)
    let cells = transport_cells(term.backend().buffer());
    assert!(
        !cells.is_empty(),
        "library-only: the strip transport paints"
    );
    assert!(
        cells.iter().all(|&(x, y)| strip.contains((x, y).into())),
        "library-only: the strip paints the frame's only transport: {cells:?}"
    );
}

#[test]
fn short_queue_panel_drops_padding_before_rows() {
    let mut app = make_queue_app(20);

    let (_term, layout) = render_queue_view_to_terminal(&mut app, 100, 12);

    assert!(
        layout.content_area.height >= 1,
        "expected at least one usable queue row on a short terminal, got {:?}",
        layout.content_area
    );
}

#[test]
fn queue_only_layout_spans_full_width() {
    let mut app = make_queue_app(20);
    app.panel_mode = crate::app::PanelMode::QueueOnly;

    let (_term, layout) = render_queue_view_to_terminal(&mut app, 80, 20);

    assert_eq!(
        layout.content_area.width, 76,
        "queue must span the full width minus inner padding"
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

        let (term, layout) = render_queue_view_to_terminal(&mut app, width, 20);
        let buf = term.backend().buffer();
        let chrome = app.compute_chrome_geometry(Rect::new(0, 0, width, 20));
        let queue = chrome.root.queue.expect("queue panel placement");
        // The zebra sequence opens on the primary fill, so the list's first
        // striped row is its second row.
        let cell = &buf[(queue.x + 4, layout.content_area.y + 1)];
        assert_eq!(
            cell.style().bg,
            Some(palette::surface_colors(palette::Surface::QueueColumn, true).fill),
            "queue-only with queue focus at width {width} must use the focused zebra stripe, got {:?}",
            cell.style().bg
        );
    }
}

#[test]
fn both_queue_column_surface_covers_outer_padding_and_boundary() {
    for focused in [true, false] {
        let mut app = make_queue_app(20);
        app.panel_focus = if focused {
            crate::app::PanelFocus::Queue
        } else {
            crate::app::PanelFocus::Library
        };
        let (term, _) = render_queue_view_to_terminal(&mut app, 80, 20);
        let buffer = term.backend().buffer();
        let chrome = app.compute_chrome_geometry(Rect::new(0, 0, 80, 20));
        let column = chrome.left_area;
        let expected = palette::surface_colors(palette::Surface::QueueColumn, focused).fill;
        // The outer padding and boundary column have no queue content or
        // title/pill decoration, so every row must be owned by the queue
        // column surface there.
        for y in column.y + 1..column.bottom() {
            for x in [column.x, column.right() - 1] {
                assert_eq!(buffer[(x, y)].style().bg, Some(expected), "({x}, {y})");
            }
        }
    }
}

#[test]
fn both_mode_focused_queue_keeps_focused_styling() {
    let mut app = make_queue_app(20);

    let (term, layout) = render_queue_view_to_terminal(&mut app, 80, 20);
    let buf = term.backend().buffer();
    let cell = &buf[(layout.content_area.x + 2, layout.content_area.y + 1)];
    assert_eq!(
        cell.style().bg,
        Some(palette::surface_colors(palette::Surface::QueueColumn, true).fill),
        "focused queue in both mode must paint the focused zebra stripe, got {:?}",
        cell.style().bg
    );
}

#[test]
fn both_mode_resting_queue_keeps_outer_and_recessed_surfaces_distinct() {
    let mut app = make_queue_app(20);
    app.panel_focus = crate::app::PanelFocus::Library;
    let (term, layout) = render_queue_view_to_terminal(&mut app, 80, 20);
    let buffer = term.backend().buffer();
    let outer = palette::surface_colors(palette::Surface::QueueColumn, false).fill;
    let inner = palette::surface_colors(palette::Surface::QueuePanel, false).fill;
    assert_eq!(
        buffer[(layout.content_area.x - 2, layout.content_area.y)].bg,
        outer
    );
    assert_eq!(
        buffer[(layout.content_area.x, layout.content_area.y + 1)].bg,
        inner
    );
    assert_ne!(outer, inner);
}

#[test]
fn mini_view_starts_at_queue_only_by_default() {
    // A fresh app on a narrow terminal, with no prior interaction, must show
    // queue-only (the default mini_view_focus), not both and not library-only.
    let mut app = make_movie_app();
    let width = crate::app::MINI_VIEW_THRESHOLD - 1;

    let (_term, layout) = render_queue_view_to_terminal(&mut app, width, 20);

    assert_eq!(
        layout.content_area.width,
        width.saturating_sub(4),
        "mini view must start queue-only: queue must span the terminal width"
    );
    assert_eq!(
        app.effective_panel_mode(),
        crate::app::PanelMode::QueueOnly,
        "fresh narrow app defaults to queue-only mini view"
    );

    // Library-only is reached through the mini-view focus hand-off. Its
    // sidebar/overlay bounds must reclaim the full terminal width rather than
    // inheriting the zero-width queue-column rectangle.
    app.mini_view_focus = crate::app::PanelFocus::Library;
    let geometry = app.compute_chrome_geometry(Rect::new(0, 0, width, 20));
    assert_eq!(geometry.root.queue, None);
    assert_eq!(geometry.panel_area.width, width);
    assert_eq!(
        crate::app::render::components::chrome::left_panel_content_area(geometry.panel_area).width,
        width.saturating_sub(4)
    );
    assert_eq!(
        app.effective_panel_mode(),
        crate::app::PanelMode::LibraryOnly,
        "the mini-view focus hand-off selects library-only"
    );
}

#[test]
fn narrowing_from_each_wide_mode_starts_queue_only_without_mutating_wide_state() {
    for (mode, focus) in [
        (crate::app::PanelMode::Both, crate::app::PanelFocus::Library),
        (
            crate::app::PanelMode::LibraryOnly,
            crate::app::PanelFocus::Library,
        ),
        (
            crate::app::PanelMode::QueueOnly,
            crate::app::PanelFocus::Queue,
        ),
    ] {
        let mut app = make_movie_app();
        app.panel_mode = mode;
        app.panel_focus = focus;
        // Production order (task 1.2): a real Resize crossing into mini view
        // runs the focus hand-off in the sync pass before the draw -- the
        // draw path only reads geometry. The fixture performs that hand-off
        // itself: the stored wide focus stays in `panel_focus`, and the
        // mini-view focus moves to Queue exactly as
        // `Model::sync_terminal_resize` does on a narrowing Resize event.
        app.mini_view_focus = crate::app::PanelFocus::Queue;

        render_app_to_terminal(&mut app, crate::app::MINI_VIEW_THRESHOLD - 1, 20);

        assert_eq!(app.effective_panel_mode(), crate::app::PanelMode::QueueOnly);
        assert_eq!(app.effective_panel_focus(), crate::app::PanelFocus::Queue);
        assert_eq!(app.panel_mode, mode);
        assert_eq!(app.panel_focus, focus);
    }
}

#[test]
fn queue_keeps_rows_formerly_reserved_for_separate_visualizer() {
    // The visualizer now shares the queue card slot, so selecting it must not
    // consume queue-list rows below the panel the way the old bottom
    // visualizer reservation did.
    let mut app = make_queue_app(20);
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.visualizer_enabled = true;

    let (_, layout_on) = render_queue_view_to_terminal(&mut app, 80, 40);
    let queue_rows_with_visualizer = layout_on.content_area.height;

    app.visualizer_enabled = false;
    let (_, layout_off) = render_queue_view_to_terminal(&mut app, 80, 40);

    assert_eq!(
        queue_rows_with_visualizer, layout_off.content_area.height,
        "selecting the visualizer must not subtract rows below the queue list"
    );
    assert!(
        queue_rows_with_visualizer > 0,
        "the queue list must still have rows to render"
    );
}
