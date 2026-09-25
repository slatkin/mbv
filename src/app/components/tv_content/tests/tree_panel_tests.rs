use super::*;
use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::Terminal;
use rstest::rstest;
use tuirealm::event::{KeyEvent, KeyModifiers};

fn tree_point<Target: PartialEq>(browser: &TreeBrowser<Target>, target: &Target) -> Position {
    (0..24)
        .flat_map(|y| (0..80).map(move |x| Position::new(x, y)))
        .find(|point| browser.resolve_current_point(*point) == Some(target))
        .expect("tree target is painted in the test frame")
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
