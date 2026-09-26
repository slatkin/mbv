use super::*;
use std::collections::HashMap;

#[test]
fn tv_enter_selects_first_episode_for_activation() {
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-id".into();
    let mut episode = make_item("Episode 1", "Episode");
    episode.id = "episode-id".into();
    let detail = crate::app::SeriesDetail {
        seasons: vec![season],
        episodes: [("season-id".into(), vec![episode])].into_iter().collect(),
    };
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series),
        Some(detail),
        0,
        None,
        false,
    ));

    let key = |code| KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    };
    // Enter on the selected series is the only way into the Episodes pane:
    // arrows never move focus between wide-library panes.
    assert!(matches!(
        owner.on_key(&key(Key::Enter)),
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::TvActivate { .. })));
    assert_eq!(
        owner.selected_episode_item().map(|episode| episode.id),
        Some("episode-id".into())
    );
    assert!(matches!(
        owner.on_key(&key(Key::Enter)),
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::TvEpisodeActivate { .. })));
}

#[test]
fn tv_right_does_not_move_focus_between_panes() {
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-id".into();
    let mut episode = make_item("Episode 1", "Episode");
    episode.id = "episode-id".into();
    let detail = crate::app::SeriesDetail {
        seasons: vec![season],
        episodes: [("season-id".into(), vec![episode])].into_iter().collect(),
    };
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series),
        Some(detail),
        0,
        None,
        false,
    ));

    let key = |code| KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    };
    assert!(matches!(
       owner.on_key(&key(Key::Right)),
       Some(Msg::Shell(ref shell_boxed))
    if matches!(shell_boxed.as_ref(), ShellRequest::TvTreeExpand {
           target: TvTreeTarget::Show(_)
       })));
    assert!(matches!(
        owner.on_key(&key(Key::Left)),
        Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
    ));
    // Right expands the selected tree branch, not the Episodes pane; Enter
    // still activates the selected show.
    assert!(matches!(
        owner.on_key(&key(Key::Enter)),
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::TvActivate { .. })));
}

#[test]
fn tv_content_refresh_clamps_episode_cursor_and_handles_empty_season() {
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    let mut season = make_item("Season 1", "Season");
    season.id = "season-id".into();
    let episode = |name: &str, id: &str| {
        let mut item = make_item(name, "Episode");
        item.id = id.into();
        item
    };
    let detail = |episodes| crate::app::SeriesDetail {
        seasons: vec![season.clone()],
        episodes: [("season-id".into(), episodes)].into_iter().collect(),
    };
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series.clone()),
        Some(detail(vec![
            episode("Episode 1", "episode-1"),
            episode("Episode 2", "episode-2"),
            episode("Episode 3", "episode-3"),
        ])),
        0,
        None,
        false,
    ));
    let key = |code| KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    };
    owner.on_key(&key(Key::Enter));
    owner.on_key(&key(Key::Down));
    owner.on_key(&key(Key::Down));
    assert_eq!(
        owner.selected_episode_item().map(|episode| episode.id),
        Some("episode-3".into())
    );

    // An unavailable detail refresh must not erase the mounted component's
    // local episode cursor while the data is loading.
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series.clone()),
        None,
        0,
        Some(2),
        false,
    ));
    assert_eq!(
        owner.episode_cursor(),
        2,
        "an unavailable detail refresh preserves the episode owner's cursor"
    );

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series.clone()),
        Some(detail(vec![episode("Episode 1", "episode-1")])),
        0,
        Some(2),
        false,
    ));
    assert_eq!(
        owner.selected_episode_item().map(|episode| episode.id),
        Some("episode-1".into())
    );
    assert!(matches!(
        owner.on_key(&KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE
        }),
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::TvEpisodeActivate { .. })));

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series),
        Some(detail(Vec::new())),
        0,
        Some(0),
        false,
    ));
    assert_eq!(owner.selected_episode_item(), None);
}

#[test]
fn tv_keyboard_leaves_key_unclaimed_when_queue_is_focused() {
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(
            vec![
                make_item("Series A", "Series"),
                make_item("Series B", "Series"),
            ],
            0,
        ),
        None,
        None,
        0,
        None,
        true,
    ));
    // Focus lives on the panel (task 8.4): an unfocused panel forwards
    // nothing to its owner, so the key is unclaimed.
    let mut panel = panel_with(owner, false);
    paint(&mut panel, 100, 20);

    let message = panel.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(message, None);
    assert_eq!(tv(&panel).cursor(), 0);
}

#[test]
fn tv_episode_brackets_with_modifiers_are_unclaimed() {
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0),
        None,
        None,
        0,
        Some(0),
        false,
    ));

    for (code, modifiers) in [
        (Key::Char('['), KeyModifiers::CONTROL),
        (Key::Char(']'), KeyModifiers::ALT),
    ] {
        let message = owner.on_key(&KeyEvent { code, modifiers });
        assert_eq!(message, None);
    }
    assert_eq!(
        owner.on_key(&KeyEvent {
            code: Key::Char(' '),
            modifiers: KeyModifiers::NONE,
        }),
        None
    );
}

#[test]
fn tv_episode_brackets_wrap_season_selection() {
    let mut series = make_item("Series", "Series");
    series.id = "series-id".into();
    let seasons = (0..3)
        .map(|index| {
            let mut season = make_item(&format!("Season {index}"), "Season");
            season.id = format!("season-{index}");
            season
        })
        .collect();
    let detail = crate::app::SeriesDetail {
        seasons,
        episodes: HashMap::default(),
    };
    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![series.clone()], 0),
        Some(series),
        Some(detail),
        0,
        None,
        false,
    ));

    let key = |code| KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    };
    owner.on_key(&key(Key::Enter));

    assert!(matches!(
        owner.on_key(&key(Key::Char('['))),
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::TvSeasonMove { delta: -1 })));
    assert_eq!(
        owner.selected_season(),
        Some(("series-id".into(), "season-2".into()))
    );

    assert!(matches!(
        owner.on_key(&key(Key::Char(']'))),
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::TvSeasonMove { delta: 1 })));
    assert_eq!(
        owner.selected_season(),
        Some(("series-id".into(), "season-0".into()))
    );
}
