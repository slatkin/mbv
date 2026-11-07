use mbv_gui::bridge::keys::{DomKey, DomModifiers, neutral_key};
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};
use rstest::rstest;

/// A press of `key` with no modifier held. `code` is carried but never read.
fn press(key: &str) -> DomKey {
    DomKey {
        key: key.to_owned(),
        code: String::new(),
        meta_key: false,
        repeat: false,
        modifiers: DomModifiers::default(),
    }
}

fn shifted(mut event: DomKey) -> DomKey {
    event.modifiers.shift_key = true;
    event
}

fn ctrl_alt(mut event: DomKey) -> DomKey {
    event.modifiers.ctrl_key = true;
    event.modifiers.alt_key = true;
    event
}

fn with_meta(mut event: DomKey) -> DomKey {
    event.meta_key = true;
    event
}

/// Each DOM key press translates to the neutral key and modifiers that the
/// TUI's crossterm edge gives the same press (design D6).
#[rstest]
#[case::plain_char(press("n"), KeyEvent::new(Key::Char('n'), KeyModifiers::NONE))]
#[case::space_is_a_char(press(" "), KeyEvent::new(Key::Char(' '), KeyModifiers::NONE))]
#[case::shift_uppercase_folds_to_lowercase(
    shifted(press("N")),
    KeyEvent::new(Key::Char('n'), KeyModifiers::SHIFT)
)]
#[case::bare_f_is_a_char_not_a_function_key(
    press("F"),
    KeyEvent::new(Key::Char('F'), KeyModifiers::NONE)
)]
#[case::ctrl_and_alt_are_both_kept(
    ctrl_alt(press("l")),
    KeyEvent::new(Key::Char('l'), KeyModifiers::CONTROL.union(KeyModifiers::ALT))
)]
#[case::plain_tab_stays_tab(press("Tab"), KeyEvent::new(Key::Tab, KeyModifiers::NONE))]
#[case::shift_tab_is_backtab_with_shift(
    shifted(press("Tab")),
    KeyEvent::new(Key::BackTab, KeyModifiers::SHIFT)
)]
#[case::shift_arrow_keeps_shift(
    shifted(press("ArrowLeft")),
    KeyEvent::new(Key::Left, KeyModifiers::SHIFT)
)]
#[case::escape_is_esc(press("Escape"), KeyEvent::new(Key::Esc, KeyModifiers::NONE))]
#[case::function_key(press("F5"), KeyEvent::new(Key::Function(5), KeyModifiers::NONE))]
#[case::meta_held_is_null(with_meta(press("l")), KeyEvent::new(Key::Null, KeyModifiers::NONE))]
// The `seek_back_10`/`seek_forward_30` default chords (add-tauri-gui-client
// row 6.4): the canvas transport's deep-step keys resolve through the
// single-character rule.
#[case::seek_back_10_bracket(press("["), KeyEvent::new(Key::Char('['), KeyModifiers::NONE))]
#[case::seek_forward_30_bracket(press("]"), KeyEvent::new(Key::Char(']'), KeyModifiers::NONE))]
// The `open_audio_popover`/`open_subtitle_popover` default chords
// (add-tauri-gui-client row 6.4): the track chips' popover keys resolve
// through the single-character rule.
#[case::open_audio_popover_a(press("a"), KeyEvent::new(Key::Char('a'), KeyModifiers::NONE))]
#[case::open_subtitle_popover_s(press("s"), KeyEvent::new(Key::Char('s'), KeyModifiers::NONE))]
fn dom_key_translates_to_neutral_key(#[case] dom: DomKey, #[case] expected: KeyEvent) {
    assert_eq!(neutral_key(dom), expected);
}
