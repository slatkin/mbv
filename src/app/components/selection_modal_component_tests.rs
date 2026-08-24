use super::msg::{LegacyTerminalEvent, Msg, ShellRequest};
use super::selection_modal::SelectionModalComponent;
use super::user_event::UserEvent;
use crate::app::types_selection_modal::{
    SelectionModal, SelectionModalFilter, SelectionModalItem, SelectionModalListState,
    SelectionModalRow, SelectionModalSource,
};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

fn item(id: &str) -> SelectionModalRow {
    SelectionModalRow::Item(SelectionModalItem {
        name: id.into(),
        meta: "3:21".into(),
        id: id.into(),
    })
}

fn modal(filter: Option<SelectionModalFilter>) -> SelectionModal {
    SelectionModal {
        source: SelectionModalSource::Album {
            album_id: "album-1".into(),
        },
        title: "Tracks".into(),
        state: SelectionModalListState::Ready(vec![item("track-a"), item("track-b")]),
        cursor: 0,
        filter,
    }
}

fn key(code: Key) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn selection_modal_moves_local_cursor_without_cross_boundary_request() {
    let mut component = SelectionModalComponent::new();
    component.set_content(&modal(None));

    let message = component.on(&key(Key::Down));

    assert_eq!(component.selected_id(), Some("track-b"));
    assert_eq!(message, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));
}

#[test]
fn selection_modal_filter_selection_is_typed_and_local() {
    let mut component = SelectionModalComponent::new();
    component.set_content(&SelectionModal {
        filter: Some(SelectionModalFilter {
            labels: vec!["All".into(), "Unplayed".into()],
            selected: 0,
        }),
        ..modal(None)
    });

    let message = component.on(&key(Key::Char(']')));

    assert_eq!(component.filter_selected(), Some(1));
    assert_eq!(
        message,
        Some(Msg::Shell(ShellRequest::SelectionModalFilterSelected))
    );
}

#[test]
fn selection_modal_activation_and_dismissal_are_typed() {
    let mut component = SelectionModalComponent::new();
    component.set_content(&modal(None));

    assert_eq!(
        component.on(&key(Key::Enter)),
        Some(Msg::Shell(ShellRequest::SelectionModalActivate(Some(
            "track-a".into()
        ),)))
    );
    assert_eq!(
        component.on(&key(Key::Esc)),
        Some(Msg::Shell(ShellRequest::DismissSelectionModal))
    );
}

#[test]
fn selection_modal_renders_without_app_state_and_records_targets() {
    let mut component = SelectionModalComponent::new();
    component.set_content(&modal(None));
    let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();

    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, 60, 16)))
        .unwrap();

    let output: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_owned())
        .collect();
    assert!(output.contains("Tracks"));
    assert!(output.contains("track-a"));
    assert_eq!(component.row_targets().len(), 2);
}

#[test]
fn selection_modal_mouse_row_uses_painted_target() {
    let mut component = SelectionModalComponent::new();
    component.set_content(&modal(None));
    let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, 60, 16)))
        .unwrap();
    let (rect, _) = component.row_targets()[1];

    assert_eq!(
        component.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x,
            row: rect.y,
            modifiers: KeyModifiers::NONE,
        })),
        Some(Msg::Shell(ShellRequest::SelectionModalActivate(Some(
            "track-b".into()
        ),)))
    );
}

#[test]
fn selection_modal_mouse_filter_updates_local_state() {
    let mut component = SelectionModalComponent::new();
    component.set_content(&SelectionModal {
        filter: Some(SelectionModalFilter {
            labels: vec!["All".into(), "Unplayed".into()],
            selected: 0,
        }),
        ..modal(None)
    });
    let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, 60, 16)))
        .unwrap();
    let (rect, _) = component.selector_targets()[1];

    let message = component.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));

    assert_eq!(component.filter_selected(), Some(1));
    assert_eq!(
        message,
        Some(Msg::Shell(ShellRequest::SelectionModalFilterSelected))
    );
}
