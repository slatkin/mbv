use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tuirealm::event::KeyModifiers;

/// Pointer input resolves against the panel's retained geometry: play/pause,
/// a fractional seek, and prev/next each resolve to their own intent, while
/// the time labels flanking the bar span resolve nothing.
#[test]
fn transport_clicks_resolve_against_retained_geometry() {
    let mut panel = QueuePlaybackPanel::new();
    panel.set_header(NowPlayingStatus::Playing);
    panel.transport.now_playing_title = Some(("Example".into(), palette::PLAYBACK_VALUE_FG));
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

/// The panel's painted header row text (placement row 1, `queue_panel_inset`).
fn header_row_text(terminal: &Terminal<TestBackend>) -> String {
    let buf = terminal.backend().buffer();
    (0..40).map(|x| buf[(x, 1)].symbol().to_string()).collect()
}

/// Row 3.2 (design D1/D3): the header row paints iff the shell's projected
/// flag; no playing state paints the removed artwork-site brand row
/// (`[mbv] ... PLAYING:<host>`).
#[test]
fn header_row_paints_iff_the_projected_flag() {
    let mut panel = QueuePlaybackPanel::new();
    panel.set_header(NowPlayingStatus::Playing);
    panel.transport.now_playing_title = Some(("Example".into(), palette::PLAYBACK_VALUE_FG));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();

    panel.transport.header_visible = HeaderVisibility::Hidden;
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    let hidden = header_row_text(&terminal);
    assert!(
        !hidden.contains("Example"),
        "a hidden header paints nothing"
    );
    assert!(
        !hidden.contains("[mbv]"),
        "a playing state paints no brand row"
    );
    assert!(
        !hidden.contains("PLAYING"),
        "a playing state paints no brand word"
    );

    panel.transport.header_visible = HeaderVisibility::Visible;
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    let shown = header_row_text(&terminal);
    assert!(
        shown.contains("Example"),
        "a visible header carries the title"
    );
    assert!(
        !shown.contains("[mbv]"),
        "the playing header is not a brand row"
    );
    assert!(
        !shown.contains("PLAYING"),
        "the playing header carries no brand word"
    );
}
