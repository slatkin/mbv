use super::*;
use crate::app::render::LibraryListRenderCtx;
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use rstest::rstest;
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

fn tv_tree_context(
    items: Vec<EmbyItem>,
    selected_id: Option<&str>,
    detail: Option<crate::app::SeriesDetail>,
    show_letter_pills: bool,
) -> TvWideRenderCtx {
    let selected = selected_id.and_then(|id| items.iter().find(|item| item.id == id).cloned());
    TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(items, 0),
        selected,
        detail,
        0,
        None,
        show_letter_pills,
    )
}

fn tv_show(name: &str, id: &str) -> EmbyItem {
    let mut item = make_item(name, "Series");
    item.id = id.into();
    item
}

#[rstest]
#[case::wide(120)]
#[case::narrow(80)]
#[case::mini(48)]
fn show_modes_paint_the_tree_in_every_panel_geometry(#[case] width: u16) {
    use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
    use crate::app::components::list::tree_browser::TreeOperation;
    use crate::app::components::tv_tree_target::TvTreeTarget;
    use crate::app::components::LibraryKind;
    use ratatui::layout::Rect;
    use tuirealm::component::Component;

    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut owner = TvContent::new();
    owner.set_content(tv_tree_context(
        vec![tv_show("Alpha", "show-a")],
        Some("show-a"),
        Some(crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: std::collections::HashMap::new(),
        }),
        false,
    ));
    let show = TvTreeTarget::Show("tv-id:6:show-a".into());
    owner
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(show));

    let key = LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-tv".into(),
        kind: LibraryKind::TvShows,
    };
    let mut panel = LibraryPanel::new();
    panel.insert_owner(key, Box::new(owner));
    panel.set_active(Some(LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-tv".into(),
        kind: LibraryKind::TvShows,
    }));
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, Rect::new(0, 0, width, 24)))
        .unwrap();
    let painted = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(
        painted.contains("Season 1"),
        "tree child missing at width {width}"
    );

    let owner = panel
        .owner(&LibraryKey::Service {
            service: mbv_core::config::ServiceKind::Emby,
            library_id: "lib-tv".into(),
            kind: LibraryKind::TvShows,
        })
        .and_then(|owner| owner.as_any().downcast_ref::<TvContent>())
        .unwrap();
    assert!(owner.browser.has_completed_paint());
    assert!(owner.episodes.current_content_rect().is_none());
    let content = owner.browser.visible_targets();
    assert_eq!(content.len(), 2);
}

#[rstest]
#[case::wide(120)]
#[case::narrow(80)]
#[case::mini(48)]
fn flat_episode_modes_keep_the_flat_list_at_every_panel_geometry(#[case] width: u16) {
    use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
    use crate::app::components::LibraryKind;
    use ratatui::layout::Rect;
    use tuirealm::component::Component;

    for mode in [
        mbv_core::config::TvContentMode::Latest,
        mbv_core::config::TvContentMode::Upcoming,
    ] {
        let mut episode = make_item("Pilot", "Episode");
        episode.id = "episode-1".into();
        let mut list = LibraryListRenderCtx::from_items(vec![episode], 0);
        list.library_total = Some(301);
        let mut context = TvWideRenderCtx::new(list, None, None, 0, None, true);
        context.set_tv_content_mode(Some(mode));
        let mut owner = TvContent::new();
        owner.set_content(context);
        let key = LibraryKey::Service {
            service: mbv_core::config::ServiceKind::Emby,
            library_id: "lib-tv".into(),
            kind: LibraryKind::TvShows,
        };
        let mut panel = LibraryPanel::new();
        panel.insert_owner(key, Box::new(owner));
        panel.set_active(Some(LibraryKey::Service {
            service: mbv_core::config::ServiceKind::Emby,
            library_id: "lib-tv".into(),
            kind: LibraryKind::TvShows,
        }));
        let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
        terminal
            .draw(|frame| Component::view(&mut panel, frame, Rect::new(0, 0, width, 24)))
            .unwrap();
        let owner = panel
            .owner(&LibraryKey::Service {
                service: mbv_core::config::ServiceKind::Emby,
                library_id: "lib-tv".into(),
                kind: LibraryKind::TvShows,
            })
            .and_then(|owner| owner.as_any().downcast_ref::<TvContent>())
            .unwrap();
        assert!(!owner.browser.has_completed_paint());
        assert!(owner.carrier.current_content_rect().is_some());
    }
}

#[test]
fn flat_modes_and_inline_search_preserve_the_settled_show_tree() {
    use crate::app::components::library_panel::ListSlot;
    use crate::app::components::list::tree_browser::TreeOperation;
    use crate::app::components::tv_tree_target::TvTreeTarget;
    use mbv_core::config::TvContentMode;

    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = make_item("Episode 1", "Episode");
    episode.id = "episode-1".into();
    let show = TvTreeTarget::Show("tv-id:6:show-a".into());
    let season_target = TvTreeTarget::Season {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        occurrence: 0,
    };
    let episode_target = TvTreeTarget::Episode {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        season_occurrence: 0,
        episode: "episode-1".into(),
        occurrence: 0,
    };
    let show_context = || {
        tv_tree_context(
            vec![tv_show("Alpha", "show-a")],
            Some("show-a"),
            Some(crate::app::SeriesDetail {
                seasons: vec![season.clone()],
                episodes: [("season-1".into(), vec![episode.clone()])]
                    .into_iter()
                    .collect(),
            }),
            true,
        )
    };
    let mut component = TvContent::new();
    let mut context = show_context();
    context.set_tv_content_mode(Some(TvContentMode::All));
    component.set_content(context);
    component
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(show.clone()));
    component
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(season_target.clone()));
    component
        .browser
        .apply(TreeOperation::Select(episode_target.clone()));

    for (mode, active_pill) in [(TvContentMode::Latest, 0), (TvContentMode::Upcoming, 1)] {
        let mut context = TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![episode.clone()], 0),
            None,
            None,
            0,
            None,
            true,
        );
        context.set_tv_content_mode(Some(mode));
        component.set_content(context);
        assert!(component.flat_episode_mode());
        assert_eq!(component.browser.selected_target(), Some(&episode_target));
        assert!(component.browser.is_expanded(&show));
        assert!(component.browser.is_expanded(&season_target));
        assert_eq!(
            component.panel_content().selector.unwrap().active,
            Some(active_pill)
        );
        assert!(!matches!(
            component.panel_content().list,
            ListSlot::Search(_)
        ));
    }

    let mut context = show_context();
    context.set_tv_content_mode(Some(TvContentMode::All));
    component.set_content(context);
    assert_eq!(component.browser.selected_target(), Some(&episode_target));
    assert!(component.browser.is_expanded(&show));
    assert!(component.browser.is_expanded(&season_target));

    assert!(component
        .test_key(&KeyEvent {
            code: Key::Char('/'),
            modifiers: KeyModifiers::NONE,
        })
        .is_some());
    let mut context = show_context();
    context.set_tv_content_mode(Some(TvContentMode::All));
    component.set_content(context);
    assert!(matches!(
        component.panel_content().list,
        ListSlot::Search(_)
    ));
    assert_eq!(component.browser.selected_target(), Some(&episode_target));
    assert!(component.browser.is_expanded(&show));
    assert!(component.browser.is_expanded(&season_target));

    component.test_key(&KeyEvent {
        code: Key::Esc,
        modifiers: KeyModifiers::NONE,
    });
    assert!(!component.inline_search_active());
    let content = component.panel_content();
    assert!(content.selector.is_some());
    assert!(!matches!(content.list, ListSlot::Search(_)));
    assert_eq!(component.browser.selected_target(), Some(&episode_target));
    assert!(component.browser.is_expanded(&show));
    assert!(component.browser.is_expanded(&season_target));
}

#[rstest]
#[case::wide(120)]
#[case::narrow(80)]
#[case::mini(48)]
fn inline_search_keeps_its_flat_controls_at_every_panel_geometry(#[case] width: u16) {
    use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
    use crate::app::components::LibraryKind;
    use ratatui::layout::Rect;
    use tuirealm::component::Component;

    let mut owner = TvContent::new();
    owner.inline_search.open();
    let key = LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-tv".into(),
        kind: LibraryKind::TvShows,
    };
    let mut panel = LibraryPanel::new();
    panel.insert_owner(key, Box::new(owner));
    panel.set_active(Some(LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-tv".into(),
        kind: LibraryKind::TvShows,
    }));
    let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, Rect::new(0, 0, width, 24)))
        .unwrap();
    let owner = panel
        .owner(&LibraryKey::Service {
            service: mbv_core::config::ServiceKind::Emby,
            library_id: "lib-tv".into(),
            kind: LibraryKind::TvShows,
        })
        .and_then(|owner| owner.as_any().downcast_ref::<TvContent>())
        .unwrap();
    assert!(owner.inline_search.is_active());
    assert!(!owner.browser.has_completed_paint());
}

#[test]
fn tree_episode_double_click_uses_its_show_target_when_selection_is_stale() {
    use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
    use crate::app::components::list::tree_browser::TreeOperation;
    use crate::app::components::media_list::MediaListSurfaceInput;
    use crate::app::components::tv_tree_target::TvTreeTarget;
    use ratatui::layout::Rect;
    use tuirealm::component::Component;

    let show = tv_show("Alpha", "show-a");
    let other_show = tv_show("Beta", "show-b");
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = make_item("Pilot", "Episode");
    episode.id = "episode-1".into();
    episode.series_id = show.id.clone();
    let detail = crate::app::SeriesDetail {
        seasons: vec![season],
        episodes: [("season-1".into(), vec![episode.clone()])]
            .into_iter()
            .collect(),
    };
    let mut owner = TvContent::new();
    owner.set_is_wide(false);
    owner.set_content(tv_tree_context(
        vec![show.clone(), other_show.clone()],
        Some("show-a"),
        Some(detail),
        false,
    ));
    let show_target = TvTreeTarget::Show("tv-id:6:show-a".into());
    let season_target = TvTreeTarget::Season {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        occurrence: 0,
    };
    let episode_target = TvTreeTarget::Episode {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        season_occurrence: 0,
        episode: "episode-1".into(),
        occurrence: 0,
    };
    owner
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(show_target));
    owner
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(season_target));

    let key = LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-tv".into(),
        kind: crate::app::components::LibraryKind::TvShows,
    };
    let mut panel = LibraryPanel::new();
    panel.insert_owner(key.clone(), Box::new(owner));
    panel.set_active(Some(key.clone()));
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, Rect::new(0, 0, 80, 20)))
        .unwrap();
    let row = panel
        .owner(&key)
        .and_then(|owner| owner.as_any().downcast_ref::<TvContent>())
        .unwrap()
        .browser
        .row_rect_for(&episode_target)
        .unwrap();

    // The expanded row remains a valid action target even if the shell's
    // selected-series projection has moved on before this event is handled.
    let owner = panel
        .owner_mut(&key)
        .and_then(|owner| owner.as_any_mut().downcast_mut::<TvContent>())
        .unwrap();
    owner.context.selected_series = Some(other_show);
    let message = owner.show_tree_list_event(MediaListSurfaceInput::DoubleClick(
        ratatui::layout::Position { x: row.x, y: row.y },
    ));
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::TvEpisodeActivate { episode: selected }))
            if selected.id == episode.id && selected.series_id == show.id
    ));
}

#[test]
fn expanded_tree_episode_coexists_with_independent_workspace_episode_and_show_hero() {
    use crate::app::components::library_panel::{LibraryKey, LibraryPanel};
    use crate::app::components::list::tree_browser::TreeOperation;
    use crate::app::components::tv_tree_target::TvTreeTarget;
    use crate::app::components::LibraryKind;
    use ratatui::layout::Rect;
    use tuirealm::component::Component;

    let show = tv_show("Alpha", "show-a");
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut pilot = make_item("Pilot", "Episode");
    pilot.id = "episode-1".into();
    let mut finale = make_item("Finale", "Episode");
    finale.id = "episode-2".into();
    let detail = crate::app::SeriesDetail {
        seasons: vec![season],
        episodes: [("season-1".into(), vec![pilot, finale])]
            .into_iter()
            .collect(),
    };
    let mut owner = TvContent::new();
    owner.set_content(tv_tree_context(
        vec![show],
        Some("show-a"),
        Some(detail),
        false,
    ));
    let show_target = TvTreeTarget::Show("tv-id:6:show-a".into());
    let season_target = TvTreeTarget::Season {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        occurrence: 0,
    };
    let episode_target = TvTreeTarget::Episode {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        season_occurrence: 0,
        episode: "episode-1".into(),
        occurrence: 0,
    };
    owner
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(show_target));
    owner
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(season_target));
    owner
        .browser
        .apply(TreeOperation::Select(episode_target.clone()));
    owner.episodes.select_index(1);

    let key = LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-tv".into(),
        kind: LibraryKind::TvShows,
    };
    let mut panel = LibraryPanel::new();
    panel.insert_owner(key, Box::new(owner));
    panel.set_active(Some(LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-tv".into(),
        kind: LibraryKind::TvShows,
    }));
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, Rect::new(0, 0, 120, 30)))
        .unwrap();

    let owner = panel
        .owner(&LibraryKey::Service {
            service: mbv_core::config::ServiceKind::Emby,
            library_id: "lib-tv".into(),
            kind: LibraryKind::TvShows,
        })
        .and_then(|owner| owner.as_any().downcast_ref::<TvContent>())
        .unwrap();
    let painted = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert_eq!(painted.matches("Pilot").count(), 2);
    assert_eq!(owner.browser.selected_target(), Some(&episode_target));
    assert_eq!(
        owner.episodes.selected_target().map(String::as_str),
        Some("episode-2")
    );
    assert_eq!(
        owner
            .selected_series_snapshot()
            .map(|item| item.id.as_str()),
        Some("show-a")
    );
    assert!(
        painted.contains("Finale"),
        "the Hero Workspace keeps its other episode"
    );
}

#[test]
fn show_tree_projects_sorted_roots_with_loaded_seasons_and_episodes_in_order() {
    use crate::app::components::tv_tree_target::TvTreeTarget;

    let alpha = tv_show("Alpha", "show-a");
    let beta = tv_show("Beta", "show-b");
    let zulu = tv_show("Zulu", "show-z");
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = make_item("Pilot", "Episode");
    episode.id = "episode-1".into();
    episode.index_number = 1;
    let detail = crate::app::SeriesDetail {
        seasons: vec![season],
        episodes: [("season-1".into(), vec![episode])].into_iter().collect(),
    };
    let context = tv_tree_context(vec![zulu, beta, alpha], Some("show-a"), Some(detail), false);
    let mut component = TvContent::new();
    component.set_content(context);

    let show = TvTreeTarget::Show("tv-id:6:show-a".into());
    let season = TvTreeTarget::Season {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        occurrence: 0,
    };
    let episode = TvTreeTarget::Episode {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        season_occurrence: 0,
        episode: "episode-1".into(),
        occurrence: 0,
    };
    assert_eq!(
        component.browser.roots(),
        vec![
            &show,
            &TvTreeTarget::Show("tv-id:6:show-b".into()),
            &TvTreeTarget::Show("tv-id:6:show-z".into())
        ]
    );
    assert_eq!(component.browser.children_of(&show), Some(vec![&season]));
    assert_eq!(component.browser.children_of(&season), Some(vec![&episode]));
    assert!(component.browser.node(&show).unwrap().expandable);
    assert!(component.browser.node(&season).unwrap().expandable);

    component.browser.apply(
        crate::app::components::list::tree_browser::TreeOperation::ToggleExpansionTarget(
            show.clone(),
        ),
    );
    component.browser.apply(
        crate::app::components::list::tree_browser::TreeOperation::ToggleExpansionTarget(
            season.clone(),
        ),
    );
    assert_eq!(
        component.browser.visible_targets(),
        vec![
            show,
            season,
            episode,
            TvTreeTarget::Show("tv-id:6:show-b".into()),
            TvTreeTarget::Show("tv-id:6:show-z".into()),
        ]
    );
}

#[rstest]
#[case::three_letter_ranges(vec![("Alpha", "a"), ("Delta", "d"), ("Zulu", "z")], vec!["A–C", "D–F", "V–Z"])]
fn show_tree_keeps_each_heading_and_spacer_at_its_group_boundary(
    #[case] shows: Vec<(&str, &str)>,
    #[case] expected_headings: Vec<&str>,
) {
    let items = shows
        .iter()
        .map(|(name, id)| tv_show(name, id))
        .collect::<Vec<_>>();
    let context = tv_tree_context(items, None, None, true);
    let projection = TvContent::tree_projection(&context);
    let shape = projection
        .iter()
        .map(|entry| match entry {
            TreeEntry::Heading(text) => format!("H:{text}"),
            TreeEntry::Spacer => "S".to_string(),
            TreeEntry::Node(node) => match &node.target {
                TvTreeTarget::Show(_) => format!("I:{}", node.title),
                _ => panic!("fixture has no child rows"),
            },
        })
        .collect::<Vec<_>>();
    let expected = vec![
        format!("H:{}", expected_headings[0]),
        "I:Alpha".into(),
        "S".into(),
        format!("H:{}", expected_headings[1]),
        "I:Delta".into(),
        "S".into(),
        format!("H:{}", expected_headings[2]),
        "I:Zulu".into(),
    ];
    assert_eq!(shape, expected);
}

#[test]
fn tree_expand_requests_shell_loading_only_on_the_open_transition() {
    use crate::app::components::list::tree_browser::TreeOperation;
    use crate::app::components::tv_tree_target::TvTreeTarget;

    let show_target = TvTreeTarget::Show("tv-id:6:show-a".into());
    let mut component = TvContent::new();
    component.set_content(tv_tree_context(
        vec![tv_show("Alpha", "show-a")],
        None,
        None,
        false,
    ));
    assert!(matches!(
        component.toggle_tree_expansion(show_target.clone()),
        Some(Msg::Shell(ShellRequest::TvTreeExpand { target })) if target == show_target
    ));
    assert!(component.browser.is_expanded(&show_target));
    assert!(component
        .toggle_tree_expansion(show_target.clone())
        .is_none());
    assert!(!component.browser.is_expanded(&show_target));

    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    component.set_content(tv_tree_context(
        vec![tv_show("Alpha", "show-a")],
        Some("show-a"),
        Some(crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: std::collections::HashMap::new(),
        }),
        false,
    ));
    component
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(show_target));
    let season_target = TvTreeTarget::Season {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        occurrence: 0,
    };
    assert!(matches!(
        component.toggle_tree_expansion(season_target.clone()),
        Some(Msg::Shell(ShellRequest::TvTreeExpand { target })) if target == season_target
    ));
}

#[test]
fn expanded_show_and_selected_target_survive_detail_completion() {
    use crate::app::components::list::tree_browser::TreeOperation;
    use crate::app::components::tv_tree_target::TvTreeTarget;

    let show_target = TvTreeTarget::Show("tv-id:6:show-a".into());
    let mut component = TvContent::new();
    component.set_content(tv_tree_context(
        vec![tv_show("Alpha", "show-a")],
        Some("show-a"),
        None,
        false,
    ));
    component
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(show_target.clone()));
    component
        .browser
        .apply(TreeOperation::Select(show_target.clone()));

    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    component.set_content(tv_tree_context(
        vec![tv_show("Alpha", "show-a")],
        Some("show-a"),
        Some(crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: std::collections::HashMap::new(),
        }),
        false,
    ));

    assert_eq!(component.browser.selected_target(), Some(&show_target));
    assert!(component.browser.is_expanded(&show_target));
    assert!(matches!(
        component.browser.visible_targets().as_slice(),
        [TvTreeTarget::Show(_), TvTreeTarget::Season { .. }]
    ));
}

#[test]
fn completed_empty_details_remove_pending_expandability() {
    use crate::app::components::tv_tree_target::TvTreeTarget;

    let show_target = TvTreeTarget::Show("tv-id:6:show-a".into());
    let mut component = TvContent::new();
    component.set_content(tv_tree_context(
        vec![tv_show("Alpha", "show-a")],
        Some("show-a"),
        Some(crate::app::SeriesDetail {
            seasons: Vec::new(),
            episodes: std::collections::HashMap::new(),
        }),
        false,
    ));
    assert!(!component.browser.node(&show_target).unwrap().expandable);

    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    component.set_content(tv_tree_context(
        vec![tv_show("Alpha", "show-a")],
        Some("show-a"),
        Some(crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: [("season-1".into(), Vec::new())].into_iter().collect(),
        }),
        false,
    ));
    let season_target = TvTreeTarget::Season {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        occurrence: 0,
    };
    assert!(!component.browser.node(&season_target).unwrap().expandable);
}

#[rstest]
#[case::duplicate_season_ids(vec!["season-1", "season-1"], vec![])]
#[case::duplicate_episode_ids(vec!["season-1"], vec!["episode-1", "episode-1"])]
#[case::colliding_idless_virtual_episodes(vec!["season-1"], vec!["", ""])]
fn duplicate_child_identities_are_scoped_to_their_parent(
    #[case] season_ids: Vec<&str>,
    #[case] episode_ids: Vec<&str>,
) {
    let show = tv_show("Alpha", "show-a");
    let seasons = season_ids
        .iter()
        .map(|id| {
            let mut season = make_item("Season", "Season");
            season.id = (*id).into();
            season
        })
        .collect::<Vec<_>>();
    let episodes = episode_ids
        .iter()
        .map(|id| {
            let mut episode = make_item("Virtual episode", "Episode");
            episode.id = (*id).into();
            episode
        })
        .collect::<Vec<_>>();
    let detail = crate::app::SeriesDetail {
        seasons,
        episodes: [("season-1".into(), episodes)].into_iter().collect(),
    };
    let mut component = TvContent::new();
    component.set_content(tv_tree_context(
        vec![show],
        Some("show-a"),
        Some(detail),
        false,
    ));

    fn collect<T: Clone + Eq + std::hash::Hash>(
        browser: &TreeBrowser<T>,
        target: &T,
        output: &mut Vec<T>,
    ) {
        output.push(target.clone());
        if let Some(children) = browser.children_of(target) {
            for child in children {
                collect(browser, child, output);
            }
        }
    }
    let mut targets = Vec::new();
    for root in component.browser.roots() {
        collect(&component.browser, root, &mut targets);
    }
    let target_count = targets.len();
    targets.sort_by_key(|target| format!("{target:?}"));
    targets.dedup();
    assert_eq!(
        target_count,
        targets.len(),
        "every duplicate row stays addressable"
    );
}

#[test]
fn show_tree_refresh_preserves_selected_identity_expansion_and_valid_viewport() {
    use crate::app::components::list::tree_browser::TreeOperation;
    use crate::app::components::tv_tree_target::TvTreeTarget;

    let show = tv_show("Alpha", "show-a");
    let mut season_item = make_item("Season 1", "Season");
    season_item.id = "season-1".into();
    let mut episode_item = make_item("Pilot", "Episode");
    episode_item.id = "episode-1".into();
    let detail = crate::app::SeriesDetail {
        seasons: vec![season_item],
        episodes: [("season-1".into(), vec![episode_item])]
            .into_iter()
            .collect(),
    };
    let context = || {
        tv_tree_context(
            vec![show.clone()],
            Some("show-a"),
            Some(detail.clone()),
            false,
        )
    };
    let show_target = TvTreeTarget::Show("tv-id:6:show-a".into());
    let season_target = TvTreeTarget::Season {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        occurrence: 0,
    };
    let episode_target = TvTreeTarget::Episode {
        show: "tv-id:6:show-a".into(),
        season: "season-1".into(),
        season_occurrence: 0,
        episode: "episode-1".into(),
        occurrence: 0,
    };
    let mut component = TvContent::new();
    component.set_content(context());
    component.browser.set_geometry(
        ratatui::layout::Rect::new(0, 0, 20, 1),
        ratatui::layout::Rect::new(0, 0, 20, 1),
    );
    component
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(show_target.clone()));
    component
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(season_target.clone()));
    component
        .browser
        .apply(TreeOperation::Select(episode_target.clone()));
    assert!(component.browser.viewport_offset() > 0);

    component.set_content(context());
    component.browser.clamp_viewport_to(1);

    assert_eq!(component.browser.selected_target(), Some(&episode_target));
    assert!(component.browser.is_expanded(&show_target));
    assert!(component.browser.is_expanded(&season_target));
    assert!(component.browser.viewport_offset() < component.browser.visible_targets().len());
}
