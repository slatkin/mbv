use super::*;
use mbv_ui_model::ui_util::continue_tab_title;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use rstest::rstest;

/// 2026-10-05 user rule: the gap between adjacent painted tab titles is two
/// columns. Each tab paints one lead space, its title, and the marker
/// column, so one tab's marker cell plus the next tab's lead space make the
/// two-space gap, whatever the selection.
#[test]
fn adjacent_tab_titles_are_two_columns_apart() {
    let titles = vec![
        "alpha".to_string(),
        "bravo".to_string(),
        "charlie".to_string(),
    ];
    let markers = [false, false, false];
    let model = TabBarModel {
        titles: &titles,
        markers: &markers,
        selected: 0,
        scroll: 0,
        hovered: None,
    };
    let mut hits = Vec::new();
    let mut terminal = Terminal::new(TestBackend::new(60, 3)).unwrap();
    terminal
        .draw(|f| render_tab_bar(f, Rect::new(0, 0, 60, 3), &model, &mut hits))
        .unwrap();

    let row: String = (0..60)
        .map(|x| terminal.backend().buffer()[(x, 1)].symbol().to_string())
        .collect();
    let alpha_end = row.find("ALPHA").unwrap() + "ALPHA".len();
    let bravo_start = row.find("BRAVO").unwrap();
    assert_eq!(
        &row[alpha_end..bravo_start],
        "  ",
        "selected tab to next tab is a two-space gap"
    );
    let bravo_end = bravo_start + "BRAVO".len();
    let charlie_start = row.find("CHARLIE").unwrap();
    assert_eq!(
        &row[bravo_end..charlie_start],
        "  ",
        "unselected tab to unselected tab is a two-space gap"
    );
}

/// 2026-10-05 user rule: selecting a tab must not move any tab text. Every
/// tab paints exactly `title + 2` cells — lead cell (space, or the accent
/// bar on the selected tab), title, marker — so the hit rect of every tab
/// sits at the same columns whatever the selection.
#[test]
fn selection_does_not_move_the_tab_titles() {
    let titles = vec!["alpha".to_string(), "bravo".to_string()];
    let markers = [false, false];
    let painted_x = |selected: usize| {
        let model = TabBarModel {
            titles: &titles,
            markers: &markers,
            selected,
            scroll: 0,
            hovered: None,
        };
        let mut hits = Vec::new();
        let mut terminal = Terminal::new(TestBackend::new(40, 3)).unwrap();
        terminal
            .draw(|f| render_tab_bar(f, Rect::new(0, 0, 40, 3), &model, &mut hits))
            .unwrap();
        hits.into_iter()
            .map(|(rect, position)| (position, rect.x))
            .collect::<Vec<_>>()
    };

    assert_eq!(painted_x(0), painted_x(1));
}

/// Render a two-tab bar at `Rect(0, 0, 40, 3)` with `selected` selected;
/// returns the terminal and the painted hit regions.
fn rendered(selected: usize) -> (Terminal<TestBackend>, Vec<(Rect, usize)>) {
    let titles = vec!["alpha".to_string(), "bravo".to_string()];
    let markers = [false, false];
    let model = TabBarModel {
        titles: &titles,
        markers: &markers,
        selected,
        scroll: 0,
        hovered: None,
    };
    let mut hits = Vec::new();
    let mut terminal = Terminal::new(TestBackend::new(40, 3)).unwrap();
    terminal
        .draw(|f| render_tab_bar(f, Rect::new(0, 0, 40, 3), &model, &mut hits))
        .unwrap();
    (terminal, hits)
}

/// 2026-10-05 user rule: the selected tab carries an eighth-block run in
/// the underline role's colour, spanning exactly its title run — its lead
/// and marker cells and every unselected tab's columns stay bare. The
/// icon-only Home tab is the exception to this rule (2026-10-09 user rule);
/// see `the_selected_home_icon_tab_paints_no_block_runs_and_a_mauve_icon`.
#[rstest]
#[case::below_the_title("▔", 2)]
#[case::above_the_title("▁", 0)]
fn the_selected_tab_gets_an_eighth_block_run_around_its_title(
    #[case] glyph: &str,
    #[case] row: u16,
) {
    let (terminal, hits) = rendered(0);
    let (rect, _) = hits[0];
    let buf = terminal.backend().buffer();
    let title_run = (rect.x + 1)..(rect.x + rect.width - 1);
    for x in title_run {
        let cell = buf[(x, row)].clone();
        assert_eq!(cell.symbol(), glyph, "column {x} carries the run");
        assert_eq!(cell.style().fg, Some(palette::TAB_SELECTED_UNDERLINE));
    }
    for x in rect.x..rect.x + rect.width {
        if !((rect.x + 1)..(rect.x + rect.width - 1)).contains(&x) {
            assert_ne!(
                buf[(x, row)].symbol(),
                glyph,
                "the lead and marker cells stay bare (column {x})"
            );
        }
    }
    let (bravo, _) = hits[1];
    for x in bravo.x..bravo.x + bravo.width {
        assert_ne!(
            buf[(x, row)].symbol(),
            glyph,
            "an unselected tab is not underlined (column {x})"
        );
    }
}

/// 2026-10-09 user rule: the icon-only Home tab is the exception to the
/// selected eighth-block runs — when it is selected, no block run is painted
/// above or below, and the house icon itself carries the underline role's
/// Mauve as its active colour, in either glyph variant (Nerd Font house,
/// Unicode house fallback).
#[rstest]
#[case::nerd_house(continue_tab_title(true))]
#[case::unicode_house(continue_tab_title(false))]
fn the_selected_home_icon_tab_paints_no_block_runs_and_a_mauve_icon(#[case] home_title: &str) {
    let titles = vec![home_title.to_string(), "alpha".to_string()];
    let markers = [false, false];
    let model = TabBarModel {
        titles: &titles,
        markers: &markers,
        selected: 0,
        scroll: 0,
        hovered: None,
    };
    let mut hits = Vec::new();
    let mut terminal = Terminal::new(TestBackend::new(40, 3)).unwrap();
    terminal
        .draw(|f| render_tab_bar(f, Rect::new(0, 0, 40, 3), &model, &mut hits))
        .unwrap();
    let buf = terminal.backend().buffer();
    for row in [0u16, 2u16] {
        for x in 0..40 {
            let symbol = buf[(x, row)].symbol().to_string();
            assert!(
                symbol != "▁" && symbol != "▔",
                "the selected Home icon tab paints no block runs (row {row}, column {x})"
            );
        }
    }
    let (home, _) = hits[0];
    assert_eq!(
        buf[(home.x + 1, 1)].style().fg,
        Some(palette::TAB_SELECTED_UNDERLINE),
        "the selected Home icon's active colour is the Mauve underline role"
    );
}
