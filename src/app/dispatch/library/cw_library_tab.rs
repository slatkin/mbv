use crate::app::state::app_struct::LaunchRestore;
use crate::app::{App, PanelFocus, TabSelection};
use mbv_config::TabIdentity;
use mbv_emby_model::EmbyItem;
use mbv_queue::ServiceKind;

impl App {
    /// Resolve the saved Home or Feeds identity on the shell sync pass.
    pub(in crate::app) fn resolve_library_tab_pending(&mut self) {
        let LaunchRestore::Pending(state) = &self.launch_restore else {
            return;
        };
        let resolved = match &state.tab {
            TabIdentity::Home => Some(TabSelection::Home),
            TabIdentity::Feeds => {
                if self.has_feeds_subscriptions() {
                    Some(TabSelection::Feeds)
                } else {
                    Some(TabSelection::Home)
                }
            }
            TabIdentity::ServiceLibrary { .. } => None,
        };
        if let Some(tab) = resolved {
            // The snapshot names a tab, not a browse position, so the
            // resolved tab must be activated exactly like a user tab switch:
            // otherwise the panel is handed an empty owner and the restored
            // tab paints blank. The destination re-anchor then applies the
            // snapshot's pill and item over this loaded root.
            self.select_tab(tab);
        }
    }

    /// Resolve a saved Service identity only after its catalog has been built.
    pub(in crate::app) fn resolve_launch_service_tab(&mut self, kind: ServiceKind) {
        let tab = match &self.launch_restore {
            LaunchRestore::Pending(state) => match &state.tab {
                TabIdentity::ServiceLibrary {
                    kind: saved_kind,
                    library_id,
                } if *saved_kind == kind => Some(match kind {
                    ServiceKind::Emby => self
                        .libs
                        .iter()
                        .position(|library| library.library.id == *library_id)
                        .map_or(TabSelection::Home, TabSelection::EmbyLibrary),
                    ServiceKind::Audiobookshelf => self
                        .audiobookshelf_libraries
                        .iter()
                        .position(|library| library.id == *library_id)
                        .map_or(TabSelection::Home, TabSelection::AudiobookshelfLibrary),
                }),
                _ => None,
            },
            _ => None,
        };
        if let Some(tab) = tab {
            self.select_tab(tab);
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
    pub(in crate::app) fn normalize_stale_browse_destination(&mut self) -> bool {
        if let Some(index) = self.tab.emby_library_index()
            && index >= self.libs.len()
        {
            self.tab = TabSelection::Home;
            return true;
        }
        if let Some(index) = self.tab.audiobookshelf_index()
            && index >= self.audiobookshelf_libraries.len()
        {
            self.tab = TabSelection::Home;
            return true;
        }
        false
    }

    /// Select and settle a tab, binding a pending launch snapshot to the
    /// resulting tab after stale-index normalization.
    fn select_tab(&mut self, tab: TabSelection) {
        self.tab = tab;
        self.settle_tab_selection();
        if let LaunchRestore::Pending(state) =
            std::mem::replace(&mut self.launch_restore, LaunchRestore::Done)
        {
            self.launch_restore = LaunchRestore::TabSettled {
                state,
                tab: self.tab,
            };
        }
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
        // An explicit move abandons the remaining launch intent.
        self.launch_restore = LaunchRestore::Done;
        self.select_tab(TabSelection::from_position_with_counts(
            pos,
            self.libs.len(),
            self.audiobookshelf_libraries.len(),
            self.has_feeds_subscriptions(),
        ));
    }

    /// Settle all state that follows from `self.tab` being set: stale
    /// destination fallback, image dims, panel focus, the selected
    /// destination's content load, tab-bar visibility, and prefs. Shared by
    /// user tab movement and launch-state tab restoration — the restore path
    /// must load the restored library's content, not just select its tab.
    fn settle_tab_selection(&mut self) {
        // A stale Service library index (libraries removed or replaced since
        // `pos` was computed) becomes Home; the pending selection stops
        // without focus, activation, or preference changes.
        if self.normalize_stale_browse_destination() {
            return;
        }
        self.images.record_card_size(0, 0); // reset stale image size for new view
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

    /// Jump directly to left-panel tab `idx` (0 = Home, `1..=libs.len()` =
    /// library index `idx - 1`, or Feeds at the end when present).
    pub(in crate::app) fn set_library_tab(&mut self, idx: usize) {
        if idx >= self.tab_count() {
            return;
        }
        self.apply_tab_position(idx);
    }

    /// Advance the left-panel tab (wrapping); load the library if needed.
    pub(in crate::app) fn library_tab_next(&mut self) {
        let n = self.tab_count();
        let pos = self
            .tab
            .to_position_with_counts(self.libs.len(), self.feeds_tab_pos());
        let new_pos = (pos + 1) % n;
        self.apply_tab_position(new_pos);
    }

    /// Retreat the left-panel tab (wrapping); load the library if needed.
    pub(in crate::app) fn library_tab_prev(&mut self) {
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
    pub(in crate::app) fn cw_play(&mut self, item: EmbyItem) {
        if item.is_folder {
            return;
        }
        self.play_home_cw_item(item);
    }

    pub(in crate::app) fn cw_enqueue(&mut self, item: EmbyItem) {
        self.enqueue_home_item(item);
    }

    pub(in crate::app) fn cw_toggle_watched(&mut self, item: &EmbyItem) {
        self.toggle_watched_home_item(item);
    }
}
