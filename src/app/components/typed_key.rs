//! Conversion from TuiRealm key events to crossterm key events.
//!
//! Components that still delegate a typed key request to the legacy `App`
//! handlers use this adapter while those handlers remain in the shell.

use tuirealm::event::{Key, KeyEvent, KeyModifiers, MediaKeyCode};

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
        // termion backend, never by crossterm. Map them to Null (a no-op for
        // every App handler) instead of inventing a crossterm KeyCode.
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
