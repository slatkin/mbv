//! Temporary `LegacyInput` bridge component (design D11/D13).
//!
//! During the mixed-framework strangler phase the shell `Model` owns `App`
//! and draws the legacy UI / runs its handlers directly. `LegacyInput` is the
//! only TuiRealm component mounted in checkpoint 1: it is **message-only**,
//! owns no `App`, and does one job — translate a TuiRealm `Event<UserEvent>`
//! into a typed `Msg::Legacy` the `Model` consumes. A component mounted inside
//! the `Application` cannot borrow the `App` its enclosing `Model` holds
//! (Rust aliasing), so the bridge carries data, not a reference (design D13).
//!
//! `on` returns a `Msg` for *every* event it receives (even ones the `Model`
//! will no-op), so a non-empty `tick` result is the "an event was processed"
//! signal that drives the existing `had_events` → `wants_terminal_render`
//! redraw path (design D12). This matches the legacy loop, which set
//! `had_events = true` whenever `event::poll` reported ready, before the
//! key-kind checks.
//!
//! TODO(migrate-tui-to-tuirealm): delete this whole module at task 5.3 when
//! `LegacyInput` is removed and the last surface has left the legacy path.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MediaKeyCode, MouseButton, MouseEvent, MouseEventKind,
};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{LegacyTerminalEvent, Msg};
use super::user_event::UserEvent;

/// Temporary message-only bridge from TuiRealm terminal events to the legacy
/// `App` input handlers. Owns no state.
// TODO(migrate-tui-to-tuirealm): remove at task 5.3.
#[derive(Default)]
pub struct LegacyInput;

impl Component for LegacyInput {
    // LegacyInput paints nothing: the `Model` draws the legacy UI directly via
    // `App::render`. TuiRealm requires a `view`, so this is a deliberate no-op.
    fn view(&mut self, _frame: &mut Frame, _area: Rect) {}

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

impl AppComponent<Msg, UserEvent> for LegacyInput {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        // Always return a Msg (see module docs): a non-empty tick result is
        // the redraw signal. Events the legacy loop no-ops map to `NoOp`.
        let legacy = match ev {
            // The crossterm adapter only emits `Keyboard` for `KeyEventKind::Press`
            // (non-Press keys collapse to `Event::None`), so a `Keyboard` event
            // reaching here is already a Press — the legacy `kind != Press`
            // guard is preserved by the adapter, not re-checked here.
            Event::Keyboard(key) => LegacyTerminalEvent::Key(to_crossterm_key_event(key)),
            Event::Mouse(mouse) => LegacyTerminalEvent::Mouse(to_crossterm_mouse_event(mouse)),
            Event::WindowResize(_, _) => LegacyTerminalEvent::Resize,
            Event::FocusGained => LegacyTerminalEvent::FocusGained,
            Event::FocusLost => LegacyTerminalEvent::FocusLost,
            // `None` = a non-Press key collapsed by the adapter; `Paste` was
            // ignored by the legacy `_ => {}` arm; `Tick`/`User` are not
            // produced in checkpoint 1 (no tick interval, no user ports) but
            // are handled here so every event maps to a Msg.
            Event::None | Event::Paste(_) | Event::Tick | Event::User(_) => {
                LegacyTerminalEvent::NoOp
            }
        };
        Some(Msg::Legacy(legacy))
    }
}

// --- TuiRealm → crossterm reconstruction -------------------------------------
//
// TuiRealm's crossterm adapter (`tuirealm::terminal::event_listener::crossterm`)
// converts crossterm events into TuiRealm's own `Event`/`KeyEvent`/`MouseEvent`
// types, which differ from crossterm's (TuiRealm's `KeyEvent` has no `kind`/
// `state`; its `Key`/`KeyModifiers`/`Mouse*` enums are parallel but distinct).
// The existing `App` handlers take crossterm types, so the bridge reconstructs
// crossterm events from the TuiRealm values. This is transmute-free and
// faithful for every event class mbv handles; the only losses (documented
// inline) are fields mbv never reads.
//
// Shared with other mixed-phase components (e.g. `ConfirmComponent`) that
// also need to forward crossterm events to the legacy `App` handlers.

pub(super) fn to_crossterm_key_event(ev: &KeyEvent) -> crossterm::event::KeyEvent {
    crossterm::event::KeyEvent {
        code: to_crossterm_key_code(ev.code),
        modifiers: to_crossterm_key_modifiers(ev.modifiers),
        // The adapter already filtered to Press; TuiRealm's KeyEvent carries
        // no kind, so Press is the only faithful reconstruction.
        kind: crossterm::event::KeyEventKind::Press,
        // TuiRealm's KeyEvent carries no state; crossterm only populates it
        // under DISAMBIGUATE_ESCAPE_CODES and no mbv handler reads it.
        state: crossterm::event::KeyEventState::NONE,
    }
}

fn to_crossterm_key_code(key: Key) -> crossterm::event::KeyCode {
    use crossterm::event::KeyCode;
    match key {
        Key::Backspace => KeyCode::Backspace,
        Key::Enter => KeyCode::Enter,
        Key::Left => KeyCode::Left,
        Key::Right => KeyCode::Right,
        Key::Up => KeyCode::Up,
        Key::Down => KeyCode::Down,
        Key::Home => KeyCode::Home,
        Key::End => KeyCode::End,
        Key::PageUp => KeyCode::PageUp,
        Key::PageDown => KeyCode::PageDown,
        Key::Tab => KeyCode::Tab,
        Key::BackTab => KeyCode::BackTab,
        Key::Delete => KeyCode::Delete,
        Key::Insert => KeyCode::Insert,
        Key::Function(f) => KeyCode::F(f),
        Key::Char(ch) => KeyCode::Char(ch),
        Key::Null => KeyCode::Null,
        Key::Esc => KeyCode::Esc,
        Key::CapsLock => KeyCode::CapsLock,
        Key::ScrollLock => KeyCode::ScrollLock,
        Key::NumLock => KeyCode::NumLock,
        Key::PrintScreen => KeyCode::PrintScreen,
        Key::Pause => KeyCode::Pause,
        Key::Menu => KeyCode::Menu,
        Key::KeypadBegin => KeyCode::KeypadBegin,
        Key::Media(m) => KeyCode::Media(to_crossterm_media_key_code(m)),
        // The ShiftLeft/AltLeft/CtrlLeft/… variants are only produced by the
        // termion backend, never by crossterm, so they cannot reach this
        // crossterm-driven bridge. Map them to Null (a no-op for every App
        // handler) instead of inventing a crossterm KeyCode.
        _ => KeyCode::Null,
    }
}

fn to_crossterm_key_modifiers(m: KeyModifiers) -> crossterm::event::KeyModifiers {
    let mut out = crossterm::event::KeyModifiers::NONE;
    // TuiRealm's KeyModifiers only carries SHIFT/CONTROL/ALT (the crossterm
    // adapter forwards exactly those). SUPER/HYPER/META are therefore not
    // reconstructed; no mbv handler tests for them.
    if m.intersects(KeyModifiers::SHIFT) {
        out.insert(crossterm::event::KeyModifiers::SHIFT);
    }
    if m.intersects(KeyModifiers::CONTROL) {
        out.insert(crossterm::event::KeyModifiers::CONTROL);
    }
    if m.intersects(KeyModifiers::ALT) {
        out.insert(crossterm::event::KeyModifiers::ALT);
    }
    out
}

fn to_crossterm_media_key_code(m: MediaKeyCode) -> crossterm::event::MediaKeyCode {
    match m {
        MediaKeyCode::Play => crossterm::event::MediaKeyCode::Play,
        MediaKeyCode::Pause => crossterm::event::MediaKeyCode::Pause,
        MediaKeyCode::PlayPause => crossterm::event::MediaKeyCode::PlayPause,
        MediaKeyCode::Reverse => crossterm::event::MediaKeyCode::Reverse,
        MediaKeyCode::Stop => crossterm::event::MediaKeyCode::Stop,
        MediaKeyCode::FastForward => crossterm::event::MediaKeyCode::FastForward,
        MediaKeyCode::Rewind => crossterm::event::MediaKeyCode::Rewind,
        MediaKeyCode::TrackNext => crossterm::event::MediaKeyCode::TrackNext,
        MediaKeyCode::TrackPrevious => crossterm::event::MediaKeyCode::TrackPrevious,
        MediaKeyCode::Record => crossterm::event::MediaKeyCode::Record,
        MediaKeyCode::LowerVolume => crossterm::event::MediaKeyCode::LowerVolume,
        MediaKeyCode::RaiseVolume => crossterm::event::MediaKeyCode::RaiseVolume,
        MediaKeyCode::MuteVolume => crossterm::event::MediaKeyCode::MuteVolume,
    }
}

pub(super) fn to_crossterm_mouse_event(ev: &MouseEvent) -> crossterm::event::MouseEvent {
    crossterm::event::MouseEvent {
        kind: to_crossterm_mouse_event_kind(ev.kind),
        column: ev.column,
        row: ev.row,
        modifiers: to_crossterm_key_modifiers(ev.modifiers),
    }
}

fn to_crossterm_mouse_event_kind(kind: MouseEventKind) -> crossterm::event::MouseEventKind {
    match kind {
        MouseEventKind::Down(b) => {
            crossterm::event::MouseEventKind::Down(to_crossterm_mouse_button(b))
        }
        MouseEventKind::Up(b) => crossterm::event::MouseEventKind::Up(to_crossterm_mouse_button(b)),
        MouseEventKind::Drag(b) => {
            crossterm::event::MouseEventKind::Drag(to_crossterm_mouse_button(b))
        }
        MouseEventKind::Moved => crossterm::event::MouseEventKind::Moved,
        MouseEventKind::ScrollDown => crossterm::event::MouseEventKind::ScrollDown,
        MouseEventKind::ScrollUp => crossterm::event::MouseEventKind::ScrollUp,
        MouseEventKind::ScrollLeft => crossterm::event::MouseEventKind::ScrollLeft,
        MouseEventKind::ScrollRight => crossterm::event::MouseEventKind::ScrollRight,
    }
}

fn to_crossterm_mouse_button(b: MouseButton) -> crossterm::event::MouseButton {
    match b {
        MouseButton::Left => crossterm::event::MouseButton::Left,
        MouseButton::Right => crossterm::event::MouseButton::Right,
        MouseButton::Middle => crossterm::event::MouseButton::Middle,
    }
}
