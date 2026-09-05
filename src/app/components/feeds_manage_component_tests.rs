//! Mouse tests for `FeedsManageComponent` (task 5.1): the click vocabulary
//! mirrors the popup's keyboard bindings — row click selects (Up/Down),
//! row double-click edits (Enter), form-field click focuses (Tab), and an
//! outside click follows the stage's Esc path (Dismiss/Cancel).

use super::feeds_manage::FeedsManageComponent;
use super::msg::{FeedsManageIntent, Msg, ShellRequest};
use crate::app::types_feeds_manage::{FeedForm, FeedFormField, FeedsManageStage};
use mbv_core::config::{FeedKind, FeedSubscription};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

fn click(x: u16, y: u16) -> Event<super::user_event::UserEvent> {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })
}

fn key(code: Key) -> Event<super::user_event::UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

fn draw(component: &mut FeedsManageComponent) {
    let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
}

fn list_component() -> FeedsManageComponent {
    let mut component = FeedsManageComponent::new();
    component.set_feeds(vec![
        FeedSubscription {
            name: "one".into(),
            url: "https://one.example/rss".into(),
            kind: FeedKind::Audio,
        },
        FeedSubscription {
            name: "two".into(),
            url: "https://two.example/rss".into(),
            kind: FeedKind::Video,
        },
    ]);
    component.set_stage(FeedsManageStage::List);
    component
}

#[test]
fn feeds_manage_row_click_selects_like_the_arrow_keys() {
    let mut component = list_component();
    draw(&mut component);
    let rows = component.test_rows().regions().to_vec();
    assert_eq!(rows.len(), 2, "both feed rows must be painted");

    component.reset_mouse_gestures_for_test();
    let (rect, index) = rows[1];
    let msg = component.on(&click(rect.x, rect.y));
    assert_eq!(msg, None, "selection is a local cursor move, no Msg");
    assert_eq!(component.cursor(), index);
}

#[test]
fn feeds_manage_row_double_click_edits_like_enter() {
    let mut component = list_component();
    draw(&mut component);
    let rows = component.test_rows().regions().to_vec();

    let (rect, _) = rows[1];
    // Two quick clicks at the same point: first selects, second activates.
    assert_eq!(component.on(&click(rect.x, rect.y)), None);
    assert!(
        matches!(
            component.on(&click(rect.x, rect.y)),
            Some(Msg::Shell(ShellRequest::FeedsManageIntent(
                FeedsManageIntent::Edit
            )))
        ),
        "double-click must emit the Enter-equivalent Edit intent"
    );
    assert_eq!(component.cursor(), 1);
}

#[test]
fn feeds_manage_outside_click_dismisses_like_esc() {
    let mut component = list_component();
    draw(&mut component);
    let frame = component.test_frame();
    let outside =
        ratatui::layout::Position::new(frame.x.saturating_sub(1), frame.y.saturating_sub(1));
    component.reset_mouse_gestures_for_test();
    assert!(
        matches!(
            component.on(&click(outside.x, outside.y)),
            Some(Msg::Shell(ShellRequest::FeedsManageIntent(
                FeedsManageIntent::Dismiss
            )))
        ),
        "outside click must mirror the Esc dismiss path"
    );
}

#[test]
fn feeds_manage_form_field_click_focuses_like_tab() {
    let mut component = FeedsManageComponent::new();
    component.set_stage(FeedsManageStage::Form(FeedForm::new_add()));
    draw(&mut component);
    let fields = component.test_fields().regions().to_vec();
    assert_eq!(fields.len(), 3, "name, url, and kind rows must be painted");

    // Click the Kind field row: focuses it, does not toggle it.
    let (kind_rect, kind_field) = fields[2];
    assert!(matches!(kind_field, FeedFormField::Kind));
    component.reset_mouse_gestures_for_test();
    assert_eq!(component.on(&click(kind_rect.x, kind_rect.y)), None);
    let Some(FeedsManageStage::Form(form)) = component.stage_clone() else {
        panic!("expected form stage");
    };
    assert!(matches!(form.focus, FeedFormField::Kind));
    assert_eq!(form.kind, FeedKind::Video, "a click focuses, never toggles");

    // Click the Name field row.
    let (name_rect, _) = fields[0];
    component.reset_mouse_gestures_for_test();
    component.on(&click(name_rect.x, name_rect.y));
    let Some(FeedsManageStage::Form(form)) = component.stage_clone() else {
        panic!("expected form stage");
    };
    assert!(matches!(form.focus, FeedFormField::Name));
}

#[test]
fn feeds_manage_edit_mode_url_click_is_a_noop_like_the_keyboard() {
    let mut component = FeedsManageComponent::new();
    let sub = FeedSubscription {
        name: "one".into(),
        url: "https://one.example/rss".into(),
        kind: FeedKind::Audio,
    };
    component.set_stage(FeedsManageStage::Form(FeedForm::new_edit(0, &sub)));
    draw(&mut component);
    let fields = component.test_fields().regions().to_vec();

    // The edit-mode URL is read-only and unreachable by Tab, so clicking it
    // must not move focus either.
    let (url_rect, _) = fields[1];
    component.reset_mouse_gestures_for_test();
    component.on(&click(url_rect.x, url_rect.y));
    let Some(FeedsManageStage::Form(form)) = component.stage_clone() else {
        panic!("expected form stage");
    };
    assert!(matches!(form.focus, FeedFormField::Name));
}

#[test]
fn feeds_manage_submitting_form_ignores_field_clicks() {
    let mut component = FeedsManageComponent::new();
    component.set_stage(FeedsManageStage::Form(FeedForm::new_add()));
    component.set_pending_add(Some(1));
    draw(&mut component);
    let fields = component.test_fields().regions().to_vec();

    // While submitting, the keyboard ignores everything but Esc; so must
    // the mouse.
    let (kind_rect, _) = fields[2];
    component.reset_mouse_gestures_for_test();
    component.on(&click(kind_rect.x, kind_rect.y));
    let Some(FeedsManageStage::Form(form)) = component.stage_clone() else {
        panic!("expected form stage");
    };
    assert!(matches!(form.focus, FeedFormField::Name));
}

#[test]
fn feeds_manage_form_outside_click_cancels_like_esc() {
    let mut component = FeedsManageComponent::new();
    component.set_stage(FeedsManageStage::Form(FeedForm::new_add()));
    draw(&mut component);
    let frame = component.test_frame();

    component.reset_mouse_gestures_for_test();
    assert!(
        matches!(
            component.on(&click(0, 0)),
            Some(Msg::Shell(ShellRequest::FeedsManageIntent(
                FeedsManageIntent::Cancel
            )))
        ),
        "outside click on the form must mirror its Esc (Cancel) path"
    );
    let _ = frame;
}

#[test]
fn feeds_manage_keyboard_paths_still_work_alongside_mouse() {
    let mut component = list_component();
    assert_eq!(
        component.on(&key(Key::Esc)),
        Some(Msg::Shell(ShellRequest::FeedsManageIntent(
            FeedsManageIntent::Dismiss
        )))
    );
}

#[test]
fn feeds_manage_outside_double_click_emits_nothing_after_the_first_dismiss() {
    let mut component = list_component();
    draw(&mut component);
    let frame = component.test_frame();
    let outside =
        ratatui::layout::Position::new(frame.x.saturating_sub(1), frame.y.saturating_sub(1));

    component.reset_mouse_gestures_for_test();
    assert!(matches!(
        component.on(&click(outside.x, outside.y)),
        Some(Msg::Shell(ShellRequest::FeedsManageIntent(
            FeedsManageIntent::Dismiss
        )))
    ));
    // The double-click arm must not re-fire the dismiss (or anything else):
    // in the real flow the first click already closed the popup.
    assert_eq!(component.on(&click(outside.x, outside.y)), None);
}
