use super::*;

#[test]
fn tv_series_rows_use_the_canonical_state_in_both_geometries() {
    let mut watched = make_item("Watched Series", "Series");
    watched.id = "series-watched".into();
    watched.played = true;

    let mut in_progress = make_item("In Progress Series", "Series");
    in_progress.id = "series-in-progress".into();
    in_progress.runtime_ticks = 1000;
    in_progress.playback_position_ticks = 500;

    let content = TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![watched.clone(), in_progress.clone()], 0),
        None,
        None,
        0,
        None,
        false,
    );

    let mut narrow = TvContent::new();
    narrow.set_is_wide(false);
    narrow.set_content(content.clone());
    let narrow_states = narrow.test_row_semantic_states();
    assert!(narrow_states
        .iter()
        .any(|state| matches!(state, MediaSemanticState::Played)));
    assert!(narrow_states
        .iter()
        .any(|state| matches!(state, MediaSemanticState::Active { .. })));

    let mut wide = TvContent::new();
    wide.set_is_wide(true);
    wide.set_content(content);
    let wide_states = wide.test_row_semantic_states();
    assert!(
        wide_states
            .iter()
            .any(|state| matches!(state, MediaSemanticState::Played)),
        "a played series paints the shared played state in Wide too"
    );
    assert!(
        wide_states
            .iter()
            .any(|state| matches!(state, MediaSemanticState::Active { .. })),
        "the one canonical derivation dims in-progress rows in Wide too"
    );
}

#[rstest]
#[case(
    Some(301),
    None,
    vec!["Latest", "Upcoming", "A-I", "J-R", "S-Z"],
    0
)]
#[case(Some(300), None, vec!["Latest", "Upcoming", "All"], 2)]
#[case(Some(301), Some(TvContentMode::Range(1)), vec!["Latest", "Upcoming", "A-I", "J-R", "S-Z"], 3)]
fn tv_mode_selector_composes_the_threshold_rows_in_order(
    #[case] library_total: Option<usize>,
    #[case] mode: Option<TvContentMode>,
    #[case] expected_pills: Vec<&str>,
    #[case] expected_active: usize,
) {
    let mut list = LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0);
    list.library_total = library_total;
    let mut context = TvWideRenderCtx::new(list, None, None, 0, None, true);
    context.set_tv_content_mode(mode);
    let mut owner = TvContent::new();
    owner.set_content(context);

    let selector = owner.content().selector.expect("TV mode selector");
    assert_eq!(
        selector.pills,
        expected_pills
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    );
    assert_eq!(selector.active, Some(expected_active));
    if library_total.is_some_and(|total| total > crate::app::render::LIBRARY_PILL_THRESHOLD) {
        assert!(!selector.pills.iter().any(|pill| pill == "All"));
    }
}

#[test]
fn tv_latest_marker_reads_shell_projection_and_acknowledgement() {
    let mut list = LibraryListRenderCtx::from_items(vec![make_item("Episode", "Episode")], 0);
    list.library_total = Some(301);
    let mut context = TvWideRenderCtx::new(list, None, None, 0, None, true);
    context.set_tv_content_mode(Some(TvContentMode::Latest));
    let mut owner = TvContent::new();
    owner.set_latest_marker(true);
    owner.set_content(context.clone());
    assert!(owner.content().selector.expect("TV mode selector").markers[0]);

    owner.set_latest_marker(false);
    owner.set_content(context);
    assert!(!owner.content().selector.expect("TV mode selector").markers[0]);
}

#[test]
fn tv_latest_mode_projects_feed_episodes_without_series_workspace() {
    let mut first = make_item("Latest Episode", "Episode");
    first.id = "latest-episode".into();
    let mut list = LibraryListRenderCtx::from_items(vec![first], 0);
    list.library_total = Some(301);
    let mut context = TvWideRenderCtx::new(list, None, None, 0, None, true);
    context.set_tv_content_mode(Some(TvContentMode::Latest));
    let mut owner = TvContent::new();
    owner.set_content(context);

    assert_eq!(owner.selected_series_snapshot(), None);
    assert_eq!(
        owner.selected_episode_item().map(|episode| episode.id),
        Some("latest-episode".into())
    );
    let content = owner.content();
    assert_eq!(content.selector.expect("TV mode selector").active, Some(0));
    assert!(
        content.hero.is_none(),
        "flat Latest has no series workspace"
    );
}

#[test]
fn tv_latest_and_upcoming_enter_play_the_selected_episode_directly() {
    for (mode, id) in [
        (TvContentMode::Latest, "latest-episode"),
        (TvContentMode::Upcoming, "upcoming-episode"),
    ] {
        let mut episode = make_item("Episode", "Episode");
        episode.id = id.into();
        let mut list = LibraryListRenderCtx::from_items(vec![episode], 0);
        list.library_total = Some(301);
        let mut context = TvWideRenderCtx::new(list, None, None, 0, None, true);
        context.set_tv_content_mode(Some(mode));
        let mut owner = TvContent::new();
        owner.set_content(context);

        let activation = owner.test_key(&KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        });
        assert!(matches!(
            activation,
            Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::TvEpisodeActivate { episode } if episode.id == id)));
        assert!(owner.selected_series_snapshot().is_none());
        assert!(owner.content().hero.is_none());
    }
}

#[test]
fn tv_flat_episode_click_moves_the_browser_carrier_without_entering_workspace() {
    let mut first = make_item("Episode A", "Episode");
    first.id = "episode-a".into();
    let mut second = make_item("Episode B", "Episode");
    second.id = "episode-b".into();
    let mut owner = TvContent::new();
    let mut list = LibraryListRenderCtx::from_items(vec![first, second], 0);
    list.library_total = Some(301);
    let mut context = TvWideRenderCtx::new(list, None, None, 0, None, true);
    context.set_tv_content_mode(Some(TvContentMode::Latest));
    owner.set_content(context);
    let mut panel = panel_with(owner, true);
    paint(&mut panel, 100, 20);
    let list_area = panel.test_wide_geometry().unwrap().list_area;

    let click = panel.on(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        list_area.x,
        list_area.y + 1,
    ));
    assert!(matches!(
        click,
        Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::TvHitClick {
            hit: TvHit::EpisodeRow(ref target),
        } if target == "episode-b")));
    assert_eq!(tv(&panel).selected_item_id(), Some("episode-b".into()));
    assert!(!tv(&panel).episode_pane_focused());
}
