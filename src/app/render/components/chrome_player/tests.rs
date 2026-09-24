use super::*;
use mbv_core::playback_queue::{PlaybackTitlePart, PlaybackTitlePartRole, PlaybackTitleParts};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Locate `needle` in the painted row and assert every one of its cells
/// carries `expected` as the foreground.
fn assert_cells_in_row(row: &str, fgs: &[Color], needle: &str, expected: Color, label: &str) {
    let start = row
        .find(needle)
        .unwrap_or_else(|| panic!("{label} not painted in the row: {row:?}"));
    for (i, _) in needle.char_indices() {
        assert_eq!(
            fgs[start + i],
            expected,
            "cell {i} of {label} must carry its role's fg: {row:?}"
        );
    }
}

#[test]
fn unplayed_seekbar_uses_the_progress_track_role() {
    let mut playback = PlaybackStripAreas::default();
    let mut terminal = Terminal::new(TestBackend::new(4, 1)).unwrap();
    terminal
        .draw(|f| {
            render_seekbar(
                f,
                Rect::new(0, 0, 4, 1),
                &mut playback,
                (0, 100, false),
                Color::Black,
            )
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert!((0..4).all(|x| buffer[(x, 0)].fg == palette::PROGRESS_TRACK));
}

/// The role-to-colour resolution point (task 2.3): a two-part now-playing
/// row paints the title part's cells in the title role's fg and the
/// context part's cells in the context role's fg, both through the
/// painter's own `title_part_fg` mapping.
#[test]
fn title_part_roles_paint_their_theme_roles_in_the_row() {
    let parts = PlaybackTitleParts {
        title: PlaybackTitlePart {
            role: PlaybackTitlePartRole::Title,
            text: "Pilot".to_string(),
        },
        context: Some(PlaybackTitlePart {
            role: PlaybackTitlePartRole::Context,
            text: "Series".to_string(),
        }),
    };
    let mut playback = PlaybackStripAreas::default();
    let mut marquee_text = String::new();
    let mut marquee_started_at = std::time::Instant::now();
    let mut ctx = PlaybackRenderContext {
        area: Rect::new(0, 0, 60, 1),
        playback: &mut playback,
        player_h: 2,
        show_controls: true,
        now_playing_title: None,
        panel: palette::Surface::PlaybackPanel,
        panel_focused: false,
        progress: (0, 0, false),
        use_nerd_fonts: false,
        stop_available: false,
        next_available: false,
        prev_available: false,
        status_indicators: None,
        title_parts: Some(parts),
        idle_feed_title: None,
        marquee_text: &mut marquee_text,
        marquee_started_at: &mut marquee_started_at,
    };
    let mut terminal = Terminal::new(TestBackend::new(60, 1)).unwrap();
    terminal
        .draw(|f| {
            render_title_row(
                f,
                Rect::new(0, 0, 60, 1),
                "",
                palette::TEXT_STRONG,
                &mut ctx,
            )
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let row = (0..60)
        .map(|x| buf[(x, 0)].symbol().to_string())
        .collect::<String>();
    let fgs: Vec<Color> = (0..60).map(|x| buf[(x, 0)].fg).collect();
    assert_cells_in_row(
        &row,
        &fgs,
        "Pilot",
        title_part_fg(PlaybackTitlePartRole::Title),
        "the title part",
    );
    assert_cells_in_row(
        &row,
        &fgs,
        "Series",
        title_part_fg(PlaybackTitlePartRole::Context),
        "the context part",
    );
    assert!(
        row.find("Series") < row.find("Pilot"),
        "the context part (show) paints before the title part, not after: {row:?}"
    );
    assert_ne!(
        title_part_fg(PlaybackTitlePartRole::Title),
        title_part_fg(PlaybackTitlePartRole::Context),
        "the test locates the parts by their roles, so the roles must differ"
    );
}

/// The prev transport glyph paints in white (TEXT_STRONG) between stop
/// and next whenever the transport buttons show, and collapses exactly
/// with them when the row is too narrow (`show_buttons` gate).
#[test]
fn prev_glyph_paints_between_stop_and_next_and_collapses_with_next() {
    let mut playback = PlaybackStripAreas::default();
    let mut marquee_text = String::new();
    let mut marquee_started_at = std::time::Instant::now();
    let mut ctx = PlaybackRenderContext {
        area: Rect::new(0, 0, 60, 1),
        playback: &mut playback,
        player_h: 2,
        show_controls: true,
        now_playing_title: None,
        panel: palette::Surface::PlaybackPanel,
        panel_focused: false,
        progress: (0, 0, false),
        use_nerd_fonts: false,
        stop_available: true,
        next_available: true,
        prev_available: true,
        status_indicators: None,
        title_parts: None,
        idle_feed_title: None,
        marquee_text: &mut marquee_text,
        marquee_started_at: &mut marquee_started_at,
    };
    let mut terminal = Terminal::new(TestBackend::new(60, 1)).unwrap();
    terminal
        .draw(|f| {
            render_title_row(
                f,
                Rect::new(0, 0, 60, 1),
                "T",
                palette::TEXT_STRONG,
                &mut ctx,
            )
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let row: String = (0..60).map(|x| buf[(x, 0)].symbol().to_string()).collect();
    let stop = row.find("X").expect("stop glyph painted");
    let prev = row.find("<<").expect("prev glyph painted");
    let next = row.find(">>").expect("next glyph painted");
    assert!(
        stop < prev && prev < next,
        "prev must paint between stop and next: {row:?}"
    );
    assert_eq!(
        buf[(prev as u16, 0)].fg,
        palette::TEXT_STRONG,
        "prev paints white: {row:?}"
    );
    // Collapse: below the buttons-fit width, prev vanishes with the rest.
    let mut playback2 = PlaybackStripAreas::default();
    let mut marquee_text2 = String::new();
    let mut marquee_started_at2 = std::time::Instant::now();
    let mut narrow = PlaybackRenderContext {
        area: Rect::new(0, 0, 12, 1),
        playback: &mut playback2,
        player_h: 2,
        show_controls: true,
        now_playing_title: None,
        panel: palette::Surface::PlaybackPanel,
        panel_focused: false,
        progress: (0, 0, false),
        use_nerd_fonts: false,
        stop_available: true,
        next_available: true,
        prev_available: true,
        status_indicators: None,
        title_parts: None,
        idle_feed_title: None,
        marquee_text: &mut marquee_text2,
        marquee_started_at: &mut marquee_started_at2,
    };
    let mut terminal = Terminal::new(TestBackend::new(12, 1)).unwrap();
    terminal
        .draw(|f| {
            render_title_row(
                f,
                Rect::new(0, 0, 12, 1),
                "T",
                palette::TEXT_STRONG,
                &mut narrow,
            )
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let row: String = (0..12).map(|x| buf[(x, 0)].symbol().to_string()).collect();
    assert!(
        !row.contains("<<"),
        "prev collapses with the transport buttons: {row:?}"
    );
}

/// The roles survive the overflow marquee (task 4.3): a two-part title
/// wider than its slot marquees, and the scrolled window still paints the
/// title part's cells in the title role and the context part's cells in
/// the context role. The marquee start time is backdated into the
/// leading hold (column = 0), where the window shows the whole context
/// part followed by the title's head.
#[test]
fn the_marquee_window_keeps_both_part_roles() {
    let title_text = format!("{}Tail", "Long Episode ".repeat(5).trim_end());
    let context_text = "Show".to_string();
    let parts = PlaybackTitleParts {
        title: PlaybackTitlePart {
            role: PlaybackTitlePartRole::Title,
            text: title_text.clone(),
        },
        context: Some(PlaybackTitlePart {
            role: PlaybackTitlePartRole::Context,
            text: context_text.clone(),
        }),
    };
    let mut playback = PlaybackStripAreas::default();
    // Pre-seed the marquee state (the parts' concatenated text is the
    // marquee key) so the draw below keeps the backdated start time
    // instead of restarting the scroll.
    let mut marquee_text = format!("{context_text} {title_text}");
    // Backdate the start time into the leading hold (column = 0, hold
    // [0, HOLD)): the window then shows the whole context part followed
    // by the title's head. The strip's elapsed-only right side leaves
    // a 46-cell window on the 73-cell two-part title (overflow 27), so
    // the hold sits at [0, 600).
    let mut marquee_started_at = std::time::Instant::now() - std::time::Duration::from_millis(300);
    let mut ctx = PlaybackRenderContext {
        area: Rect::new(0, 0, 60, 1),
        playback: &mut playback,
        player_h: 2,
        show_controls: true,
        now_playing_title: None,
        panel: palette::Surface::PlaybackPanel,
        panel_focused: false,
        progress: (0, 0, false),
        use_nerd_fonts: false,
        stop_available: false,
        next_available: false,
        prev_available: false,
        status_indicators: None,
        title_parts: Some(parts),
        idle_feed_title: None,
        marquee_text: &mut marquee_text,
        marquee_started_at: &mut marquee_started_at,
    };
    let mut terminal = Terminal::new(TestBackend::new(60, 1)).unwrap();
    terminal
        .draw(|f| {
            render_title_row(
                f,
                Rect::new(0, 0, 60, 1),
                "",
                palette::TEXT_STRONG,
                &mut ctx,
            )
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let row: String = (0..60).map(|x| buf[(x, 0)].symbol().to_string()).collect();
    let fgs: Vec<Color> = (0..60).map(|x| buf[(x, 0)].fg).collect();
    // The marquee engaged: the two-part title is far wider than the
    // window the row can spend on it, so the full title never paints.
    assert!(
        !row.contains(&format!("{context_text} {title_text}")),
        "the two-part title must overflow the slot: {row:?}"
    );
    assert_cells_in_row(
        &row,
        &fgs,
        &context_text,
        title_part_fg(PlaybackTitlePartRole::Context),
        "the context part",
    );
    assert_cells_in_row(
        &row,
        &fgs,
        "Long Episode",
        title_part_fg(PlaybackTitlePartRole::Title),
        "the title part's head",
    );
}

/// The expanded title band (a context part and an `extra` row): the
/// show and the `pos / dur` time paint on the middle row — the show in
/// the context role, the time in the meta role — and the title alone on
/// the row below in the title role, with no time beside it.
#[test]
fn expanded_title_band_paints_show_and_time_above_the_title() {
    let parts = PlaybackTitleParts {
        title: PlaybackTitlePart {
            role: PlaybackTitlePartRole::Title,
            text: "Pilot".to_string(),
        },
        context: Some(PlaybackTitlePart {
            role: PlaybackTitlePartRole::Context,
            text: "Series".to_string(),
        }),
    };
    let mut playback = PlaybackStripAreas::default();
    let mut marquee_text = String::new();
    let mut marquee_started_at = std::time::Instant::now();
    let mut ctx = PlaybackRenderContext {
        area: Rect::new(0, 0, 40, 3),
        playback: &mut playback,
        player_h: 4,
        show_controls: true,
        now_playing_title: None,
        panel: palette::Surface::QueueOnlyPlaybackPanel,
        panel_focused: false,
        progress: (77 * mbv_core::api::TICKS_PER_SECOND, 0, false),
        use_nerd_fonts: false,
        stop_available: false,
        next_available: false,
        prev_available: false,
        status_indicators: None,
        title_parts: Some(parts),
        idle_feed_title: None,
        marquee_text: &mut marquee_text,
        marquee_started_at: &mut marquee_started_at,
    };
    let mut terminal = Terminal::new(TestBackend::new(40, 3)).unwrap();
    terminal
        .draw(|f| {
            render_queue_title_rows(
                f,
                Rect::new(0, 0, 40, 1),
                Rect::new(0, 1, 40, 1),
                Some(Rect::new(0, 2, 40, 1)),
                "",
                palette::TEXT_STRONG,
                &mut ctx,
            )
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let row = |y: u16| {
        (0..40)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect::<String>()
    };
    let fgs = |y: u16| (0..40).map(|x| buf[(x, y)].fg).collect::<Vec<Color>>();
    let show_row = row(0);
    let title_row = row(1);
    let controls_row = row(2);
    assert_cells_in_row(
        &show_row,
        &fgs(0),
        "Series",
        title_part_fg(PlaybackTitlePartRole::Context),
        "the show",
    );
    assert!(
        show_row.contains("1:17/0:00"),
        "the elapsed/duration time rides the show row: {show_row:?}"
    );
    assert!(
        !show_row.contains("Pilot"),
        "the title is not on the show row"
    );
    assert_cells_in_row(
        &title_row,
        &fgs(1),
        "Pilot",
        title_part_fg(PlaybackTitlePartRole::Title),
        "the title",
    );
    assert!(!title_row.contains("Series"), "the show stays on its row");
    assert!(!title_row.contains('/'), "no time on the title row");
    // The transport controls land on the band's bottom row.
    assert!(
        controls_row.contains('X'),
        "the stop glyph paints on the bottom row: {controls_row:?}"
    );
    assert!(!controls_row.contains("Pilot") && !controls_row.contains("Series"));
}

/// The queue column's bottom controls row pads the status pill on both
/// sides: the value (e.g. FLAC) must not touch the panel fill on the
/// right, mirroring the leading pad `status_pill_spans` opens with.
#[test]
fn bottom_controls_row_pads_the_status_pill_on_both_sides() {
    let pill_bg = palette::surface_colors(palette::Surface::PlaybackStatusPill, false).fill;
    let panel_bg = palette::surface_colors(palette::Surface::QueueOnlyPlaybackPanel, false).fill;
    assert_ne!(
        pill_bg, panel_bg,
        "the test locates the pill by its fill, so the fills must differ"
    );
    let mut playback = PlaybackStripAreas::default();
    let mut marquee_text = String::new();
    let mut marquee_started_at = std::time::Instant::now();
    let mut ctx = PlaybackRenderContext {
        area: Rect::new(0, 0, 40, 2),
        playback: &mut playback,
        player_h: 3,
        show_controls: true,
        now_playing_title: None,
        panel: palette::Surface::QueueOnlyPlaybackPanel,
        panel_focused: false,
        progress: (0, 0, false),
        use_nerd_fonts: false,
        stop_available: false,
        next_available: false,
        prev_available: false,
        status_indicators: Some(vec![Span::raw("CODEC "), Span::raw("FLAC")]),
        title_parts: None,
        idle_feed_title: None,
        marquee_text: &mut marquee_text,
        marquee_started_at: &mut marquee_started_at,
    };
    let mut terminal = Terminal::new(TestBackend::new(40, 2)).unwrap();
    terminal
        .draw(|f| {
            render_queue_title_rows(
                f,
                Rect::new(0, 0, 40, 1),
                Rect::new(0, 1, 40, 1),
                None,
                "Title",
                palette::TEXT_STRONG,
                &mut ctx,
            )
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let pill_cells = (0..40)
        .filter(|&x| buf[(x, 1)].bg == pill_bg)
        .collect::<Vec<_>>();
    assert!(
        !pill_cells.is_empty(),
        "no pill painted on the controls row"
    );
    let row_text = (0..40)
        .map(|x| buf[(x, 1)].symbol().to_string())
        .collect::<String>();
    assert!(
        row_text.contains("FLAC"),
        "expected the codec value: {row_text:?}"
    );
    assert_eq!(
        buf[(pill_cells[0], 1)].symbol(),
        " ",
        "leading pill pad: {row_text:?}"
    );
    assert_eq!(
        buf[(*pill_cells.last().unwrap(), 1)].symbol(),
        " ",
        "trailing pill pad: {row_text:?}"
    );
    assert_eq!(
        *pill_cells.last().unwrap(),
        38,
        "the padded pill runs flush to the inset row edge: {row_text:?}"
    );
}

/// The pill owns the padding on both sides: the keyvalue cluster carries
/// no outer space of its own, so the painted pill is exactly
/// ` FHD ⧸ EN ⧸ CC ` — one pad each side, never two on the right.
#[test]
fn status_pill_pads_the_keyvalue_cluster_once_on_each_side() {
    use super::super::indicators::{indicator_spans, IndicatorData, IndicatorStyle};
    let pill_bg = palette::surface_colors(palette::Surface::PlaybackStatusPill, false).fill;
    let panel_bg = palette::surface_colors(palette::Surface::PlaybackPanel, false).fill;
    assert_ne!(
        pill_bg, panel_bg,
        "the test locates the pill by its fill, so the fills must differ"
    );
    let cluster = indicator_spans(
        IndicatorStyle::KeyValue,
        &IndicatorData {
            res_label: "FHD".into(),
            res_dim: false,
            audio_label: "en".into(),
            audio_dim: false,
            audio_only: false,
            sub_label: "CC".into(),
            sub_on: false,
        },
        false,
    );
    let mut playback = PlaybackStripAreas::default();
    let mut marquee_text = String::new();
    let mut marquee_started_at = std::time::Instant::now();
    let mut ctx = PlaybackRenderContext {
        area: Rect::new(0, 0, 40, 1),
        playback: &mut playback,
        player_h: 3,
        show_controls: true,
        now_playing_title: None,
        panel: palette::Surface::PlaybackPanel,
        panel_focused: false,
        progress: (0, 0, false),
        use_nerd_fonts: false,
        stop_available: false,
        next_available: false,
        prev_available: false,
        status_indicators: Some(cluster),
        title_parts: None,
        idle_feed_title: None,
        marquee_text: &mut marquee_text,
        marquee_started_at: &mut marquee_started_at,
    };
    let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
    terminal
        .draw(|f| {
            render_title_row(
                f,
                Rect::new(0, 0, 40, 1),
                "Title",
                palette::TEXT_STRONG,
                &mut ctx,
            )
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let pill: String = (0..40)
        .filter(|&x| buf[(x, 0)].bg == pill_bg)
        .map(|x| buf[(x, 0)].symbol())
        .collect();
    assert_eq!(pill, " FHD ⧸ EN ⧸ CC ", "padded pill content");
}
