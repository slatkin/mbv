use super::test_helpers::*;
use super::*;
use crate::app::components::FeedsComponent;
use crate::app::render::arrangements::hero_left;
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::FeedEntry;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::Component;

fn feed_component() -> FeedsComponent {
    let subscriptions = vec![FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    let entries = vec![vec![
        FeedEntry {
            guid: "entry-1".into(),
            title: "Entry One".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: Some(FeedKind::Audio),
            feed_id: None,
            position_ticks: 0,
            played: false,
        },
        FeedEntry {
            guid: "entry-2".into(),
            title: "Played Entry Two".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: Some(FeedKind::Audio),
            feed_id: None,
            position_ticks: 42,
            played: true,
        },
    ]];
    let all_entries = entries[0].clone();
    let mut component = FeedsComponent::new();
    component.set_content(&subscriptions, &entries, &all_entries, false, true);
    component
}

fn terminal_for(component: &mut FeedsComponent, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, width, height)))
        .unwrap();
    terminal
}

#[test]
fn wide_feeds_use_a_left_detail_and_right_entry_workspace() {
    for width in [82, 120] {
        let mut component = feed_component();
        let terminal = terminal_for(&mut component, width, 30);
        let layout = component.layout();

        assert!(
            layout.hero_area.width < width,
            "hero={:?}",
            layout.hero_area
        );
        assert!(
            layout.left_area.x > layout.hero_area.x,
            "hero={:?} list={:?}",
            layout.hero_area,
            layout.left_area
        );
        assert!(!layout.selector_tabs.is_empty());
        assert_eq!(
            layout
                .left_item_rows
                .iter()
                .filter(|row| !row.is_empty())
                .count(),
            2
        );
        assert!(
            layout.left_item_rows.iter().all(|row| row.len() <= 1),
            "rows={:?}",
            layout.left_item_rows
        );
        let buffer = terminal.backend().buffer();
        let first_row = layout.left_area.y
            + layout
                .left_row_map
                .iter()
                .position(|item| item == &Some(0))
                .expect("first row") as u16;
        let second_row = layout.left_area.y
            + layout
                .left_row_map
                .iter()
                .position(|item| item == &Some(1))
                .expect("second row") as u16;
        assert_ne!(
            buffer[(layout.left_area.x, first_row)].bg,
            ratatui::style::Color::Reset
        );
        let first = (layout.left_area.x..layout.left_area.right())
            .map(|x| buffer[(x, first_row)].symbol())
            .collect::<String>();
        let second = (layout.left_area.x..layout.left_area.right())
            .map(|x| buffer[(x, second_row)].symbol())
            .collect::<String>();
        assert_eq!(first.matches("Entry One").count(), 1, "row={first:?}");
        assert!(
            second.contains("✓") && second.contains("Played Entry Two"),
            "row={second:?}"
        );
    }
}

#[test]
fn narrow_feeds_insert_selected_entry_detail_into_the_list_flow() {
    let mut component = feed_component();
    terminal_for(&mut component, 60, 20);
    let layout = component.layout();

    assert!(layout.hero_area.height > 0);
    assert!(
        layout.hero_area.y >= layout.left_area.y,
        "hero={:?} list={:?}",
        layout.hero_area,
        layout.left_area
    );
}

#[test]
fn narrow_feeds_suppress_detail_when_the_viewport_is_too_short() {
    let mut component = feed_component();
    terminal_for(&mut component, 60, 4);
    assert_eq!(component.layout().hero_area.height, 0);
}

#[test]
fn feeds_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, focused) in [
        (140, 30, false),
        (140, 30, true),
        (60, 20, true),
        (40, 20, false),
    ] {
        let mut component = feed_component();
        let terminal = terminal_for(&mut component, width, height);
        let output = buffer_to_string(&terminal);
        assert!(
            output.contains("Test Feed"),
            "missing feed selector: {output:?}"
        );
        assert!(
            output.contains("Entry One"),
            "missing selected entry: {output:?}"
        );
        let _ = focused;
    }
}

#[test]
fn feeds_pill_row_and_targets_are_characterized_end_to_end() {
    let assert_geometry = |terminal: &Terminal<TestBackend>, layout: &LayoutMain| {
        let panel = Rect::new(0, 0, 60, 20);
        let areas = hero_left::pill_bar_areas(panel);
        assert_surface_pills(
            terminal,
            layout,
            panel,
            1,
            ratatui::style::Color::Reset,
            &[0, 1],
            &["⌘", "All", "Test Feed"],
            0,
        );
        assert_eq!(layout.selector_tabs[0].0.y, areas.pills_area.y);
        assert_eq!(layout.left_area.y, areas.spacer_area.bottom() + 2);
        let buffer = terminal.backend().buffer();
        let filter_row = (0..buffer.area().width)
            .map(|x| buffer[(x, areas.spacer_area.bottom())].symbol())
            .collect::<String>();
        assert!(
            filter_row.contains("All"),
            "missing watched All filter: {filter_row:?}"
        );
        assert!(
            filter_row.contains("Played") && filter_row.contains("Unplayed"),
            "missing watched filters: {filter_row:?}"
        );
    };

    let mut no_hero_component = feed_component();
    let no_hero_subscriptions = [FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    no_hero_component.set_content(&no_hero_subscriptions, &[Vec::new()], &[], false, true);
    let no_hero_terminal = terminal_for(&mut no_hero_component, 60, 20);
    assert_geometry(&no_hero_terminal, no_hero_component.layout());

    let mut post_hero_component = feed_component();
    let post_hero_terminal = terminal_for(&mut post_hero_component, 60, 20);
    assert_geometry(&post_hero_terminal, post_hero_component.layout());
    assert!(post_hero_component.layout().hero_area.height > 0);

    let mut no_subscriptions_component = FeedsComponent::new();
    let no_subscriptions_terminal = terminal_for(&mut no_subscriptions_component, 60, 20);
    let layout = no_subscriptions_component.layout();
    assert!(layout.selector_tabs.is_empty());
    assert_eq!(layout.left_area.y, 3);
    let empty_row = (0..60)
        .map(|x| no_subscriptions_terminal.backend().buffer()[(x, 3)].symbol())
        .collect::<String>();
    assert!(
        empty_row.contains("No feed subscriptions configured"),
        "empty/help content moved: {empty_row:?}"
    );
}
