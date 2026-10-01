use super::*;
use mbv_queue::{PlaybackTitlePart, PlaybackTitlePartRole, PlaybackTitleParts};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use rstest::rstest;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

/// One painted band row: its text and each cell's foreground and
/// background.
type PaintedRow = (String, Vec<Color>, Vec<Color>);

/// The header row plus the band's four rows.
type BandRows = (PaintedRow, PaintedRow, PaintedRow, PaintedRow, PaintedRow);

/// The media-type families of the requirements table, as the projection
/// carries them (title part, optional context part). The mapping itself is
/// core's table (task 1.1); these fixtures pin the painted behaviour per
/// media type — on the queue column's split lower title row, the same
/// content the Library strip paints.
fn parts_for(title: &str, context: Option<&str>) -> PlaybackTitleParts {
    PlaybackTitleParts {
        title: PlaybackTitlePart {
            role: PlaybackTitlePartRole::Title,
            text: title.to_string(),
        },
        context: context.map(|text| PlaybackTitlePart {
            role: PlaybackTitlePartRole::Context,
            text: text.to_string(),
        }),
    }
}

/// Paint the panel and return the header row plus the band's rows
/// (y 1..y 5), each as (text, fgs, bgs). The header row (y 1) carries
/// the now-playing title while a target plays. The band is always four
/// rows: the controls on y 2, the former title row kept blank on y 3,
/// the seekbar flanked by its times on y 4, one blank row on y 5.
fn painted_band_rows(parts: PlaybackTitleParts) -> BandRows {
    let mut panel = QueuePlaybackPanel::new();
    panel.set_header(NowPlayingStatus::Playing, "music-box".into(), false);
    panel.transport.show_controls = true;
    panel.transport.now_playing_title = Some(("Fallback".into(), palette::PLAYBACK_VALUE_FG));
    panel.transport.title_parts = Some(parts);
    panel.set_transport_area(Some(Rect::new(0, 2, 40, 4)));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    let buf = terminal.backend().buffer();
    let row = |y: u16| {
        (
            (0..40).map(|x| buf[(x, y)].symbol().to_string()).collect(),
            (0..40).map(|x| buf[(x, y)].fg).collect(),
            (0..40).map(|x| buf[(x, y)].bg).collect(),
        )
    };
    (row(1), row(2), row(3), row(4), row(5))
}

/// A blank band row's contract (the former title row and the gap row):
/// no text, panel fill edge to edge.
fn assert_row_blank(text: &str, bgs: &[Color]) {
    assert!(text.trim().is_empty(), "the row carries no text: {text:?}");
    let panel_bg = palette::surface_colors(palette::Surface::QueueOnlyPlaybackPanel, false).fill;
    assert!(
        bgs.iter().all(|bg| *bg == panel_bg),
        "the row keeps the panel fill: {text:?}"
    );
}

/// The top controls row's contract: the slate fill edge to edge.
fn assert_controls_row_fill(text: &str, bgs: &[Color]) {
    assert!(
        bgs.iter().all(|bg| *bg == palette::SURFACE_BACKDROP),
        "the controls row sits on the slate fill edge to edge: {text:?}"
    );
}

fn assert_cells_carry(text: &str, fgs: &[Color], needle: &str, expected: Color, label: &str) {
    let start = text
        .find(needle)
        .unwrap_or_else(|| panic!("{label} not painted in the row: {text:?}"));
    for (i, _) in needle.char_indices() {
        assert_eq!(
            fgs[start + i],
            expected,
            "cell {i} of {label} must carry its role's fg: {text:?}"
        );
    }
}

/// The painted media-type table (tasks 4.1, 4.2, 4.4) on the queue
/// column's header row: the now-playing title rides the header while a
/// target plays. A two-part title shares the row — the context part
/// (the show) left-aligned in the yellow context role, the title part
/// right-aligned in the aqua title role, neither carrying a time. A
/// single title paints yellow. The band keeps the controls on top, the
/// former title row blank, the seekbar, and the gap row. (The Library
/// strip's combined row keeps its own contract, owned by
/// `chrome_player.rs`'s painter test.)
fn band_for(title: &str, context: Option<&str>) -> BandRows {
    painted_band_rows(parts_for(title, context))
}

#[rstest]
#[case::emby_movie("Movie Name", None)]
#[case::emby_episode("Pilot", Some("Series"))]
fn band_keeps_slate_controls_blank_title_row_and_blank_gap_row(
    #[case] title: &str,
    #[case] context: Option<&str>,
) {
    let (_, (first, _, first_bgs), (title_row, _, title_row_bgs), _, (gap, _, gap_bgs)) =
        band_for(title, context);
    assert_controls_row_fill(&first, &first_bgs);
    assert_row_blank(&title_row, &title_row_bgs);
    assert_row_blank(&gap, &gap_bgs);
}

#[rstest]
#[case::emby_movie("Movie Name", None)]
#[case::emby_episode("Pilot", Some("Series"))]
fn seek_row_carries_no_title(#[case] title: &str, #[case] context: Option<&str>) {
    let (_, _, _, (seek, _, _), _) = band_for(title, context);
    assert!(
        !seek.contains(title),
        "the title stays off the seek row: {seek:?}"
    );
}

#[rstest]
#[case::emby_movie("Movie Name", None)]
#[case::emby_episode("Pilot", Some("Series"))]
fn controls_row_carries_the_stop_glyph_and_neither_title_nor_time(
    #[case] title: &str,
    #[case] context: Option<&str>,
) {
    let (_, (controls, _, _), _, _, _) = band_for(title, context);
    assert!(
        controls.contains('X'),
        "the stop glyph paints on the controls row: {controls:?}"
    );
    assert!(
        !controls.contains(title) && !controls.contains("0:00"),
        "no title or time on the controls row: {controls:?}"
    );
}

#[test]
fn two_part_header_paints_context_and_title_in_their_roles() {
    let ((header, fgs, _), ..) = band_for("Pilot", Some("Series"));
    assert_cells_carry(
        &header,
        &fgs,
        "Series",
        palette::PLAYBACK_CONTEXT_FG,
        "the context part",
    );
    assert_cells_carry(
        &header,
        &fgs,
        "Pilot",
        palette::PLAYBACK_TITLE_FG,
        "the title part",
    );
}

#[test]
fn two_part_header_hugs_both_indents_and_carries_no_time() {
    let ((header, _, _), ..) = band_for("Pilot", Some("Series"));
    assert!(
        header.starts_with("   > Series"),
        "the aqua play icon leads and the show hugs the left indent: {header:?}"
    );
    assert!(
        header.ends_with("Pilot   "),
        "the title hugs the right indent (one plus two inset cells): {header:?}"
    );
    assert!(
        !header.contains('/'),
        "no time rides the shared header row: {header:?}"
    );
}

/// Paint the panel at the artwork title site and return the header row's
/// text plus each cell's fg and modifier.
fn artwork_site_header(host: &str, host_is_remote: bool) -> (String, Vec<Color>, Vec<Modifier>) {
    let mut panel = QueuePlaybackPanel::new();
    panel.set_header(NowPlayingStatus::Playing, host.into(), host_is_remote);
    panel.transport.now_playing_title = Some(("Example".into(), palette::PLAYBACK_VALUE_FG));
    panel.transport.title_parts = Some(parts_for("Pilot", Some("Series")));
    panel.transport.title_site = mbv_ui_model::playback::NowPlayingTitleSite::Artwork;
    panel.set_transport_area(Some(Rect::new(0, 2, 40, 4)));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    let buf = terminal.backend().buffer();
    let text = (0..40).map(|x| buf[(x, 1)].symbol().to_string()).collect();
    let fgs = (0..40).map(|x| buf[(x, 1)].fg).collect();
    let mods = (0..40).map(|x| buf[(x, 1)].modifier).collect();
    (text, fgs, mods)
}

#[test]
fn artwork_title_site_paints_the_mbv_brand_row() {
    let (text, fgs, mods) = artwork_site_header("music-box", false);
    // The header row sits inside the queue column's two-column inset.
    let row = &text[2..38];
    assert!(
        row.starts_with(" [mbv]"),
        "the brand hugs the left edge: {text:?}"
    );
    assert!(
        row.ends_with("PLAYING:music-box "),
        "the status and host hug the right edge: {text:?}"
    );
    assert_cells_carry(
        &text,
        &fgs,
        "[",
        palette::TEXT_EMPHASIS,
        "the opening bracket",
    );
    assert_cells_carry(&text, &fgs, "mbv", palette::TEXT_METADATA, "the brand");
    assert_cells_carry(
        &text,
        &fgs,
        "]",
        palette::TEXT_EMPHASIS,
        "the closing bracket",
    );
    assert_cells_carry(&text, &fgs, "PLAYING", palette::ACCENT, "the status word");
    assert_cells_carry(&text, &fgs, ":", palette::TEXT_EMPHASIS, "the colon");
    assert_cells_carry(
        &text,
        &fgs,
        "music-box",
        palette::TEXT_EMPHASIS,
        "the local host",
    );
    let playing = text.find("PLAYING").unwrap();
    assert!(
        mods[playing].contains(Modifier::BOLD),
        "the status word paints bold: {text:?}"
    );
    let brand = text.find("mbv").unwrap();
    assert!(
        !mods[brand].contains(Modifier::BOLD),
        "the brand paints unbold: {text:?}"
    );
}

#[test]
fn artwork_title_site_remote_host_carries_the_remote_role() {
    let (text, fgs, _) = artwork_site_header("tv-box", true);
    let row = &text[2..38];
    assert!(
        row.starts_with(" [mbv]") && row.ends_with("PLAYING:tv-box "),
        "the brand row reads `[mbv]` left and `PLAYING:<host>` right: {text:?}"
    );
    assert_cells_carry(
        &text,
        &fgs,
        "tv-box",
        palette::PLAYBACK_HOST_REMOTE_FG,
        "the remote host",
    );
}

#[test]
fn artwork_title_site_paints_no_title_parts() {
    let (text, _, _) = artwork_site_header("music-box", false);
    assert!(
        !text.contains("Pilot") && !text.contains("Series"),
        "{text:?}"
    );
}

#[test]
fn single_title_header_paints_the_context_role_behind_the_play_icon() {
    let ((header, fgs, _), ..) = band_for("Movie Name", None);
    assert!(
        header.starts_with("   > Movie Name"),
        "the play icon leads the lone title: {header:?}"
    );
    assert!(
        !header.contains('/'),
        "no time rides the header row: {header:?}"
    );
    assert_cells_carry(
        &header,
        &fgs,
        "Movie Name",
        palette::PLAYBACK_CONTEXT_FG,
        "the lone title",
    );
}

#[test]
fn header_carries_the_title_while_playing_and_idle_status_when_not() {
    // While a target plays the header row paints the aqua play icon and
    // the now-playing title (no status word, no host); paused swaps in
    // the yellow pause icon; idle paints the brand row, `[mbv] ... IDLE`.
    let painted = |status: NowPlayingStatus, paused: bool| {
        let mut panel = QueuePlaybackPanel::new();
        panel.set_header(status, "music-box".into(), false);
        panel.transport.now_playing_title = Some(("Example".into(), palette::PLAYBACK_VALUE_FG));
        panel.transport.state.paused = paused;
        panel.set_transport_area(Some(Rect::new(0, 2, 40, 4)));
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
            .unwrap();
        let buf = terminal.backend().buffer();
        let text = (0..40)
            .map(|x| buf[(x, 1)].symbol().to_string())
            .collect::<String>();
        let fgs: Vec<Color> = (0..40).map(|x| buf[(x, 1)].fg).collect();
        (text, fgs)
    };
    let (playing, fgs) = painted(NowPlayingStatus::Playing, false);
    assert!(
        playing.starts_with("   > Example"),
        "the aqua play icon leads the title while playing: {playing:?}"
    );
    assert!(
        !playing.contains("PLAYING") && !playing.contains("music-box"),
        "no status word or host beside the title: {playing:?}"
    );
    assert_eq!(
        fgs[3],
        palette::ACCENT,
        "the play icon paints aqua: {playing:?}"
    );
    let (paused, fgs) = painted(NowPlayingStatus::Paused, true);
    assert!(
        paused.starts_with("   || Example"),
        "the pause icon replaces the play icon while paused: {paused:?}"
    );
    assert_eq!(
        fgs[3],
        palette::TEXT_FOCUS_ACCENT,
        "the pause icon paints yellow: {paused:?}"
    );
    let (idle, idle_fgs) = painted(NowPlayingStatus::Idle, false);
    let idle_row = &idle[2..38];
    assert!(
        idle_row.starts_with(" [mbv]") && idle_row.ends_with("IDLE "),
        "idle paints the brand row with a right-anchored IDLE: {idle:?}"
    );
    assert!(!idle.contains("music-box"), "idle drops the host: {idle:?}");
    assert_cells_carry(
        &idle,
        &idle_fgs,
        "IDLE",
        palette::TEXT_MUTED,
        "the idle status word",
    );
}

#[test]
fn controls_row_paints_indicators_as_spaced_plain_text() {
    // A keyvalue-style cluster (slash separators) plus one chip-style
    // span: separators drop, items join with one space, no pill fill
    // anywhere on the row, and the chip text recovers its fill colour.
    use ratatui::style::{Modifier, Style};
    use ratatui::text::Span;
    let mut panel = QueuePlaybackPanel::new();
    panel.set_header(NowPlayingStatus::Playing, "music-box".into(), false);
    panel.transport.now_playing_title = Some(("Example".into(), palette::PLAYBACK_VALUE_FG));
    panel.transport.show_controls = true;
    panel.transport.status_indicators = Some(vec![
        Span::styled(
            "FHD",
            Style::default()
                .fg(palette::INDICATOR_RESOLUTION_FG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ⧸ ", Style::default().fg(palette::TEXT_MUTED)),
        Span::styled(
            " FLAC ",
            Style::default()
                .fg(palette::TEXT_ON_ACCENT)
                .bg(palette::INDICATOR_AUDIO_FG),
        ),
        Span::styled(" ⧸ ", Style::default().fg(palette::TEXT_MUTED)),
        Span::styled("en", Style::default().fg(palette::TEXT_SECONDARY)),
    ]);
    panel.set_transport_area(Some(Rect::new(0, 2, 40, 4)));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
        .unwrap();
    let buf = terminal.backend().buffer();
    let text: String = (0..40).map(|x| buf[(x, 2)].symbol().to_string()).collect();
    let fgs: Vec<Color> = (0..40).map(|x| buf[(x, 2)].fg).collect();
    let bgs: Vec<Color> = (0..40).map(|x| buf[(x, 2)].bg).collect();
    assert!(
        text.contains("FHD FLAC EN "),
        "items single-spaced and uppercased: {text:?}"
    );
    assert!(!text.contains('⧸'), "no slash separators: {text:?}");
    assert!(
        bgs.iter().all(|bg| *bg == palette::SURFACE_BACKDROP),
        "no pill fill anywhere on the row: {text:?}"
    );
    // Each item keeps its own foreground: res orange, the chip's mauve
    // fill recovered as text, lang secondary.
    let start = text.find("FHD").unwrap();
    assert!(
        fgs[start..start + 3]
            .iter()
            .all(|fg| *fg == palette::INDICATOR_RESOLUTION_FG),
        "the res label keeps its colour: {text:?}"
    );
    let start = text.find("FLAC").unwrap();
    assert!(
        fgs[start..start + 4]
            .iter()
            .all(|fg| *fg == palette::INDICATOR_AUDIO_FG),
        "the chip text recovers its fill colour: {text:?}"
    );
    let start = text.find(" EN ").unwrap() + 1;
    assert!(
        fgs[start..start + 2]
            .iter()
            .all(|fg| *fg == palette::TEXT_SECONDARY),
        "the lang label keeps its colour: {text:?}"
    );
}

#[test]
fn transport_clicks_resolve_against_retained_geometry() {
    let mut panel = QueuePlaybackPanel::new();
    panel.set_header(NowPlayingStatus::Playing, "music-box".into(), false);
    panel.transport.now_playing_title = Some(("Example".into(), palette::PLAYBACK_VALUE_FG));
    panel.transport.show_controls = true;
    // 37.5s of 5:00 is 3.5 of the 28-cell bar: the leading half cell is a
    // partial-cell seek target like any other column.
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
    // Either flanking time label resolves no seek intent.
    assert!(
        panel.on(&click(seekbar.x - 3, seekbar.y)).is_none(),
        "the elapsed label never seeks"
    );
    assert!(
        panel.on(&click(seekbar.right() + 1, seekbar.y)).is_none(),
        "the total label never seeks"
    );
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
    let _ = KeyEvent::new(Key::Null, KeyModifiers::NONE);
}
