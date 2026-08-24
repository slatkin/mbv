//! Interactive Component for the nested Settings Feed-management popup.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::to_crossterm_key_event;
use super::msg::{LegacyTerminalEvent, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::{render_feeds_manage_content, FeedsManageRenderModel};
use crate::app::types_feeds_manage::{FeedForm, FeedFormField, FeedsManagePopup, FeedsManageStage};
use mbv_core::config::{FeedKind, FeedSubscription};

pub struct FeedsManageComponent {
    feeds: Vec<FeedSubscription>,
    stage: Option<FeedsManageStage>,
    cursor: usize,
    pending_add: Option<u64>,
    dim_backdrop_active: bool,
}

impl FeedsManageComponent {
    pub fn new() -> Self {
        Self {
            feeds: Vec::new(),
            stage: None,
            cursor: 0,
            pending_add: None,
            dim_backdrop_active: false,
        }
    }

    /// Mirror the shell snapshot without overwriting local form edits. A
    /// changed stage is an App action result and replaces the local draft.
    pub(in crate::app) fn set_content(
        &mut self,
        popup: &FeedsManagePopup,
        feeds: Vec<FeedSubscription>,
    ) {
        let same_stage = self
            .stage
            .as_ref()
            .is_some_and(|stage| same_stage_kind(stage, &popup.stage));
        if !same_stage {
            self.stage = Some(popup.stage.clone());
            self.cursor = popup.cursor;
        }
        self.feeds = feeds;
        self.pending_add = popup.pending_add;
        self.cursor = self.cursor.min(self.feeds.len().saturating_sub(1));
    }

    pub(in crate::app) fn snapshot(&self) -> Option<(FeedsManageStage, usize)> {
        self.stage.clone().map(|stage| (stage, self.cursor))
    }

    fn submitting(&self) -> bool {
        self.pending_add.is_some()
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        let Some(stage) = self.stage.clone() else {
            return Some(Msg::Legacy(LegacyTerminalEvent::NoOp));
        };
        match stage {
            FeedsManageStage::List => self.handle_list_key(key),
            FeedsManageStage::Form(form) => self.handle_form_key(key, &form),
        }
    }

    fn handle_list_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            Key::Down => {
                if !self.feeds.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.feeds.len() - 1);
                }
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            Key::Esc | Key::Char('a') | Key::Enter | Key::Char('e') | Key::Char('d') => {
                self.shell_key(key)
            }
            _ => Some(Msg::Legacy(LegacyTerminalEvent::NoOp)),
        }
    }

    fn handle_form_key(&mut self, key: &KeyEvent, form: &FeedForm) -> Option<Msg> {
        if matches!(key.code, Key::Esc)
            || (!self.submitting()
                && matches!(
                    key.code,
                    Key::Tab | Key::BackTab | Key::Enter | Key::Backspace | Key::Left | Key::Right
                )
                && (matches!(key.code, Key::Enter | Key::Tab | Key::BackTab)
                    || form.focus == FeedFormField::Kind
                    || matches!(key.code, Key::Backspace)))
        {
            match key.code {
                Key::Tab => self.next_field(),
                Key::BackTab => self.previous_field(),
                Key::Left | Key::Right if form.focus == FeedFormField::Kind => self.toggle_kind(),
                Key::Backspace => self.backspace(),
                _ if matches!(key.code, Key::Enter | Key::Esc) => return self.shell_key(key),
                _ => {}
            }
            return Some(Msg::Legacy(LegacyTerminalEvent::NoOp));
        }
        if !self.submitting()
            && matches!(key.code, Key::Char(_))
            && (key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT)
        {
            if let Key::Char(c) = key.code {
                self.push_char(c);
            }
        }
        Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
    }

    fn shell_key(&self, key: &KeyEvent) -> Option<Msg> {
        Some(Msg::Shell(ShellRequest::FeedsManageKey(
            to_crossterm_key_event(key),
        )))
    }

    fn next_field(&mut self) {
        let Some(FeedsManageStage::Form(form)) = self.stage.as_mut() else {
            return;
        };
        form.focus = match (form.focus, form.editing_index.is_some()) {
            (FeedFormField::Name, true) => FeedFormField::Kind,
            (FeedFormField::Name, false) => FeedFormField::Url,
            (FeedFormField::Url, _) => FeedFormField::Kind,
            (FeedFormField::Kind, _) => FeedFormField::Name,
        };
    }

    fn previous_field(&mut self) {
        let Some(FeedsManageStage::Form(form)) = self.stage.as_mut() else {
            return;
        };
        form.focus = match (form.focus, form.editing_index.is_some()) {
            (FeedFormField::Name, _) => FeedFormField::Kind,
            (FeedFormField::Url, _) => FeedFormField::Name,
            (FeedFormField::Kind, true) => FeedFormField::Name,
            (FeedFormField::Kind, false) => FeedFormField::Url,
        };
    }

    fn toggle_kind(&mut self) {
        let Some(FeedsManageStage::Form(form)) = self.stage.as_mut() else {
            return;
        };
        form.kind = match form.kind {
            FeedKind::Audio => FeedKind::Video,
            FeedKind::Video => FeedKind::Audio,
        };
    }

    fn push_char(&mut self, c: char) {
        let Some(FeedsManageStage::Form(form)) = self.stage.as_mut() else {
            return;
        };
        match form.focus {
            FeedFormField::Name => form.name.push(c),
            FeedFormField::Url if form.editing_index.is_none() => form.url.push(c),
            _ => {}
        }
    }

    fn backspace(&mut self) {
        let Some(FeedsManageStage::Form(form)) = self.stage.as_mut() else {
            return;
        };
        match form.focus {
            FeedFormField::Name => {
                form.name.pop();
            }
            FeedFormField::Url if form.editing_index.is_none() => {
                form.url.pop();
            }
            _ => {}
        }
    }
}

fn same_stage_kind(left: &FeedsManageStage, right: &FeedsManageStage) -> bool {
    matches!(
        (left, right),
        (FeedsManageStage::List, FeedsManageStage::List)
            | (FeedsManageStage::Form(_), FeedsManageStage::Form(_))
    )
}

impl Default for FeedsManageComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for FeedsManageComponent {
    fn view(&mut self, f: &mut Frame, _area: Rect) {
        let Some(stage) = self.stage.as_ref() else {
            return;
        };
        render_feeds_manage_content(
            f,
            &mut self.dim_backdrop_active,
            FeedsManageRenderModel {
                feeds: &self.feeds,
                stage,
                cursor: self.cursor,
                pending_add: self.pending_add,
            },
        );
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
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

impl AppComponent<Msg, UserEvent> for FeedsManageComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => self.handle_key(key),
            _ => Some(Msg::Legacy(LegacyTerminalEvent::NoOp)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::event::KeyModifiers;

    fn popup() -> FeedsManagePopup {
        FeedsManagePopup::new()
    }

    fn key(code: Key) -> Event<UserEvent> {
        Event::Keyboard(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        })
    }

    #[test]
    fn settings_popup_feeds_manage_form_edits_are_local() {
        let mut component = FeedsManageComponent::new();
        let mut popup = popup();
        popup.stage = FeedsManageStage::Form(FeedForm::new_add());
        component.set_content(&popup, Vec::new());
        component.on(&key(Key::Char('x')));

        let Some(FeedsManageStage::Form(form)) = component.stage else {
            panic!("expected form stage");
        };
        assert_eq!(form.name, "x");
    }

    #[test]
    fn settings_popup_feeds_manage_submit_is_typed() {
        let mut component = FeedsManageComponent::new();
        let mut popup = popup();
        popup.stage = FeedsManageStage::Form(FeedForm::new_add());
        component.set_content(&popup, Vec::new());

        assert!(matches!(
            component.on(&key(Key::Enter)),
            Some(Msg::Shell(ShellRequest::FeedsManageKey(_)))
        ));
    }

    #[test]
    fn settings_popup_feeds_manage_renders_without_app_state() {
        let mut component = FeedsManageComponent::new();
        component.set_content(&popup(), Vec::new());
        let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect();
        assert!(output.contains("Manage Feeds"));
        assert!(output.contains("No feed subscriptions"));
    }
}
