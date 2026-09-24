use super::*;
use rstest::rstest;

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
fn expanded_show_keeps_loaded_children_when_selection_moves_to_another_show() {
    use crate::app::components::list::tree_browser::TreeOperation;
    use crate::app::components::tv_tree_target::TvTreeTarget;

    let mut season_a = make_item("Season 1", "Season");
    season_a.id = "season-a1".into();
    let mut season_b = make_item("Season 1", "Season");
    season_b.id = "season-b1".into();
    let detail_a = crate::app::SeriesDetail {
        seasons: vec![season_a],
        episodes: std::collections::HashMap::new(),
    };
    let detail_b = crate::app::SeriesDetail {
        seasons: vec![season_b],
        episodes: std::collections::HashMap::new(),
    };
    let shows = || vec![tv_show("Alpha", "show-a"), tv_show("Beta", "show-b")];
    let mut context = tv_tree_context(shows(), Some("show-a"), Some(detail_a.clone()), false);
    context.series_details = [
        ("show-a".to_string(), detail_a.clone()),
        ("show-b".to_string(), detail_b.clone()),
    ]
    .into_iter()
    .collect();
    let mut component = TvContent::new();
    component.set_content(context);
    let show_a = TvTreeTarget::Show("tv-id:6:show-a".into());
    component
        .browser
        .apply(TreeOperation::ToggleExpansionTarget(show_a.clone()));
    assert!(component.browser.is_expanded(&show_a));

    // The shell moves its selected-series projection to B while both details
    // stay cached; A's expanded branch must keep its loaded children.
    let mut context = tv_tree_context(shows(), Some("show-b"), Some(detail_b.clone()), false);
    context.series_details = [
        ("show-a".to_string(), detail_a),
        ("show-b".to_string(), detail_b),
    ]
    .into_iter()
    .collect();
    component.set_content(context);

    assert!(component.browser.is_expanded(&show_a));
    assert!(
        component
            .browser
            .node(&TvTreeTarget::Season {
                show: "tv-id:6:show-a".into(),
                season: "season-a1".into(),
                occurrence: 0,
            })
            .is_some(),
        "expanded show A keeps its loaded season after selection moves to B"
    );
    assert!(
        component
            .browser
            .node(&TvTreeTarget::Season {
                show: "tv-id:6:show-b".into(),
                season: "season-b1".into(),
                occurrence: 0,
            })
            .is_some(),
        "selected show B projects its own loaded season"
    );
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
