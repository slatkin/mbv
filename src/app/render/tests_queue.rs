use super::test_helpers::*;
use crate::app::palette;
use crate::app::tests::make_session;

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
    // This characterization is about the full-width layout; row text is
    // intentionally not asserted because card truncation varies by width.
    let _ = term;
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
            Some(palette::SURFACE_ACCENT_SOFT),
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
        Some(palette::SURFACE_ACCENT_SOFT),
        "focused queue in both mode must keep the queue's focused frame background, got {:?}",
        cell.style().bg
    );
}

#[test]
fn mini_view_starts_at_queue_only_by_default() {
    // A fresh app on a narrow terminal, with no prior interaction, must show
    // queue-only (the default mini_view_focus), not both and not library-only.
    let mut app = make_movie_app();
    let width = crate::app::MINI_VIEW_THRESHOLD - 1;

    let layout = render_view(&mut app, width, 20);

    assert_eq!(
        layout.queue_area.width,
        width.saturating_sub(4),
        "mini view must start queue-only: queue must span the terminal width"
    );
    assert_eq!(
        layout.panel_area.width, width,
        "mini queue-only panel must span the terminal width"
    );
    assert_eq!(
        layout.panel_content_area.width,
        width.saturating_sub(4),
        "mini queue-only mouse content bounds must span the terminal width"
    );
    assert_eq!(
        app.effective_panel_mode(),
        crate::app::PanelMode::QueueOnly,
        "fresh narrow app defaults to queue-only mini view"
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
        app.mini_view_focus = crate::app::PanelFocus::Library;

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

    let (_, layout_on) = render_view_to_terminal(&mut app, 80, 40);
    let queue_rows_with_visualizer = layout_on.queue_area.height;

    app.visualizer_enabled = false;
    let (_, layout_off) = render_view_to_terminal(&mut app, 80, 40);

    assert_eq!(
        queue_rows_with_visualizer, layout_off.queue_area.height,
        "selecting the visualizer must not subtract rows below the queue list"
    );
    assert!(
        queue_rows_with_visualizer > 0,
        "the queue list must still have rows to render"
    );
}

#[test]
fn idle_queue_only_hides_card_and_panel_at_both_widths() {
    for width in [80, 120] {
        let mut app = make_queue_app(5);
        app.panel_mode = crate::app::PanelMode::QueueOnly;
        let (term, layout) = render_view_to_terminal(&mut app, width, 40);
        let buf = term.backend().buffer();
        let before_queue = (0..layout.queue_area.y)
            .flat_map(|y| (0..width).map(move |x| buf[(x, y)].symbol()))
            .collect::<String>();

        assert_eq!(layout.card.height, 0);
        assert_eq!(layout.card.width, 0);
        assert!(!before_queue.contains('\u{2594}'));
        assert!(!before_queue.contains("On Now:"));
    }

    let mut app = make_queue_app(5);
    app.mini_view_focus = crate::app::PanelFocus::Queue;
    let (term, layout) = render_view_to_terminal(&mut app, crate::app::MINI_VIEW_THRESHOLD - 1, 40);
    let buf = term.backend().buffer();
    assert_eq!(layout.card.height, 0);
    assert!((0..layout.queue_area.y)
        .flat_map(|y| (0..buf.area().width).map(move |x| buf[(x, y)].symbol()))
        .all(|symbol| symbol != "▔"));
}

#[test]
fn idle_queue_only_reclaims_card_and_panel_rows_until_playback_starts() {
    let width = 80;
    let height = 60;
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::QueueOnly;

    let _ = render_app_to_terminal(&mut app, width, height);
    let idle_queue_area = app.layout.main.queue_area;
    let idle_card_height = app.layout.main.card.height;
    let idle_title_area = app
        .layout
        .main
        .queue_title_area
        .expect("idle queue should retain its title row");
    let chrome = app.compute_chrome_geometry(ratatui::layout::Rect::new(0, 0, width, height));
    let queue_panel_y = idle_title_area.y - 1;
    assert_eq!(
        queue_panel_y,
        chrome.left_content.y + 1,
        "queue panel keeps the existing one-row separator below left content"
    );
    assert!(
        idle_queue_area.y > chrome.left_content.y,
        "queue content must begin below the left-content separator"
    );
    assert_eq!(idle_card_height, 0);

    let mut status = app.player.status.lock().unwrap();
    status.active = true;
    drop(status);
    let _ = render_app_to_terminal(&mut app, width, height);
    let active_queue_area = app.layout.main.queue_area;
    let active_card_height = app.layout.main.card.height;

    assert!(active_card_height > 0, "playback must restore the card");
    assert_eq!(
        idle_queue_area.height,
        active_queue_area.height + active_card_height + 4,
        "idle queue gains the card and four playback-panel rows"
    );
    assert_eq!(
        active_queue_area.y - idle_queue_area.y,
        active_card_height + 4,
        "active queue moves down by the card and four playback-panel rows"
    );
}

#[test]
fn connected_idle_queue_only_keeps_panel_but_collapses_card() {
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.connected_session_state = Some(make_session("remote-host", "Emby"));
    app.connected_session_id = Some("remote-host".into());
    let term = render_app_to_terminal(&mut app, 80, 40);
    let card_height = app.layout.main.card.height;

    assert_eq!(card_height, 0);
    assert!(app.layout.playback.seekbar_area.height > 0);
    assert!(buffer_to_string(&term).contains('\u{2594}'));
}

#[test]
fn paused_queue_only_keeps_card_and_panel() {
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.paused = true;
    }
    let term = render_app_to_terminal(&mut app, 80, 40);
    let card_height = app.layout.main.card.height;

    assert!(card_height > 0);
    assert!(app.layout.playback.seekbar_area.height > 0);
    assert!(buffer_to_string(&term).contains('\u{2594}'));
}

#[test]
fn wide_queue_only_leftover_rows_stay_dark_bg_without_duplicate_visualizer() {
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.visualizer_enabled = true;
    app.player.status.lock().unwrap().active = true;
    app.visualizer_window.samples = vec![crate::app::visualizer_worker::StereoSample {
        left: 1.0,
        right: 1.0,
    }];

    let (term, _layout) = render_view_to_terminal(&mut app, 120, 40);
    let buf = term.backend().buffer();

    // With no previous artwork geometry, the initial visualizer reservation
    // is (x=2, y=1, w=48, h=24); the wide playback panel starts at x=52 and
    // the 4-row player content tops it, so
    // leftover rows below it must stay on the dark chrome background rather
    // than hosting a second visualizer.
    let leftover_cell = &buf[(30, 10)];
    assert_eq!(
        leftover_cell.style().bg,
        Some(palette::SURFACE_CHROME),
        "wide playback leftovers must keep DARK_BG, got {:?}",
        leftover_cell.style().bg
    );
    // Region the removed wide-panel visualizer branch used to paint.
    let mut duplicate = false;
    'scan: for y in 5..25 {
        for x in 52..buf.area().width {
            if buf[(x, y)].symbol() == crate::config::DEFAULT_VISUALIZER_GLYPH {
                duplicate = true;
                break 'scan;
            }
        }
    }
    assert!(
        !duplicate,
        "the visualizer must only render inside the queue card slot, never in playback-panel leftovers"
    );
    let mut card_visualizer = false;
    'card: for y in 1..25 {
        for x in 2..50 {
            if buf[(x, y)].symbol() == crate::config::DEFAULT_VISUALIZER_GLYPH {
                card_visualizer = true;
                break 'card;
            }
        }
    }
    assert!(
        card_visualizer,
        "the selected visualizer must render inside the queue card slot"
    );
}
