use super::*;
use crate::mouse::seekbar::SEEKBAR_SCRUB_INTERVAL;
use mbv_ui_model::playback::NowPlayingTitleSite;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tuirealm::event::KeyModifiers;

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event<UserEvent> {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

/// A left press at the cell.
fn click(column: u16, row: u16) -> Event<UserEvent> {
    mouse(MouseEventKind::Down(MouseButton::Left), column, row)
}

/// A left-button motion at the cell: the second half of a drag gesture.
fn motion(column: u16, row: u16) -> Event<UserEvent> {
    mouse(MouseEventKind::Drag(MouseButton::Left), column, row)
}

/// A panel with a live transport painted into a 40x4 band, so the retained
/// hit geometry is the geometry this frame painted. 37.5s of 5:00 lands
/// mid-cell: the leading partial cell is a seek target like any other column.
fn painted_panel() -> QueuePlaybackPanel {
    let mut panel = QueuePlaybackPanel::new();
    panel.set_header(NowPlayingStatus::Playing, String::new(), false);
    panel.transport.now_playing_title =
        Some(("Example".into(), palette::Role::PlaybackValueFg.color()));
    panel.transport.show_controls = true;
    panel.transport.state.position_ticks = 375_000_000;
    panel.transport.state.runtime_ticks = 300 * mbv_emby_model::TICKS_PER_SECOND;
    panel.set_transport_area(Some(Rect::new(0, 2, 40, 4)));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    panel
}

/// Pointer input resolves against the panel's retained geometry: play/pause,
/// a fractional seek, and prev/next each resolve to their own intent, while
/// the time labels flanking the bar span resolve nothing.
#[test]
fn transport_clicks_resolve_against_retained_geometry() {
    let mut panel = painted_panel();
    let (play_pause, seekbar) = panel.transport_hits();
    assert!(matches!(
        panel.on(&click(play_pause.x + 1, play_pause.y)),
        Some(Msg::Playback(PlaybackRequest::TogglePlayPause))
    ));
    let partial_column = seekbar.x + 3;
    // The bar needs a double-click: the press that opens the pair resolves
    // nothing, so a stray single click cannot jump the position.
    assert!(
        panel.on(&click(partial_column, seekbar.y)).is_none(),
        "the first press of a double-click resolves no seek"
    );
    assert!(matches!(
        panel.on(&click(partial_column, seekbar.y)),
        Some(Msg::Playback(PlaybackRequest::SeekTo(f)))
            if (f - 3.0 / f64::from(seekbar.width)).abs() < 1e-9
    ));
    // The time labels flank the bar span and resolve no seek intent: the
    // columns just outside the bar span resolve nothing.
    assert!(panel.on(&click(seekbar.x - 1, seekbar.y)).is_none());
    assert!(panel.on(&click(seekbar.right(), seekbar.y)).is_none());
    // Prev and next keep distinct painted rects and resolve to their own
    // intents (the prev control was painted without a hit rect until now).
    panel.transport.availability.previous = true;
    panel.transport.availability.next = true;
    let (prev, next) = panel.transport_nav_hits();
    assert!(prev.width > 0 && next.width > 0);
    assert!(prev.right() <= next.x, "prev={prev:?} next={next:?}");
    assert!(matches!(
        panel.on(&click(prev.x, prev.y)),
        Some(Msg::Playback(PlaybackRequest::Previous))
    ));
    assert!(matches!(
        panel.on(&click(next.x, next.y)),
        Some(Msg::Playback(PlaybackRequest::Next))
    ));
    // A collapsed panel (no transport painted) resolves nothing.
    panel.set_transport_area(None);
    assert!(panel.on(&click(5, 4)).is_none());
}

/// A seekbar scrub admits one seek per throttle interval, and only for a
/// drag that anchored inside the bar span: a drag that merely crosses the
/// bar is not a scrub of it. The throttle records the outcome of the interval
/// passing, so the test never sleeps.
#[test]
fn seekbar_scrub_needs_an_in_bar_anchor_and_one_seek_per_interval() {
    let mut panel = painted_panel();
    let (_, seekbar) = panel.transport_hits();
    let drag = |panel: &mut QueuePlaybackPanel, from: u16, to: u16| {
        // The press arms the drag anchor and resolves nothing on the bar.
        let _ = panel.on(&click(from, seekbar.y));
        panel.on(&motion(to, seekbar.y))
    };

    let target = seekbar.x + seekbar.width / 2;
    assert!(
        matches!(
            drag(&mut panel, seekbar.x, target),
            Some(Msg::Playback(PlaybackRequest::SeekTo(f))) if (f - 0.5).abs() < 1e-6
        ),
        "an in-bar drag scrubs to the pointer column"
    );
    assert!(
        drag(&mut panel, seekbar.x + 1, target + 1).is_none(),
        "a second scrub inside the interval is throttled"
    );
    panel.scrub_at = Instant::now().checked_sub(SEEKBAR_SCRUB_INTERVAL);
    assert!(
        drag(&mut panel, seekbar.x + 2, target + 1).is_some(),
        "a scrub after the interval is admitted again"
    );
    assert!(
        drag(&mut panel, seekbar.x - 1, target).is_none(),
        "a drag that anchored outside the bar never scrubs"
    );
}

/// The visual slot's right-click gesture: the panel reports the image anchor
/// to the shell, and resolves nothing outside the projected slot rectangle,
/// or when no rectangle is projected at all (idle, hidden, or absent).
#[test]
fn visual_slot_right_click_reports_the_image_anchor() {
    let mut panel = QueuePlaybackPanel::new();
    panel.set_visual_slot_area(Some(Rect::new(4, 2, 20, 8)));
    let right_click = |column: u16, row: u16| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    };

    assert_eq!(
        panel.on(&right_click(5, 3)),
        Some(Msg::Shell(Box::new(ShellRequest::VisualSlotContextMenu {
            x: 5,
            y: 3,
        })))
    );
    assert!(panel.on(&right_click(0, 3)).is_none());
    assert!(panel.on(&right_click(5, 0)).is_none());
    panel.set_visual_slot_area(None);
    assert!(panel.on(&right_click(5, 3)).is_none());
}

/// The header row's two sites: the row always paints the playback-target
/// indicator (the target icon in the target colour — yellow while local,
/// green while remote, a local target shows no host) and the right-aligned
/// bold status word; while the header is the title's home (the `Header`
/// site) the title fills the zone between them; with the `Artwork` site the
/// title lives on the artwork alone and the row paints the indicator and the
/// right-aligned status only.
#[test]
fn header_row_paints_the_target_status_and_the_header_site_title() {
    let mut panel = QueuePlaybackPanel::new();
    panel.set_header(NowPlayingStatus::Playing, "Living Room".into(), false);
    panel.transport.use_nerd_fonts = true;
    panel.transport.now_playing_title =
        Some(("Example".into(), palette::Role::PlaybackValueFg.color()));
    panel.set_transport_area(Some(Rect::new(0, 2, 40, 6)));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    let header_row = |terminal: &Terminal<TestBackend>| {
        (2..38)
            .map(|x| terminal.backend().buffer()[(x, 1)].symbol().to_string())
            .collect::<String>()
    };

    // Header site: the local icon leads, the title fills the zone, and the
    // status word closes the row right-aligned.
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    let row = header_row(&terminal);
    assert!(
        row.starts_with(" \u{f0118} Example"),
        "the header paints interior padding, the local icon, the title, then the status: {row}"
    );
    assert!(
        row.ends_with("PLAYING "),
        "the status word is right-aligned inside the right padding: {row}"
    );
    assert!(
        !row.contains("Living Room"),
        "a local target paints no host: {row}"
    );

    // Artwork site: the title lives on the artwork alone, and the status word
    // still closes the row right-aligned.
    panel.transport.title_site = NowPlayingTitleSite::Artwork;
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    let row = header_row(&terminal);
    assert!(
        row.starts_with(" \u{f0118} "),
        "the Artwork site paints the padded local icon left-anchored: {row}"
    );
    assert!(
        row.ends_with("PLAYING "),
        "the Artwork site keeps the status word right-aligned: {row}"
    );
    assert!(
        !row.contains("Example"),
        "the title lives on the artwork alone: {row}"
    );

    // The target icon paints the local target colour on the panel fill; the
    // right-aligned status word keeps the panel fill too. The header row is
    // inset two columns and carries one cell of interior padding each side,
    // so the icon is the first content cell and the status word is the seven
    // cells before the trailing padding.
    let buffer = terminal.backend().buffer();
    let icon_cell = buffer[(3, 1)].clone();
    assert_eq!(
        (icon_cell.fg, icon_cell.bg),
        (
            palette::Role::PlaybackTargetLocalFg.color(),
            palette::surface_colors(palette::Surface::QueueOnlyPlaybackPanel, false).fill
        ),
        "the local target icon paints the target colour on the panel fill"
    );
    let word_cell = buffer[(30, 1)].clone();
    assert_eq!(
        (word_cell.symbol().to_string(), word_cell.bg),
        (
            "P".to_string(),
            palette::surface_colors(palette::Surface::QueueOnlyPlaybackPanel, false).fill
        ),
        "the right-aligned status word keeps the panel fill"
    );
}

/// The playback-target indicator carries the host name only while a remote
/// host owns playback: the green target icon, a space, the plain host name.
/// The whole indicator is the F3 sessions shortcut's click target; a click
/// elsewhere on the header row resolves nothing.
#[test]
fn the_target_indicator_carries_the_remote_host_and_opens_sessions_on_click() {
    let mut panel = QueuePlaybackPanel::new();
    panel.set_header(NowPlayingStatus::Playing, "Living Room".into(), true);
    panel.transport.use_nerd_fonts = true;
    panel.set_transport_area(None);
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    let row: String = (2..38)
        .map(|x| terminal.backend().buffer()[(x, 1)].symbol().to_string())
        .collect();
    assert!(
        row.starts_with(" \u{f0119} Living Room"),
        "the remote indicator carries the plain host after the green icon: {row}"
    );
    assert!(
        row.ends_with("PLAYING "),
        "the status word is right-aligned inside the right padding: {row}"
    );
    let icon_cell = terminal.backend().buffer()[(3, 1)].clone();
    assert_eq!(
        (icon_cell.fg, icon_cell.bg),
        (
            palette::Role::PlaybackTargetRemoteFg.color(),
            palette::surface_colors(palette::Surface::QueueOnlyPlaybackPanel, false).fill
        ),
        "the remote target icon paints the target colour on the panel fill"
    );

    // The whole playback-target indicator is the F3 sessions shortcut's
    // click target.
    let target = panel.target_area_for_test();
    let click = |column: u16, row: u16| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    };
    assert!(
        matches!(
            panel.on(&click(target.x + target.width - 1, target.y)),
            Some(Msg::Shell(request)) if matches!(*request, ShellRequest::OpenSessions)
        ),
        "a click on the indicator's last cell opens Sessions"
    );
    assert!(
        panel
            .on(&click(target.x + target.width, target.y))
            .is_none(),
        "a click just outside the indicator resolves nothing"
    );
}
