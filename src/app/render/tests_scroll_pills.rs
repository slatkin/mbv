use super::components::widgets::{render_pill_bar, PillBar, PillBarWindow};
use super::test_helpers::*;
use crate::app::palette;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Terminal;

#[test]
fn pill_bar_does_not_paint_the_reserved_spacer_row() {
    let labels = vec!["All".to_string(), "A-C".to_string()];
    let ids = vec![0, 1];
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();

    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 20, 2);
            f.render_widget(
                Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP.color())),
                area,
            );
            render_pill_bar(
                f,
                area,
                PillBar {
                    labels: &labels,
                    ids: &ids,
                    selected_pos: 0,
                    hovered: None,
                    prefix: None,
                    window: PillBarWindow::default(),
                },
            );
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(19, 0)].bg, palette::PILL_ROW_BG.color());
    for x in 0..20 {
        assert_eq!(buffer[(x, 1)].bg, palette::SURFACE_BACKDROP.color());
    }
}

#[test]
fn pill_bar_hitboxes_carry_caller_ids_not_display_positions() {
    let labels: Vec<String> = ["Alpha", "Beta", "Gamma"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let ids = vec![10usize, 11, 12];

    let tabs = render_pill_bar_hitboxes(&labels, &ids, 0, 60);
    assert_eq!(
        tabs.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
        vec![10, 11, 12],
    );
    for pair in tabs.windows(2) {
        assert!(pair[0].0.x + pair[0].0.width <= pair[1].0.x);
    }
}

#[test]
fn pill_bar_scrolls_to_keep_selected_visible_and_maps_its_id() {
    let labels: Vec<String> = (0..6).map(|i| format!("Group{i}")).collect();
    let ids: Vec<usize> = (0..6).map(|i| 20 + i).collect();

    let tabs = render_pill_bar_hitboxes(&labels, &ids, 5, 18);

    assert!(!tabs.is_empty(), "expected at least one visible pill");
    assert!(
        tabs.iter().any(|(_, id)| *id == 25),
        "selected pill's id should be visible after scrolling, got {:?}",
        tabs.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
    );
    assert!(tabs.iter().all(|(_, id)| (20..=25).contains(id)));
    assert!(
        tabs.len() < labels.len(),
        "narrow row should not fit all six pills"
    );
}

#[test]
fn pill_bar_keeps_all_pills_visible_when_they_fit() {
    let labels: Vec<String> = (0..4).map(|i| format!("Season {i}")).collect();
    let ids: Vec<usize> = (0..4).collect();

    for selected in 0..4 {
        let tabs = render_pill_bar_hitboxes(&labels, &ids, selected, 60);
        assert_eq!(
            tabs.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
            ids,
            "all four pills fit, so selecting {selected} must not scroll any out"
        );
    }
}

#[test]
fn pill_bar_does_not_pin_a_backwards_selection_to_the_trailing_edge() {
    let labels: Vec<String> = (0..8).map(|i| format!("Group{i}")).collect();
    let ids: Vec<usize> = (0..8).map(|i| 20 + i).collect();

    let tabs = render_pill_bar_hitboxes(&labels, &ids, 4, 38);
    let Some(selected) = tabs.iter().position(|(_, id)| *id == 24) else {
        panic!("selected pill should be visible");
    };

    assert!(
        selected > 0,
        "selected pill should have a visible predecessor"
    );
    assert!(
        selected + 1 < tabs.len(),
        "selected pill should have a visible successor"
    );
}

fn pill_ids(tabs: &[(ratatui::layout::Rect, usize)]) -> Vec<usize> {
    tabs.iter().map(|(_, id)| *id).collect()
}

#[test]
fn pill_bar_keeps_its_window_when_the_selection_is_already_painted() {
    let labels: Vec<String> = (0..10).map(|i| format!("Group{i}")).collect();
    let ids: Vec<usize> = (0..10).collect();

    // First paint: no retention, the window centers on the selection.
    let (tabs, window) =
        render_pill_bar_hitboxes_with_window(&labels, &ids, 5, 30, PillBarWindow::default());
    let first_ids = pill_ids(&tabs);
    assert!(
        first_ids.contains(&5),
        "selected pill painted: {first_ids:?}"
    );

    // A pointer click on an already-painted, non-centered pill must not
    // slide the bar: the same window repaints, only the highlight moves.
    let clicked = *first_ids.last().unwrap();
    let (tabs2, window2) = render_pill_bar_hitboxes_with_window(&labels, &ids, clicked, 30, window);
    assert_eq!(pill_ids(&tabs2), first_ids, "window must not move");
    assert_eq!(window2.start, window.start, "retained window unchanged");
}

#[test]
fn pill_bar_scrolls_minimally_right_when_the_selection_leaves_the_window() {
    let labels: Vec<String> = (0..10).map(|i| format!("Group{i}")).collect();
    let ids: Vec<usize> = (0..10).collect();

    let (tabs, window) =
        render_pill_bar_hitboxes_with_window(&labels, &ids, 2, 30, PillBarWindow::default());
    let kept_first = *pill_ids(&tabs).first().unwrap();
    let hidden = 9; // past the window's trailing edge

    let (tabs2, _) = render_pill_bar_hitboxes_with_window(&labels, &ids, hidden, 30, window);
    let ids2 = pill_ids(&tabs2);
    assert!(ids2.contains(&hidden), "selection painted: {ids2:?}");
    // Minimal slide: the window moved forward from its retained start and
    // the selection is its trailing pill, not re-centered mid-row.
    let new_first = ids2.first().unwrap();
    assert!(
        *new_first > kept_first,
        "window moved right: {kept_first} -> {new_first}"
    );
    assert_eq!(
        *ids2.last().unwrap(),
        hidden,
        "selection landed at the trailing edge"
    );
}

#[test]
fn pill_bar_scrolls_left_to_put_a_leading_selection_on_the_leading_edge() {
    let labels: Vec<String> = (0..10).map(|i| format!("Group{i}")).collect();
    let ids: Vec<usize> = (0..10).collect();

    let (_, window) =
        render_pill_bar_hitboxes_with_window(&labels, &ids, 7, 30, PillBarWindow::default());
    let hidden = 1; // before the window's leading edge

    let (tabs2, _) = render_pill_bar_hitboxes_with_window(&labels, &ids, hidden, 30, window);
    assert_eq!(
        pill_ids(&tabs2).first().copied(),
        Some(hidden),
        "moving left lands the selection on the leading edge"
    );
}

#[test]
fn pill_bar_recenters_when_the_retained_window_start_is_stale() {
    let labels: Vec<String> = (0..10).map(|i| format!("Group{i}")).collect();
    let ids: Vec<usize> = (0..10).collect();

    // A stale retention (pill list shrank below the window start) must not
    // strand the bar: the row repaints with the selection visible.
    let stale = PillBarWindow { start: Some(8) };
    let (tabs, _) = render_pill_bar_hitboxes_with_window(&labels, &ids, 1, 30, stale);
    assert!(
        pill_ids(&tabs).contains(&1),
        "selection painted after stale retention"
    );
}
