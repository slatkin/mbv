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

/// The header row's two sites: while the title lives on the artwork (the
/// `Artwork` site) the header paints the playing brand row — `PLAYING::[host]`
/// left — so the header panel stays visible and the title is not duplicated;
/// with the `Header` site the same projection paints the title there.
#[test]
fn header_row_paints_the_brand_row_while_the_title_lives_on_the_artwork() {
    let mut panel = QueuePlaybackPanel::new();
    panel.set_header(NowPlayingStatus::Playing, "Living Room".into(), false);
    panel.transport.now_playing_title =
        Some(("Example".into(), palette::Role::PlaybackValueFg.color()));
    panel.set_transport_area(Some(Rect::new(0, 2, 40, 6)));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    let header_row = |terminal: &Terminal<TestBackend>| {
        (2..38)
            .map(|x| terminal.backend().buffer()[(x, 1)].symbol().to_string())
            .collect::<String>()
    };

    // Header site: the header row carries the title.
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    assert!(
        header_row(&terminal).contains("Example"),
        "the Header site paints the title on the header row"
    );

    // Artwork site: the header row carries the playing brand row, host
    // included, and never the title (the artwork carries it).
    panel.transport.title_site = NowPlayingTitleSite::Artwork;
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    let row = header_row(&terminal);
    assert!(
        row.starts_with(" PLAYING::[Living Room]"),
        "the Artwork site paints the playing brand row, left-anchored: {row}"
    );
    assert!(
        !row.contains("[mbv]"),
        "the brand anchor is gone from the header row: {row}"
    );
    assert!(
        !row.contains("Example"),
        "the title lives on the artwork alone: {row}"
    );

    // The brackets paint foam and the local host cream (row index i is
    // buffer column i + 2: the header row is inset two columns).
    let bracket_at = row.find('[').expect("the host is bracketed");
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(u16::try_from(bracket_at + 2).unwrap(), 1)].fg,
        palette::Role::TextMetadata.color(),
        "the host brackets paint foam"
    );
    assert_eq!(
        buffer[(u16::try_from(bracket_at + 3).unwrap(), 1)].fg,
        palette::Role::TextEmphasis.color(),
        "the local host paints cream"
    );
}
