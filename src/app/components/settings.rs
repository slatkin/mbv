use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use super::msg::{
    LeafKeyResult, Msg, ServiceRequest, SettingsIntent, ShellRequest, TerminalObserverEvent,
};
use super::user_event::UserEvent;
use crate::app::render::{render_settings_content, SettingsRenderGeometry, SettingsRenderModel};
use crate::app::state::types::settings::SettingsDestination;

mod setup;

/// The Esc/F3/F4/q intent shared by every Settings destination.
fn settings_intent_for_key(code: Key) -> Option<SettingsIntent> {
    match code {
        Key::Esc => Some(SettingsIntent::Back),
        Key::Function(3) => Some(SettingsIntent::OpenSessions),
        Key::Function(4) => Some(SettingsIntent::OpenPlaylists),
        Key::Char('q') => Some(SettingsIntent::Quit),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SettingsRow {
    pub label: String,
    pub value: String,
    pub section: bool,
    pub cursor: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ServiceRow {
    pub name: String,
    pub detail: String,
    pub muted: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SetupDraft {
    Emby {
        fields: [String; 3],
        focus: usize,
        busy: bool,
        error: String,
    },
    Audiobookshelf {
        fields: [String; 2],
        focus: usize,
        busy: bool,
        error: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SettingsSnapshot {
    pub destination: SettingsDestination,
    pub rows: Vec<SettingsRow>,
    pub services: Vec<ServiceRow>,
    /// The read-only Keys destination's content (design D7): group-header
    /// rows (`section: true`) for each populated `KeySection` plus one row
    /// per declared registry action with its configured chords. The
    /// destination is read-only — chords change in the config file, never
    /// in place (ADR 0023).
    pub keys: Vec<SettingsRow>,
    pub setup: Option<SetupDraft>,
    pub area: Rect,
}

pub struct SettingsComponent {
    destination: SettingsDestination,
    rows: Vec<SettingsRow>,
    services: Vec<ServiceRow>,
    keys: Vec<SettingsRow>,
    setup: Option<SetupDraft>,
    cursor: usize,
    services_cursor: usize,
    keys_cursor: usize,
    scroll: usize,
    area: Rect,
    geometry: SettingsRenderGeometry,
    /// Irregular painted chrome (task 5.2, design.md D6): the
    /// cursor-activatable rows, repopulated in `view()` from the geometry
    /// the painter just produced. Tag = the row's cursor index.
    hit_rows: HitRegions<usize>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3).
    mouse_gestures: MouseGestureState,
    initialized: bool,
}

impl SettingsComponent {
    pub fn new() -> Self {
        Self {
            destination: SettingsDestination::Main,
            rows: Vec::new(),
            services: Vec::new(),
            keys: Vec::new(),
            setup: None,
            cursor: 0,
            services_cursor: 0,
            keys_cursor: 0,
            scroll: 0,
            area: Rect::default(),
            geometry: SettingsRenderGeometry::default(),
            hit_rows: HitRegions::new(),
            mouse_gestures: MouseGestureState::new(),
            initialized: false,
        }
    }

    pub(in crate::app) fn set_content(&mut self, snapshot: SettingsSnapshot) {
        let same_setup = matches!(
            (&self.setup, &snapshot.setup),
            (Some(SetupDraft::Emby { .. }), Some(SetupDraft::Emby { .. }))
                | (
                    Some(SetupDraft::Audiobookshelf { .. }),
                    Some(SetupDraft::Audiobookshelf { .. }),
                )
        );
        if !same_setup {
            self.setup = snapshot.setup;
        } else if let (Some(current), Some(incoming)) = (&mut self.setup, snapshot.setup) {
            match (current, incoming) {
                (
                    SetupDraft::Emby { busy, error, .. },
                    SetupDraft::Emby {
                        busy: next_busy,
                        error: next_error,
                        ..
                    },
                )
                | (
                    SetupDraft::Audiobookshelf { busy, error, .. },
                    SetupDraft::Audiobookshelf {
                        busy: next_busy,
                        error: next_error,
                        ..
                    },
                ) => {
                    *busy = next_busy;
                    *error = next_error;
                }
                _ => {}
            }
        }
        let destination_changed = self.destination != snapshot.destination;
        self.destination = snapshot.destination;
        self.rows = snapshot.rows;
        self.services = snapshot.services;
        self.keys = snapshot.keys;
        // The component owns its interaction state; content pushes never
        // carry cursor/scroll values. First content (and each destination
        // change) starts from the component-local defaults — Services
        // re-entry is already handled by the Back reset, so a fresh
        // destination begins at the top. Only the bounds follow the pushed
        // content, so a cursor can never land on a removed row.
        if !self.initialized || destination_changed {
            self.cursor = 0;
            self.services_cursor = 0;
            self.keys_cursor = 0;
            self.scroll = 0;
        }
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
        self.services_cursor = self
            .services_cursor
            .min(self.services.len().saturating_sub(1));
        // The Keys cursor numbers actions only (headers carry no cursor),
        // matching the painter's highlight and the `cursor_lines` geometry
        // indexed by action ordinal.
        self.keys_cursor = self
            .keys_cursor
            .min(Self::action_count(&self.keys).saturating_sub(1));
        self.area = snapshot.area;
        self.initialized = true;
    }

    /// Cursor-numbered action rows in one list (group/section headers carry
    /// no cursor and are not addressable). The cursor is an ordinal over
    /// actions so it stays aligned with the painter's highlight and the
    /// render geometry's `cursor_lines`; the Down clamp counts the same rows
    /// so it cannot park past the last action.
    fn action_count(rows: &[SettingsRow]) -> usize {
        rows.iter().filter(|row| row.cursor.is_some()).count()
    }

    /// Largest valid scroll offset: the last geometry line fully in view.
    fn max_scroll(&self) -> usize {
        self.geometry
            .cursor_lines
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .saturating_sub((self.geometry.content_area.height as usize).saturating_sub(1))
    }

    /// Keep one cursor's document line inside the scrolled window
    /// (geometry from the latest paint; a no-op before the first paint).
    fn scroll_cursor_into_view(&mut self, cursor: usize) {
        let Some(&line) = self.geometry.cursor_lines.get(cursor) else {
            return;
        };
        let height = self.geometry.content_area.height as usize;
        if height == 0 {
            return;
        }
        if line < self.scroll {
            self.scroll = line;
        } else if line >= self.scroll + height {
            self.scroll = line + 1 - height;
        }
    }

    /// After a page/edge scroll move, keep the highlight on-screen: if the
    /// cursor's document line fell outside the scrolled window, move it to
    /// the nearest cursor-numbered action that is visible (a no-op before
    /// the first paint; `cursor_lines` is ordered, so the first/last
    /// visible row is the nearest one).
    fn clamp_keys_cursor_to_window(&mut self) {
        let height = self.geometry.content_area.height as usize;
        if height == 0 {
            return;
        }
        let Some(&line) = self.geometry.cursor_lines.get(self.keys_cursor) else {
            return;
        };
        let window = self.scroll..self.scroll + height;
        if window.contains(&line) {
            return;
        }
        let visible = self
            .geometry
            .cursor_lines
            .iter()
            .enumerate()
            .filter(|(_, l)| window.contains(l));
        let nearest = if line < self.scroll {
            visible.map(|(idx, _)| idx).next()
        } else {
            visible.map(|(idx, _)| idx).next_back()
        };
        if let Some(idx) = nearest {
            self.keys_cursor = idx;
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.setup.is_some() {
            return self.setup_key(key);
        }
        if self.destination == SettingsDestination::Keys {
            return match key.code {
                Key::Up => {
                    self.keys_cursor = self.keys_cursor.saturating_sub(1);
                    self.scroll_cursor_into_view(self.keys_cursor);
                    None
                }
                Key::Down => {
                    self.keys_cursor = (self.keys_cursor + 1)
                        .min(Self::action_count(&self.keys).saturating_sub(1));
                    self.scroll_cursor_into_view(self.keys_cursor);
                    None
                }
                Key::PageUp => {
                    self.scroll = self.scroll.saturating_sub(10);
                    self.clamp_keys_cursor_to_window();
                    None
                }
                Key::PageDown => {
                    self.scroll = self.scroll.saturating_add(10).min(self.max_scroll());
                    self.clamp_keys_cursor_to_window();
                    None
                }
                Key::Home => {
                    self.scroll = 0;
                    self.clamp_keys_cursor_to_window();
                    None
                }
                Key::End => {
                    self.scroll = self.max_scroll();
                    self.clamp_keys_cursor_to_window();
                    None
                }
                // Read-only destination (design D7): Enter/Space select
                // nothing — the chord changes in the config file, never in
                // place (ADR 0023). Returning None lets `on()`'s fallback
                // claim them like the cursor moves.
                Key::Esc | Key::Function(3) | Key::Function(4) | Key::Char('q') => {
                    let intent = settings_intent_for_key(key.code)?;
                    if matches!(intent, SettingsIntent::Back) {
                        // Leaving Keys zeroes the local cursor so the next
                        // entry starts at the top (the Services precedent).
                        self.keys_cursor = 0;
                    }
                    Some(Msg::Shell(Box::new(ShellRequest::SettingsIntent(intent))))
                }
                _ => None,
            };
        }
        if self.destination == SettingsDestination::Services {
            return match key.code {
                Key::Up => {
                    self.services_cursor = self.services_cursor.saturating_sub(1);
                    None
                }
                Key::Down => {
                    self.services_cursor =
                        (self.services_cursor + 1).min(self.services.len().saturating_sub(1));
                    None
                }
                Key::Enter
                | Key::Char(' ')
                | Key::Char('d')
                | Key::Char('D')
                | Key::Char('t')
                | Key::Char('T')
                | Key::Char('r')
                | Key::Char('R') => self.service_key(key),
                Key::Esc | Key::Function(3) | Key::Function(4) | Key::Char('q') => {
                    let intent = settings_intent_for_key(key.code)?;
                    if matches!(intent, SettingsIntent::Back) {
                        // Leaving Services zeroes the local cursor so the next
                        // entry starts at the top; the shell-side mirror of
                        // this reset is being deleted.
                        self.services_cursor = 0;
                    }
                    Some(Msg::Shell(Box::new(ShellRequest::SettingsIntent(intent))))
                }
                _ => None,
            };
        }
        match key.code {
            Key::Esc | Key::Function(3) | Key::Function(4) | Key::Char('q') => {
                settings_intent_for_key(key.code)
                    .map(|intent| Msg::Shell(Box::new(ShellRequest::SettingsIntent(intent))))
            }
            Key::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                self.scroll_cursor_into_view(self.cursor);
                None
            }
            Key::Down => {
                self.cursor =
                    (self.cursor + 1).min(Self::action_count(&self.rows).saturating_sub(1));
                self.scroll_cursor_into_view(self.cursor);
                None
            }
            Key::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                None
            }
            Key::PageDown => {
                self.scroll = self.scroll.saturating_add(10).min(self.max_scroll());
                None
            }
            Key::Left | Key::Right | Key::Char(' ') | Key::Enter => Some(Msg::Shell(Box::new(
                ShellRequest::SettingsIntent(SettingsIntent::Activate(self.cursor)),
            ))),
            _ => None,
        }
    }

    /// Mouse handling (task 5.2): recognition via the component's own
    /// `MouseGestureState` (ADR 0024, design.md D3); row geometry via
    /// `HitRegions` (D6). Behaviour unchanged from the ad-hoc handler: a
    /// click outside the panel dismisses, a click on a cursor-activatable
    /// row selects and activates it (the Enter/Space equivalent), and the
    /// focused overlay's wheel scrolls by one document line per throttled step.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Click { at, .. } | MouseGesture::DoubleClick(at) => {
                if !self.geometry.panel_area.contains(at) {
                    return Some(Msg::Shell(Box::new(ShellRequest::DismissSettings)));
                }
                let &cursor = self.hit_rows.resolve(at)?;
                if self.destination == SettingsDestination::Services {
                    self.services_cursor = cursor;
                    return Some(Msg::Service(ServiceRequest::ActivateService(cursor)));
                }
                if self.destination == SettingsDestination::Keys {
                    // Read-only destination: a click selects the row; there
                    // is no activation.
                    self.keys_cursor = cursor;
                    return None;
                }
                self.cursor = cursor;
                Some(Msg::Shell(Box::new(ShellRequest::SettingsIntent(
                    SettingsIntent::Activate(cursor),
                ))))
            }
            MouseGesture::Scroll { delta, .. } => {
                let max_scroll = self.max_scroll();
                self.scroll = self
                    .scroll
                    .saturating_add_signed(delta as isize)
                    .min(max_scroll);
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_rows(&self) -> &HitRegions<usize> {
        &self.hit_rows
    }
}

impl Default for SettingsComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SettingsComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = if self.area.width > 0 { self.area } else { area };
        // The Keys destination paints through the same row painter as the
        // main list (group headers + label/value rows); only the content
        // source and its cursor differ.
        let (rows, cursor) = if self.destination == SettingsDestination::Keys {
            (&self.keys, self.keys_cursor)
        } else {
            (&self.rows, self.cursor)
        };
        render_settings_content(
            frame,
            area,
            SettingsRenderModel {
                destination: self.destination,
                rows,
                services: &self.services,
                setup: self.setup.as_ref(),
                cursor,
                services_cursor: self.services_cursor,
                scroll: self.scroll,
            },
            &mut self.geometry,
        );
        // Adopt the cursor-activatable rows the painter just produced into
        // the irregular-chrome registry (task 5.2, design.md D6). The
        // painter's `cursor_lines` are document line numbers indexed by
        // cursor; document line `line` paints at `content_area.y + line -
        // scroll`.
        self.hit_rows.clear();
        for (cursor, &line) in self.geometry.cursor_lines.iter().enumerate() {
            if line < self.scroll {
                continue;
            }
            let offset = (line - self.scroll) as u16;
            if offset >= self.geometry.content_area.height {
                continue;
            }
            self.hit_rows.push(
                Rect {
                    x: self.geometry.content_area.x,
                    y: self.geometry.content_area.y + offset,
                    width: self.geometry.content_area.width,
                    height: 1,
                },
                cursor,
            );
        }
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

impl AppComponent<Msg, UserEvent> for SettingsComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => match self.handle_key(key) {
                Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))).into_option(),
                None if self.destination == SettingsDestination::Keys
                    && matches!(key.code, Key::PageUp | Key::PageDown | Key::Home | Key::End) =>
                {
                    // The Keys destination consumed the chord as a local
                    // scroll move; claim it like the cursor moves.
                    LeafKeyResult::Consumed(None).into_option()
                }
                None if matches!(
                    key.code,
                    Key::Up
                        | Key::Down
                        | Key::Tab
                        | Key::BackTab
                        | Key::Enter
                        | Key::Backspace
                        | Key::Esc
                        | Key::Char(_)
                ) =>
                {
                    LeafKeyResult::Consumed(None).into_option()
                }
                None => LeafKeyResult::Unhandled.into_option(),
            },
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
