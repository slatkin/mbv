//! The focus-while-open hook: the one copy of the mounted-field focus effect
//! the Library's text fields share. A field that takes the keyboard while a
//! fact holds — the list's `/` filter, the collection dropdown's filter, the
//! Search field — attaches the returned handle to its `onmounted`, and the
//! effect reads the watched fact and writes no signal — the DOM focus is the
//! write.

use dioxus_native::prelude::*;

/// Hand the mounted field the keyboard while `open` holds, and take it back
/// when `open` retracts: the `use_effect` reads `open` (a closure over the
/// store's watched signal, so the effect re-runs when the fact changes) and
/// calls `set_focus` on the mounted field.
#[must_use]
pub fn use_focus_while(open: impl Fn() -> bool + 'static) -> Signal<Option<Event<MountedData>>> {
    let mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    use_effect(move || {
        let open = open();
        let mounted = mounted.cloned();
        spawn(async move {
            if let Some(event) = mounted {
                let _ = event.set_focus(open).await;
            }
        });
    });
    mounted
}
