//! The TUI input edge: terminal input reaches the Keyboard Router here, in the
//! shell, so the router only ever reads neutral events (`extract-client-core`
//! design D4). `mbv-components` owns the `TuiRealm` to neutral translation
//! (`root::neutral_key`); this module is the shell's one seam onto it, so a
//! core module never names a toolkit type.
//!
//! The router and its policy match [`KeyChord`]; see `mbv_client_core::input::router`.

use mbv_client_core::input::resolver::KeyChord;
use mbv_client_core::input::router::{RouterNow, RouterOutcome, RouterSnapshot};
use mbv_keybinds::Keybinds;
use mbv_ui_msg::ComponentId;

/// The TUI's one routing entry: translate the terminal key into the neutral
/// chord, then hand the chord to the router. The shell's callers reach the
/// router only through here, so the translation and the resolution stay one
/// step and the router sees a neutral chord alone.
pub(in crate::tui) fn resolve_tui_key(
    key: mbv_ui_msg::KeyEvent,
    snapshot: &RouterSnapshot,
    now: &dyn RouterNow,
    focused: Option<&ComponentId>,
    keybinds: &Keybinds,
) -> RouterOutcome {
    mbv_client_core::input::router::resolve_router_outcome(
        KeyChord::from_key(key),
        snapshot,
        now,
        focused,
        keybinds,
    )
}

/// The TUI edge translation itself, from the toolkit key the listener
/// delivers. `mbv-components` maps the toolkit event onto the neutral
/// vocabulary; the chord normalizations (`BackTab`'s redundant SHIFT,
/// SHIFT-case folding) then apply in [`resolve_tui_key`]. Test-only: the shell
/// tests build the neutral key the same way production's edge does, so a test
/// never carries a toolkit key into the router.
#[cfg(test)]
pub(in crate::tui) fn translate_key(key: tuirealm::event::KeyEvent) -> mbv_ui_msg::KeyEvent {
    mbv_components::root::neutral_key(key)
}
