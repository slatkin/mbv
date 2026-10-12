use super::*;
use mbv_ui_model::playback::NowPlayingTitleSite;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tuirealm::event::KeyModifiers;

/// Pointer input resolves against the panel's retained geometry: play/pause,
/// a fractional seek, and prev/next each resolve to their own intent, while
/// the time labels flanking the bar span resolve nothing.
#[test]
fn transport_clicks_resolve_against_retained_geometry() {
    let mut panel = QueuePlaybackPanel::new();
    panel.set_header(NowPlayingStatus::Playing, String::new(), false);
    panel.transport.now_playing_title =
        Some(("Example".into(), palette::Role::PlaybackValueFg.color()));
    panel.transport.show_controls = true;
    // 37.5s of 5:00 lands mid-cell: the leading partial cell is a seek target
    // like any other column.
    panel.transport.state.position_ticks = 375_000_000;
    panel.transport.state.runtime_ticks = 300 * mbv_emby_model::TICKS_PER_SECOND;
    panel.set_transport_area(Some(Rect::new(0, 2, 40, 4)));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();

    let (play_pause, seekbar) = panel.transport_hits();
    let click = |column: u16, row: u16| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    };
    assert!(matches!(
        panel.on(&click(play_pause.x + 1, play_pause.y)),
        Some(Msg::Playback(PlaybackRequest::TogglePlayPause))
    ));
    let partial_column = seekbar.x + 3;
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

/// The header row's two sites: the row always paints the target pill (icon
/// in ink on the yellow local fill — a local target shows no host) and the
/// bold status word; while the header is the title's home (the `Header`
/// site) the title rides the remaining space; with the `Artwork` site the
/// title lives on the artwork alone and the row paints the status part
/// only.
#[test]
fn header_row_paints_the_status_brand_and_the_header_site_title() {
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

    // Header site: the status part leads and the title rides the row.
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    let row = header_row(&terminal);
    assert!(
        row.starts_with(" \u{f0118}  PLAYING "),
        "the header paints the local pill and status word, left-anchored: {row}"
    );
    assert!(
        !row.contains("Living Room"),
        "a local target paints no host: {row}"
    );
    assert!(
        row.contains("Example"),
        "the Header site paints the title on the header row: {row}"
    );

    // Artwork site: the title lives on the artwork alone.
    panel.transport.title_site = NowPlayingTitleSite::Artwork;
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    let row = header_row(&terminal);
    assert!(
        row.starts_with(" \u{f0118}  PLAYING "),
        "the Artwork site paints the status part, left-anchored: {row}"
    );
    assert!(
        !row.contains("Example"),
        "the title lives on the artwork alone: {row}"
    );

    // The pill paints ink-on-yellow: the icon cell (row char index 1, the
    // pill's second cell) carries the TargetPillFg foreground on the yellow
    // fill; the status word outside the pill keeps the panel fill (row char
    // index i is buffer column i + 2: the header row is inset two columns).
    let buffer = terminal.backend().buffer();
    let icon_cell = buffer[(3, 1)].clone();
    assert_eq!(
        (icon_cell.fg, icon_cell.bg),
        (
            palette::Role::TargetPillFg.color(),
            palette::surface_colors(palette::Surface::TargetPillLocal, false).fill
        ),
        "the pill's icon paints ink on the yellow local fill"
    );
    let word_cell = buffer[(
        u16::try_from(" \u{f0118}  ".chars().count() + 2).unwrap(),
        1,
    )]
        .clone();
    assert_eq!(
        (word_cell.symbol().to_string(), word_cell.bg),
        (
            "P".to_string(),
            palette::surface_colors(palette::Surface::QueueOnlyPlaybackPanel, false).fill
        ),
        "the status word outside the pill keeps the panel fill"
    );
}

/// The target pill carries the host name only while a remote host owns
/// playback: ` 󰄙 Living Room ` on the green fill. The whole pill is the
/// F3 sessions shortcut's click target; a click elsewhere on the header
/// row resolves nothing.
#[test]
fn the_target_pill_carries_the_remote_host_and_opens_sessions_on_click() {
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
        row.starts_with(" \u{f0119} Living Room   PLAYING"),
        "the remote pill carries the host on the green fill: {row}"
    );
    let icon_cell = terminal.backend().buffer()[(3, 1)].clone();
    assert_eq!(
        (icon_cell.fg, icon_cell.bg),
        (
            palette::Role::TargetPillFg.color(),
            palette::surface_colors(palette::Surface::TargetPillRemote, false).fill
        ),
        "the pill's icon paints ink on the green remote fill"
    );

    // The whole pill is the F3 sessions shortcut's click target.
    let pill = panel.target_pill_area_for_test();
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
            panel.on(&click(pill.x + pill.width - 1, pill.y)),
            Some(Msg::Shell(request)) if matches!(*request, ShellRequest::OpenSessions)
        ),
        "a click on the pill's last cell opens Sessions"
    );
    assert!(
        panel.on(&click(pill.x + pill.width, pill.y)).is_none(),
        "a click just outside the pill resolves nothing"
    );
}
