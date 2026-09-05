//! Central input resolution: the single, testable seam that turns a key press
//! (in a given UI context) into a semantic `Command`, a `Swallow`, or a
//! `FallThrough`. See `docs/adr/0002-centralized-input-handling.md`.
//!
//! The legacy `CONTEXT_STACK` and its handler functions were removed in the
//! keyboard-endpoint deletion (task 8.1). All input resolution is now done
//! by the central Keyboard Router (`key_policy.rs` + `router.rs`).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A normalized key press: physical key code plus active modifiers, with the
/// terminal-specific `kind`/`state` fields of `KeyEvent` dropped. This is the
/// unit the resolver matches bindings against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct KeyChord {
    pub(super) code: KeyCode,
    pub(super) mods: KeyModifiers,
}

impl KeyChord {
    pub(super) fn new(code: KeyCode, mods: KeyModifiers) -> Self {
        Self { code, mods }
    }

    pub(super) fn from_key(key: KeyEvent) -> Self {
        Self::new(key.code, key.modifiers)
    }
}

/// Convert a TuiRealm `KeyEvent` to a crossterm `KeyEvent` for the
/// central keyboard router.
pub(super) fn tuirealm_key_to_crossterm(
    key: tuirealm::event::KeyEvent,
) -> crossterm::event::KeyEvent {
    use tuirealm::event::{Key as TuiKey, KeyModifiers as TuiMods};
    let code = match key.code {
        TuiKey::Backspace => crossterm::event::KeyCode::Backspace,
        TuiKey::Char(c) => crossterm::event::KeyCode::Char(c),
        TuiKey::Enter => crossterm::event::KeyCode::Enter,
        TuiKey::Left => crossterm::event::KeyCode::Left,
        TuiKey::Right => crossterm::event::KeyCode::Right,
        TuiKey::Up => crossterm::event::KeyCode::Up,
        TuiKey::Down => crossterm::event::KeyCode::Down,
        TuiKey::Home => crossterm::event::KeyCode::Home,
        TuiKey::End => crossterm::event::KeyCode::End,
        TuiKey::PageUp => crossterm::event::KeyCode::PageUp,
        TuiKey::PageDown => crossterm::event::KeyCode::PageDown,
        TuiKey::Tab => crossterm::event::KeyCode::Tab,
        TuiKey::BackTab => crossterm::event::KeyCode::BackTab,
        TuiKey::Delete => crossterm::event::KeyCode::Delete,
        TuiKey::Insert => crossterm::event::KeyCode::Insert,
        TuiKey::Esc => crossterm::event::KeyCode::Esc,
        TuiKey::Null => crossterm::event::KeyCode::Null,
        TuiKey::Function(n) => crossterm::event::KeyCode::F(n),
        _ => crossterm::event::KeyCode::Null,
    };
    let mut modifiers = crossterm::event::KeyModifiers::empty();
    if key.modifiers.contains(TuiMods::SHIFT) {
        modifiers.insert(crossterm::event::KeyModifiers::SHIFT);
    }
    if key.modifiers.contains(TuiMods::CONTROL) {
        modifiers.insert(crossterm::event::KeyModifiers::CONTROL);
    }
    if key.modifiers.contains(TuiMods::ALT) {
        modifiers.insert(crossterm::event::KeyModifiers::ALT);
    }
    crossterm::event::KeyEvent::new(code, modifiers)
}
/// UI context for key resolution — which surface "owns" the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputContext {
    Playback,
}

/// Plain-data snapshot of playback state for key resolution.
#[derive(Debug, Clone, Copy)]
pub(super) struct InputSnapshot {
    pub player_active: bool,
    pub has_remote_session: bool,
}

/// Outcome of resolving a key within a context.
#[derive(Debug, Clone, PartialEq)]
// TODO(interactive-surface-ledger): retain Swallow for the shared resolver outcome shape.
#[allow(dead_code)]
pub(super) enum KeyResolution {
    Command(super::action::Command),
    Swallow,
    FallThrough,
}

pub(super) fn resolve_key(
    context: InputContext,
    snapshot: &InputSnapshot,
    chord: KeyChord,
) -> KeyResolution {
    match context {
        InputContext::Playback => super::action::playback_command_for_key(
            chord,
            snapshot.player_active,
            snapshot.has_remote_session,
        )
        .map_or(KeyResolution::FallThrough, KeyResolution::Command),
    }
}
