use super::test_helpers::*;
use crate::app::palette;
use crate::app::tests::make_session;
use crate::App;
use ratatui::layout::Rect;

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
#[ignore = "obsolete legacy-render characterization"]
fn short_window_keeps_queue_in_left_column() {
    let mut app = make_movie_app();
    app.queue_column_width = 40;

    let (_term, layout) = render_queue_view_to_terminal(&mut app, 100, 12);

    assert!(
        layout.content_area.x < app.queue_column_width,
        "expected short-height queue to stay in the left column, got {:?}",
        layout.content_area
    );
    assert!(
        app.layout.main.left_area.x >= app.queue_column_width,
        "expected library area to remain in the right column, got {:?}",
        app.layout.main.left_area
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
    assert_eq!(
        app.layout.main.panel_area.width, 80,
        "left panel must span full width in QueueOnly"
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
        let cell = &buf[(layout.content_area.x + 1, layout.content_area.y + 1)];
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

    let (term, layout) = render_queue_view_to_terminal(&mut app, 80, 20);
    let buf = term.backend().buffer();
    let cell = &buf[(layout.content_area.x + 1, layout.content_area.y + 1)];
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

    let (_term, layout) = render_queue_view_to_terminal(&mut app, width, 20);

    assert_eq!(
        layout.content_area.width,
        width.saturating_sub(4),
        "mini view must start queue-only: queue must span the terminal width"
    );
    assert_eq!(
        app.layout.main.panel_area.width, width,
        "mini queue-only panel must span the terminal width"
    );
    assert_eq!(
        app.layout.main.panel_content_area.width,
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
        // below the header row, its separator, and the panel's title band;
        // no card/panel/track content paints above it.
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

    assert_eq!(idle.layout.main.card.height, 0);
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

    assert!(active.layout.main.card.height > 0);
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
    // hands every reclaimed row to the queue; only the header row (and the
    // separator above the panel) stays reserved (task 3.2).
    assert_eq!(app.layout.main.card.height, 0);
    let idle_screen = buffer_to_string(&idle_term);
    assert!(!idle_screen.contains("On Now:"));

    let mut status = app.player.status.lock().unwrap();
    status.active = true;
    drop(status);
    let (active_term, active_layout) = render_queue_view_to_terminal(&mut app, width, height);

    // Playback restores the card and the seekbar/panel rows, pushing the
    // queue down and shrinking it by the same rows.
    assert!(
        app.layout.main.card.height > 0,
        "playback must restore the card"
    );
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

    assert_eq!(app.layout.main.card.height, 0);
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

    assert!(app.layout.main.card.height > 0);
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

    assert!(app.layout.main.card.height > 0);
    assert!(screen.contains('\u{2594}'), "the transport seekbar paints");
    assert!(screen.contains("PAUSED"), "the header states PAUSED");
    // The queue column's width at 100 columns is below the side-by-side
    // threshold, so the transport stacks in the queue column — the right
    // column's reserved strip band paints nothing (task 3.5, D10).
    let chrome = app.compute_chrome_geometry(Rect::new(0, 0, 100, 40));
    assert!(chrome.root.queue_playback.is_some());
}

/// Task 3.5: the header states the status and the target at 80 and 100+
/// columns alike, through the real shell path (the header is the Queue
/// playback panel's always-painted row).
#[test]
fn queue_playback_header_states_status_and_target_at_both_widths() {
    for (width, mode) in [
        (80, crate::app::PanelMode::Both),
        (120, crate::app::PanelMode::QueueOnly),
        (120, crate::app::PanelMode::Both),
    ] {
        let mut app = make_queue_app(5);
        app.panel_mode = mode;
        {
            let mut status = app.player.status.lock().unwrap();
            status.active = true;
            status.paused = false;
        }
        let (term, _) = render_queue_view_to_terminal(&mut app, width, 40);
        let screen = buffer_to_string(&term);
        assert!(
            screen.contains("PLAYING"),
            "width {width} {mode:?}: the header states PLAYING"
        );
        assert!(
            screen.contains(&format!("on {}", mbv_core::api::device_name())),
            "width {width} {mode:?}: the header names the local target"
        );
    }
}

/// Task 3.5: the header follows the playback target, not the viewed queue —
/// a remote-attached frame with the Local scope selected still names the
/// remote target.
#[test]
fn remote_attached_header_names_the_remote_target_with_local_scope() {
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.connected_session_state = Some(make_session("remote-host", "Emby"));
    app.connected_session_id = Some("remote-host".into());
    assert_eq!(app.viewed_queue_scope(), crate::app::QueueScope::Local);
    let (term, _) = render_queue_view_to_terminal(&mut app, 80, 40);
    let screen = buffer_to_string(&term);
    assert!(
        screen.contains("on remote-host"),
        "the header follows the playback target, not the viewed scope"
    );
    assert!(
        !screen.contains("TRACKING"),
        "no tracking suffix on the header"
    );
}

/// Task 3.5 (D1 mount rule): the Queue playback panel is mounted in every
/// queue-visible layout and unmounted in library-only.
#[test]
fn queue_playback_panel_unmounts_in_library_only() {
    use crate::app::components::ComponentId;
    use crate::app::shell::Model;

    let mut app = make_queue_app(5);
    app.terminal_width = 80;
    app.terminal_height = 40;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.panel_focus = crate::app::PanelFocus::Library;
    app.mini_view_focus = crate::app::PanelFocus::Library;
    let mut model = Model::new(app);
    model.sync_mounted_surfaces();
    assert!(!model.application.mounted(&ComponentId::QueuePlaybackPanel));

    for mode in [
        crate::app::PanelMode::Both,
        crate::app::PanelMode::QueueOnly,
    ] {
        let mut app = make_queue_app(5);
        app.terminal_width = 80;
        app.terminal_height = 40;
        app.panel_mode = mode;
        let mut model = Model::new(app);
        model.sync_mounted_surfaces();
        assert!(
            model.application.mounted(&ComponentId::QueuePlaybackPanel),
            "{mode:?}: the panel is mounted in every queue-visible layout"
        );
    }
}

#[test]
#[ignore = "obsolete legacy-render characterization"]
fn wide_active_queue_starts_below_panel_rows() {
    // Wide queue-only paints the panel beside the card, so on a frame where
    // the card is shorter than the four panel rows the queue must begin
    // below the panel rect — otherwise list rows overpaint the panel.
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.player.status.lock().unwrap().active = true;
    let width = 120;
    let height = 40;
    let (_term, layout) = render_queue_view_to_terminal(&mut app, width, height);
    let chrome = app.compute_chrome_geometry(Rect::new(0, 0, width, height));
    let panel_rows = app.layout.main.card.height.max(4);
    assert!(
        layout.content_area.y > chrome.left_content.y + panel_rows,
        "queue must start below the painted wide panel rows (plus the header row and its separator)"
    );
    // The panel background must still fill the wide side-by-side slot.
    assert!(
        layout.content_area.y > chrome.left_content.y,
        "queue must sit below the left-column content top"
    );
}

#[test]
#[ignore = "obsolete legacy-render characterization"]
fn wide_queue_only_leftover_rows_stay_dark_bg_without_duplicate_visualizer() {
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.visualizer_enabled = true;
    app.player.status.lock().unwrap().active = true;
    app.visualizer_window.samples = vec![crate::app::visualizer_worker::StereoSample {
        left: 1.0,
        right: 1.0,
    }];

    let (term, _layout) = render_queue_view_to_terminal(&mut app, 120, 40);
    let buf = term.backend().buffer();

    // With no previous artwork geometry, the initial visualizer reservation
    // is (x=2, y=1, w=48, h=24) — the full 24-row cap, because the real
    // shell paint path normalizes `terminal_height` from the drawn frame —
    // and the wide playback panel starts at x=52, its 4-row player content
    // topped by the panel background. The side-by-side slot below that
    // content stays on the dark chrome background rather than hosting a
    // second visualizer.
    let leftover_cell = &buf[(60, 20)];
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
