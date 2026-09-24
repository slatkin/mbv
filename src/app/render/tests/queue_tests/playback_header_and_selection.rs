use super::*;

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
    /// located by its play glyph. Unselected rows use Aqua; a selected row's
    /// marker uses the selected-row Ink foreground.
    fn now_playing_cells(buf: &ratatui::buffer::Buffer, title: &str) -> Vec<(u16, u16)> {
        let mut hits = Vec::new();
        for y in 0..buf.area().height {
            let has_icon = (0..buf.area().width).any(|x| {
                buf[(x, y)].symbol() == "▶"
                    && matches!(
                        buf[(x, y)].style().fg,
                        Some(palette::ACCENT) | Some(palette::SELECTED_ROW_FG)
                    )
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
        frame_text(buf).contains("0:45/1:30"),
        "the panel starts on the playing item's live position"
    );

    app.dispatch(crate::app::dispatch::action::Command::QueuePlayCursor(1));
    let (term, _) = render_queue_view_to_terminal(&mut app, 100, 40);
    let buf = term.backend().buffer();

    assert!(
        !now_playing_cells(buf, "Selected Film").is_empty(),
        "the selected row paints as now-playing before the owner confirms it"
    );
    let text = frame_text(buf);
    assert!(
        text.contains("Selected Film") && text.contains("0:00/10:00"),
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

/// Selected-row bar: the queue's selected row fills the panel with the bar
/// and keeps its ordinary title role (no accent title, no bold); the panel
/// paints nothing outside the recessed box edge.
#[test]
fn queue_selection_paints_the_bar_without_an_outside_marker() {
    let mut app = make_queue_app(3);
    let (term, _) = render_queue_view_to_terminal(&mut app, 100, 40);
    let buf = term.backend().buffer();
    let chrome = app.compute_chrome_geometry(Rect::new(0, 0, 100, 40));
    let queue = chrome.root.queue.expect("queue panel placed");
    let box_area = super::super::arrangements::queue::queue_list_box(queue);
    // The cursor sits on item 0: the first content row (below the title
    // row when the box reserves one).
    let marker_y = super::super::arrangements::queue::queue_panel_subareas(box_area).y;
    // Nothing paints outside the recessed box edge.
    assert_eq!(
        buf[(box_area.x - 1, marker_y)].symbol(),
        " ",
        "no marker may paint outside the box edge"
    );
    // The selected row paints the Iris bar with the selected-row Ink title,
    // not bold.
    let title_x = box_area.x + 2;
    assert_eq!(buf[(title_x, marker_y)].bg, palette::SELECTED_ROW_BG);
    assert_eq!(buf[(title_x, marker_y)].fg, palette::SELECTED_ROW_FG);
    assert!(!buf[(title_x, marker_y)].modifier.contains(Modifier::BOLD));

    // Without panel focus the row keeps the ordinary emphasis title and is
    // not bold. Its fill cannot prove the bar absent here: the bar shares the
    // queue box's resting fill value.
    app.panel_focus = crate::app::PanelFocus::Library;
    let (term, _) = render_queue_view_to_terminal(&mut app, 100, 40);
    let buf = term.backend().buffer();
    assert_eq!(buf[(title_x, marker_y)].fg, palette::TEXT_EMPHASIS);
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
    let panel_box = super::super::arrangements::queue::queue_list_box(queue);
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
