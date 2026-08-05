use super::test_helpers::*;

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
