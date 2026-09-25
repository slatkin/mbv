use super::*;

#[test]
fn tv_tree_key_actions_use_stable_targets_in_wide_and_narrow() {
    for is_wide in [true, false] {
        let mut show = make_item("Series A", "Series");
        show.id = "series-a".into();
        let mut season = make_item("Season 1", "Season");
        season.id = "season-1".into();
        let mut episode = make_item("Episode 1", "Episode");
        episode.id = "episode-1".into();
        let detail = crate::app::SeriesDetail {
            seasons: vec![season.clone()],
            episodes: [(season.id.clone(), vec![episode.clone()])]
                .into_iter()
                .collect(),
        };
        let mut owner = TvContent::new();
        owner.set_is_wide(is_wide);
        owner.set_content(TvWideRenderCtx::new(
            LibraryListRenderCtx::from_items(vec![show.clone()], 0),
            Some(show.clone()),
            Some(detail),
            0,
            None,
            false,
        ));
        let show_target = owner.selected_tree_target().cloned().unwrap();
        assert!(owner.on_key(&key(Key::Char('p'))).is_none());
        assert!(matches!(
            owner.on_key(&key(Key::Right)),
            Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::TvTreeExpand { target } if *target == show_target)));
        assert!(matches!(
            owner.on_key(&KeyEvent { code: Key::Char('p'), modifiers: KeyModifiers::CONTROL }),
            Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::EmbyLibraryPlay { item } if item.id == show.id)));
        assert!(matches!(
            owner.on_key(&key(Key::Char('.'))),
            Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::RowContextMenu(
                crate::app::state::types::context_menu::ContextMenuTargets::Emby(items), None
            ) if items.len() == 1 && items[0].id == show.id)));
        assert!(matches!(
            owner.on_key(&key(Key::Down)),
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        ));
        let season_target = owner.selected_tree_target().cloned().unwrap();
        assert!(matches!(season_target, TvTreeTarget::Season { .. }));
        assert!(matches!(
            owner.on_key(&KeyEvent { code: Key::Char('p'), modifiers: KeyModifiers::CONTROL }),
            Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::EmbyLibraryPlay { item } if item.id == season.id)));
        assert!(matches!(
            owner.on_key(&key(Key::Char('.'))),
            Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::RowContextMenu(
                crate::app::state::types::context_menu::ContextMenuTargets::Emby(items), None
            ) if items.len() == 1 && items[0].id == season.id)));
        assert!(matches!(
            owner.on_key(&key(Key::Enter)),
            Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::TvTreeExpand { target } if *target == season_target)));
        assert!(matches!(
            owner.on_key(&key(Key::Down)),
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        ));
        assert!(matches!(
            owner.selected_tree_target(),
            Some(TvTreeTarget::Episode { .. })
        ));
        assert!(matches!(
            owner.on_key(&key(Key::Enter)),
            Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::TvEpisodeActivate { episode: selected } if selected.id == episode.id)));
        assert!(matches!(
            owner.on_key(&KeyEvent { code: Key::Char('p'), modifiers: KeyModifiers::CONTROL }),
            Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::EmbyLibraryPlay { item } if item.id == episode.id)));
        assert!(matches!(
            owner.on_key(&key(Key::Char('.'))),
            Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::RowContextMenu(
                crate::app::state::types::context_menu::ContextMenuTargets::Emby(items), None
            ) if items.len() == 1 && items[0].id == episode.id)));
        owner.on_key(&key(Key::Home));
        assert!(matches!(
            owner.on_key(&key(Key::Enter)),
            Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::TvActivate { item } if item.id == show.id)));
    }
}

#[test]
fn tv_show_mode_escape_and_backspace_return_to_tv_back_in_both_geometries() {
    let mut show = make_item("Series A", "Series");
    show.id = "series-a".into();
    let mut context = TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![show], 0),
        None,
        None,
        0,
        None,
        true,
    );
    context.set_tv_content_mode(Some(TvContentMode::Range(0)));

    for is_wide in [false, true] {
        for code in [Key::Esc, Key::Backspace] {
            let mut owner = TvContent::new();
            owner.set_is_wide(is_wide);
            owner.set_content(context.clone());

            assert!(matches!(
                owner.on_key(&key(code)),
                Some(Msg::Shell(ref shell_boxed))
             if matches!(shell_boxed.as_ref(), ShellRequest::TvBack)));
        }
    }
}

#[test]
fn tv_tree_does_not_enter_the_flat_visual_mode_or_act_on_headings() {
    let mut first = make_item("Alpha", "Series");
    first.id = "alpha".into();
    let mut second = make_item("Zulu", "Series");
    second.id = "zulu".into();
    let mut owner = TvContent::new();
    let list = LibraryListRenderCtx::from_items(vec![first, second], 0);
    let mut context = TvWideRenderCtx::new(list, None, None, 0, None, true);
    context.set_tv_content_mode(Some(TvContentMode::Range(0)));
    owner.set_is_wide(false);
    owner.set_content(context.clone());
    let selected_before_visual = owner.selected_tree_target().cloned();
    assert!(owner
        .on_key(&KeyEvent {
            code: Key::Char('V'),
            modifiers: KeyModifiers::SHIFT,
        })
        .is_none());
    assert_eq!(
        owner.selected_tree_target(),
        selected_before_visual.as_ref()
    );

    owner.set_is_wide(true);
    let mut panel = panel_with(owner, true);
    paint(&mut panel, 100, 20);
    let list_area = panel.test_wide_geometry().unwrap().list_area;
    assert!(panel
        .on(&mouse(
            MouseEventKind::Down(MouseButton::Left),
            list_area.x,
            list_area.y
        ))
        .is_none());
    let before = tv(&panel).selection_summary();
    let visual = panel.on(&Event::Keyboard(key(Key::Char('V'))));
    assert!(visual.is_none());
    assert_eq!(tv(&panel).selection_summary(), before);
    assert!(tv(&panel).selected_tree_target().is_some());
}

#[test]
fn tv_first_mount_seeds_the_stable_target_and_renders_sorted_rows() {
    let mut items = vec![
        make_item("Zulu", "Series"),
        make_item("Alpha", "Series"),
        make_item("Beta", "Series"),
    ];
    items.extend((3..50).map(|index| make_item(&format!("Series {index}"), "Series")));
    // Stable targets must be unique: the target-addressed list cannot move
    // between rows that share the default fixture id.
    for (index, item) in items.iter_mut().enumerate() {
        item.id = format!("tv-series-{index}");
    }

    let mut owner = TvContent::new();

    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(items, 1),
        None,
        None,
        0,
        None,
        false,
    ));
    let mut panel = panel_with(owner, true);
    paint(&mut panel, 100, 20);
    let owner = tv(&panel);
    assert_eq!(
        owner.selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:11:tv-series-1".into())),
        "first mount must resolve the stable target in natural-sort order"
    );

    let message = tv_mut(&mut panel).on_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
        message,
        Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::TvHitClick {
            hit: TvHit::SeriesRow(ref id)
        } if id == "tv-series-2")));
    assert_eq!(
        tv(&panel).selected_tree_target(),
        Some(&TvTreeTarget::Show("tv-id:11:tv-series-2".into()))
    );
    assert_eq!(
        tv(&panel).cursor(),
        0,
        "tree movement leaves the Workspace carrier cursor alone"
    );
}
