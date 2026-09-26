use super::*;

#[test]
fn tv_series_clicks_use_the_rendered_series_row_for_left_and_right_clicks() {
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
        false,
    ));
    let mut panel = panel_with(owner, true);
    paint(&mut panel, 100, 20);
    let list_area = panel.test_wide_geometry().unwrap().list_area;
    let row = list_area.y + 1;
    let col = list_area.x;

    let left = panel.on(&mouse(MouseEventKind::Down(MouseButton::Left), col, row));
    assert!(matches!(
        left,
        Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::TvHitClick {
            hit: TvHit::SeriesRow(ref target),
        } if target == "id")));

    // Selection invalidates the tree's retained hit geometry; the next input
    // must resolve against a newly painted frame.
    paint(&mut panel, 100, 20);
    let right = panel.on(&mouse(MouseEventKind::Down(MouseButton::Right), col, row));
    assert!(matches!(
        right,
        Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::RowContextMenu(crate::app::state::types::context_menu::ContextMenuTargets::Emby(ref items), _) if items.len() == 1 && items[0].id == "id")));
}

#[test]
fn tv_keyboard_context_menu_uses_all_selected_rows_and_single_row_without_selection() {
    let mut first = make_item("Series A", "Series");
    first.id = "series-a".into();
    let mut second = make_item("Series B", "Series");
    second.id = "series-b".into();
    let mut content = TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![first, second], 0),
        None,
        None,
        0,
        None,
        false,
    );
    content.set_tv_content_mode(Some(TvContentMode::Latest));

    let mut selected = TvContent::new();
    selected.set_content(content.clone());
    selected.select_targets_for_test(&["series-a".into(), "series-b".into()]);
    assert!(matches!(
        down(&mut selected, Key::Char('.')),
        Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::RowContextMenu(
            crate::app::state::types::context_menu::ContextMenuTargets::Emby(items),
            None
        ) if items.iter().map(|item| item.id.as_str()).collect::<Vec<_>>() == vec!["series-a", "series-b"])));

    let mut single = TvContent::new();
    single.set_content(content);
    assert!(matches!(
        down(&mut single, Key::Char('.')),
        Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::RowContextMenu(
            crate::app::state::types::context_menu::ContextMenuTargets::Emby(items),
            None
        ) if items.len() == 1 && items[0].id == "series-a")));
}

#[test]
fn tv_context_click_outside_selection_forwards_cleared_selection() {
    let mut first = make_item("Series A", "Series");
    first.id = "series-a".into();
    let mut second = make_item("Series B", "Series");
    second.id = "series-b".into();
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![first, second], 0),
        None,
        None,
        0,
        None,
        false,
    ));
    let mut panel = panel_with(owner, true);
    paint(&mut panel, 100, 20);
    tv_mut(&mut panel).select_targets_for_test(&["series-b".into()]);
    let count = tv_mut(&mut panel).context_click_for_test("series-a".into());
    assert_eq!(count, 0);
}

#[test]
fn tv_series_hits_use_retained_rows_and_wheel_moves_the_control() {
    let mut first = make_item("Series A", "Series");
    first.id = "series-a".into();
    let mut second = make_item("Series B", "Series");
    second.id = "series-b".into();
    let mut owner = TvContent::new();
    owner.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![first, second], 0),
        None,
        None,
        0,
        None,
        false,
    ));
    let mut panel = panel_with(owner, true);
    paint(&mut panel, 100, 20);
    let (col, row) = {
        let list_area = panel.test_wide_geometry().unwrap().list_area;
        (list_area.x, list_area.y)
    };

    let click = panel.on(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        col,
        row + 1,
    ));
    assert!(matches!(
        click,
        Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::TvHitClick {
            hit: TvHit::SeriesRow(ref target),
        } if target == "series-b")));
    assert_eq!(
        tv(&panel).selected_tree_target(),
        Some(&crate::app::components::tv_tree_target::TvTreeTarget::Show(
            "tv-id:8:series-b".into()
        ))
    );

    let blank = panel.on(&mouse(
        MouseEventKind::Down(MouseButton::Left),
        col,
        row.saturating_sub(1),
    ));
    assert!(blank.is_none());

    // The click changed tree selection and invalidated the prior hit frame.
    paint(&mut panel, 100, 20);
    let wheel = panel.on(&mouse(MouseEventKind::ScrollDown, col, row));
    assert!(matches!(
        wheel,
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));
    assert_eq!(
        tv(&panel).selected_tree_target(),
        Some(&crate::app::components::tv_tree_target::TvTreeTarget::Show(
            "tv-id:8:series-b".into()
        ))
    );
}
