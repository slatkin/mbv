use super::test_helpers::buffer_to_string;
use crate::app::palette;
use crate::app::tests::{make_app_stub, make_session};
use mbv_core::api::SessionInfo;
use mbv_core::cast_discovery::CastReceiver;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Terminal;

fn render_sessions(width: u16, height: u16, selected: bool, loading: bool) -> String {
    let mut app = make_app_stub();
    app.sessions_loading = loading;
    if !loading {
        app.sessions = vec![make_session("Living Room", "Emby")];
    }
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let targets = crate::app::panel_targets::build_panel_targets(&app.sessions, &[]);
            let mut cursor = usize::from(selected);
            let mut scroll = 0;
            crate::app::render::render_sessions_overlay_content(
                f,
                Some(Rect::new(0, 0, width, height)),
                &targets,
                app.sessions_loading,
                &mut cursor,
                &mut scroll,
                None,
                None,
                false,
            );
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn sessions_none_fallback_paints_the_fullscreen_shell() {
    let mut app = make_app_stub();
    app.sessions = vec![make_session("Living Room", "Emby")];
    let width = 40;
    let height = 12;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut panel_area = None;
    terminal
        .draw(|f| {
            let targets = crate::app::panel_targets::build_panel_targets(&app.sessions, &[]);
            let mut cursor = 0;
            let mut scroll = 0;
            panel_area = Some(
                crate::app::render::render_sessions_overlay_content(
                    f,
                    None,
                    &targets,
                    false,
                    &mut cursor,
                    &mut scroll,
                    None,
                    None,
                    false,
                )
                .0,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(panel_area, Some(Rect::new(0, 0, width, height)));
    assert_eq!(
        buffer[(0, 0)].bg,
        palette::surface_colors(palette::Surface::SidebarBody, false).fill
    );
    assert_eq!(
        buffer[(2, 1)].bg,
        palette::surface_colors(palette::Surface::SidebarBand, false).fill
    );
    assert_eq!(buffer[(3, 1)].symbol(), "R");
    assert_eq!(buffer[(width - 1, 2)].symbol(), " ");
}

#[test]
fn sessions_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, selected, loading) in [
        (50, 12, false, false),
        (50, 12, true, false),
        (18, 8, true, false),
        (30, 8, false, true),
    ] {
        let output = render_sessions(width, height, selected, loading);
        assert!(
            output.contains("REMOTE"),
            "sessions shell missing: {output:?}"
        );
    }
}

/// Renders one merged target list at a fixed size and returns the terminal
/// plus the card rect the painter reports for each row (the row's own
/// geometry, so the coverage assertions below are not pinned to the panel's
/// column arithmetic).
fn render_targets(
    sessions: Vec<SessionInfo>,
    cast_receivers: &[CastReceiver],
    connected_session_id: Option<&str>,
    cast_attachment_id: Option<&str>,
) -> (Terminal<TestBackend>, Vec<(Rect, usize)>) {
    let width = 50u16;
    let height = 12u16;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut cards = Vec::new();
    terminal
        .draw(|f| {
            let targets = crate::app::panel_targets::build_panel_targets(&sessions, cast_receivers);
            let mut cursor = 0;
            let mut scroll = 0;
            cards = crate::app::render::render_sessions_overlay_content(
                f,
                Some(Rect::new(0, 0, width, height)),
                &targets,
                false,
                &mut cursor,
                &mut scroll,
                connected_session_id,
                cast_attachment_id,
                false,
            )
            .1;
        })
        .unwrap();
    (terminal, cards)
}

/// The first cell on `row` holding `symbol`: content-located, so a row's
/// lines can be asserted without naming their column.
fn cell_x(buf: &Buffer, row: u16, symbol: &str) -> u16 {
    (0..buf.area.width)
        .find(|&x| buf[(x, row)].symbol() == symbol)
        .unwrap_or_else(|| panic!("no {symbol:?} painted on row {row}"))
}

fn panel_body_fill() -> Color {
    palette::surface_colors(palette::Surface::SidebarBody, false).fill
}

fn assert_card_bg_uniform(buf: &Buffer, card: Rect, want: Color) {
    for y in card.y..card.y + card.height {
        for x in card.x..card.x + card.width {
            assert_eq!(buf[(x, y)].bg, want, "card cell ({x}, {y})");
        }
    }
}

#[test]
fn a_connected_session_row_paints_the_iris_bar_with_the_bars_own_ink_text() {
    let (terminal, cards) = render_targets(
        vec![make_session("Living Room", "Emby")],
        &[],
        Some("sess-1"),
        None,
    );
    let buf = terminal.backend().buffer();
    let card = cards[0].0;
    assert_card_bg_uniform(buf, card, palette::SELECTED_ROW_BG);
    assert_eq!(
        buf[(card.x, card.y + card.height)].bg,
        panel_body_fill(),
        "the divider row under the card stays panel background"
    );
    assert_eq!(
        buf[(cell_x(buf, card.y, "L"), card.y)].fg,
        palette::SELECTED_ROW_FG,
        "the title takes the bar's own foreground"
    );
    assert_eq!(
        buf[(cell_x(buf, card.y + 1, "E"), card.y + 1)].fg,
        palette::SELECTED_ROW_FG,
        "the meta line takes the bar's own foreground"
    );
    assert_eq!(
        buf[(cell_x(buf, card.y + 2, "■"), card.y + 2)].fg,
        palette::SELECTED_ROW_FG,
        "the state line takes the bar's own foreground"
    );
}

#[test]
fn a_session_row_without_a_connection_keeps_the_panel_background() {
    let (terminal, cards) =
        render_targets(vec![make_session("Living Room", "Emby")], &[], None, None);
    let buf = terminal.backend().buffer();
    let card = cards[0].0;
    assert_card_bg_uniform(buf, card, panel_body_fill());
    assert_eq!(
        buf[(cell_x(buf, card.y, "L"), card.y)].fg,
        palette::ACCENT_ACTIVE,
        "the unconnected row keeps its ordinary selection colour"
    );
}

#[test]
fn an_attached_cast_receiver_row_takes_the_same_iris_bar() {
    let receiver = CastReceiver {
        id: "cast-1".to_string(),
        friendly_name: "Living Room".to_string(),
        host: "192.168.0.5".to_string(),
        port: 8009,
    };
    let (terminal, cards) = render_targets(vec![], &[receiver], None, Some("cast-1"));
    let buf = terminal.backend().buffer();
    let card = cards[0].0;
    assert_card_bg_uniform(buf, card, palette::SELECTED_ROW_BG);
    assert_eq!(
        buf[(cell_x(buf, card.y, "L"), card.y)].fg,
        palette::SELECTED_ROW_FG,
        "the attached row's title takes the bar's own foreground"
    );
}
