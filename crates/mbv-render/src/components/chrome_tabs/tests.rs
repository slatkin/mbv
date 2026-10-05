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
