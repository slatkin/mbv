use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;
use tuirealm::subscription::{EventClause, Sub, SubClause};

use mbv_ui_msg::{
    ComponentId, Key as NeutralKey, KeyEvent as NeutralKeyEvent, KeyModifiers as NeutralMods,
    ModalId, Msg, OverlayId, PointerEvent, PointerKind, PopupId, Position, TerminalObserverEvent,
    UserEvent,
};

/// Convert a `TuiRealm` keyboard event to the neutral `KeyEvent`
/// (`add-gui-front-end` design D4): the router resolves chords only from the
/// neutral shape, and the TUI adapter owns the translation. Terminal keys the
/// neutral vocabulary cannot represent become `Key::Null`, which matches no
/// chord, exactly as the unmapped codes did before the conversion existed.
///
/// The modifier lowercase rule: when SHIFT is held, an uppercase ASCII
/// character is reported as its lowercase letter with SHIFT kept. Crossterm
/// delivers Shift+n as `Char('N')` + SHIFT, while a GUI toolkit reports the
/// caseless key with the modifier set; folding the case into the modifier
/// gives both Clients one chord for one physical press. The resolver applies
/// the same rule to configured chords, which the keybind registry stores in
/// crossterm-case (`Shift+N`).
#[must_use]
pub fn neutral_key(key: tuirealm::event::KeyEvent) -> NeutralKeyEvent {
    use tuirealm::event::Key as TuiKey;
    const KEY_CODES: &[(TuiKey, NeutralKey)] = &[
        (TuiKey::Backspace, NeutralKey::Backspace),
        (TuiKey::Enter, NeutralKey::Enter),
        (TuiKey::Left, NeutralKey::Left),
        (TuiKey::Right, NeutralKey::Right),
        (TuiKey::Up, NeutralKey::Up),
        (TuiKey::Down, NeutralKey::Down),
        (TuiKey::Home, NeutralKey::Home),
        (TuiKey::End, NeutralKey::End),
        (TuiKey::PageUp, NeutralKey::PageUp),
        (TuiKey::PageDown, NeutralKey::PageDown),
        (TuiKey::Tab, NeutralKey::Tab),
        (TuiKey::BackTab, NeutralKey::BackTab),
        (TuiKey::Delete, NeutralKey::Delete),
        (TuiKey::Insert, NeutralKey::Insert),
        (TuiKey::Esc, NeutralKey::Esc),
        (TuiKey::Null, NeutralKey::Null),
    ];
    let code = match key.code {
        TuiKey::Char(c) => NeutralKey::Char(c),
        TuiKey::Function(n) => NeutralKey::Function(n),
        code => KEY_CODES
            .iter()
            .find_map(|(tui_code, mapped_code)| (*tui_code == code).then_some(*mapped_code))
            .unwrap_or(NeutralKey::Null),
    };
    let mut modifiers = NeutralMods::NONE;
    if key.modifiers.contains(tuirealm::event::KeyModifiers::SHIFT) {
        modifiers = modifiers.union(NeutralMods::SHIFT);
    }
    if key
        .modifiers
        .contains(tuirealm::event::KeyModifiers::CONTROL)
    {
        modifiers = modifiers.union(NeutralMods::CONTROL);
    }
    if key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) {
        modifiers = modifiers.union(NeutralMods::ALT);
    }
    let code = match code {
        NeutralKey::Char(c) if modifiers.contains(NeutralMods::SHIFT) && c.is_ascii_uppercase() => {
            NeutralKey::Char(c.to_ascii_lowercase())
        }
        code => code,
    };
    NeutralKeyEvent::new(code, modifiers)
}

/// Convert a `TuiRealm` mouse event to the neutral `PointerEvent`
/// (`add-gui-front-end` design D4). The terminal reports cells, so a cell is
/// one logical unit: `column`/`row` map onto `x`/`y` unchanged. A mouse kind
/// the neutral vocabulary cannot represent (other buttons, drags, horizontal
/// wheel) yields `None`, which routes to no target, exactly as the events the
/// components' kind filters already drop.
#[must_use]
pub fn neutral_pointer(mouse: tuirealm::event::MouseEvent) -> Option<PointerEvent> {
    use tuirealm::event::MouseButton as TuiMouseButton;
    use tuirealm::event::MouseEventKind as TuiMouseKind;
    let kind = match mouse.kind {
        TuiMouseKind::Down(TuiMouseButton::Left) => PointerKind::Press,
        TuiMouseKind::Up(TuiMouseButton::Left) => PointerKind::Release,
        TuiMouseKind::Moved => PointerKind::Move,
        TuiMouseKind::ScrollDown => PointerKind::ScrollDown,
        TuiMouseKind::ScrollUp => PointerKind::ScrollUp,
        _ => return None,
    };
    Some(PointerEvent::new(
        kind,
        Position {
            x: f32::from(mouse.column),
            y: f32::from(mouse.row),
        },
    ))
}

const OVERLAY_IDS: &[ComponentId] = &[
    ComponentId::Overlay(OverlayId::Settings),
    ComponentId::Overlay(OverlayId::Playlists),
    ComponentId::Modal(ModalId::SavePlaylist),
    ComponentId::Modal(ModalId::TwitchUrl),
    ComponentId::Overlay(OverlayId::Help),
    ComponentId::Modal(ModalId::Confirm),
    ComponentId::Modal(ModalId::DaemonLost),
    ComponentId::Overlay(OverlayId::ContextMenu),
    ComponentId::Popup(PopupId::Multiselect),
    ComponentId::Popup(PopupId::LibraryRoutes),
    ComponentId::Popup(PopupId::FeedManage),
    ComponentId::Overlay(OverlayId::Search),
    ComponentId::Overlay(OverlayId::Sessions),
];

/// Root routing owns overlay z-order from a fixed canonical mount order;
/// `TuiRealm` owns focus and its LIFO stack.
#[derive(Debug)]
pub struct UiRootComponent;

impl Default for UiRootComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl UiRootComponent {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Subscribe the root to every terminal event so the shell can distinguish
    /// a processed event from an empty component message. The root remains a
    /// permanent observer even while another component owns focus.
    #[must_use]
    pub fn subscriptions() -> Vec<Sub<ComponentId, UserEvent>> {
        vec![Sub::new(EventClause::Any, SubClause::Always)]
    }

    #[must_use]
    pub fn overlay_ids() -> &'static [ComponentId] {
        OVERLAY_IDS
    }
}

impl Component for UiRootComponent {
    fn view(&mut self, _frame: &mut Frame, _area: Rect) {}

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

impl AppComponent<Msg, UserEvent> for UiRootComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        let observed = match ev {
            Event::Keyboard(key) => TerminalObserverEvent::Key(neutral_key(*key)),
            Event::WindowResize(width, height) => TerminalObserverEvent::Resize {
                width: *width,
                height: *height,
            },
            Event::FocusGained => TerminalObserverEvent::FocusGained,
            Event::FocusLost => TerminalObserverEvent::FocusLost,
            // Mouse events are otherwise delivered to components through
            // `mouse_sub()` subscriptions (ADR 0024); the observer only needs
            // them as a redraw signal, same as the other non-chord events. A
            // left-click press is distinguished from the other mouse kinds
            // because it used to be the shell-resolved tab-bar click (task
            // 6.5); the tab bar is a mounted `TabPanel` now (task 2.1), so a
            // click is only the redraw echo and no shell geometry is read.
            // The other mouse kinds get their own marker so the shell can
            // silently disarm prefix mode on any mouse event (design D6,
            // task 6.1) without mistaking them for the `NoOp` redraw echo.
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                TerminalObserverEvent::MouseClick {
                    column: mouse.column,
                    row: mouse.row,
                }
            }
            Event::Mouse(_) => TerminalObserverEvent::Mouse,
            Event::None | Event::Paste(_) | Event::Tick | Event::User(_) => {
                TerminalObserverEvent::NoOp
            }
        };
        Some(Msg::TerminalEvent(observed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use tuirealm::event::{Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    /// One table over the listener-edge mapping (add-gui-front-end task 2.1):
    /// the crossterm-delivered key and mouse events the `UiRoot` observer
    /// sees, and the neutral event each must become. The key rows pin the
    /// modifier lowercase rule; the pointer rows pin the cell-to-logical-unit
    /// position and the kind fold.
    #[derive(Debug, PartialEq)]
    enum Raw {
        Key(KeyEvent),
        Mouse(MouseEvent),
    }

    #[derive(Debug, PartialEq)]
    enum Mapped {
        Key(NeutralKeyEvent),
        Pointer(Option<PointerEvent>),
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn pointer(kind: PointerKind, x: f32, y: f32) -> Mapped {
        Mapped::Pointer(Some(PointerEvent::new(kind, Position { x, y })))
    }

    #[rstest]
    #[case::plain_char(
        Raw::Key(KeyEvent { code: Key::Char('n'), modifiers: KeyModifiers::NONE }),
        Mapped::Key(NeutralKeyEvent::new(NeutralKey::Char('n'), NeutralMods::NONE))
    )]
    #[case::shift_uppercase_char_lowercases(
        // The modifier lowercase rule: crossterm delivers Shift+n as
        // `Char('N')` + SHIFT; the neutral chord reports the lowercase
        // letter with SHIFT kept, so a GUI toolkit's caseless key plus
        // SHIFT denotes the same chord.
        Raw::Key(KeyEvent { code: Key::Char('N'), modifiers: KeyModifiers::SHIFT }),
        Mapped::Key(NeutralKeyEvent::new(NeutralKey::Char('n'), NeutralMods::SHIFT))
    )]
    #[case::ctrl_letter_arrives_lowercase(
        Raw::Key(KeyEvent { code: Key::Char('l'), modifiers: KeyModifiers::CONTROL }),
        Mapped::Key(NeutralKeyEvent::new(NeutralKey::Char('l'), NeutralMods::CONTROL))
    )]
    #[case::shift_non_char_keeps_shift(
        Raw::Key(KeyEvent { code: Key::Left, modifiers: KeyModifiers::SHIFT }),
        Mapped::Key(NeutralKeyEvent::new(NeutralKey::Left, NeutralMods::SHIFT))
    )]
    #[case::backtab_keeps_shift(
        Raw::Key(KeyEvent { code: Key::BackTab, modifiers: KeyModifiers::SHIFT }),
        Mapped::Key(NeutralKeyEvent::new(NeutralKey::BackTab, NeutralMods::SHIFT))
    )]
    #[case::function_key(
        Raw::Key(KeyEvent { code: Key::Function(5), modifiers: KeyModifiers::NONE }),
        Mapped::Key(NeutralKeyEvent::new(NeutralKey::Function(5), NeutralMods::NONE))
    )]
    #[case::unrepresentable_key_maps_to_null(
        Raw::Key(KeyEvent { code: Key::CapsLock, modifiers: KeyModifiers::NONE }),
        Mapped::Key(NeutralKeyEvent::new(NeutralKey::Null, NeutralMods::NONE))
    )]
    #[case::left_press(
        Raw::Mouse(mouse(MouseEventKind::Down(MouseButton::Left), 3, 7)),
        pointer(PointerKind::Press, 3.0, 7.0)
    )]
    #[case::left_release(
        Raw::Mouse(mouse(MouseEventKind::Up(MouseButton::Left), 3, 7)),
        pointer(PointerKind::Release, 3.0, 7.0)
    )]
    #[case::pointer_move(
        Raw::Mouse(mouse(MouseEventKind::Moved, 12, 4)),
        pointer(PointerKind::Move, 12.0, 4.0)
    )]
    #[case::wheel_down(
        Raw::Mouse(mouse(MouseEventKind::ScrollDown, 0, 0)),
        pointer(PointerKind::ScrollDown, 0.0, 0.0)
    )]
    #[case::wheel_up(
        Raw::Mouse(mouse(MouseEventKind::ScrollUp, 9, 2)),
        pointer(PointerKind::ScrollUp, 9.0, 2.0)
    )]
    #[case::other_button_has_no_neutral_kind(
        Raw::Mouse(mouse(MouseEventKind::Down(MouseButton::Right), 1, 1)),
        Mapped::Pointer(None)
    )]
    #[case::drag_has_no_neutral_kind(
        Raw::Mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 1, 1)),
        Mapped::Pointer(None)
    )]
    fn terminal_events_translate_to_the_neutral_vocabulary(
        #[case] raw: Raw,
        #[case] expected: Mapped,
    ) {
        let translated = match raw {
            Raw::Key(key) => Mapped::Key(neutral_key(key)),
            Raw::Mouse(mouse) => Mapped::Pointer(neutral_pointer(mouse)),
        };
        assert_eq!(translated, expected);
    }

    #[test]
    fn root_observer_marks_none_returning_local_key_as_processed() {
        let event = Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        });
        // Any component whose local handler leaves the key unclaimed (here the
        // status panel, which has no keyboard interpretation) must still be
        // observed as processed by the root's terminal fold.
        let mut panel = crate::status_bar_panel::StatusBarPanel::new();
        assert!(panel.on(&event).is_none());

        let mut root = UiRootComponent::new();
        assert!(matches!(
            root.on(&event),
            Some(Msg::TerminalEvent(TerminalObserverEvent::Key(_)))
        ));
    }
}
