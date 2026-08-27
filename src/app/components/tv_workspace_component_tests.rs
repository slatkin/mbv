use super::msg::{LegacyTerminalEvent, Msg, ShellRequest, TvHit, TvHitRegion};
use super::tv_workspace::TvWorkspaceComponent;
use crate::app::render::{LibraryListRenderCtx, TvWideRenderCtx};
use crate::app::tests::make_item;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

#[test]
fn tv_series_clicks_use_the_rendered_series_row_for_left_and_right_clicks() {
    let mut component = TvWorkspaceComponent::new();
    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(
            vec![
                make_item("Series A", "Series"),
                make_item("Series B", "Series"),
            ],
            0,
            0,
        ),
        None,
        None,
        0,
        None,
        true,
        false,
    ));
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let layout = component.test_layout();
    let row = layout.tv_wide_list_area.y + 1;
    let col = layout.tv_wide_list_area.x;

    let left = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        left,
        Some(Msg::Shell(ShellRequest::TvClick {
            region: TvHitRegion::Hit(TvHit::SeriesRow(1)),
            ..
        }))
    ));

    let right = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        right,
        Some(Msg::Shell(ShellRequest::TvClick {
            region: TvHitRegion::ContextMenu(TvHit::SeriesRow(1)),
            ..
        }))
    ));
}

#[test]
fn tv_keyboard_falls_through_to_legacy_when_queue_is_focused() {
    let mut component = TvWorkspaceComponent::new();
    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(
            vec![
                make_item("Series A", "Series"),
                make_item("Series B", "Series"),
            ],
            0,
            0,
        ),
        None,
        None,
        0,
        None,
        false,
        true,
    ));

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        message,
        Some(Msg::Legacy(LegacyTerminalEvent::Key(_)))
    ));
    assert_eq!(component.cursor(), 0);
}

#[test]
fn tv_episode_brackets_with_modifiers_fall_through_to_legacy() {
    let mut component = TvWorkspaceComponent::new();
    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(vec![make_item("Series", "Series")], 0, 0),
        None,
        None,
        0,
        Some(0),
        true,
        false,
    ));

    for (code, modifiers) in [
        (Key::Char('['), KeyModifiers::CONTROL),
        (Key::Char(']'), KeyModifiers::ALT),
    ] {
        let message = component.on(&Event::Keyboard(KeyEvent { code, modifiers }));
        assert!(matches!(
            message,
            Some(Msg::Legacy(LegacyTerminalEvent::Key(_)))
        ));
    }
}

#[test]
fn tv_grouped_cursor_mirrors_rendered_sorted_rows() {
    let mut items = vec![
        make_item("Zulu", "Series"),
        make_item("Alpha", "Series"),
        make_item("Beta", "Series"),
    ];
    items.extend((3..50).map(|index| make_item(&format!("Series {index}"), "Series")));

    let mut component = TvWorkspaceComponent::new();
    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(items, 1, 0),
        None,
        None,
        0,
        None,
        true,
        false,
    ));
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    assert_eq!(&component.test_layout().left_sorted_indices[..2], &[1, 2]);

    let message = component.on(&Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::TvMoveRows { rows: 1 }))
    ));
    assert_eq!(component.cursor(), 2);
}

#[test]
fn tv_keyboard_uses_typed_requests_and_routes_brackets_by_pane() {
    let mut component = TvWorkspaceComponent::new();
    component.set_content(TvWideRenderCtx::new(
        LibraryListRenderCtx::from_items(
            vec![
                make_item("Series A", "Series"),
                make_item("Series B", "Series"),
            ],
            0,
            0,
        ),
        None,
        None,
        0,
        None,
        true,
        true,
    ));

    let key = |code| {
        Event::Keyboard(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        })
    };
    assert!(matches!(
        component.on(&key(Key::Down)),
        Some(Msg::Shell(ShellRequest::TvMoveRows { rows: 1 }))
    ));
    assert!(matches!(
        component.on(&key(Key::Char('['))),
        Some(Msg::Shell(ShellRequest::TvCycleLetterPill { delta: -1 }))
    ));
    assert!(matches!(
        component.on(&key(Key::Enter)),
        Some(Msg::Shell(ShellRequest::TvActivate))
    ));
    assert!(matches!(
        component.on(&key(Key::Up)),
        Some(Msg::Shell(ShellRequest::TvEpisodeMove { delta: -1 }))
    ));
    assert!(matches!(
        component.on(&key(Key::Char(']'))),
        Some(Msg::Shell(ShellRequest::TvSeasonMove { delta: 1 }))
    ));
    assert!(matches!(
        component.on(&key(Key::Esc)),
        Some(Msg::Shell(ShellRequest::TvBack))
    ));

    component.on(&key(Key::Enter));
    assert!(matches!(
        component.on(&key(Key::Enter)),
        Some(Msg::Shell(ShellRequest::TvEpisodeActivate))
    ));
}
