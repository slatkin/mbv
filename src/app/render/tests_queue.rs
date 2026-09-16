use super::test_helpers::*;
use crate::app::palette;
use crate::app::tests::make_session;
use crate::App;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;

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
            Some(Color::from_u32(0x003c4841)),
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
        Some(Color::from_u32(0x003c4841)),
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
        // below the header row and the panel's title band; no card/panel/track
        // content paints above it.
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

/// The header is recessed in the queue column: the row above it and the two
/// columns each side carry the column's own gutter surface rather than the
/// header band, so the header is not flush with the column's top, left, or
/// right edge. It keeps no bottom padding: the row below it is the
/// slot/transport band's first row (the arrangement tests pin that start).
#[test]
fn queue_playback_header_is_recessed_in_the_queue_column() {
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.paused = false;
    }
    let (term, _) = render_queue_view_to_terminal(&mut app, 80, 40);
    let buf = term.backend().buffer();
    let band = palette::surface_colors(palette::Surface::QueueOnlyPlaybackPanel, false).fill;
    // The header band's paint starts two columns in, on the second row.
    assert_eq!(buf[(2, 1)].style().bg, Some(band), "header paints inset");
    // The columns left of the header are the queue column's own gutter, not
    // the header band; the same value paints the placement's first row.
    let gutter = buf[(0, 1)].style().bg;
    assert_ne!(gutter, Some(band), "the header is not flush on the left");
    assert_eq!(
        gutter,
        buf[(0, 2)].style().bg,
        "the gutter is the column fill"
    );
    assert_ne!(
        buf[(2, 0)].style().bg,
        Some(band),
        "the row above the header is not the header band"
    );
    assert_eq!(
        buf[(2, 0)].style().bg,
        gutter,
        "the row above the header is the column gutter"
    );
    assert_ne!(
        buf[(79, 1)].style().bg,
        Some(band),
        "the header is not flush on the right"
    );
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

/// A locally selected row paints as now-playing, and the playback panel
/// follows it, in the same frame — without waiting for the playback owner to
/// report the track change (queue-canonical-list, "Selecting a different item
/// to play"). The regression this guards is the projection gate: while
/// something is already playing, an optimistic selection changes no observed
/// active slot, so a fingerprint that ignores it rebuilds no rows and both
/// surfaces stay on the outgoing item until the owner confirms.
#[test]
fn local_play_selection_moves_the_playhead_on_both_surfaces_immediately() {
    /// Screen positions of `title` painted on a live (now-playing) row,
    /// located by the row's aqua play glyph.
    fn now_playing_cells(buf: &ratatui::buffer::Buffer, title: &str) -> Vec<(u16, u16)> {
        let mut hits = Vec::new();
        for y in 0..buf.area().height {
            let has_icon = (0..buf.area().width).any(|x| {
                buf[(x, y)].symbol() == "▶" && buf[(x, y)].style().fg == Some(palette::ACCENT)
            });
            if !has_icon {
                continue;
            }
            let text: String = (0..buf.area().width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            if let Some(x) = text.find(title) {
                hits.push((x as u16, y));
            }
        }
        hits
    }

    fn frame_text(buf: &ratatui::buffer::Buffer) -> String {
        (0..buf.area().height)
            .map(|y| {
                (0..buf.area().width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Distinct runtimes so the panel's time text names the item it paints.
    fn queue_item(name: &str, runtime_secs: i64) -> mbv_core::api::EmbyItem {
        let mut item = crate::app::tests::make_item(name, "Movie");
        item.id = name.to_string();
        item.runtime_ticks = runtime_secs * mbv_core::api::TICKS_PER_SECOND;
        item
    }

    let mut app = make_queue_app(0);
    app.player_tab.set_items(
        vec![
            queue_item("Playing Film", 90),
            queue_item("Selected Film", 600),
        ],
        0,
    );
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.queue_len = 2;
        status.current_idx = 0;
        status.position_ticks = 45 * mbv_core::api::TICKS_PER_SECOND;
        status.runtime_ticks = 90 * mbv_core::api::TICKS_PER_SECOND;
    }
    // Warm-up frame: the queue panel settles its placement before the
    // assertion frame.
    render_queue_view_to_terminal(&mut app, 100, 40);
    let (term, _) = render_queue_view_to_terminal(&mut app, 100, 40);
    let buf = term.backend().buffer();
    assert!(
        !now_playing_cells(buf, "Playing Film").is_empty(),
        "the playing row starts as now-playing"
    );
    assert!(
        now_playing_cells(buf, "Selected Film").is_empty(),
        "the idle row starts without the now-playing colour"
    );
    assert!(
        frame_text(buf).contains("0:45 / 1:30"),
        "the panel starts on the playing item's live position"
    );

    app.dispatch(crate::app::action::Command::QueuePlayCursor(1));
    let (term, _) = render_queue_view_to_terminal(&mut app, 100, 40);
    let buf = term.backend().buffer();

    assert!(
        !now_playing_cells(buf, "Selected Film").is_empty(),
        "the selected row paints as now-playing before the owner confirms it"
    );
    let text = frame_text(buf);
    assert!(
        text.contains("Selected Film") && text.contains("0:00 / 10:00"),
        "the panel predicts the selection at a fresh start: {text}"
    );
    // The predicted row keeps its total duration: only live progress is
    // withheld until the owner confirms.
    let row_y = now_playing_cells(buf, "Selected Film")
        .first()
        .map(|(_, y)| *y)
        .expect("the selected row paints as now-playing");
    let row_line: String = (0..buf.area().width)
        .map(|x| buf[(x, row_y)].symbol().to_string())
        .collect();
    assert!(
        row_line.contains("10:00"),
        "the predicted row keeps its duration: {row_line:?}"
    );
}

/// Gutter-accent selection: the marker is a foam Nerd Font glyph
/// (U+F0BBA) in the selected row's leading gutter inside the panel; the
/// panel paints no marker outside the box edge; the selected title is
/// bold selected-row text inside the panel.
#[test]
fn queue_selection_title_is_bold_selected_row_no_outside_marker() {
    let mut app = make_queue_app(3);
    let (term, _) = render_queue_view_to_terminal(&mut app, 100, 40);
    let buf = term.backend().buffer();
    let chrome = app.compute_chrome_geometry(Rect::new(0, 0, 100, 40));
    let queue = chrome.root.queue.expect("queue panel placed");
    let box_area = super::arrangements::queue::queue_list_box(queue);
    // The cursor sits on item 0: the first content row (below the title
    // row when the box reserves one).
    let marker_y = super::arrangements::queue::queue_panel_subareas(box_area).y;
    // Nothing paints outside the recessed box edge.
    assert_eq!(
        buf[(box_area.x - 1, marker_y)].symbol(),
        " ",
        "no marker may paint outside the box edge"
    );
    // The selected title paints bold in the selected-row role.
    let title_x = box_area.x + 2;
    assert_eq!(buf[(title_x, marker_y)].fg, palette::TEXT_SELECTED_ROW);
    assert!(buf[(title_x, marker_y)].modifier.contains(Modifier::BOLD));

    // Without panel focus the title is neither accent nor bold.
    app.panel_focus = crate::app::PanelFocus::Library;
    let (term, _) = render_queue_view_to_terminal(&mut app, 100, 40);
    let buf = term.backend().buffer();
    assert_ne!(
        buf[(title_x, marker_y)].fg,
        palette::TEXT_SELECTED_ROW,
        "the unfocused queue title keeps the default colour"
    );
    assert!(!buf[(title_x, marker_y)].modifier.contains(Modifier::BOLD));
}

/// The QueueColumn footer: the status bar sits outside the recessed
/// queue-list box, with one gap row above and below it, as wide as the
/// QueueColumn header — on the QueueColumn surface, not the recessed panel.
#[test]
fn queue_status_bar_paints_as_a_column_footer_below_the_panel() {
    let mut app = make_queue_app(20);
    app.panel_mode = crate::app::PanelMode::Both;
    let (term, _) = render_queue_view_to_terminal(&mut app, 80, 30);
    let buf = term.backend().buffer();
    let chrome = app.compute_chrome_geometry(Rect::new(0, 0, 80, 30));
    let queue = chrome.root.queue.expect("queue panel placement");
    let footer_y = queue.bottom() - 2;
    // As wide as the QueueColumn header (the column's canonical inset).
    assert!(queue.width.saturating_sub(4) > 0);
    let band = palette::surface_colors(palette::Surface::QueuePanelBand, false).fill;
    let column = palette::surface_colors(palette::Surface::QueueColumn, chrome.queue_focused).fill;
    // The footer row carries the status band across the header width.
    for x in [queue.x + 2, queue.right() - 3] {
        assert_eq!(
            buf[(x, footer_y)].style().bg,
            Some(band),
            "the footer row paints the status band at ({x}, {footer_y})"
        );
    }
    // One QueueColumn gap row above and below the footer: the recessed box
    // ends above the upper gap.
    for (y, label) in [(footer_y - 1, "above"), (footer_y + 1, "below")] {
        assert_eq!(
            buf[(queue.x + 2, y)].style().bg,
            Some(column),
            "the gap row {label} the footer keeps the column surface"
        );
    }
    let panel_box = super::arrangements::queue::queue_list_box(queue);
    assert_eq!(
        panel_box.bottom() + 1,
        footer_y,
        "the recessed box ends one gap row above the footer"
    );
    assert_eq!(
        (panel_box.x, panel_box.width),
        (queue.x + 2, queue.width.saturating_sub(4)),
        "the footer spans the recessed box width (the header width)"
    );
}

/// A watched remote Session playing content the local queue does not hold
/// still paints the Now Playing panel with its transport: the observed remote
/// title, live progress, controls, and no local row wearing the playhead.
#[test]
fn attached_session_playing_foreign_content_paints_the_panel() {
    let mut app = make_queue_app(5);
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.connected_session_id = Some("sess-1".into());
    app.connected_session_state = Some({
        let mut s = make_session("remote-host", "Emby");
        s.now_playing = Some("Foreign Movie".into());
        s.now_playing_item_id = Some("not-in-local-queue".into());
        s.position_s = 30;
        s.runtime_s = 600;
        s.position_ticks = 30 * mbv_core::api::TICKS_PER_SECOND;
        s.runtime_ticks = 600 * mbv_core::api::TICKS_PER_SECOND;
        s
    });

    let (term, _layout) = render_queue_view_to_terminal(&mut app, 80, 40);
    let screen = buffer_to_string(&term);

    assert_eq!(
        app.effective_playback_state().active_idx,
        None,
        "no local slot backs the observed remote item"
    );
    assert!(
        app.layout.card.height > 0,
        "the visual slot is reserved while the transport is active"
    );
    assert!(
        screen.contains("PLAYING"),
        "the header states the observed transport:\n{screen}"
    );
    assert!(
        screen.contains("Foreign Movie"),
        "the title row carries the observed remote title:\n{screen}"
    );
    assert!(screen.contains('\u{2594}'), "the seekbar paints:\n{screen}");
}
