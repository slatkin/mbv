//! Global "Reset UI State" coordinator (change
//! `separate-data-refresh-from-ui-reset`, row 5.1; issue #745).
//!
//! The F2 Settings action (row 5.2, not yet wired) calls
//! [`Model::reset_ui_state`]. The coordinator returns live presentation state
//! to its defaults without touching the protected domain-state inventory
//! (playback transport, queue contents/link, volume/mute, credentials,
//! Service setup, caches, launch-window acknowledgements), then clears saved
//! presentation state through `mbv_config::clear_saved_ui_presentation_state`
//! (row 4.1).
//!
//! Ordering is load-bearing: pending launch/UI intent is abandoned *before*
//! Home is selected through the settled-tab path, so a late catalog arrival
//! cannot re-settle the snapshot the reset discarded (invariant 11,
//! `docs/invariants/11-tab-changes-settle-launch-restoration.md`), and the
//! saved-state clear runs last, after every live write, so a later sync
//! cannot recreate what it cleared.

use super::Model;
use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use mbv_components::library_panel::LibraryPanel;
use mbv_components::queue::QueueComponent;
use mbv_render::arrangements::queue::normalize_queue_column_width;

use mbv_ui_msg::{ComponentId, PopupId};

impl Model {
    /// The shell's reader for the queue-column clamp (row 4.3 of design D2):
    /// the terminal width and the queue-column rules are both shell facts, so
    /// `normalize_queue_column_width` is applied here, not in the core.
    pub(in crate::app) fn clamp_queue_column_width(&mut self) -> bool {
        let normalized =
            normalize_queue_column_width(self.app.queue_column_width(), self.terminal_width);
        if normalized == self.app.queue_column_width() {
            return false;
        }
        self.app.set_queue_column_width(normalized);
        true
    }

    /// Reset the running TUI's live presentation and saved presentation
    /// state (#745). Implements the `tui-state-reset` capability: Home is
    /// selected, every retained destination owner (active and inactive) and
    /// the Queue's local presentation return to defaults, transient overlays
    /// are dismissed, and the saved launch/position/layout state is cleared
    /// last. The protected domain state is never touched.
    pub(crate) fn reset_ui_state(&mut self) {
        self.abandon_pending_ui_intents();
        // Existing settled-tab path. `set_library_tab(0)` also marks launch
        // restoration `Done` before it calls `select_tab`, so the pending
        // snapshot can no longer be wrapped into `TabSettled`.
        self.app.set_library_tab(0);
        self.restore_default_presentation_layout();
        self.app.reset_browse_root_defaults();
        self.dismiss_transient_overlays();
        self.reset_mounted_presentation();
        // Saved-state clearing runs last: every live/prefs write above has
        // already happened, so nothing recreates the cleared keys.
        self.persist_ui_state_reset();
    }

    /// Cancel pending launch, navigation, gesture and search intent before the
    /// default destination is selected. Launch restoration is abandoned
    /// first, so a catalog arrival racing the reset cannot re-settle it.
    fn abandon_pending_ui_intents(&mut self) {
        self.app.abandon_pending_ui_intents();
        // Prefix capture: the core's armed bit lifted with the intents above,
        // so the focus stack's hold lifts here, and the next focus step
        // re-derives the focus.
        self.focus_stack.release_focus();
        // Shell-owned transient UI intent (selection summaries, context menu,
        // cross-surface selection hand-offs, inline-search/track focus).
        self.visual_selection = None;
        self.context_menu_origin = None;
        self.context_action_snapshot = None;
        self.music_track_focus_request = None;
        self.pending_episode_selection = None;
        self.pending_music_track_selection = None;
        self.pending_music_reanchor = None;
    }

    /// Write the shell-owned list-pane width preference (design D4): the field
    /// stays on `App`, but its prefs key belongs to the TUI Client.
    pub(in crate::app) fn save_client_list_pane_width(&self) {
        App::save_client_pref(
            "list_pane_width",
            serde_json::json!(self.app.list_pane_width()),
        );
    }

    /// Restore the presentation layout and the geometry-specific startup
    /// focus rule: ordinary two-panel geometry focuses the Library; the
    /// geometry-forced Mini keeps its startup Queue focus.
    fn restore_default_presentation_layout(&mut self) {
        self.app.reset_presentation();
        self.clamp_queue_column_width();
        self.save_client_list_pane_width();
        self.app.stop_visualizer_capture();
        self.tab_scroll = 0;
    }

    /// Dismiss every transient overlay through the existing `TuiRealm` lifecycle
    /// helpers, so a pending draft, popup or search worker cannot re-open.
    fn dismiss_transient_overlays(&mut self) {
        self.dismiss_sidebars();
        self.umount_help();
        self.dismiss_blocking_modals();
        self.dismiss_feeds_manage();
        self.dismiss_modal(&ComponentId::Popup(PopupId::Multiselect));
        self.dismiss_modal(&ComponentId::Popup(PopupId::LibraryRoutes));
    }

    /// Reset the mounted presentation owners: the Library panel resets every
    /// retained owner (active and inactive) plus its own overlay/gesture
    /// state, and the Queue component resets its cursor/marks/scroll without
    /// changing the viewed link or queue data.
    fn reset_mounted_presentation(&mut self) {
        if let Some(panel) = self
            .application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
        {
            panel.reset_presentation();
        }
        if let Some(queue) = self
            .application
            .get_component_mut(&ComponentId::Queue)
            .and_then(|component| component.as_any_mut().downcast_mut::<QueueComponent>())
        {
            queue.reset_presentation();
        }
    }

    /// Clear saved presentation state (row 4.1). The live reset above stays
    /// applied even when clearing fails; only a fully successful clear claims
    /// success, and a partial failure is reported as an error toast.
    fn persist_ui_state_reset(&mut self) {
        match mbv_config::clear_saved_ui_presentation_state() {
            Ok(()) => self
                .app
                .flash("UI state reset".to_string(), ToastSeverity::Success),
            Err(error) => self.app.flash_error(format!(
                "saved UI state could not be fully cleared: {error}"
            )),
        }
    }
}

#[cfg(test)]
mod tests;
