use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// 2026-10-05 user rule: the gap between adjacent painted tab titles is two
/// columns. Each tab paints one leading space, its title, and the marker
/// column, so one tab's marker cell plus the next tab's leading space make
/// the two-space gap — after the selected tab (whose leading pair is the
/// accent bar plus a space) and between two unselected tabs alike.
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
