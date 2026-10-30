use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tuirealm::event::KeyModifiers;

fn tree_point<Target: PartialEq>(browser: &TreeBrowser<Target>, target: &Target) -> Position {
    (0..24)
        .flat_map(|y| (0..80).map(move |x| Position::new(x, y)))
        .find(|point| browser.resolve_current_point(*point) == Some(target))
        .expect("tree target is painted in the test frame")
}

fn assert_settled_show_tree(
    component: &TvContent,
    show: &TvTreeTarget,
    season: &TvTreeTarget,
    episode: &TvTreeTarget,
) {
    assert_eq!(component.browser.selected_target(), Some(episode));
    assert!(component.browser.is_expanded(show));
    assert!(component.browser.is_expanded(season));
}

#[test]
fn flat_modes_and_inline_search_preserve_the_settled_show_tree() {
    use crate::library_panel::ListSlot;
    use crate::list::tree_browser::TreeOperation;
    use mbv_queue::TvContentMode;
    use mbv_ui_msg::TvTreeTarget;

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
            Some(mbv_ui_model::browse::SeriesDetail {
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
        assert_settled_show_tree(&component, &show, &season_target, &episode_target);
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
    assert_settled_show_tree(&component, &show, &season_target, &episode_target);

    assert!(
        component
            .test_key(&KeyEvent {
                code: Key::Char('/'),
                modifiers: KeyModifiers::NONE,
            })
            .is_some()
    );
    let mut context = show_context();
    context.set_tv_content_mode(Some(TvContentMode::All));
    component.set_content(context);
    assert!(matches!(
        component.panel_content().list,
        ListSlot::Search(_)
    ));
    assert_settled_show_tree(&component, &show, &season_target, &episode_target);

    component.test_key(&KeyEvent {
        code: Key::Esc,
        modifiers: KeyModifiers::NONE,
    });
    assert!(!component.inline_search_active());
    let panel_content = component.panel_content();
    assert!(panel_content.selector.is_some());
    assert!(!matches!(panel_content.list, ListSlot::Search(_)));
    assert_settled_show_tree(&component, &show, &season_target, &episode_target);
}

#[test]
fn tree_episode_double_click_uses_its_show_target_when_selection_is_stale() {
    use crate::library_panel::LibraryPanel;
    use crate::list::tree_browser::TreeOperation;
    use crate::media_list::MediaListSurfaceInput;
    use mbv_ui_model::library::LibraryKey;
    use mbv_ui_msg::TvTreeTarget;
    use ratatui::layout::Rect;
    use tuirealm::component::Component;

    let show = tv_show("Alpha", "show-a");
    let other_show = tv_show("Beta", "show-b");
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = make_item("Pilot", "Episode");
    episode.id = "episode-1".into();
    episode.series_id = show.id.clone();
    let detail = mbv_ui_model::browse::SeriesDetail {
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
        service: mbv_queue::ServiceKind::Emby,
        library_id: "lib-tv".into(),
        kind: mbv_ui_model::library::LibraryKind::TvShows,
    };
    let mut panel = LibraryPanel::new();
    panel.insert_owner(key.clone(), Box::new(owner));
    panel.set_active(Some(key.clone()));
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, Rect::new(0, 0, 80, 20)))
        .unwrap();
    let row = tree_point(
        &panel
            .owner(&key)
            .and_then(|owner| owner.as_any().downcast_ref::<TvContent>())
            .unwrap()
            .browser,
        &episode_target,
    );

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
        Some(Msg::Shell(ref shell_boxed))
             if matches!(shell_boxed.as_ref(), ShellRequest::TvEpisodeActivate { episode: selected } if selected.id == episode.id && selected.series_id == show.id)));
}

#[test]
fn show_tree_double_click_toggles_the_branch() {
    use crate::library_panel::LibraryPanel;
    use crate::media_list::MediaListSurfaceInput;
    use mbv_ui_model::library::LibraryKey;
    use mbv_ui_msg::TvTreeTarget;
    use ratatui::layout::Rect;
    use tuirealm::component::Component;

    // Guards the show-row arm of `show_tree_list_double_click`: a parent
    // row's double click toggles its branch (the shared tree double-click
    // rule) instead of re-sending an activate the shell resolves against a
    // cached series detail, which was a visible no-op.
    let show = tv_show("Alpha", "show-a");
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let detail = mbv_ui_model::browse::SeriesDetail {
        seasons: vec![season],
        episodes: std::collections::HashMap::new(),
    };
    let mut owner = TvContent::new();
    owner.set_is_wide(false);
    owner.set_content(tv_tree_context(
        vec![show],
        Some("show-a"),
        Some(detail),
        false,
    ));
    let show_target = TvTreeTarget::Show("tv-id:6:show-a".into());
    let key = LibraryKey::Service {
        service: mbv_queue::ServiceKind::Emby,
        library_id: "lib-tv".into(),
        kind: mbv_ui_model::library::LibraryKind::TvShows,
    };
    let mut panel = LibraryPanel::new();
    panel.insert_owner(key.clone(), Box::new(owner));
    panel.set_active(Some(key.clone()));
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| Component::view(&mut panel, frame, Rect::new(0, 0, 80, 20)))
        .unwrap();
    let row = tree_point(
        &panel
            .owner(&key)
            .and_then(|owner| owner.as_any().downcast_ref::<TvContent>())
            .unwrap()
            .browser,
        &show_target,
    );
    let at = ratatui::layout::Position { x: row.x, y: row.y };

    {
        let owner = panel
            .owner_mut(&key)
            .and_then(|owner| owner.as_any_mut().downcast_mut::<TvContent>())
            .unwrap();
        let expand = owner.show_tree_list_event(MediaListSurfaceInput::DoubleClick(at));
        assert!(matches!(
            expand,
            Some(Msg::Shell(ref shell_boxed))
                if matches!(shell_boxed.as_ref(), ShellRequest::TvTreeExpand { target } if *target == show_target)
        ));
        assert!(owner.browser.is_expanded(&show_target));
    };

    // The shell re-pushes and repaints after `TvTreeExpand`, so the collapse
    // click resolves against refreshed geometry like the live flow does.
    terminal
        .draw(|frame| Component::view(&mut panel, frame, Rect::new(0, 0, 80, 20)))
        .unwrap();

    let owner = panel
        .owner_mut(&key)
        .and_then(|owner| owner.as_any_mut().downcast_mut::<TvContent>())
        .unwrap();
    let collapse = owner.show_tree_list_event(MediaListSurfaceInput::DoubleClick(at));
    assert!(collapse.is_none());
    assert!(!owner.browser.is_expanded(&show_target));
}
