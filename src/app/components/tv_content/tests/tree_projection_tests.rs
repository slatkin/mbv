use super::*;
use crate::app::components::list::tree_browser::TreeEntry;
use rstest::rstest;

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
        Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::TvTreeExpand { target } if *target == show_target)));
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
        Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::TvTreeExpand { target } if *target == season_target)));
}

#[rstest]
#[case::duplicate_season_ids(vec!["season-1", "season-1"], vec![])]
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
    use ratatui::backend::TestBackend;
    use ratatui::layout::Position;
    use ratatui::Terminal;
    use tuirealm::component::Component;

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
    component.set_content(context());
    component.browser.clamp_viewport_to(1);

    assert_eq!(component.browser.selected_target(), Some(&episode_target));
    assert!(component.browser.is_expanded(&show_target));
    assert!(component.browser.is_expanded(&season_target));
    let area = ratatui::layout::Rect::new(0, 0, 20, 1);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| component.browser.view(frame, area))
        .unwrap();
    let selected_row = component
        .browser
        .selected_row_rect()
        .expect("the selected episode has painted geometry");
    assert!(
        selected_row.y >= area.y && selected_row.bottom() <= area.bottom(),
        "the selected episode must remain inside the painted viewport"
    );
    assert_eq!(
        component
            .browser
            .resolve_current_point(Position::new(selected_row.x, selected_row.y)),
        Some(&episode_target),
        "the painted viewport must resolve the selected episode"
    );
}
