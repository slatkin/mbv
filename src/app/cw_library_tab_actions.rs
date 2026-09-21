use super::{App, PanelFocus, TabSelection};
use mbv_core::api::EmbyItem;
use mbv_core::config::{ServiceKind, TabIdentity};

impl App {
    /// Resolve the one startup launch intent against the current live
    /// Service catalogs. Stable IDs are matched only after their owning
    /// catalog has arrived; no saved presentation index is consulted.
    ///
    /// The tab identity is consumed here, while the complete snapshot stays
    /// pending for destination-level restoration (task 3.2). Explicit tab
    /// movement clears both levels in `apply_tab_position`.
    pub(super) fn resolve_library_tab_pending(&mut self) {
        // Keep the pre-launch-state numeric fallback alive for the existing
        // migration seam; task 4.2 will replace it with stable legacy
        // identity recovery. New snapshots always take the branch below.
        if self.pending_launch_tab.is_none()
            && self.library_tab_pending > 0
            && (!self.libs.is_empty() || !self.audiobookshelf_libraries.is_empty())
        {
            let fp = self.feeds_tab_pos();
            let emby = self.libs.len();
            let audio = self.audiobookshelf_libraries.len();
            let max_pos = fp.unwrap_or(emby + audio);
            let pos = self.library_tab_pending.min(max_pos);
            self.tab = TabSelection::from_position_with_counts(pos, emby, audio, fp.is_some());
            self.library_tab_pending = 0;
            return;
        }
        let Some(identity) = self.pending_launch_tab.clone() else {
            return;
        };
        let resolved = match identity {
            TabIdentity::Home => Some(TabSelection::Home),
            TabIdentity::Feeds => {
                if self.has_feeds_subscriptions() {
                    Some(TabSelection::Feeds)
                } else {
                    Some(TabSelection::Home)
                }
            }
            TabIdentity::ServiceLibrary { kind, library_id } => {
                self.resolve_service_tab(kind, &library_id)
            }
        };
        if let Some(tab) = resolved {
            self.tab = tab;
            self.pending_launch_tab = None;
        }
    }

    fn resolve_service_tab(&self, kind: ServiceKind, library_id: &str) -> Option<TabSelection> {
        match kind {
            ServiceKind::Emby => {
                if !self.emby_catalog_ready {
                    return None;
                }
                Some(
                    self.libs
                        .iter()
                        .position(|library| library.library.id == library_id)
                        .map(TabSelection::EmbyLibrary)
                        .unwrap_or(TabSelection::Home),
                )
            }
            ServiceKind::Audiobookshelf => {
                if !self.audiobookshelf_catalog_ready {
                    return None;
                }
                Some(
                    self.audiobookshelf_libraries
                        .iter()
                        .position(|library| library.id == library_id)
                        .map(TabSelection::AudiobookshelfLibrary)
                        .unwrap_or(TabSelection::Home),
                )
            }
        }
    }

    /// Normalizes a selected Service library index that no longer exists.
    ///
    /// A `TabSelection::EmbyLibrary(index)` with `index >= self.libs.len()`,
    /// or a `TabSelection::AudiobookshelfLibrary(index)` with `index >=
    /// self.audiobookshelf_libraries.len()`, selects Home and returns
    /// `true`: the caller must stop the triggering destination-specific
    /// operation (no further destination mutation). Any other tab is left
    /// unchanged and returns `false`. This owns asynchronous Service
    /// removal/replacement invalidation; downstream Service helpers may
    /// still bounds-check defensively, but never choose another destination.
    pub(super) fn normalize_stale_browse_destination(&mut self) -> bool {
        if let Some(index) = self.tab.emby_library_index() {
            if index >= self.libs.len() {
                self.tab = TabSelection::Home;
                return true;
            }
        }
        if let Some(index) = self.tab.audiobookshelf_index() {
            if index >= self.audiobookshelf_libraries.len() {
                self.tab = TabSelection::Home;
                return true;
            }
        }
        false
    }

    /// Move to left-panel tab `pos` and settle all state that follows from a
    /// tab change (panel focus, stale image dims, library activation).
    fn apply_tab_position(&mut self, pos: usize) {
        // Any tab change abandons the deferred cross-surface navigations (U2
        // correction): the user moved on, so a later drain must never yank
        // the tab to the navigated library. A landing's own switch happens
        // after it consumed its pending state, so it is unaffected.
        self.pending_navigate_tab_switch = None;
        self.pending_series_landing = None;
        self.pending_series_handoff = None;
        // A user-selected tab owns the rest of the launch intent. Once the
        // user moves explicitly, a later catalog refresh must not replay the
        // saved tab or its destination identities.
        self.pending_launch_tab = None;
        self.pending_launch_state = None;
        self.tab = TabSelection::from_position_with_counts(
            pos,
            self.libs.len(),
            self.audiobookshelf_libraries.len(),
            self.has_feeds_subscriptions(),
        );
        // A stale Service library index (libraries removed or replaced since
        // `pos` was computed) becomes Home; the pending selection stops
        // without focus, activation, or preference changes.
        if self.normalize_stale_browse_destination() {
            return;
        }
        self.last_card_height = 0; // reset stale image height for new view
        self.last_card_width = 0;
        match self.tab {
            TabSelection::Home => {}
            TabSelection::EmbyLibrary(lib_idx) => {
                self.set_panel_focus(PanelFocus::Library);
                self.activate_library_position(lib_idx);
            }
            TabSelection::AudiobookshelfLibrary(index) => {
                self.set_panel_focus(PanelFocus::Library);
                self.activate_audiobookshelf_position(index);
                self.activate_audiobookshelf_book_position(index);
            }
            TabSelection::Feeds => {
                self.set_panel_focus(PanelFocus::Library);
            }
        }
        self.ensure_tab_visible();
        self.save_prefs();
    }

    /// Jump directly to left-panel tab `idx` (0 = Home, 1..=libs.len() =
    /// library index `idx - 1`, or Feeds at the end when present).
    pub(super) fn set_library_tab(&mut self, idx: usize) {
        if idx >= self.tab_count() {
            return;
        }
        self.apply_tab_position(idx);
    }

    /// Advance the left-panel tab (wrapping); load the library if needed.
    pub(super) fn library_tab_next(&mut self) {
        let n = self.tab_count();
        let pos = self
            .tab
            .to_position_with_counts(self.libs.len(), self.feeds_tab_pos());
        let new_pos = (pos + 1) % n;
        self.apply_tab_position(new_pos);
    }

    /// Retreat the left-panel tab (wrapping); load the library if needed.
    pub(super) fn library_tab_prev(&mut self) {
        let n = self.tab_count();
        let pos = self
            .tab
            .to_position_with_counts(self.libs.len(), self.feeds_tab_pos());
        let new_pos = (pos + n - 1) % n;
        self.apply_tab_position(new_pos);
    }

    // The Continue Watching column shares state with the Home tab's
    // Continue Watching section, so these act on the column's own
    // `continue_cursor` item directly (task 5.3d, Home effect decoupling):
    // the shell resolves the item under `continue_cursor` from
    // Model-owned `home_content` and passes it into the item-targeted
    // effect helper, instead of the App re-reading a (now deleted)
    // `home.continue_items`/`continue_cursor`. `continue_cursor` stays the
    // sole, unchanged authoritative target.
    pub(super) fn cw_play(&mut self, item: EmbyItem) {
        if item.is_folder {
            return;
        }
        self.play_home_cw_item(item);
    }

    pub(super) fn cw_enqueue(&mut self, item: EmbyItem) {
        self.enqueue_home_item(item);
    }

    pub(super) fn cw_toggle_watched(&mut self, item: EmbyItem) {
        self.toggle_watched_home_item(item);
    }
}
