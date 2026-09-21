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
