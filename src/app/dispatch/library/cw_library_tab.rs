use crate::app::{App, PanelFocus, TabSelection};
use mbv_core::api::EmbyItem;
use mbv_core::config::{
    AudiobookshelfBookBucket, AudiobookshelfSelectorKey, EmbyLetterBucket, EmbySelectorKey,
    LaunchPanelFocus, LibraryItemIdentity, SelectorIdentity, ServiceKind, TabIdentity,
    TuiLaunchState, TUI_LAUNCH_STATE_VERSION,
};

impl App {
    /// Resolve the one startup launch intent against the current live
    /// Service catalogs. Stable IDs are matched only after their owning
    /// catalog has arrived; no saved presentation index is consulted.
    ///
    /// The tab identity is consumed here, while the complete snapshot stays
    /// pending for destination-level restoration (task 3.2). Explicit tab
    /// movement clears both levels in `apply_tab_position`.
    pub(in crate::app) fn resolve_library_tab_pending(&mut self) {
        self.migrate_legacy_launch_state();
        // Keep the pre-launch-state numeric fallback alive for the existing
        // test seam. A legacy on-disk preference is converted above before
        // this path runs; new snapshots always take the branch below.
        if self.pending_launch_state.is_none()
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
        let Some(state) = self.pending_launch_state.as_ref() else {
            return;
        };
        if self.pending_launch_tab_resolved {
            return;
        }
        let resolved = match &state.tab {
            TabIdentity::Home => Some(TabSelection::Home),
            TabIdentity::Feeds => {
                if self.has_feeds_subscriptions() {
                    Some(TabSelection::Feeds)
                } else {
                    Some(TabSelection::Home)
                }
            }
            TabIdentity::ServiceLibrary { kind, library_id } => {
                self.resolve_service_tab(*kind, library_id)
            }
        };
        if let Some(tab) = resolved {
            self.tab = tab;
            self.pending_launch_tab_resolved = true;
            // The snapshot names a tab, not a browse position, so the
            // resolved tab must be activated exactly like a user tab switch:
            // otherwise the panel is handed an empty owner and the restored
            // tab paints blank. The destination re-anchor then applies the
            // snapshot's pill and item over this loaded root.
            self.settle_tab_selection();
        }
    }

    /// Derive one bounded launch snapshot from the old selected-tab and
    /// browse-position files. The old tab position is used only to identify
    /// the currently selected destination while its catalog is available; it
    /// is never copied into the new snapshot. Browse cursors are deliberately
    /// ignored: only stable focused-item IDs and fixed letter buckets are
    /// recoverable identities.
    fn migrate_legacy_launch_state(&mut self) {
        if self.pending_launch_state.is_some() || self.legacy_launch_migration_attempted {
            return;
        }
        let Some(position) = self.legacy_launch_tab else {
            self.legacy_launch_migration_attempted = true;
            return;
        };
        let Some(tab) = self.legacy_tab_identity(position) else {
            // The selected Service catalog has not arrived yet. Keep the
            // legacy input armed, but do not derive from a partial catalog.
            return;
        };
        self.legacy_launch_migration_attempted = true;

        let (selector, item) = match tab {
            TabSelection::EmbyLibrary(index) => self
                .libs
                .get(index)
                .and_then(|library| self.legacy_position_for_key(&library.library.id))
                .map_or((None, None), |position| {
                    Self::legacy_identities(&position, false)
                }),
            TabSelection::AudiobookshelfLibrary(index) => self
                .audiobookshelf_position_key(index)
                .and_then(|key| self.legacy_position_for_key(key.as_str()))
                .map_or((None, None), |position| {
                    Self::legacy_identities(&position, true)
                }),
            TabSelection::Home | TabSelection::Feeds => (None, None),
        };
        let tab = match tab {
            TabSelection::Home => TabIdentity::Home,
            TabSelection::Feeds => TabIdentity::Feeds,
            TabSelection::EmbyLibrary(index) => TabIdentity::ServiceLibrary {
                kind: ServiceKind::Emby,
                library_id: self.libs[index].library.id.clone(),
            },
            TabSelection::AudiobookshelfLibrary(index) => TabIdentity::ServiceLibrary {
                kind: ServiceKind::Audiobookshelf,
                library_id: self.audiobookshelf_libraries[index].id.clone(),
            },
        };
        self.pending_launch_state = Some(TuiLaunchState {
            version: TUI_LAUNCH_STATE_VERSION,
            tab,
            panel_focus: match self.panel_focus {
                PanelFocus::Library => LaunchPanelFocus::Library,
                PanelFocus::Queue => LaunchPanelFocus::Queue,
            },
            selector,
            item,
        });
        self.pending_launch_tab_resolved = false;
    }

    fn legacy_tab_identity(&self, position: usize) -> Option<TabSelection> {
        if position == 0 {
            return Some(TabSelection::Home);
        }
        let emby_configured = self.config.lock().unwrap().emby_setup.is_some();
        if emby_configured && !self.emby_catalog_ready {
            return None;
        }
        if position <= self.libs.len() {
            return Some(TabSelection::EmbyLibrary(position - 1));
        }
        let audiobookshelf_configured = self.config.lock().unwrap().audiobookshelf_setup.is_some();
        if audiobookshelf_configured && !self.audiobookshelf_catalog_ready {
            return None;
        }
        if position <= self.libs.len() + self.audiobookshelf_libraries.len() {
            return Some(TabSelection::AudiobookshelfLibrary(
                position - self.libs.len() - 1,
            ));
        }
        Some(if self.has_feeds_subscriptions() {
            TabSelection::Feeds
        } else {
            TabSelection::Home
        })
    }

    fn legacy_position_for_key(&self, key: &str) -> Option<crate::config::LibraryPosition> {
        self.library_position_state.libraries.get(key).cloned()
    }

    fn legacy_identities(
        position: &crate::config::LibraryPosition,
        audiobookshelf: bool,
    ) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        let root = position.levels.first();
        let selector = if audiobookshelf {
            root.and_then(|level| {
                level
                    .item_types
                    .as_deref()
                    .filter(|kind| *kind == "book")
                    .and_then(|_| {
                        level
                            .letter_filter_index
                            .and_then(AudiobookshelfBookBucket::from_bucket_index)
                    })
                    .map(|key| SelectorIdentity::Audiobookshelf {
                        key: AudiobookshelfSelectorKey::BookBucket(key),
                    })
            })
        } else {
            root.and_then(|level| {
                level
                    .letter_filter_index
                    .and_then(EmbyLetterBucket::from_index)
                    .map(|key| SelectorIdentity::Emby {
                        key: EmbySelectorKey::Letter(key),
                    })
            })
        };
        let item_level =
            root.filter(|level| !audiobookshelf || level.item_types.as_deref() == Some("book"));
        let item = item_level
            .and_then(|level| level.focused_item_id.clone())
            .map(|id| {
                if audiobookshelf {
                    LibraryItemIdentity::Audiobookshelf { id }
                } else {
                    LibraryItemIdentity::Emby { id }
                }
            });
        (selector, item)
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
                        .map_or(TabSelection::Home, TabSelection::EmbyLibrary),
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
                        .map_or(TabSelection::Home, TabSelection::AudiobookshelfLibrary),
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
    pub(in crate::app) fn normalize_stale_browse_destination(&mut self) -> bool {
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
        self.pending_launch_tab_resolved = false;
        self.pending_launch_state = None;
        self.legacy_launch_tab = None;
        self.legacy_launch_migration_attempted = true;
        self.tab = TabSelection::from_position_with_counts(
            pos,
            self.libs.len(),
            self.audiobookshelf_libraries.len(),
            self.has_feeds_subscriptions(),
        );
        self.settle_tab_selection();
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
