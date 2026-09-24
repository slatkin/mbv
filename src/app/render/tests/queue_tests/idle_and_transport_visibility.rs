use super::*;

#[test]
fn idle_queue_only_hides_card_and_panel_at_both_widths() {
    for width in [80, 120] {
        let mut app = make_queue_app(5);
        app.panel_mode = crate::app::PanelMode::QueueOnly;
        let (term, layout) = render_queue_view_to_terminal(&mut app, width, 40);
        let screen = buffer_to_string(&term);
        let before_queue = screen
            .lines()
            .take(layout.content_area.y as usize)
            .collect::<Vec<_>>()
            .join("\n");

        // No card surface is published, so the queue takes the whole column
        // below the header row; no card/panel/track content paints above it.
        assert!(!before_queue.contains('\u{2594}'));
        assert!(!before_queue.contains("On Now:"));
        assert!(layout.content_area.height > 0);
    }

    let mut app = make_queue_app(5);
    app.mini_view_focus = crate::app::PanelFocus::Queue;
    let width = crate::app::MINI_VIEW_THRESHOLD - 1;
    let (term, layout) = render_queue_view_to_terminal(&mut app, width, 40);
    let screen = buffer_to_string(&term);
    assert!(screen
        .lines()
        .take(layout.content_area.y as usize)
        .all(|row| !row.contains('\u{2594}') && !row.contains("On Now:")));
}

#[test]
fn idle_both_hides_card_and_reclaims_queue_rows() {
    let width = 80;
    let height = 60;
    let mut idle = make_queue_app(5);
    let (idle_term, idle_layout) = render_queue_view_to_terminal(&mut idle, width, height);

    assert_eq!(idle.layout.card.height, 0);
    assert!(idle_layout.content_area.height > 0);
    let before_queue = buffer_to_string(&idle_term)
        .lines()
        .take(idle_layout.content_area.y as usize)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!before_queue.contains("On Now:"));

    let mut active = make_queue_app(5);
    active.player.status.lock().unwrap().active = true;
    let (_, active_layout) = render_queue_view_to_terminal(&mut active, width, height);

    assert!(active.layout.card.height > 0);
    assert!(
        idle_layout.content_area.height > active_layout.content_area.height,
        "idle queue must reclaim the card rows in Both mode"
    );
}

#[test]
fn idle_queue_only_reclaims_card_and_panel_rows_until_playback_starts() {
    let width = 80;
    let height = 60;
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::QueueOnly;

    let (idle_term, idle_layout) = render_queue_view_to_terminal(&mut app, width, height);
    let idle_queue_area = idle_layout.content_area;
    let _ = &idle_queue_area;
    // Idle queue-only hides the separator row along with the card/panel and
    // hands every reclaimed row to the queue; only the header row stays
    // reserved above the panel (task 3.2; the panel's recessed inset is the
    // single space row below the header).
    assert_eq!(app.layout.card.height, 0);
    let idle_screen = buffer_to_string(&idle_term);
    assert!(!idle_screen.contains("On Now:"));

    let mut status = app.player.status.lock().unwrap();
    status.active = true;
    drop(status);
    let (active_term, active_layout) = render_queue_view_to_terminal(&mut app, width, height);

    // Playback restores the card and the seekbar/panel rows, pushing the
    // queue down and shrinking it by the same rows.
    assert!(app.layout.card.height > 0, "playback must restore the card");
    assert!(buffer_to_string(&active_term).contains('\u{2594}'));
    assert!(active_queue_area_y(&app) > idle_layout.content_area.y);
    assert!(idle_layout.content_area.height > active_layout.content_area.height);
}

/// The queue panel's content-area y after the last render (component-retained
/// geometry re-read through the shell's placement helper, task 3.1).
fn active_queue_area_y(app: &App) -> u16 {
    app.queue_panel_placement().content_area.y
}

/// Task 3.6 (D10): the connected-idle exception is deleted. A connected but
/// idle transport keeps only the Queue playback panel's header row — the slot
/// and the transport collapse exactly as in a disconnected idle frame, and
/// the header names the connected target.
#[test]
fn connected_idle_queue_only_collapses_to_the_header_row() {
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.connected_session_state = Some(make_session("remote-host", "Emby"));
    app.connected_session_id = Some("remote-host".into());
    let (term, _) = render_queue_view_to_terminal(&mut app, 80, 40);
    let screen = buffer_to_string(&term);

    assert_eq!(app.layout.card.height, 0);
    assert!(screen.contains("IDLE"), "the header states the idle status");
    assert!(
        screen.contains("on remote-host"),
        "the header names the target"
    );
    assert!(
        !screen.contains('\u{2594}'),
        "no seekbar paints while idle, connected or not"
    );
}

/// Task 3.6 (D10): paused counts as active — the slot and the transport stay
/// painted (the transport routes through the shared width-driven arrangement
/// the Library playback panel also uses).
#[test]
fn paused_queue_only_keeps_card_and_panel() {
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.paused = true;
    }
    let (term, _) = render_queue_view_to_terminal(&mut app, 80, 40);
    let screen = buffer_to_string(&term);

    assert!(app.layout.card.height > 0);
    assert!(screen.contains('\u{2594}'), "the transport seekbar paints");
    assert!(screen.contains("PAUSED"), "the header states PAUSED");
}

/// Task 3.6's `both` counterpart: paused playback in the two-panel layout
/// keeps the slot and the transport in the queue column (the right-column
/// strip paints only when the queue column is hidden, D10).
#[test]
fn paused_both_keeps_card_and_queue_column_transport() {
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::Both;
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.paused = true;
    }
    let (term, _) = render_queue_view_to_terminal(&mut app, 100, 40);
    let screen = buffer_to_string(&term);

    assert!(app.layout.card.height > 0);
    assert!(screen.contains('\u{2594}'), "the transport seekbar paints");
    assert!(screen.contains("PAUSED"), "the header states PAUSED");
    // The queue column's width at 100 columns is below the side-by-side
    // threshold, so the transport stacks in the queue column — the right
    // column's reserved strip band paints nothing (task 3.5, D10).
    let chrome = app.compute_chrome_geometry(Rect::new(0, 0, 100, 40));
    assert!(chrome.root.queue_playback.is_some());
}
