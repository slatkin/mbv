use super::*;
use crate::app::render::LibraryListRenderCtx;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{KeyEvent, KeyModifiers};

/// Task 4.2d: the embedded episode `WideMediaList` field replaces the
/// old `Option<usize>` episode cursor. This exercises the same
/// component-local persistence through the canonical control -- moving
/// the cursor via keyboard, then re-syncing the same series/season data,
/// must preserve it (target-preserving `WideMediaList::set_content`).
#[test]
fn tv_workspace_keeps_episode_pane_cursor_local_between_syncs() {
    let mut component = TvContent::new();
    component.set_focused(true);
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let episode = |name: &str, id: &str| {
        let mut item = make_item(name, "Episode");
        item.id = id.into();
        item
    };
    let detail = crate::app::SeriesDetail {
        seasons: vec![season],
        episodes: [(
            "season-1".into(),
            vec![
                episode("Episode 1", "episode-1"),
                episode("Episode 2", "episode-2"),
            ],
        )]
        .into_iter()
        .collect(),
    };
    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series.clone()),
        Some(detail.clone()),
        0,
        None,
        false,
    ));
    component.test_key(&KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    });
    let message = component.test_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::TvEpisodeMove { delta: 1 }))
    ));
    assert_eq!(component.episodes.cursor(), 1);

    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series),
        Some(detail),
        0,
        None,
        false,
    ));
    assert_eq!(component.episodes.cursor(), 1);
}

#[test]
fn tv_workspace_series_change_resets_local_selection() {
    let mut component = TvContent::new();
    component.set_focused(true);
    let mut season_one = make_item("Season 1", "Season");
    season_one.id = "season-1".into();
    let mut season_two = make_item("Season 2", "Season");
    season_two.id = "season-2".into();
    let detail = crate::app::SeriesDetail {
        seasons: vec![season_one, season_two],
        episodes: std::collections::HashMap::new(),
    };
    let mut series_a = make_item("Series A", "Series");
    series_a.id = "series-a".into();
    let mut series_b = make_item("Series B", "Series");
    series_b.id = "series-b".into();

    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series_a.clone()], 0),
        Some(series_a),
        Some(detail.clone()),
        0,
        None,
        false,
    ));
    component.move_season(1);

    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series_b.clone()], 0),
        Some(series_b),
        Some(detail),
        0,
        None,
        false,
    ));

    assert_eq!(component.season_cursor, 0);
    assert!(component.episodes.is_empty());
    assert!(matches!(component.pane, Pane::Series));
}

#[test]
fn tv_workspace_renders_the_wide_workspace_without_app() {
    let mut component = TvContent::new();
    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0),
        None,
        None,
        0,
        None,
        false,
    ));
    let mut panel = crate::app::components::library_panel::LibraryPanel::new();
    panel.insert_owner(
        crate::app::components::library_panel::LibraryKey::Service {
            service: mbv_core::config::ServiceKind::Emby,
            library_id: "lib".into(),
            kind: crate::app::components::LibraryKind::TvShows,
        },
        Box::new(component),
    );
    panel.set_active(Some(
        crate::app::components::library_panel::LibraryKey::Service {
            service: mbv_core::config::ServiceKind::Emby,
            library_id: "lib".into(),
            kind: crate::app::components::LibraryKind::TvShows,
        },
    ));
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal
        .draw(|frame| tuirealm::component::Component::view(&mut panel, frame, frame.area()))
        .unwrap();
    assert!(terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .any(|cell| cell.symbol() == "S"));
}

#[test]
fn tv_workspace_renders_the_narrow_series_list_without_app() {
    let mut component = TvContent::new();
    component.set_is_wide(false);
    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0),
        None,
        None,
        0,
        None,
        false,
    ));
    let key = crate::app::components::library_panel::LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib".into(),
        kind: crate::app::components::LibraryKind::TvShows,
    };
    let mut panel = crate::app::components::library_panel::LibraryPanel::new();
    panel.insert_owner(key.clone(), Box::new(component));
    panel.set_active(Some(key));
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal
        .draw(|frame| tuirealm::component::Component::view(&mut panel, frame, frame.area()))
        .unwrap();
    assert!(terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .any(|cell| cell.symbol() == "S"));
}

/// The grouped-row gate reads the Inline Search session for the first time
/// (task 4.3, review round 2): with grouping otherwise on, an active search
/// session flattens the pushed series rows to `Item`-only, and closing it
/// restores the `Heading`/`Spacer` shape on the next `set_content` — the
/// shell's per-frame sync pass re-enters `set_content` after the session
/// closes, so the browse rows are not stuck flat forever.
#[test]
fn tv_grouped_rows_flatten_while_search_is_open_and_restore_after_close() {
    let item = |name: &str, id: &str| {
        let mut series = make_item(name, "Series");
        series.id = id.into();
        series
    };
    let context = || {
        // Grouping predicate on via `show_letter_pills` (the last push arg).
        TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(
                vec![
                    item("Alpha Series", "s1"),
                    item("Beta Series", "s2"),
                    item("Gamma Series", "s3"),
                ],
                0,
            ),
            None,
            None,
            0,
            None,
            true,
        )
    };
    let items = |rows: &[MediaListRow<String>]| {
        rows.iter()
            .filter(|row| matches!(row, MediaListRow::Item { .. }))
            .count()
    };
    let headings = |rows: &[MediaListRow<String>]| {
        rows.iter()
            .filter(|row| matches!(row, MediaListRow::Heading { .. }))
            .count()
    };
    let spacers = |rows: &[MediaListRow<String>]| {
        rows.iter()
            .filter(|row| matches!(row, MediaListRow::Spacer))
            .count()
    };

    let mut component = TvContent::new();

    // Session inactive: the pushed rows carry the grouped shape.
    component.set_content(context());
    let rows = component.carrier.rows().to_vec();
    assert_eq!(items(&rows), 3);
    assert!(
        headings(&rows) >= 2 && spacers(&rows) >= 1,
        "grouped shape while idle: {rows:?}"
    );

    // Open the session through the owner's own chord; the same content now
    // pushes flat `Item`-only rows.
    component.set_focused(true);
    assert!(component
        .on_key(&KeyEvent {
            code: Key::Char('/'),
            modifiers: KeyModifiers::NONE,
        })
        .is_some());
    assert!(component.inline_search.is_active());
    component.set_content(context());
    let rows = component.carrier.rows().to_vec();
    assert!(
        rows.iter()
            .all(|row| matches!(row, MediaListRow::Item { .. })),
        "flat rows while the search session is active: {rows:?}"
    );

    // Close it with the session's own dismiss (Esc); the next push restores
    // the grouped shape.
    component.on_key(&KeyEvent {
        code: Key::Esc,
        modifiers: KeyModifiers::NONE,
    });
    assert!(!component.inline_search.is_active());
    component.set_content(context());
    let rows = component.carrier.rows().to_vec();
    assert_eq!(items(&rows), 3);
    assert!(
        headings(&rows) >= 2 && spacers(&rows) >= 1,
        "grouped shape restored after close: {rows:?}"
    );
}

/// A played episode projects the shared `Played` state, so the episode list
/// paints the one played-row colour used by every other tab.
#[test]
fn played_episode_rows_project_the_shared_played_state() {
    let mut played = make_item("Watched Episode", "Episode");
    played.played = true;
    let fresh = make_item("Unwatched Episode", "Episode");
    let rows = build_episode_rows(&[played, fresh]);
    let states: Vec<&MediaSemanticState> = rows
        .iter()
        .map(|row| match row {
            MediaListRow::Item { semantic_state, .. } => semantic_state,
            _ => panic!("episode rows are items"),
        })
        .collect();
    assert_eq!(states[0], &MediaSemanticState::Played);
    assert_eq!(states[1], &MediaSemanticState::Ordinary);
}

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
fn upcoming_rows_without_dates_have_no_headings() {
    let rows = upcoming_episode_rows(
        &[
            make_item("First", "Episode"),
            make_item("Second", "Episode"),
        ],
        time::Date::from_calendar_date(2026, time::Month::September, 23).unwrap(),
    );
    assert!(rows
        .iter()
        .all(|row| !matches!(row, MediaListRow::Heading { .. })));
}

#[test]
fn virtual_idless_upcoming_rows_show_series_and_episode_context() {
    let mut virtual_episode = make_item("Pilot", "Episode");
    virtual_episode.id.clear();
    virtual_episode.series_name = "Example Show".into();
    virtual_episode.parent_index_number = 2;
    virtual_episode.index_number = 7;
    virtual_episode.premiere_date = "2026-09-23".into();

    let rows = upcoming_episode_rows(
        &[virtual_episode],
        time::Date::from_calendar_date(2026, time::Month::September, 23).unwrap(),
    );
    let Some(MediaListRow::Item {
        target,
        primary,
        secondary,
        ..
    }) = rows
        .iter()
        .find(|row| matches!(row, MediaListRow::Item { .. }))
    else {
        panic!("upcoming episode row missing");
    };
    assert!(!target.is_empty());
    assert_eq!(primary, "Example Show");
    assert_eq!(secondary.as_deref(), Some("S02:E07 — Pilot"));
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
