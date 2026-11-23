//! The panel, presentation, and launch facts a Client changes.
//!
//! Every write here carries the decision the Client made from its own facts
//! (its terminal geometry, its composed frame, its keyboard layer) and keeps
//! the field, the clamp, and the ordering in the core.

use crate::app::App;
use crate::app::state::app_struct::LaunchRestore;
use mbv_ui_model::settings::{PanelFocus, PanelMode};

impl App {
    /// Whether prefix mode is armed. The keyboard layer owns the arm and
    /// disarm decisions; the core holds the bit the router reads.
    #[must_use]
    pub fn arm_prefix_mode(&mut self) -> bool {
        if self.prefix_armed {
            return false;
        }
        self.prefix_armed = true;
        true
    }

    /// Clear the armed prefix bit. Answers `false` when it was not armed, so
    /// the keyboard layer releases only the hold it took.
    pub fn disarm_prefix_mode(&mut self) -> bool {
        if !self.prefix_armed {
            return false;
        }
        self.prefix_armed = false;
        true
    }

    /// Return the panel focus to the Library after the displayed queue
    /// emptied while the focus rested on it (`hide-empty-queue-column` D2).
    ///
    /// The stored focus moves without the queue initial-item path
    /// `set_panel_focus` runs: the queue is about to be left, so a focus
    /// change to the Queue there would re-select a row the hidden column no
    /// longer shows.
    pub fn leave_emptied_queue_focus(&mut self) {
        self.panel_focus = PanelFocus::Library;
    }

    /// Focus the Queue after the terminal crossed into the mini view. The
    /// stored wide focus is untouched: only the mini view's own target moves.
    pub fn focus_queue_for_mini_view(&mut self) {
        self.mini_view_focus = PanelFocus::Queue;
    }

    /// Return every presentation field to its default for a UI-state reset
    /// (#745): the Panel mode and its startup focus, the queue column's
    /// default width, the persisted list-pane override (cleared here, saved by
    /// the Client that owns the prefs key), the visual slot, the visualizer,
    /// and the Settings destination.
    ///
    /// The queue column is held at [`LEFT_WIDTH_DEFAULT`](mbv_ui_model::layout::LEFT_WIDTH_DEFAULT)
    /// as the reset wrote it before this method existed; the Client's own
    /// terminal clamp runs after it.
    pub fn reset_presentation(&mut self) {
        self.panel_mode = PanelMode::default();
        self.panel_focus = PanelFocus::Library;
        self.mini_view_focus = PanelFocus::Queue;
        self.queue_column_width = mbv_ui_model::layout::LEFT_WIDTH_DEFAULT;
        self.list_pane_width = None;
        self.visual_slot_hidden = false;
        self.visualizer_enabled = false;
        self.settings_destination = mbv_ui_model::settings::SettingsDestination::Main;
    }

    /// Abandon the pending launch, navigation, landing, deep-selection, and
    /// overlay intent a UI-state reset discards (#745).
    ///
    /// Launch restoration is abandoned first, so a catalog arrival racing the
    /// reset cannot re-settle the snapshot (invariant
    /// `11-tab-changes-settle-launch-restoration`). The prefix bit and the
    /// confirm prompt go with it; the core focus stack's hold is the Client's
    /// to release, as it is the Client's to take.
    pub fn abandon_pending_ui_intents(&mut self) {
        self.launch_restore = LaunchRestore::Done;
        self.pending_navigate_tab_switch = None;
        self.pending_series_landing = None;
        self.pending_series_handoff = None;
        self.pending_track_selection = None;
        self.pending_queue_cursor_reanchor = None;
        self.pending_queue_surface_reset = false;
        self.pending_overlay = None;
        self.confirm_logout = false;
        self.prefix_armed = false;
    }

    /// Finish the launch restore a settled destination just consumed: the
    /// snapshot is spent, so it never re-applies.
    pub fn finish_launch_restore(&mut self) {
        self.launch_restore = LaunchRestore::Done;
    }

    /// Hold the given width as the queue column's width (the Client's
    /// terminal clamp supplied it). Nothing is normalized here: the column
    /// rules and the terminal width are Client facts.
    pub fn set_queue_column_width(&mut self, width: u16) {
        self.queue_column_width = width;
    }

    /// Hold the given normalized width as the Wide hero list-pane width. The
    /// content-area clamp is the presenting Client's arrangement fact, so the
    /// Client supplies the settled value.
    pub fn set_list_pane_width(&mut self, width: Option<u16>) {
        self.list_pane_width = width;
    }

    /// Hold the Wide TV series Workspace breakpoint of this pass: the one
    /// library index whose Workspace the Client paints wide, or `None` when
    /// none does. Terminal geometry is a Client fact, so the Client computes
    /// it; the core reads the write-back in the Series-activation branch both
    /// Clients reach (design D2).
    pub fn publish_wide_tv_workspace(&mut self, library: Option<usize>) {
        self.tv_workspace_wide.clear();
        if let Some(index) = library {
            self.tv_workspace_wide.insert(index);
        }
    }

    /// Hold the plain cache key the queue card paints when its title-overlay
    /// variant is not encoded yet (invariant 16).
    pub fn set_queue_card_plain_key(&mut self, key: String) {
        self.queue_card_projection.plain_cache_key = Some(key);
    }

    /// Hold the composed title-overlay variant's key as the one the queue card
    /// paints.
    pub fn set_queue_card_variant_key(&mut self, key: String) {
        self.queue_card_projection.cache_key = Some(key);
    }

    /// Return the queue card's title site to the header: the visual slot is
    /// hidden while playback continues, so the artwork can no longer carry the
    /// title (invariant 16).
    pub fn move_queue_card_title_to_header(&mut self) {
        self.queue_card_projection.title_site = mbv_ui_model::playback::NowPlayingTitleSite::Header;
    }

    /// Hold the terminal mouse-capture preference the Settings row toggled:
    /// the run loop applies it to the session once per tick.
    pub fn set_mouse_capture_pending(&mut self, capture: bool) {
        self.mouse_capture_pending = Some(capture);
    }

    /// Forget the custom accent colour held while Settings was open. A fresh
    /// Settings mount starts from the configured palette value (issue #875).
    pub fn clear_accent_custom(&mut self) {
        self.accent_custom = None;
    }

    /// Mark the next frame as painting from a cleared screen (invariant 17's
    /// one exception: a palette, a family, or a protocol change leaves no
    /// cell of the previous frame valid).
    pub fn request_force_clear(&mut self) {
        self.force_clear = true;
    }

    /// Record that the artwork entry stored under `key` now carries the logo
    /// `logo_key` in its built protocol. The painter built the protocol, so it
    /// supplies the logo it applied; an entry that is already gone is left
    /// alone.
    pub fn record_image_logo(&mut self, key: &str, logo_key: Option<String>) {
        if let Some(entry) = self.images.image_mut(key) {
            entry.applied_logo_key = logo_key;
        }
    }

    /// Record the hero box (cells) the artwork entry under `key` now carries in
    /// its built protocol, beside the logo it applied (task 5.10, design D5's
    /// cover fit).
    pub fn record_image_cover_box(
        &mut self,
        key: &str,
        cover_box: (u16, u16),
        logo_key: Option<String>,
    ) {
        if let Some(entry) = self.images.image_mut(key) {
            entry.cover_box = Some(cover_box);
            entry.applied_logo_key = logo_key;
        }
    }

    /// Consume the force-clear request: answers whether the run loop must
    /// clear the terminal before this frame, and drops the request so the
    /// next frame repaints incrementally again.
    #[must_use]
    pub fn take_force_clear(&mut self) -> bool {
        std::mem::take(&mut self.force_clear)
    }
}
