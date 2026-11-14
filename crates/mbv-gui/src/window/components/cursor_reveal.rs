//! The Settings cursor reveal (`replace-tauri-with-blitz` task 8.3, the port
//! of the `scrollIntoView` effects of `SettingsScreen.svelte` and
//! `SettingsSidebar.svelte`; add-gui-settings-screen design D15's scroll
//! rule): the row that paints `data-settings-cursor` already knows it is the
//! cursor, so each such row calls this once per render with its own `cursor`
//! fact, attaches the returned handle with `onmounted`, and the effect keeps
//! the row in view while the keyboard walks the pane.
//!
//! The Svelte effects queried the pane for its cursor row; Blitz exposes no
//! descendant query, so the mounted row reveals itself. The effect reads the
//! cursor fact, the layout epoch (the `ResizeObserver` the Svelte effect
//! watched), and the mounted handle, and writes no signal it reads.

use dioxus_native::prelude::*;

use crate::window::layout::LayoutEpoch;

/// Hand one cursor-painting row its mounted handle and keep it in view: the
/// `use_effect` scrolls the row to `block: 'nearest', inline: 'nearest'`
/// whenever the cursor lands on it or the layout changes, as the Svelte
/// `row.scrollIntoView({ block: 'nearest' })` did. A test host provides no
/// [`LayoutEpoch`], and the reveal then follows the mount and the cursor
/// alone.
pub fn cursor_reveal(cursor: bool) -> Signal<Option<Event<MountedData>>> {
    let mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let epoch = try_consume_context::<LayoutEpoch>().map(|epoch| epoch.0);
    use_effect(use_reactive(&(cursor,), move |(cursor,)| {
        if let Some(epoch) = epoch {
            let _epoch = epoch.cloned();
        }
        if !cursor {
            return;
        }
        let mounted = mounted.cloned();
        spawn(async move {
            let Some(event) = mounted else {
                return;
            };
            let _ = event
                .scroll_to_with_options(ScrollToOptions {
                    behavior: ScrollBehavior::Instant,
                    vertical: ScrollLogicalPosition::Nearest,
                    horizontal: ScrollLogicalPosition::Nearest,
                })
                .await;
        });
    }));
    mounted
}
