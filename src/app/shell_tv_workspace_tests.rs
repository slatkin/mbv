use super::*;
use crate::app::components::{Msg, ShellRequest, TvWorkspaceComponent};
use crate::app::render::make_movie_app;
use ratatui::layout::Rect;
use tuirealm::component::AppComponent;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

fn mounted_tv_model() -> Model {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
    }
    app.layout.main.tv_wide_right_area = Rect::new(40, 0, 60, 20);
    let mut model = Model::new(app);
    model.sync_tv_workspace();
    model
}

#[test]
fn push_tv_workspace_content_projects_selected_series_on_mount() {
    let model = mounted_tv_model();
    let id = model
        .tv_workspace_id
        .as_ref()
        .expect("TV workspace mounted");
    let component = model
        .application
        .get_component(id)
        .expect("TV workspace component mounted")
        .as_any()
        .downcast_ref::<TvWorkspaceComponent>()
        .expect("TV workspace component type");
    assert_eq!(component.selected_item_id(), Some("movie-focused".into()));
}

/// keep-destination-components-mounted task 3.1: the TV workspace stays
/// mounted across wide→narrow→wide layout resizes (keep-mounted, D1).
/// Because the component is never unmounted/remounted, its private
/// pane/cursor state survives the round trip: after re-point it is not
/// reset to the fresh-mount default (cursor 0 / Series pane).
#[test]
fn tv_workspace_stays_mounted_and_preserves_pane_cursor_across_resize() {
    let mut model = mounted_tv_model();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");
    let mounted_before = model.application.mounted(&id);
    assert!(mounted_before);

    // Move the component cursor to row 1 (movie-second) and Enter to the
    // Episodes pane: both are non-default state that a remount would reset.
    let move_request = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        move_request,
        Some(Msg::Shell(ShellRequest::TvMoveRows { rows: 1 }))
    ));
    let selected_id = |model: &mut Model| {
        model
            .application
            .get_component(&model.tv_workspace_id.clone().expect("TV workspace mounted"))
            .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
            .and_then(TvWorkspaceComponent::selected_item_id)
    };
    assert_eq!(selected_id(&mut model), Some("movie-second".into()));

    // Seed detail for the selected series (movie-second, the row the
    // component cursor sits on after Down) and Enter it so the component
    // enters the Episodes pane (episode_cursor becomes Some(0)).
    let mut season = crate::app::tests::make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = crate::app::tests::make_item("Episode 1", "Episode");
    episode.id = "episode-1".into();
    episode.series_id = "movie-second".into();
    let mut episodes = std::collections::HashMap::new();
    episodes.insert("season-1".into(), vec![episode]);
    model.app.series_detail_cache.insert(
        "movie-second".into(),
        crate::app::SeriesDetail {
            seasons: vec![season],
            episodes,
        },
    );
    model.push_tv_workspace_content();
    let enter = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace mounted")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        enter,
        Some(Msg::Shell(ShellRequest::TvActivate { .. }))
    ));
    assert_eq!(
        model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<TvWorkspaceComponent>()
            .unwrap()
            .episode_activation_selection(),
        Some(("movie-second".into(), 0, 0)),
        "Enter must put the component in the Episodes pane for its selected series"
    );

    // Narrow: the mount gate (is_wide_tv_active) returns None, so the
    // pointer is cleared but the component stays mounted (keep-mounted).
    model.app.layout.main.tv_wide_right_area = Rect::default();
    model.sync_tv_workspace();
    assert_eq!(model.tv_workspace_id, None);
    assert!(
        model.application.mounted(&id),
        "the TV workspace must stay mounted across the narrow resize"
    );

    // Wide again: the same component is re-pointed, not remounted.
    model.app.layout.main.tv_wide_right_area = Rect::new(40, 0, 60, 20);
    model.sync_tv_workspace();
    assert_eq!(
        model.tv_workspace_id.as_ref(),
        Some(&id),
        "re-point must restore the same component id"
    );
    assert!(model.application.mounted(&id));
    assert_eq!(
        selected_id(&mut model),
        Some("movie-second".into()),
        "the component cursor must survive the wide→narrow→wide round trip"
    );
    assert_eq!(
        model
            .application
            .get_component(&id)
            .unwrap()
            .as_any()
            .downcast_ref::<TvWorkspaceComponent>()
            .unwrap()
            .episode_activation_selection(),
        Some(("movie-second".into(), 0, 0)),
        "the Episodes pane must survive the wide→narrow→wide round trip"
    );
}

#[test]
fn typed_tv_requests_keep_component_cursor_authoritative() {
    // Each cursor-moving request is driven from a *fresh* mount so no
    // action's side effects can leak into the next: a chained sequence
    // (Down, End, ']', Right) would let a later assertion pass because of
    // the preceding action's state (e.g. End after Down both land on the
    // last row, and ']' clears the pill-cycled list). A fresh mount
    // guarantees component cursor 0, pane Series, and App browse cursor 0
    // before every key, isolating exactly one request per model.
    fn drive(code: Key) -> (Model, ShellRequest) {
        let mut model = mounted_tv_model();
        // Letter pills need a captured total for TvCycleLetterPill to run.
        model.app.libs[0].library_total = Some(1000);
        let id = model.tv_workspace_id.clone().expect("TV workspace mounted");
        let request = model
            .application
            .get_component_mut(&id)
            .expect("TV workspace component mounted")
            .as_any_mut()
            .downcast_mut::<TvWorkspaceComponent>()
            .expect("TV workspace component type")
            .on(&Event::Keyboard(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
            }));
        let Some(Msg::Shell(request)) = request else {
            panic!("TV key {code:?} must produce a typed shell request");
        };
        (model, request)
    }

    // TvMoveRows (Down): the component cursor moves 0 -> 1 (movie-second)
    // and the emitted request carries rows: 1; App's browse cursor stays 0
    // — the removed mirror's former effect would have written 1 here.
    let (mut model, request) = drive(Key::Down);
    assert!(matches!(request, ShellRequest::TvMoveRows { rows: 1 }));
    model.handle_tv_request(request);
    assert_eq!(
        model.app.libs[0].nav_stack[0].cursor, 0,
        "TvMoveRows must not write the component cursor into App's browse level"
    );
    let selected_id = model
        .application
        .get_component(&model.tv_workspace_id.clone().expect("TV workspace mounted"))
        .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
        .and_then(TvWorkspaceComponent::selected_item_id);
    assert_eq!(
        selected_id,
        Some("movie-second".into()),
        "the component cursor must have moved while App's cursor stayed put"
    );

    // TvJumpCursor (End): fresh mount again — the component jumps to the
    // last row; the request carries to_end: true (distinct from Home's
    // to_end: false); App's browse cursor still stays 0.
    let (mut model, request) = drive(Key::End);
    assert!(matches!(
        request,
        ShellRequest::TvJumpCursor { to_end: true }
    ));
    model.handle_tv_request(request);
    assert_eq!(
        model.app.libs[0].nav_stack[0].cursor, 0,
        "TvJumpCursor must not write the component cursor into App's browse level"
    );
    let selected_id = model
        .application
        .get_component(&model.tv_workspace_id.clone().expect("TV workspace mounted"))
        .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
        .and_then(TvWorkspaceComponent::selected_item_id);
    assert_eq!(
        selected_id,
        Some("movie-second".into()),
        "the component cursor must have jumped while App's cursor stayed put"
    );

    // TvCycleLetterPill (']' in the Series pane): fresh mount with a
    // captured total — the pill advances the letter filter; the request
    // carries delta: 1 (distinct from '[''s delta: -1); App's browse
    // cursor stays 0 (select_letter_pill's own reset is not a mirror).
    let (mut model, request) = drive(Key::Char(']'));
    assert!(matches!(
        request,
        ShellRequest::TvCycleLetterPill { delta: 1 }
    ));
    model.handle_tv_request(request);
    assert_eq!(
        model.app.libs[0].nav_stack[0].cursor, 0,
        "TvCycleLetterPill must not write the component cursor into App's browse level"
    );

    // TvMoveColumn (Right): fresh mount again — the pane moves to
    // Episodes; the request carries delta: 1 (distinct from Left's
    // delta: -1); App's browse cursor still stays 0.
    let (mut model, request) = drive(Key::Right);
    assert!(matches!(request, ShellRequest::TvMoveColumn { delta: 1 }));
    model.handle_tv_request(request);
    assert_eq!(
        model.app.libs[0].nav_stack[0].cursor, 0,
        "TvMoveColumn must not write the component cursor into App's browse level"
    );
}

#[test]
fn tv_series_enter_carries_the_component_selected_item() {
    let mut model = mounted_tv_model();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");

    // Park the App browse cursor somewhere other than the component's
    // selection: the emitted TvActivate must carry the component's own
    // selected Series, not the (mirrored) App cursor's item.
    model.app.libs[0].nav_stack[0].cursor = 1;
    let request = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace component mounted")
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .expect("TV workspace component type")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));
    let Some(Msg::Shell(ShellRequest::TvActivate { item })) = request else {
        panic!("series Enter must emit TvActivate carrying the selected item");
    };
    assert_eq!(
        item.id, "movie-focused",
        "TvActivate must carry the component's selected Series, not the stale App cursor"
    );
    assert_eq!(item.item_type, "Series");
}

#[test]
fn push_tv_workspace_content_uses_component_selection_over_stale_app_cursor() {
    let mut model = mounted_tv_model();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");

    // Seed detail for the second series so the pushed snapshot's target is
    // observable via the component's selected_series_snapshot().
    model.app.series_detail_cache.insert(
        "movie-second".into(),
        crate::app::SeriesDetail {
            seasons: vec![],
            episodes: std::collections::HashMap::new(),
        },
    );

    // Component-local selection: move the component cursor onto the second
    // series (index 1) while the App browse cursor stays at 0 — the
    // divergence the removed mirror used to hide.
    let moved = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace component mounted")
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .expect("TV workspace component type")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        moved,
        Some(Msg::Shell(ShellRequest::TvMoveRows { rows: 1 }))
    ));
    assert_eq!(
        model.app.libs[0].nav_stack[0].cursor, 0,
        "App browse cursor must stay stale (no mirror)"
    );

    // The push must derive the Series snapshot from the component's
    // authoritative selection, not the stale App cursor.
    model.push_tv_workspace_content();
    let pushed = model
        .application
        .get_component(&id)
        .and_then(|component| component.as_any().downcast_ref::<TvWorkspaceComponent>())
        .and_then(TvWorkspaceComponent::selected_series_snapshot)
        .map(|item| item.id.clone());
    assert_eq!(
        pushed,
        Some("movie-second".into()),
        "pushed TV detail must follow the component selection, not the stale App cursor"
    );
}

#[test]
fn tv_episode_activation_uses_component_cursors_and_cached_season_id() {
    let mut model = mounted_tv_model();
    let mut season_one = crate::app::tests::make_item("Season 1", "Season");
    season_one.id = "season-1".into();
    let mut season_two = crate::app::tests::make_item("Season 2", "Season");
    season_two.id = "season-2".into();
    let mut episode = crate::app::tests::make_item("Episode 2", "Episode");
    episode.id = "episode-2".into();
    episode.series_id = "movie-focused".into();
    let mut episodes = std::collections::HashMap::new();
    episodes.insert("season-2".into(), vec![episode]);
    model.app.series_detail_cache.insert(
        "movie-focused".into(),
        crate::app::SeriesDetail {
            seasons: vec![season_one, season_two],
            episodes,
        },
    );
    model.push_tv_workspace_content();
    let id = model.tv_workspace_id.clone().expect("TV workspace mounted");

    let enter_series = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace component mounted")
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .expect("TV workspace component type")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));
    let Some(Msg::Shell(enter_series)) = enter_series else {
        panic!("series Enter must produce a typed request");
    };
    model.handle_tv_request(enter_series);

    let season = model
        .application
        .get_component_mut(&id)
        .expect("TV workspace component mounted")
        .as_any_mut()
        .downcast_mut::<TvWorkspaceComponent>()
        .expect("TV workspace component type")
        .on(&Event::Keyboard(KeyEvent {
            code: Key::Char(']'),
            modifiers: KeyModifiers::NONE,
        }));
    assert!(matches!(
        season,
        Some(Msg::Shell(ShellRequest::TvSeasonMove { delta: 1 }))
    ));
    // Make the App library cursor stale after the component has selected
    // the series; episode activation must not consult that cursor.
    model.app.libs[0].nav_stack[0].cursor = 1;

    let episode_request = model
        .application
        .get_component(&id)
        .expect("TV workspace component mounted")
        .as_any()
        .downcast_ref::<TvWorkspaceComponent>()
        .expect("TV workspace component type")
        .episode_activation_selection();
    assert_eq!(episode_request, Some(("movie-focused".into(), 1, 0)));
    model.handle_tv_request(ShellRequest::TvEpisodeActivate);
    assert!(model.app.play_tv_episode("movie-focused", 1, 0));
    assert!(!model.app.play_tv_episode("movie-focused", 0, 0));
    assert!(!model.app.play_tv_episode("movie-focused", 1, 1));
    assert!(!model.app.play_tv_episode("missing-series", 1, 0));

    // TvBack after activation must restore the parent series-list cursor
    // via go_back's own parent_id lookup — not via any mirror. The stale
    // App cursor (1, "movie-second") diverges from the component's
    // selection ("movie-focused" at row 0). Append a third series so the
    // child's parent_id can target a *discriminating nonzero row*: the
    // seasons child's parent_id "movie-third" restores the series cursor
    // to row 2, so a reset-to-0 implementation (0), a child-cursor
    // implementation (99), and a stale-mirror implementation (1) all
    // fail.
    let mut third = crate::app::tests::make_item("Third Series", "Series");
    third.id = "movie-third".into();
    model.app.libs[0].nav_stack[0].items.push(third);
    assert_eq!(
        model.app.libs[0].nav_stack[0].cursor, 1,
        "the stale App cursor must still diverge before TvBack"
    );
    model.app.libs[0].nav_stack.push(crate::app::BrowseLevel {
        parent_id: "movie-third".into(),
        title: "Seasons".into(),
        items: vec![],
        total_count: 0,
        cursor: 99,
        scroll: 0,
        item_types: Some("Season".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    });
    model.handle_tv_request(ShellRequest::TvBack);
    assert_eq!(
        model.app.libs[0].nav_stack.len(),
        1,
        "TvBack must pop the seasons child level"
    );
    assert_eq!(
            model.app.libs[0].nav_stack[0].cursor, 2,
            "TvBack restores the series cursor by parent_id (row of movie-third), not a reset 0, the popped child cursor 99, or the stale 1"
        );
}

/// Build a two-level stack: a Series parent list whose cursor is parked
/// off the child's parent, plus an empty Seasons child whose `parent_id`
/// points back at parent item 0. Used to prove `go_back` restores the
/// parent cursor by `parent_id`, never by the popped (mirrored) cursor.
/// Build a two-level stack: a Series parent list (three rows, cursor
/// parked off the child's parent) plus a Seasons child whose `parent_id`
/// targets a discriminating nonzero parent row (index 2) and whose items
/// contain the component's selected id so the mirror actually mutates
/// this (last) level's cursor. Used to prove `go_back` restores the parent
/// cursor by `parent_id`, never by the popped (mirrored) cursor.
fn tv_two_level_model() -> Model {
    let mut model = mounted_tv_model();
    let mut third = crate::app::tests::make_item("Third Series", "Series");
    third.id = "movie-third".into();
    model.app.libs[0].nav_stack[0].items.push(third);
    model.app.libs[0].nav_stack[0].cursor = 0;
    let mut mirror_target = crate::app::tests::make_item("S", "Season");
    mirror_target.id = "movie-focused".into();
    model.app.libs[0].nav_stack.push(crate::app::BrowseLevel {
        parent_id: "movie-third".into(),
        title: "Seasons".into(),
        items: vec![
            crate::app::tests::make_item("Season A", "Season"),
            mirror_target,
        ],
        total_count: 2,
        cursor: 0,
        scroll: 0,
        item_types: Some("Season".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    });
    model
}

/// Build a three-level stack: Series parent -> Seasons child -> Episodes
/// grandchild, so a single `go_back` from Episodes must auto-skip the
/// Season level and still restore the Series cursor by `parent_id`.
/// Build a three-level stack: Series parent (three rows) -> Seasons child
/// -> Episodes grandchild, where the Season level's `parent_id` targets a
/// discriminating nonzero parent row and the Episodes level's items
/// contain the component's selected id so the mirror mutates it. A single
/// `go_back` from Episodes must auto-skip the Season level and still
/// restore the Series cursor by `parent_id`.
fn tv_season_skip_model() -> Model {
    let mut model = mounted_tv_model();
    let mut third = crate::app::tests::make_item("Third Series", "Series");
    third.id = "movie-third".into();
    model.app.libs[0].nav_stack[0].items.push(third);
    model.app.libs[0].nav_stack[0].cursor = 0;
    let mut season = crate::app::tests::make_item("Season 1", "Season");
    season.id = "season-1".into();
    model.app.libs[0].nav_stack.push(crate::app::BrowseLevel {
        parent_id: "movie-third".into(),
        title: "Seasons".into(),
        items: vec![season],
        total_count: 1,
        cursor: 0,
        scroll: 0,
        item_types: Some("Season".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    });
    let mut mirror_target = crate::app::tests::make_item("E", "Episode");
    mirror_target.id = "movie-focused".into();
    model.app.libs[0].nav_stack.push(crate::app::BrowseLevel {
        parent_id: "season-1".into(),
        title: "Episodes".into(),
        items: vec![
            crate::app::tests::make_item("Episode 1", "Episode"),
            mirror_target,
        ],
        total_count: 2,
        cursor: 0,
        scroll: 0,
        item_types: Some("Episode".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    });
    model
}

#[test]
fn activate_selected_series_resolves_mirrored_cursor_and_guards_series() {
    let mut model = mounted_tv_model();

    // Divergence: park App's cursor on "movie-second" (index 1) while the
    // mounted component still selects "movie-focused". Without the mirror
    // the resolution follows the stale App cursor; writing the cursor
    // back to 0 (the mirror's former effect) realigns it to the
    // component's selection.
    model.app.libs[0].nav_stack[0].cursor = 1;
    assert_eq!(
        model.app.selected_series_item(0).map(|i| i.id),
        Some("movie-second".into()),
        "stale App cursor resolves a different Series before realignment"
    );
    model.app.libs[0].nav_stack[0].cursor = 0;
    assert_eq!(
        model.app.selected_series_item(0).map(|i| i.id),
        Some("movie-focused".into()),
        "realigned cursor resolves the component's selected Series"
    );

    // Wide TV layout => enter_series_selection targets the component's
    // Series (asserted by the resolved target, not merely the bool).
    let wide_target = model.app.selected_series_item(0).expect("series");
    assert_eq!(wide_target.id, "movie-focused");
    assert!(model.app.activate_selected_series(0));

    // Narrow layout => open_series_selection_modal targets the same
    // Series, proven by the modal's Series source id.
    model.app.layout.main.tv_wide_right_area = ratatui::layout::Rect::default();
    model.app.libs[0].nav_stack[0].cursor = 0;
    model.app.activate_selected_series(0);
    match model.app.pending_overlay.as_ref() {
        Some(crate::app::types_overlay::OverlayRequest::SelectionModal(modal)) => {
            if let crate::app::types_selection_modal::SelectionModalSource::Series { series_id } =
                &modal.source
            {
                assert_eq!(
                    series_id.as_str(),
                    "movie-focused",
                    "narrow activation must target the component's selected Series"
                );
            } else {
                panic!("narrow activation must open a Series selection modal");
            }
        }
        _ => panic!("narrow layout must open the series selection modal"),
    }
    model.app.pending_overlay = None;

    // Guard 1: a non-tvshows collection_type rejects.
    model.app.libs[0].library.collection_type = "movies".into();
    assert!(!model.app.activate_selected_series(0));

    // Guard 2: a selected item that is not a Series rejects.
    model.app.libs[0].library.collection_type = "tvshows".into();
    model.app.libs[0].nav_stack[0].items[0].item_type = "Movie".into();
    assert!(!model.app.activate_selected_series(0));
}

#[test]
fn go_back_ignores_popped_level_cursor_and_restores_by_parent_id() {
    // Without a prior mirror: the popped child cursor is deliberately
    // stale (99), yet go_back restores the parent cursor to row 2 (the
    // position of the child's parent_id "movie-third") -- by parent_id,
    // not the stale 99 and not a reset 0.
    let mut no_mirror = tv_two_level_model();
    no_mirror.app.libs[0].nav_stack.last_mut().unwrap().cursor = 99;
    no_mirror.app.go_back(0);
    assert_eq!(no_mirror.app.libs[0].nav_stack.len(), 1);
    assert_eq!(no_mirror.app.libs[0].nav_stack[0].cursor, 2);

    // With a prior mirror call that actually mutates the popped child
    // cursor (to 1, the position of the component's selected id within the
    // child items): the restored parent cursor is still 2, identical and
    // not the mutated child cursor.
    let mut with_mirror = tv_two_level_model();
    with_mirror.app.libs[0].nav_stack.last_mut().unwrap().cursor = 99;
    with_mirror.app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
    assert_eq!(
        with_mirror.app.libs[0].nav_stack.last().unwrap().cursor,
        1,
        "mirror must mutate the popped child cursor to the component selection"
    );
    with_mirror.app.go_back(0);
    assert_eq!(with_mirror.app.libs[0].nav_stack.len(), 1);
    assert_eq!(with_mirror.app.libs[0].nav_stack[0].cursor, 2);

    // Season auto-skip: from the Episodes level, one go_back skips the
    // Season level and restores the Series cursor by parent_id (row 2),
    // despite a stale popped cursor (42).
    let mut skip = tv_season_skip_model();
    skip.app.libs[0].nav_stack.last_mut().unwrap().cursor = 42;
    skip.app.go_back(0);
    assert_eq!(skip.app.libs[0].nav_stack.len(), 1);
    assert_eq!(skip.app.libs[0].nav_stack[0].cursor, 2);
}

#[test]
fn cycle_letter_pill_derives_from_filter_not_cursor() {
    // A tvshows library large enough to surface letter pills, at its top
    // browse level with pill bucket 0 (A–C) selected.
    let mut model = mounted_tv_model();
    model.app.libs[0].library_total = Some(1000);
    model.app.libs[0].nav_stack[0].letter_filter =
        Some(crate::app::render::LetterFilter::for_index(0).unwrap());

    // Stale cursor: cycle_letter_pill must ignore `level.cursor` and
    // advance the filter 0 -> 1 purely from `letter_filter`.
    model.app.libs[0].nav_stack[0].cursor = 7;
    model.app.cycle_letter_pill(0, 1);
    let after_stale = model.app.libs[0].nav_stack[0].letter_filter.clone();
    assert_eq!(after_stale.as_ref().map(|f| f.index), Some(1));
    assert!(model.app.libs[0].nav_stack[0].loading);
    // select_letter_pill intentionally resets the level cursor to 0,
    // regardless of its prior (stale) value.
    assert_eq!(
        model.app.libs[0].nav_stack[0].cursor, 0,
        "select_letter_pill resets the level cursor regardless of its prior value"
    );

    // Fresh cursor at a different value: identical filter result, proving
    // the cycle never consults `level.cursor`.
    let mut fresh = mounted_tv_model();
    fresh.app.libs[0].library_total = Some(1000);
    fresh.app.libs[0].nav_stack[0].letter_filter =
        Some(crate::app::render::LetterFilter::for_index(0).unwrap());
    fresh.app.libs[0].nav_stack[0].cursor = 0;
    fresh.app.cycle_letter_pill(0, 1);
    assert_eq!(
        fresh.app.libs[0].nav_stack[0].letter_filter, after_stale,
        "cycle_letter_pill result must not depend on level.cursor"
    );
}
