//! The GUI Client's egui translation edge (`add-gui-front-end` design D4,
//! row 6.3): the frame's raw egui events become the neutral
//! [`KeyEvent`](mbv_ui_msg::KeyEvent)/[`PointerEvent`](mbv_ui_msg::PointerEvent)
//! vocabulary, mirroring the TUI listener edge (`mbv_components::root`), so
//! one physical press resolves to the same neutral chord in both Clients.
//!
//! egui reports one printable key press as an `Event::Key` over a caseless
//! key plus an `Event::Text` twin carrying the produced character, and it
//! suppresses the twin when Ctrl or the platform command key is held. The
//! character twin is authoritative for printable presses — it carries the
//! shifted symbol a punctuation key produces, exactly as the terminal
//! delivers it — so a `Key` event for a character key stands down whenever
//! its character arrived as text in the same frame, and it maps only when
//! the twin cannot exist (a named or function key, or a Ctrl/command-held
//! press). That keeps one press one chord, with no double dispatch and no
//! wildcard mapping: an egui key the neutral vocabulary cannot represent
//! yields nothing, exactly as the TUI edge's `Key::Null` matches no chord.

use eframe::egui;
use mbv::GuiInput;
use mbv_ui_msg::{
    Key as NeutralKey, KeyEvent as NeutralKeyEvent, KeyModifiers as NeutralMods, PointerEvent,
    PointerKind, Position,
};

/// Translate one frame's raw egui events into the neutral input vocabulary,
/// in arrival order. `frame_modifiers` is the modifier state the frame's
/// text events carry (`Event::Text` has no modifiers of its own);
/// `primary_down` is whether the primary button is held (a movement with it
/// held is a drag); `pointer_pos` is the pointer's latest position, which a
/// wheel event resolves against because egui's wheel event carries none.
#[must_use]
pub(crate) fn neutral_input(
    events: &[egui::Event],
    frame_modifiers: egui::Modifiers,
    primary_down: bool,
    pointer_pos: Option<egui::Pos2>,
) -> Vec<GuiInput> {
    let text_chars: Vec<char> = events
        .iter()
        .filter_map(text_char)
        .map(|c| {
            // The folded code the character's `Key` twin maps to: egui
            // reports letter keys caselessly, so an uppercase text character
            // and its caseless key denote one press.
            if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect();
    let mut out = Vec::new();
    for event in events {
        if let Some(key) = neutral_key(event, frame_modifiers, &text_chars) {
            out.push(GuiInput::Key(key));
        }
        if let Some(pointer) = neutral_pointer(event, frame_modifiers, primary_down, pointer_pos) {
            out.push(GuiInput::Pointer(pointer));
        }
    }
    out
}

/// The one printable character a `Text` event carries, `None` for every
/// other event shape (multi-character text is an IME or paste sequence, not
/// a chord).
fn text_char(event: &egui::Event) -> Option<char> {
    let egui::Event::Text(text) = event else {
        return None;
    };
    let mut chars = text.chars();
    let c = chars.next()?;
    (chars.next().is_none() && !c.is_ascii_control()).then_some(c)
}

/// The character an egui character key produces unshifted. egui reports
/// character keys caselessly (`Key::N`), and its punctuation keys name the
/// unshifted symbol (`Key::Minus`), so this is the twin the `Text` event
/// carries.
fn character_key_char(key: egui::Key) -> Option<char> {
    Some(match key {
        egui::Key::Space => ' ',
        egui::Key::Colon => ':',
        egui::Key::Comma => ',',
        egui::Key::Backslash => '\\',
        egui::Key::Slash => '/',
        egui::Key::Pipe => '|',
        egui::Key::Questionmark => '?',
        egui::Key::Exclamationmark => '!',
        egui::Key::OpenBracket => '[',
        egui::Key::CloseBracket => ']',
        egui::Key::OpenCurlyBracket => '{',
        egui::Key::CloseCurlyBracket => '}',
        egui::Key::Backtick => '`',
        egui::Key::Minus => '-',
        egui::Key::Period => '.',
        egui::Key::Plus => '+',
        egui::Key::Equals => '=',
        egui::Key::Semicolon => ';',
        egui::Key::Quote => '\'',
        egui::Key::Num0 => '0',
        egui::Key::Num1 => '1',
        egui::Key::Num2 => '2',
        egui::Key::Num3 => '3',
        egui::Key::Num4 => '4',
        egui::Key::Num5 => '5',
        egui::Key::Num6 => '6',
        egui::Key::Num7 => '7',
        egui::Key::Num8 => '8',
        egui::Key::Num9 => '9',
        egui::Key::A => 'a',
        egui::Key::B => 'b',
        egui::Key::C => 'c',
        egui::Key::D => 'd',
        egui::Key::E => 'e',
        egui::Key::F => 'f',
        egui::Key::G => 'g',
        egui::Key::H => 'h',
        egui::Key::I => 'i',
        egui::Key::J => 'j',
        egui::Key::K => 'k',
        egui::Key::L => 'l',
        egui::Key::M => 'm',
        egui::Key::N => 'n',
        egui::Key::O => 'o',
        egui::Key::P => 'p',
        egui::Key::Q => 'q',
        egui::Key::R => 'r',
        egui::Key::S => 's',
        egui::Key::T => 't',
        egui::Key::U => 'u',
        egui::Key::V => 'v',
        egui::Key::W => 'w',
        egui::Key::X => 'x',
        egui::Key::Y => 'y',
        egui::Key::Z => 'z',
        _ => return None,
    })
}

/// The named and function keys the neutral vocabulary represents, beside
/// the character keys [`character_key_char`] covers.
const KEY_CODES: &[(egui::Key, NeutralKey)] = &[
    (egui::Key::Backspace, NeutralKey::Backspace),
    (egui::Key::Enter, NeutralKey::Enter),
    (egui::Key::ArrowLeft, NeutralKey::Left),
    (egui::Key::ArrowRight, NeutralKey::Right),
    (egui::Key::ArrowUp, NeutralKey::Up),
    (egui::Key::ArrowDown, NeutralKey::Down),
    (egui::Key::Home, NeutralKey::Home),
    (egui::Key::End, NeutralKey::End),
    (egui::Key::PageUp, NeutralKey::PageUp),
    (egui::Key::PageDown, NeutralKey::PageDown),
    (egui::Key::Tab, NeutralKey::Tab),
    (egui::Key::Delete, NeutralKey::Delete),
    (egui::Key::Insert, NeutralKey::Insert),
    (egui::Key::Escape, NeutralKey::Esc),
    (egui::Key::F1, NeutralKey::Function(1)),
    (egui::Key::F2, NeutralKey::Function(2)),
    (egui::Key::F3, NeutralKey::Function(3)),
    (egui::Key::F4, NeutralKey::Function(4)),
    (egui::Key::F5, NeutralKey::Function(5)),
    (egui::Key::F6, NeutralKey::Function(6)),
    (egui::Key::F7, NeutralKey::Function(7)),
    (egui::Key::F8, NeutralKey::Function(8)),
    (egui::Key::F9, NeutralKey::Function(9)),
    (egui::Key::F10, NeutralKey::Function(10)),
    (egui::Key::F11, NeutralKey::Function(11)),
    (egui::Key::F12, NeutralKey::Function(12)),
    (egui::Key::F13, NeutralKey::Function(13)),
    (egui::Key::F14, NeutralKey::Function(14)),
    (egui::Key::F15, NeutralKey::Function(15)),
    (egui::Key::F16, NeutralKey::Function(16)),
    (egui::Key::F17, NeutralKey::Function(17)),
    (egui::Key::F18, NeutralKey::Function(18)),
    (egui::Key::F19, NeutralKey::Function(19)),
    (egui::Key::F20, NeutralKey::Function(20)),
    (egui::Key::F21, NeutralKey::Function(21)),
    (egui::Key::F22, NeutralKey::Function(22)),
    (egui::Key::F23, NeutralKey::Function(23)),
    (egui::Key::F24, NeutralKey::Function(24)),
];

/// The neutral key code an egui key maps to, `None` for the keys the
/// neutral vocabulary cannot represent (no wildcard that silently maps to
/// something). Function keys F1-F24 map onto [`NeutralKey::Function`].
fn neutral_key_code(key: egui::Key) -> Option<NeutralKey> {
    if let Some(c) = character_key_char(key) {
        return Some(NeutralKey::Char(c));
    }
    KEY_CODES
        .iter()
        .find_map(|(egui_key, mapped_key)| (*egui_key == key).then_some(*mapped_key))
}

/// The neutral modifiers an egui modifier set maps to. The platform command
/// key is the Ctrl of the shortcuts egui reports (`Modifiers::command` is
/// `ctrl` on Windows and Linux), so it folds into CONTROL.
#[must_use]
fn neutral_mods(modifiers: egui::Modifiers) -> NeutralMods {
    let mut mods = NeutralMods::NONE;
    if modifiers.shift {
        mods = mods.union(NeutralMods::SHIFT);
    }
    if modifiers.ctrl || modifiers.command {
        mods = mods.union(NeutralMods::CONTROL);
    }
    if modifiers.alt {
        mods = mods.union(NeutralMods::ALT);
    }
    mods
}

/// Translate one egui event to the neutral key chord it carries, `None` when
/// it carries none. See the module docs for the character-key rule.
fn neutral_key(
    event: &egui::Event,
    frame_modifiers: egui::Modifiers,
    text_chars: &[char],
) -> Option<NeutralKeyEvent> {
    let egui::Event::Key {
        key,
        pressed,
        modifiers,
        ..
    } = event
    else {
        return text_key(event, frame_modifiers);
    };
    if !pressed {
        return None;
    }
    let mods = neutral_mods(*modifiers);
    let mut code = neutral_key_code(*key)?;
    // egui reports Shift+Tab as the caseless `Key::Tab` with SHIFT; the
    // terminal delivers the same press as `BackTab` (the resolver drops the
    // redundant SHIFT from a `BackTab` chord).
    if *key == egui::Key::Tab && modifiers.shift {
        code = NeutralKey::BackTab;
    }
    // A character key's press has a `Text` twin unless Ctrl or the platform
    // command key is held (egui-winit suppresses it); the twin carries the
    // produced character, so the `Key` event stands down for it.
    let has_text_twin = !modifiers.ctrl
        && !modifiers.command
        && matches!(code, NeutralKey::Char(c) if text_chars.contains(&c));
    if has_text_twin {
        return None;
    }
    Some(NeutralKeyEvent::new(code, mods))
}

/// Translate a `Text` event to its neutral chord: the produced character
/// with the frame's modifiers. The shift a symbol press carries is folded
/// into the character itself (the terminal reports `?`, not `/` plus
/// SHIFT), so SHIFT stays only where the TUI edge keeps it: on an uppercase
/// letter, which the resolver's lowercase rule folds to the same chord the
/// terminal delivers.
fn text_key(event: &egui::Event, frame_modifiers: egui::Modifiers) -> Option<NeutralKeyEvent> {
    let c = text_char(event)?;
    let mut mods = neutral_mods(frame_modifiers);
    if !c.is_ascii_uppercase() {
        mods = mods.without(NeutralMods::SHIFT);
    }
    let code = if mods.contains(NeutralMods::SHIFT) && c.is_ascii_uppercase() {
        NeutralKey::Char(c.to_ascii_lowercase())
    } else {
        NeutralKey::Char(c)
    };
    Some(NeutralKeyEvent::new(code, mods))
}

/// Translate one egui event to the neutral pointer event it carries, `None`
/// when it carries none. A movement with the primary button held is a drag;
/// a movement without one is a hover move, which the gesture policy
/// recognizes as nothing.
fn neutral_pointer(
    event: &egui::Event,
    frame_modifiers: egui::Modifiers,
    primary_down: bool,
    pointer_pos: Option<egui::Pos2>,
) -> Option<PointerEvent> {
    let (kind, position, modifiers) = match event {
        egui::Event::PointerMoved(pos) => (
            if primary_down {
                PointerKind::Drag
            } else {
                PointerKind::Move
            },
            *pos,
            frame_modifiers,
        ),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        } => (
            if *pressed {
                PointerKind::Press
            } else {
                PointerKind::Release
            },
            *pos,
            *modifiers,
        ),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers,
        } => (PointerKind::RightPress, *pos, *modifiers),
        egui::Event::MouseWheel {
            delta, modifiers, ..
        } => {
            // winit's wheel delta is positive away from the user, so a
            // positive delta is a wheel-up step. egui's wheel event carries
            // no position; the pointer's latest one stands in, and with no
            // pointer on the window the step resolves against no surface.
            let kind = match delta.y.partial_cmp(&0.0) {
                Some(std::cmp::Ordering::Greater) => PointerKind::ScrollUp,
                Some(std::cmp::Ordering::Less) => PointerKind::ScrollDown,
                _ => return None,
            };
            let pos = pointer_pos?;
            (kind, pos, *modifiers)
        }
        _ => return None,
    };
    Some(PointerEvent::new(
        kind,
        Position {
            x: position.x,
            y: position.y,
        },
        neutral_mods(modifiers),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// One pressed egui key event.
    fn pressed(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    /// The neutral key chord a translated frame carries, for the single-key
    /// frames the table below drives. `frame_modifiers` is the modifier
    /// state the frame's text events carry.
    fn translated(
        events: &[egui::Event],
        frame_modifiers: egui::Modifiers,
    ) -> Vec<NeutralKeyEvent> {
        neutral_input(events, frame_modifiers, false, None)
            .into_iter()
            .filter_map(|input| match input {
                GuiInput::Key(key) => Some(key),
                GuiInput::Pointer(_) => None,
            })
            .collect()
    }

    /// One table over the egui key mapping (add-gui-front-end task 6.3):
    /// the egui events one physical press produces, and the neutral chord
    /// each must become — the same chord the TUI edge produces for the same
    /// press. The character rows pin the `Text`-twin rule (a character key's
    /// chord comes from the produced character, so a shifted punctuation key
    /// is not a base key plus SHIFT) and the modifier lowercase rule.
    #[rstest]
    #[case::plain_char(
        egui::Modifiers::NONE,
        &[pressed(egui::Key::N, egui::Modifiers::NONE), egui::Event::Text("n".to_string())],
        &[NeutralKeyEvent::new(NeutralKey::Char('n'), NeutralMods::NONE)]
    )]
    #[case::modified_char_has_no_text_twin(
        egui::Modifiers::NONE,
        &[pressed(egui::Key::N, egui::Modifiers::CTRL)],
        &[NeutralKeyEvent::new(NeutralKey::Char('n'), NeutralMods::CONTROL)]
    )]
    #[case::named_key(
        egui::Modifiers::NONE,
        &[pressed(egui::Key::ArrowDown, egui::Modifiers::NONE)],
        &[NeutralKeyEvent::new(NeutralKey::Down, NeutralMods::NONE)]
    )]
    #[case::shift_tab_is_backtab(
        egui::Modifiers::NONE,
        &[pressed(egui::Key::Tab, egui::Modifiers::SHIFT)],
        &[NeutralKeyEvent::new(NeutralKey::BackTab, NeutralMods::SHIFT)]
    )]
    #[case::shifted_punctuation_comes_from_the_text_twin(
        egui::Modifiers::SHIFT,
        &[pressed(egui::Key::Questionmark, egui::Modifiers::SHIFT), egui::Event::Text("?".to_string())],
        &[NeutralKeyEvent::new(NeutralKey::Char('?'), NeutralMods::NONE)]
    )]
    #[case::shift_letter_folds_like_the_tui_edge(
        egui::Modifiers::SHIFT,
        &[pressed(egui::Key::N, egui::Modifiers::SHIFT), egui::Event::Text("N".to_string())],
        &[NeutralKeyEvent::new(NeutralKey::Char('n'), NeutralMods::SHIFT)]
    )]
    #[case::key_release_maps_to_nothing(
        egui::Modifiers::NONE,
        &[egui::Event::Key { key: egui::Key::N, physical_key: None, pressed: false, repeat: false, modifiers: egui::Modifiers::NONE }],
        &[]
    )]
    #[case::unmappable_key_maps_to_nothing(
        egui::Modifiers::NONE,
        &[pressed(egui::Key::BrowserBack, egui::Modifiers::NONE)],
        &[]
    )]
    fn egui_keys_translate_to_the_neutral_chords(
        #[case] frame_modifiers: egui::Modifiers,
        #[case] events: &[egui::Event],
        #[case] expected: &[NeutralKeyEvent],
    ) {
        assert_eq!(translated(events, frame_modifiers), expected);
    }
}
