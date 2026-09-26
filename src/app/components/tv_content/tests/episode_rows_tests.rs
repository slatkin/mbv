use super::*;

/// A played episode projects the shared `Played` state, so the episode list
/// paints the one played-row colour used by every other tab.
#[test]
fn episode_rows_project_runtime_in_the_green_gutter() {
    let mut episode = make_item("Episode", "Episode");
    episode.runtime_ticks = 3_661 * mbv_core::api::TICKS_PER_SECOND;
    let rows = build_episode_rows(&[episode]);
    let MediaListRow::Item {
        trailing, duration, ..
    } = &rows[0]
    else {
        panic!("episode rows are items");
    };
    assert_eq!(trailing, &Some(MediaListTrailing::Gutter("1:01".into())));
    assert_eq!(duration, &None);
}

#[test]
fn upcoming_episode_rows_group_dates_in_first_seen_order_with_relative_headings() {
    let today = time::Date::from_calendar_date(2026, time::Month::September, 23).unwrap();
    let mut yesterday = make_item("Yesterday episode", "Episode");
    yesterday.series_name = "Yesterday episode".into();
    yesterday.premiere_date = "2026-09-22T00:00:00Z".into();
    let mut today_episode = make_item("Today episode", "Episode");
    today_episode.series_name = "Today episode".into();
    today_episode.premiere_date = "2026-09-23".into();
    let mut another_yesterday = make_item("Another yesterday", "Episode");
    another_yesterday.series_name = "Another yesterday".into();
    another_yesterday.premiere_date = "2026-09-22".into();
    let mut older_episode = make_item("Older episode", "Episode");
    older_episode.series_name = "Older episode".into();
    older_episode.premiere_date = "2026-09-21".into();

    let rows = upcoming_episode_rows(
        &[yesterday, today_episode, another_yesterday, older_episode],
        today,
    );
    let headings: Vec<&str> = rows
        .iter()
        .filter_map(|row| match row {
            MediaListRow::Heading { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(headings, ["Yesterday", "Today", "Monday, September 21"]);
    assert_eq!(
        rows.iter()
            .filter_map(|row| match row {
                MediaListRow::Item { primary, .. } => Some(primary.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            "Yesterday episode",
            "Another yesterday",
            "Today episode",
            "Older episode"
        ]
    );
}

#[test]
fn idless_upcoming_rows_have_stable_distinct_targets_and_resolve_selection() {
    let mut first = make_item("First episode", "Episode");
    first.id.clear();
    first.series_id = "series-id".into();
    first.series_name = "Example Show".into();
    first.parent_index_number = 2;
    first.index_number = 7;
    let mut second = first.clone();
    second.name = "Second episode".into();
    second.index_number = 8;

    let rows = upcoming_episode_rows(
        &[first.clone(), second.clone()],
        time::Date::from_calendar_date(2026, time::Month::September, 23).unwrap(),
    );
    let targets: Vec<&str> = rows
        .iter()
        .filter_map(|row| match row {
            MediaListRow::Item { target, .. } => Some(target.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(targets.len(), 2);
    assert_ne!(targets[0], targets[1]);
    assert_eq!(targets[0], upcoming_episode_target(&first));
    assert_eq!(targets[1], upcoming_episode_target(&second));
    assert_eq!(
        targets,
        upcoming_episode_rows(
            &[first.clone(), second.clone()],
            time::Date::from_calendar_date(2026, time::Month::September, 23).unwrap(),
        )
        .iter()
        .filter_map(|row| match row {
            MediaListRow::Item { target, .. } => Some(target.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
    );

    let mut context = TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![first, second], 0),
        None,
        None,
        0,
        None,
        false,
    );
    context.set_tv_content_mode(Some(mbv_core::config::TvContentMode::Upcoming));
    let mut component = TvContent::new();
    component.set_content(context);
    component.carrier.select_index(1);
    assert_eq!(
        component.carrier.selected_target().map(String::as_str),
        Some(targets[1])
    );
    assert_eq!(
        component
            .selected_episode_item()
            .as_ref()
            .map(|item| item.name.as_str()),
        Some("Second episode")
    );
}
