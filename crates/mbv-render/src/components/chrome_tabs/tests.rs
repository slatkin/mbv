use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

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

/// 2026-10-05 user rule: the selected tab is underlined by upper-eighth
/// blocks in the underline role's colour, spanning exactly its title run —
/// its lead and marker cells and every unselected tab's columns stay bare.
#[test]
fn the_selected_tab_underlines_exactly_its_title_run() {
    let (terminal, hits) = rendered(0);
    let (rect, _) = hits[0];
    let buf = terminal.backend().buffer();
    let title_run = (rect.x + 1)..(rect.x + rect.width - 1);
    for x in title_run {
        let cell = buf[(x, 2)].clone();
        assert_eq!(cell.symbol(), "▔", "column {x} underlines the title");
        assert_eq!(cell.style().fg, Some(palette::TAB_SELECTED_UNDERLINE));
    }
    for x in rect.x..rect.x + rect.width {
        if !((rect.x + 1)..(rect.x + rect.width - 1)).contains(&x) {
            assert_ne!(
                buf[(x, 2)].symbol(),
                "▔",
                "the lead and marker cells stay bare (column {x})"
            );
        }
    }
    let (bravo, _) = hits[1];
    for x in bravo.x..bravo.x + bravo.width {
        assert_ne!(
            buf[(x, 2)].symbol(),
            "▔",
            "an unselected tab is not underlined (column {x})"
        );
    }
}

/// 2026-10-05 user rule: the row above the selected title carries a
/// lower-eighth block run in the underline role's colour, spanning exactly
/// the title — the lead and marker cells and unselected tabs' columns stay
/// bare.
#[test]
fn the_selected_tab_gets_a_lower_eighth_run_on_the_row_above() {
    let (terminal, hits) = rendered(0);
    let (rect, _) = hits[0];
    let buf = terminal.backend().buffer();
    for x in (rect.x + 1)..(rect.x + rect.width - 1) {
        let cell = buf[(x, 0)].clone();
        assert_eq!(
            cell.symbol(),
            "▁",
            "column {x} carries the run above the title"
        );
        assert_eq!(cell.style().fg, Some(palette::TAB_SELECTED_UNDERLINE));
    }
    for x in rect.x..rect.x + rect.width {
        if !((rect.x + 1)..(rect.x + rect.width - 1)).contains(&x) {
            assert_ne!(
                buf[(x, 0)].symbol(),
                "▁",
                "the lead and marker cells stay bare (column {x})"
            );
        }
    }
    let (bravo, _) = hits[1];
    for x in bravo.x..bravo.x + bravo.width {
        assert_ne!(
            buf[(x, 0)].symbol(),
            "▁",
            "an unselected tab has no run above (column {x})"
        );
    }
}
