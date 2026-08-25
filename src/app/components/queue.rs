use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{LegacyTerminalEvent, Msg, QueueHitRegion, QueueMove, QueueRequest, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::{
    render_queue_content, render_queue_title_content, QueueRenderGeometry, QueueTitleModel,
};
use crate::app::types_playback::{PlaybackState, QueueScope};
use mbv_core::playback_queue::QueueSlot;

pub struct QueueComponent {
    slots: Vec<QueueSlot>,
    cursor: usize,
    scroll: usize,
    scope: QueueScope,
    focused: bool,
    playback: PlaybackState,
    empty_text: String,
    title: Option<QueueTitleModel>,
    title_area: Option<Rect>,
    area: Rect,
    geometry: QueueRenderGeometry,
}

impl QueueComponent {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            cursor: 0,
            scroll: 0,
            scope: QueueScope::Local,
            focused: false,
            playback: PlaybackState::default(),
            empty_text: String::new(),
            title: None,
            title_area: None,
            area: Rect::default(),
            geometry: QueueRenderGeometry::default(),
        }
    }

    pub(in crate::app) fn set_content(
        &mut self,
        slots: Vec<QueueSlot>,
        cursor: usize,
        scroll: usize,
        scope: QueueScope,
        focused: bool,
        playback: PlaybackState,
        title: QueueTitleModel,
    ) {
        let selected_slot = self.slots.get(self.cursor).map(|slot| slot.slot_id);
        self.slots = slots;
        self.cursor = selected_slot
            .and_then(|slot_id| self.slots.iter().position(|slot| slot.slot_id == slot_id))
            .unwrap_or_else(|| cursor.min(self.slots.len().saturating_sub(1)));
        self.scroll = self.scroll.max(scroll).min(self.cursor);
        self.scope = scope;
        self.focused = focused;
        self.playback = playback;
        self.empty_text = if scope == QueueScope::Local {
            "  Add items with p from Home or library tabs".into()
        } else {
            "  Remote queue is empty".into()
        };
        self.title = Some(title);
    }

    pub(in crate::app) fn set_area(&mut self, area: Rect) {
        self.area = area;
    }

    pub(in crate::app) fn set_title_area(&mut self, area: Option<Rect>) {
        self.title_area = area;
    }

    fn cursor_message(&self) -> Option<Msg> {
        self.slots.get(self.cursor).map(|slot| {
            Msg::Queue(QueueRequest::Cursor {
                scope: self.scope,
                slot_id: slot.slot_id,
            })
        })
    }

    fn move_cursor(&mut self, delta: isize) -> Option<Msg> {
        let last = self.slots.len().saturating_sub(1) as isize;
        if last >= 0 {
            self.cursor = (self.cursor as isize + delta).clamp(0, last) as usize;
        }
        self.cursor_message()
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Char('[') if key.modifiers.is_empty() => {
                self.scope = QueueScope::Local;
                return Some(Msg::Queue(QueueRequest::Scope(self.scope)));
            }
            Key::Char(']') if key.modifiers.is_empty() => {
                self.scope = QueueScope::Remote;
                return Some(Msg::Queue(QueueRequest::Scope(self.scope)));
            }
            Key::Up if key.modifiers.is_empty() => return self.move_cursor(-1),
            Key::Down if key.modifiers.is_empty() => return self.move_cursor(1),
            Key::PageUp if key.modifiers.is_empty() => {
                return self.move_cursor(-(self.area.height.saturating_sub(1).max(1) as isize));
            }
            Key::PageDown if key.modifiers.is_empty() => {
                return self.move_cursor(self.area.height.saturating_sub(1).max(1) as isize);
            }
            Key::Home if key.modifiers.is_empty() => {
                self.cursor = 0;
                return self.cursor_message();
            }
            Key::End if key.modifiers.is_empty() => {
                self.cursor = self.slots.len().saturating_sub(1);
                return self.cursor_message();
            }
            Key::Enter => {
                return self.slots.get(self.cursor).map(|slot| {
                    Msg::Queue(QueueRequest::Play {
                        scope: self.scope,
                        slot_id: slot.slot_id,
                    })
                });
            }
            Key::Delete => {
                return self.slots.get(self.cursor).map(|slot| {
                    Msg::Queue(QueueRequest::Remove {
                        scope: self.scope,
                        slot_id: slot.slot_id,
                    })
                });
            }
            Key::Up if key.modifiers.contains(tuirealm::event::KeyModifiers::SHIFT) => {
                return self.slots.get(self.cursor).map(|slot| {
                    Msg::Queue(QueueRequest::Move {
                        scope: self.scope,
                        slot_id: slot.slot_id,
                        direction: QueueMove::Up,
                    })
                });
            }
            Key::Down if key.modifiers.contains(tuirealm::event::KeyModifiers::SHIFT) => {
                return self.slots.get(self.cursor).map(|slot| {
                    Msg::Queue(QueueRequest::Move {
                        scope: self.scope,
                        slot_id: slot.slot_id,
                        direction: QueueMove::Down,
                    })
                });
            }
            _ => {}
        }
        Some(Msg::Shell(ShellRequest::QueueKey(to_crossterm_key_event(
            key,
        ))))
    }

    /// The component owns *where* a Queue event lands: it hit-tests its
    /// painted geometry (`area`, `rows`, and scope-pill targets, rebuilt
    /// every `view`) and emits typed shell intent. It holds no double-click
    /// or scroll timing; the shell decides *when* using App's shared fields.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let mouse = to_crossterm_mouse_event(mouse);
        let position: Position = (mouse.column, mouse.row).into();
        match mouse.kind {
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                if self.title.is_some() {
                    if self.geometry.scope_local_area.contains(position) {
                        self.scope = QueueScope::Local;
                        return Some(Msg::Shell(ShellRequest::QueueClick {
                            region: QueueHitRegion::ScopeLocal,
                            col: mouse.column,
                            row: mouse.row,
                        }));
                    }
                    if self.geometry.scope_remote_area.contains(position) {
                        self.scope = QueueScope::Remote;
                        return Some(Msg::Shell(ShellRequest::QueueClick {
                            region: QueueHitRegion::ScopeRemote,
                            col: mouse.column,
                            row: mouse.row,
                        }));
                    }
                }
                if self.area.contains(position) {
                    if let Some((_, slot_id)) = self
                        .geometry
                        .rows
                        .iter()
                        .find(|(rect, _)| rect.contains(position))
                    {
                        if let Some(index) =
                            self.slots.iter().position(|slot| slot.slot_id == *slot_id)
                        {
                            self.cursor = index;
                        }
                    }
                    return Some(Msg::Shell(ShellRequest::QueueClick {
                        region: QueueHitRegion::Row,
                        col: mouse.column,
                        row: mouse.row,
                    }));
                }
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
                if self.area.contains(position) {
                    return Some(Msg::Shell(ShellRequest::QueueClick {
                        region: QueueHitRegion::ContextMenu,
                        col: mouse.column,
                        row: mouse.row,
                    }));
                }
            }
            crossterm::event::MouseEventKind::ScrollUp
            | crossterm::event::MouseEventKind::ScrollDown
                if self.area.contains(position) =>
            {
                let delta: i64 = if matches!(mouse.kind, crossterm::event::MouseEventKind::ScrollUp)
                {
                    -1
                } else {
                    1
                };
                return Some(Msg::Shell(ShellRequest::QueueScroll { delta }));
            }
            _ => {}
        }
        Some(Msg::Legacy(LegacyTerminalEvent::Mouse(mouse)))
    }
}

impl Default for QueueComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for QueueComponent {
    fn view(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let area = if self.area.width > 0 { self.area } else { area };
        self.geometry = QueueRenderGeometry::default();
        if let (Some(title_area), Some(title)) = (self.title_area, self.title.as_ref()) {
            render_queue_title_content(frame, title_area, title, &mut self.geometry);
        }
        render_queue_content(
            frame,
            area,
            self.focused,
            &self.slots,
            &mut self.cursor,
            &mut self.scroll,
            self.playback,
            &self.empty_text,
            &mut self.geometry,
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

impl AppComponent<Msg, UserEvent> for QueueComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
