use super::test_helpers::*;
use super::*;
use crate::app::layout::LayoutPlayback;
use crate::app::tests::make_app_stub;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn surface_fill(surface: palette::Surface, focused: bool) -> ratatui::style::Color {
    palette::surface_colors_for_column_focus(surface, focused).fill
}

fn queue_only_fill(surface: palette::Surface) -> ratatui::style::Color {
    palette::surface_colors(
        surface,
        &crate::app::layout::FocusState::queue_only_for_test(false),
    )
    .fill
}

#[test]
fn title_row_next_area_matches_rendered_next_glyph_width_and_position() {
    let mut app = make_app_stub();
    app.use_nerd_fonts = false;
    let next_glyph = ">>";
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 2;
        st.current_idx = 0;
        st.runtime_ticks = 90 * TICKS_PER_SECOND;
    }

    let backend = TestBackend::new(60, 1);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutPlayback::default();
    term.draw(|f| {
        let mut context = app.playback_panel_context(
            Rect::new(0, 0, 60, 1),
            &mut layout,
            1,
            true,
            &Some((
                "Title".into(),
                surface_fill(palette::Surface::PlaybackPanel, true),
            )),
            surface_fill(palette::Surface::PlaybackPanel, false),
        );
        render_title_row(
            f,
            Rect::new(0, 0, 60, 1),
            "Title",
            surface_fill(palette::Surface::PlaybackPanel, true),
            &mut context,
        );
    })
    .unwrap();

    let line = buffer_to_string(&term).lines().next().unwrap().to_string();
    let next_byte = line.find(next_glyph).unwrap();
    let next_x = line[..next_byte].width() as u16;

    assert_eq!(layout.next_area.x, next_x);
    assert_eq!(layout.next_area.width, next_glyph.width() as u16);
}

#[test]
fn title_row_next_area_matches_nerd_font_glyph_width_and_position() {
    let mut app = make_app_stub();
    app.use_nerd_fonts = true;
    let next_glyph = "\u{f051}";
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 2;
        st.current_idx = 0;
        st.runtime_ticks = 90 * TICKS_PER_SECOND;
    }

    let backend = TestBackend::new(60, 1);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutPlayback::default();
    term.draw(|f| {
        let mut context = app.playback_panel_context(
            Rect::new(0, 0, 60, 1),
            &mut layout,
            1,
            true,
            &Some((
                "Title".into(),
                surface_fill(palette::Surface::PlaybackPanel, true),
            )),
            surface_fill(palette::Surface::PlaybackPanel, false),
        );
        render_title_row(
            f,
            Rect::new(0, 0, 60, 1),
            "Title",
            surface_fill(palette::Surface::PlaybackPanel, true),
            &mut context,
        );
    })
    .unwrap();

    let line = buffer_to_string(&term).lines().next().unwrap().to_string();
    let next_byte = line.find(next_glyph).unwrap();
    let next_x = line[..next_byte].width() as u16;

    assert_eq!(layout.next_area.x, next_x);
    assert_eq!(layout.next_area.width, next_glyph.width() as u16);
}

/// `remove-migrated-surface-underpaint` 3.9 (D4): the right-column player
/// chrome is painted solely by the mounted `PlaybackComponent`. The legacy
/// base frame (`App::render`) still reserves `player_area` as the placement
/// hand-off, but paints no seekbar or transport row there. Mirrors
/// `wide_movies_legacy_base_frame_publishes_geometry_but_paints_no_rows`.
#[test]
fn player_chrome_legacy_base_frame_publishes_geometry_but_paints_no_panel() {
    let mut app = make_movie_app();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 1;
        st.current_idx = 0;
        st.runtime_ticks = 90 * TICKS_PER_SECOND;
    }

    let terminal = render_app_to_terminal(&mut app, 100, 20);

    let player_area = app.layout.playback.player_area;
    assert!(
        player_area.height > 0 && player_area.width > 0,
        "player_area must still be reserved for the component: {player_area:?}"
    );
    let buf = terminal.backend().buffer();
    for y in player_area.y..player_area.y + player_area.height {
        for x in player_area.x..player_area.x + player_area.width {
            assert_eq!(
                buf[(x, y)].symbol().trim(),
                "",
                "legacy base frame painted into the player panel at ({x}, {y})"
            );
        }
    }
}

/// `unify-surface-colour` section 1 (D5): the row directly above the pill bar
/// belongs to the library column, not the playback panel. In the wide layouts
/// the panel paints only its three content rows; the bottom row shows the
/// column's chrome backdrop — `SURFACE_FOCUSED` while the library holds focus,
/// `SURFACE_BACKDROP` once the queue takes it. The panel's own rows keep their
/// existing fill in both states.
#[test]
fn wide_panel_bottom_row_follows_the_library_column_surface() {
    fn render(panel_focus: crate::app::PanelFocus) -> (Terminal<TestBackend>, Rect) {
        let mut app = make_app_stub();
        app.panel_mode = crate::app::PanelMode::Both;
        app.panel_focus = panel_focus;
        app.terminal_width = 120;
        app.use_nerd_fonts = false;
        {
            let mut st = app.player.status.lock().unwrap();
            st.active = true;
            st.queue_len = 1;
            st.current_idx = 0;
            st.runtime_ticks = 90 * TICKS_PER_SECOND;
        }
        let mut model = crate::app::shell::Model::new(app);
        let mut terminal = Terminal::new(TestBackend::new(120, 20)).unwrap();
        terminal
            .draw(|f| model.app.compose_base_frame(f, None))
            .unwrap();
        model.sync_playback();
        terminal
            .draw(|f| {
                model.app.compose_base_frame(f, None);
                model.render_playback_component(f);
            })
            .unwrap();
        let area = model.app.layout.playback.player_area;
        (terminal, area)
    }

    let (terminal, area) = render(crate::app::PanelFocus::Library);
    assert!(
        area.height >= 4,
        "wide Both reserves the full player box: {area:?}"
    );
    let buf = terminal.backend().buffer();
    assert_eq!(
        buf[(area.x, area.y)].bg,
        surface_fill(palette::Surface::PlaybackPanel, false),
        "the panel's seekbar row keeps its fill while the library holds focus"
    );
    assert_eq!(
        buf[(area.x, area.y + 2)].bg,
        surface_fill(palette::Surface::PlaybackPanel, false),
        "the panel's blank row keeps its fill while the library holds focus"
    );
    assert_eq!(
        buf[(area.x, area.y + 3)].bg,
        surface_fill(palette::Surface::LibraryColumn, true),
        "the row above the pill bar follows the focused library column"
    );
    // The only assertion that fails if the focused and resting surface
    // constants ever collapse to one value — the failure class this change
    // exists to expose.
    assert_ne!(
        buf[(area.x, area.y + 3)].bg,
        surface_fill(palette::Surface::LibraryColumn, false),
        "the focused library column must light that row up"
    );

    let (terminal, area) = render(crate::app::PanelFocus::Queue);
    let buf = terminal.backend().buffer();
    assert_eq!(
        buf[(area.x, area.y)].bg,
        surface_fill(palette::Surface::PlaybackPanel, true),
        "the queue-focused panel content row keeps its fill"
    );
    assert_eq!(
        buf[(area.x, area.y + 1)].bg,
        surface_fill(palette::Surface::PlaybackPanel, true),
        "the queue-focused panel title row keeps its fill"
    );
    assert_eq!(
        buf[(area.x, area.y + 2)].bg,
        surface_fill(palette::Surface::PlaybackPanel, true),
        "the queue-focused panel blank row keeps its fill"
    );
    assert_eq!(
        buf[(area.x, area.y + 3)].bg,
        surface_fill(palette::Surface::LibraryColumn, false),
        "the row above the pill bar rests with the unfocused library column"
    );
}

#[test]
fn narrow_queue_only_panel_puts_title_on_bottom_now_playing_row() {
    let mut app = make_app_stub();
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.terminal_width = 120; // >= MINI_VIEW_THRESHOLD, so stored panel_mode applies
    app.use_nerd_fonts = false;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 1;
        st.current_idx = 0;
        st.runtime_ticks = 60 * TICKS_PER_SECOND;
    }

    let backend = TestBackend::new(60, 5);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutPlayback::default();
    term.draw(|f| {
        render_player_panel(
            f,
            app.playback_panel_context(
                Rect::new(0, 0, 60, 5),
                &mut layout,
                4,
                true,
                &Some(("My Title".to_string(), palette::TEXT_STRONG)),
                queue_only_fill(palette::Surface::PlaybackPanel),
            ),
        );
    })
    .unwrap();

    let text = buffer_to_string(&term);
    let lines: Vec<&str> = text.lines().collect();
    // Title row (y+1) must NOT contain the title.
    assert!(
        !lines[1].contains("My Title"),
        "title row held title:\n{}",
        lines[1]
    );
    // Bottom row (y+3) must carry the prefixed title.
    assert!(
        lines[3].contains("On Now: My Title"),
        "bottom row:\n{}",
        lines[3]
    );
}

#[test]
fn narrow_now_playing_row_indents_and_marquees_a_long_title() {
    let mut app = make_app_stub();
    app.panel_mode = crate::app::PanelMode::QueueOnly;
    app.terminal_width = 120;
    app.use_nerd_fonts = false;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.queue_len = 1;
        st.current_idx = 0;
        st.runtime_ticks = 60 * TICKS_PER_SECOND;
    }
    let long_title = "A Very Long Album Title That Cannot Possibly Fit";

    let backend = TestBackend::new(30, 5);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutPlayback::default();
    term.draw(|f| {
        render_player_panel(
            f,
            app.playback_panel_context(
                Rect::new(0, 0, 30, 5),
                &mut layout,
                4,
                true,
                &Some((long_title.to_string(), palette::TEXT_STRONG)),
                queue_only_fill(palette::Surface::PlaybackPanel),
            ),
        );
    })
    .unwrap();

    let text = buffer_to_string(&term);
    let lines: Vec<&str> = text.lines().collect();
    let bottom = lines[3];
    // Indent: the row's first and last columns stay blank rather than
    // butting text against the panel edges.
    assert_eq!(
        bottom.chars().next(),
        Some(' '),
        "no left indent:\n{bottom}"
    );
    assert_eq!(
        bottom.chars().last(),
        Some(' '),
        "no right indent:\n{bottom}"
    );
    // Marquee: freshly opened (still in its initial hold), the window shows
    // the start of the label rather than being hard-truncated with "...".
    assert!(
        bottom.contains("On Now: A Very"),
        "expected marquee start of label:\n{bottom}"
    );
    assert!(
        !bottom.contains('\u{2026}'),
        "should not ellipsis-truncate marquee text:\n{bottom}"
    );

    // Advance the marquee clock past its initial hold, into the scroll.
    // The "On Now: " prefix must stay put -- only the title pans.
    app.marquee_started_at =
        std::time::Instant::now() - std::time::Duration::from_millis(1200 + 200 * 5);
    let mut term2 = Terminal::new(TestBackend::new(30, 5)).unwrap();
    term2
        .draw(|f| {
            render_player_panel(
                f,
                app.playback_panel_context(
                    Rect::new(0, 0, 30, 5),
                    &mut layout,
                    4,
                    true,
                    &Some((long_title.to_string(), palette::TEXT_STRONG)),
                    queue_only_fill(palette::Surface::PlaybackPanel),
                ),
            );
        })
        .unwrap();
    let text2 = buffer_to_string(&term2);
    let bottom2: &str = text2.lines().collect::<Vec<_>>()[3];
    assert!(
        bottom2.trim().starts_with("On Now:"),
        "prefix must stay fixed while title scrolls:\n{bottom2}"
    );
    assert!(
        !bottom2.contains("On Now: A Very"),
        "title window should have scrolled past its start:\n{bottom2}"
    );
}

#[test]
fn standard_title_row_showcases_instead_of_truncating_a_long_title() {
    let mut app = make_app_stub();
    let long_title = "A Very Long Album Title That Cannot Possibly Fit In This Row";
    let mut layout = LayoutPlayback::default();

    let render = |app: &mut crate::app::App, layout: &mut LayoutPlayback| -> String {
        let backend = TestBackend::new(30, 1);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let mut context = app.playback_panel_context(
                Rect::new(0, 0, 30, 1),
                layout,
                1,
                true,
                &Some((long_title.to_string(), palette::TEXT_STRONG)),
                queue_only_fill(palette::Surface::PlaybackPanel),
            );
            render_title_row(
                f,
                Rect::new(0, 0, 30, 1),
                long_title,
                palette::TEXT_STRONG,
                &mut context,
            );
        })
        .unwrap();
        buffer_to_string(&term).lines().next().unwrap().to_string()
    };

    let first = render(&mut app, &mut layout);
    assert!(
        !first.contains('\u{2026}'),
        "should showcase, not ellipsis-truncate:\n{first}"
    );
    assert!(
        first.contains("A Very Long"),
        "expected the start of the title at rest:\n{first}"
    );

    // Advance the shared marquee clock past its initial hold.
    app.marquee_started_at =
        std::time::Instant::now() - std::time::Duration::from_millis(1200 + 200 * 5);
    let later = render(&mut app, &mut layout);
    assert!(
        !later.contains('\u{2026}'),
        "should showcase, not ellipsis-truncate:\n{later}"
    );
    assert_ne!(first, later, "title window should have scrolled");
}

#[test]
fn idle_feed_title_marquees_instead_of_truncating() {
    use crate::app::types_feed::{IdleFeed, IdleFeedItem};
    use std::sync::mpsc;

    let mut app = make_app_stub();
    let (items_tx, items_rx) = mpsc::channel();
    app.idle_feed = Some(IdleFeed {
        items: vec![IdleFeedItem {
            title: "A Very Long Novara Media Episode Title That Cannot Fit".to_string(),
            link: Some("https://example.com/ep".to_string()),
        }],
        current_index: 0,
        last_rotation: std::time::Instant::now(),
        last_fetch: std::time::Instant::now(),
        items_tx,
        items_rx,
    });

    let render = |app: &mut crate::app::App, layout: &mut LayoutPlayback| -> String {
        let backend = TestBackend::new(30, 4);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            render_player_panel(
                f,
                app.playback_panel_context(
                    Rect::new(0, 0, 30, 4),
                    layout,
                    4,
                    false, // !show_controls => idle state
                    &None,
                    queue_only_fill(palette::Surface::PlaybackPanel),
                ),
            );
        })
        .unwrap();
        buffer_to_string(&term).lines().nth(1).unwrap().to_string()
    };

    let mut layout = LayoutPlayback::default();
    let first = render(&mut app, &mut layout);
    assert!(
        !first.contains('\u{2026}'),
        "should marquee, not ellipsis-truncate:\n{first}"
    );
    assert!(
        first.contains("A Very Long"),
        "expected the start of the title at rest:\n{first}"
    );

    // Advance the shared marquee clock past its initial hold.
    app.marquee_started_at =
        std::time::Instant::now() - std::time::Duration::from_millis(1200 + 200 * 5);
    let later = render(&mut app, &mut layout);
    assert!(
        !later.contains('\u{2026}'),
        "should marquee, not ellipsis-truncate:\n{later}"
    );
    assert_ne!(first, later, "title window should have scrolled");
}
