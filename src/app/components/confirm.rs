//! Interactive Component for the Confirm modal overlay (design D3–D9).
//!
//! Owns the modal's display content (title, message, hint) set by the shell
//! via downcast before each render. The component owns key interpretation and
//! emits semantic confirmation intents; the shell owns the `ConfirmAction` and
//! effect dispatch. Non-key events return `None` because the permanent UiRoot
//! observer owns the redraw signal (design D12).

use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{ConfirmIntent, LeafKeyResult, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::render_confirm_modal_content;
use crate::app::state::types::confirm::{ConfirmAction, ConfirmModal};

/// The Interactive Component for the Confirm modal.
///
/// Owns display content (title, message, hint) set by the shell via
/// `get_component_mut`+downcast before each render. The
/// `dim_backdrop_active` field is a scratch flag for `render_modal_frame`
/// (design D9: the visual backdrop is painted by `dim_backdrop`; the flag
/// is written-to but not read by the modal, and `App::render` resets
/// `App::dim_backdrop_active` from `any_dim_modal_open()` before each frame's
/// image lookups, so no shell↔component sync is needed).
pub struct ConfirmComponent {
    title: String,
    message: String,
    hint: String,
    on_confirm: Option<ConfirmAction>,
    dim_backdrop_active: bool,
}

impl ConfirmComponent {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            message: String::new(),
            hint: String::new(),
            on_confirm: None,
            dim_backdrop_active: false,
        }
    }

    /// Set the modal's display content from a shell request. Called by
    /// the shell via `get_component_mut`+downcast before each render.
    pub(in crate::app) fn set_content(&mut self, title: &str, message: &str, hint: &str) {
        self.title.clear();
        self.title.push_str(title);
        self.message.clear();
        self.message.push_str(message);
        self.hint.clear();
        self.hint.push_str(hint);
    }

    pub(in crate::app) fn set_modal(&mut self, modal: &ConfirmModal) {
        self.set_content(&modal.title, &modal.message, &modal.hint);
        self.on_confirm = Some(modal.on_confirm.clone());
    }

    pub(in crate::app) fn confirm_action(&self) -> Option<ConfirmAction> {
        self.on_confirm.clone()
    }
}

impl Default for ConfirmComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ConfirmComponent {
    fn view(&mut self, frame: &mut Frame, _area: ratatui::layout::Rect) {
        render_confirm_modal_content(
            frame,
            &mut self.dim_backdrop_active,
            &self.title,
            &self.message,
            &self.hint,
        );
    }

    fn query(&self, _attr: Attribute) -> Option<QueryResult<'_>> {
        None
    }

    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl ConfirmComponent {
    fn key_result(&self, key: &tuirealm::event::KeyEvent) -> LeafKeyResult {
        let Some(action) = self.on_confirm.as_ref() else {
            return LeafKeyResult::Unhandled;
        };
        let Some(intent) = confirm_intent_for_key(action, key.code) else {
            return LeafKeyResult::Unhandled;
        };
        LeafKeyResult::Consumed(Some(Box::new(Msg::Shell(Box::new(
            ShellRequest::ConfirmIntent(intent),
        )))))
    }
}

impl AppComponent<Msg, UserEvent> for ConfirmComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => self.key_result(key).into_option(),
            _ => None,
        }
    }
}

fn confirm_intent_for_key(action: &ConfirmAction, key: Key) -> Option<ConfirmIntent> {
    match action {
        ConfirmAction::ClearQueue | ConfirmAction::ReplacePopulatedQueue => {
            Some(clear_or_replace_queue_intent(key))
        }
        ConfirmAction::RemoveActiveQueueItem(_) | ConfirmAction::RemoveFeedSubscription(_) => {
            Some(remove_item_intent(key))
        }
        ConfirmAction::RescanLibrary(_)
        | ConfirmAction::RemoveEmby
        | ConfirmAction::ReplaceEmby(_)
        | ConfirmAction::RemoveAudiobookshelf
        | ConfirmAction::ReplaceAudiobookshelf(_) => service_intent(key),
        ConfirmAction::SaveOverwritePlaylist { .. } | ConfirmAction::DeletePlaylist { .. } => {
            playlist_intent(key)
        }
        ConfirmAction::PlayLocallyInstead => play_locally_intent(key),
        ConfirmAction::DiscardOrSaveDirtyPlaylist => dirty_playlist_intent(key),
    }
}

fn clear_or_replace_queue_intent(key: Key) -> ConfirmIntent {
    match key {
        Key::Char('y' | 'Y') | Key::Enter => ConfirmIntent::Accept,
        Key::Esc => ConfirmIntent::Cancel,
        _ => ConfirmIntent::Dismiss,
    }
}

fn remove_item_intent(key: Key) -> ConfirmIntent {
    match key {
        Key::Char('y') => ConfirmIntent::Accept,
        Key::Esc => ConfirmIntent::Cancel,
        _ => ConfirmIntent::Dismiss,
    }
}

fn service_intent(key: Key) -> Option<ConfirmIntent> {
    match key {
        Key::Char('y' | 'Y') | Key::Enter => Some(ConfirmIntent::Accept),
        Key::Esc => Some(ConfirmIntent::Cancel),
        _ => None,
    }
}

fn playlist_intent(key: Key) -> Option<ConfirmIntent> {
    match key {
        Key::Char('y') => Some(ConfirmIntent::Accept),
        Key::Esc => Some(ConfirmIntent::Cancel),
        _ => None,
    }
}

fn play_locally_intent(key: Key) -> Option<ConfirmIntent> {
    match key {
        Key::Char('y' | 'Y') | Key::Enter => Some(ConfirmIntent::Accept),
        Key::Char('n' | 'N') | Key::Esc => Some(ConfirmIntent::Cancel),
        _ => None,
    }
}

fn dirty_playlist_intent(key: Key) -> Option<ConfirmIntent> {
    match key {
        Key::Char('s' | 'S') => Some(ConfirmIntent::Save),
        Key::Char('d' | 'D') => Some(ConfirmIntent::Discard),
        Key::Char('c' | 'C') | Key::Esc => Some(ConfirmIntent::Cancel),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use tuirealm::event::{Key, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    fn make_key(code: Key, modifiers: KeyModifiers) -> tuirealm::event::KeyEvent {
        tuirealm::event::KeyEvent { code, modifiers }
    }

    #[test]
    fn key_emits_accept_intent() {
        let mut comp = ConfirmComponent::new();
        comp.set_modal(&ConfirmModal {
            title: String::new(),
            message: String::new(),
            hint: String::new(),
            on_confirm: ConfirmAction::ClearQueue,
        });
        let msg = comp.on(&Event::Keyboard(make_key(
            Key::Char('y'),
            KeyModifiers::NONE,
        )));
        assert!(matches!(
           msg,
           Some(Msg::Shell(ref shell_boxed))
        if matches!(shell_boxed.as_ref(), ShellRequest::ConfirmIntent(
               ConfirmIntent::Accept
           ))));
    }

    #[test]
    fn local_play_keys_accept_cancel_and_swallow() {
        let action = ConfirmAction::PlayLocallyInstead;
        for key in [Key::Char('y'), Key::Char('Y'), Key::Enter] {
            assert_eq!(
                confirm_intent_for_key(&action, key),
                Some(ConfirmIntent::Accept)
            );
        }
        for key in [Key::Char('n'), Key::Char('N'), Key::Esc] {
            assert_eq!(
                confirm_intent_for_key(&action, key),
                Some(ConfirmIntent::Cancel)
            );
        }
        assert_eq!(confirm_intent_for_key(&action, Key::Char('x')), None);
    }

    #[rstest]
    #[case::clear_queue_dismisses_unknown(
        Key::Char('x'),
        ConfirmAction::ClearQueue,
        Some(ConfirmIntent::Dismiss)
    )]
    #[case::remove_item_dismisses_unknown(
        Key::Enter,
        ConfirmAction::RemoveActiveQueueItem(1),
        Some(ConfirmIntent::Dismiss)
    )]
    #[case::service_removal_leaves_unknown_unhandled(
        Key::Char('x'),
        ConfirmAction::RemoveEmby,
        None
    )]
    #[case::playlist_overwrite_leaves_unknown_unhandled(Key::Enter, ConfirmAction::SaveOverwritePlaylist { existing_id: "id".into(), name: "name".into() }, None)]
    #[case::dirty_playlist_saves(
        Key::Char('S'),
        ConfirmAction::DiscardOrSaveDirtyPlaylist,
        Some(ConfirmIntent::Save)
    )]
    #[case::dirty_playlist_discards(
        Key::Char('D'),
        ConfirmAction::DiscardOrSaveDirtyPlaylist,
        Some(ConfirmIntent::Discard)
    )]
    #[case::queue_replacement_dismisses_unknown(
        Key::Char('x'),
        ConfirmAction::ReplacePopulatedQueue,
        Some(ConfirmIntent::Dismiss)
    )]
    fn action_specific_key_intents(
        #[case] key: Key,
        #[case] action: ConfirmAction,
        #[case] expected: Option<ConfirmIntent>,
    ) {
        assert_eq!(confirm_intent_for_key(&action, key), expected);
    }

    #[test]
    fn esc_emits_cancel_intent() {
        let mut comp = ConfirmComponent::new();
        comp.set_modal(&ConfirmModal {
            title: String::new(),
            message: String::new(),
            hint: String::new(),
            on_confirm: ConfirmAction::ClearQueue,
        });
        let msg = comp.on(&Event::Keyboard(make_key(Key::Esc, KeyModifiers::NONE)));
        assert!(matches!(
           msg,
           Some(Msg::Shell(ref shell_boxed))
        if matches!(shell_boxed.as_ref(), ShellRequest::ConfirmIntent(
               ConfirmIntent::Cancel
           ))));
    }

    #[test]
    fn unbound_key_is_swallowed_locally() {
        let mut comp = ConfirmComponent::new();
        comp.set_modal(&ConfirmModal {
            title: String::new(),
            message: String::new(),
            hint: String::new(),
            on_confirm: ConfirmAction::ClearQueue,
        });
        assert_eq!(
            comp.on(&Event::Keyboard(make_key(
                Key::Char('x'),
                KeyModifiers::NONE,
            ))),
            Some(Msg::Shell(Box::new(ShellRequest::ConfirmIntent(
                ConfirmIntent::Dismiss
            ))))
        );
    }

    #[test]
    fn non_keyboard_events_return_none() {
        let mut comp = ConfirmComponent::new();
        assert_eq!(comp.on(&Event::<UserEvent>::None), None);
        assert_eq!(comp.on(&Event::<UserEvent>::Tick), None);
        assert_eq!(
            comp.on(&Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 10,
                row: 5,
                modifiers: KeyModifiers::NONE,
            })),
            None
        );
    }

    #[test]
    fn set_content_updates_title_message_hint() {
        let mut comp = ConfirmComponent::new();
        comp.set_content(" Title ", "Are you sure?", "[y] Yes    [Esc] Cancel");
        assert_eq!(comp.title, " Title ");
        assert_eq!(comp.message, "Are you sure?");
        assert_eq!(comp.hint, "[y] Yes    [Esc] Cancel");
    }
}
