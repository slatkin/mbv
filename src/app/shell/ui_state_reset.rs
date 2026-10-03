//! Global "Reset UI State" coordinator (change
//! `separate-data-refresh-from-ui-reset`, row 5.1; issue #745).
//!
//! The F2 Settings action (row 5.2, not yet wired) calls
//! [`Model::reset_ui_state`]. The coordinator returns live presentation state
//! to its defaults without touching the protected domain-state inventory
//! (playback transport, queue contents/scope, volume/mute, credentials,
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

use super::{Model, PanelFocus, PanelMode};
use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::app_struct::LaunchRestore;
use mbv_components::library_panel::LibraryPanel;
use mbv_components::queue::QueueComponent;
use mbv_queue::TvContentMode;
use mbv_render::layout::LEFT_WIDTH_DEFAULT;
use mbv_ui_model::browse::BrowseResting;
use mbv_ui_model::settings::SettingsDestination;
use mbv_ui_msg::{ComponentId, PopupId};

impl Model {
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
        self.app.launch_restore = LaunchRestore::Done;
        self.app.pending_navigate_tab_switch = None;
        self.app.pending_series_landing = None;
        self.app.pending_series_handoff = None;
        self.app.pending_track_selection = None;
        self.app.pending_queue_cursor_reanchor = None;
        self.app.pending_overlay = None;
        self.app.confirm_logout = false;
        // Prefix/Esc capture and the resolved-focus hand-off it displaced.
        self.app.prefix_armed = false;
        self.prefix_armed_focus = None;
        self.last_esc = None;
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

    /// Restore the presentation layout and the geometry-specific startup
    /// focus rule: ordinary two-panel geometry focuses the Library; the
    /// geometry-forced Mini keeps its startup Queue focus.
    fn restore_default_presentation_layout(&mut self) {
        self.app.panel_mode = PanelMode::default();
        self.app.panel_focus = PanelFocus::Library;
        self.app.mini_view_focus = PanelFocus::Queue;
        self.app.queue_column_width = LEFT_WIDTH_DEFAULT;
        self.app.clamp_queue_column_width();
        self.app.list_pane_width = None;
        self.app.visual_slot_hidden = false;
        self.app.visualizer_enabled = false;
        self.app.stop_visualizer_capture();
        self.app.tab_scroll = 0;
        self.app.settings_destination = SettingsDestination::Main;
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
    /// changing the viewed scope or queue data.
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

impl App {
    /// Return every Emby library's shell-owned browse stack to its root and
    /// its root level to default filter/sort/scope fields, retaining fetched
    /// content (no cache invalidation, no refetch). Deep levels are dropped
    /// because the destination's default context is its root.
    pub(in crate::app) fn reset_browse_root_defaults(&mut self) {
        for idx in 0..self.libs.len() {
            self.reset_library_browse_root(idx);
        }
    }

    fn reset_library_browse_root(&mut self, idx: usize) {
        if self.libs[idx].nav_stack.is_empty() {
            return;
        }
        let is_tv = self.libs[idx].library.collection_type == "tvshows";
        let (item_types, unplayed_only, sort_by, sort_order) =
            self.default_library_level_fields(idx);
        // The TV content mode is shell-pushed (`context.tv_content_mode`); a
        // TV reset must resolve the existing count-dependent default here, not
        // only in the owner. Small libraries default to `All`, large to
        // `Latest`.
        let tv_mode = is_tv.then(|| {
            mbv_ui_model::sort_filter::resolve_tv_content_mode(
                self.libs[idx].library_total.unwrap_or_default(),
                None,
            )
        });
        let root_item_types = if is_tv {
            Some(
                match tv_mode {
                    Some(TvContentMode::Latest | TvContentMode::Upcoming) => "Episode",
                    _ => "Series",
                }
                .to_string(),
            )
        } else {
            item_types
        };
        let lib = &mut self.libs[idx];
        lib.nav_stack.truncate(1);
        if let Some(root) = lib.nav_stack.first_mut() {
            root.resting = BrowseResting::new(0, 0);
            root.item_types = root_item_types;
            root.unplayed_only = unplayed_only;
            root.sort_by = sort_by;
            root.sort_order = sort_order;
            root.letter_filter = None;
            root.tv_content_mode.clone_from(&tv_mode);
        }
        lib.tv_content_mode = tv_mode;
        // A feed-view library's root scope is its first group.
        if let Some(state) = lib.feed_home_video.as_mut() {
            state.selected_group = 0;
            state.video_cursor = 0;
            state.video_scroll = 0;
        }
    }
}

#[cfg(test)]
mod tests;
