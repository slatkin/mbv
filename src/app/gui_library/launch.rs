//! The GUI's launch location (add-gui-library-panel design D9): the
//! destination, the selected collections of each destination, and Panel
//! focus, saved at orderly exit and restored after the catalog lands.
//!
//! The GUI keeps its own file ([`mbv_config::save_gui_launch_state`]): the TUI
//! launch state never moves this Client's tab or Panel focus, and this file
//! never holds a TUI launch location. Identities are stable Service ids, so a
//! resolution that finds no match falls back instead of opening a stale
//! position.

use mbv_config::{GUI_LAUNCH_STATE_VERSION, GuiDestinationCollections, GuiLaunchState};
use mbv_config::{LaunchPanelFocus, TabIdentity};
use mbv_queue::ServiceKind;
use mbv_ui_model::browse::BrowseLevel;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_model::tab_selection::TabSelection;

use crate::app::App;

impl App {
    /// The GUI's launch location (design D9): the active destination, the
    /// selected collections of each destination, and Panel focus. Read at
    /// orderly exit only; nothing writes it while the window is open.
    #[must_use]
    pub fn gui_launch_state(&self) -> GuiLaunchState {
        GuiLaunchState {
            version: GUI_LAUNCH_STATE_VERSION,
            destination: self.gui_launch_destination(),
            panel_focus: match self.gui_panel_focus() {
                PanelFocus::Library => LaunchPanelFocus::Library,
                PanelFocus::Queue => LaunchPanelFocus::Queue,
            },
            collections: self.gui_launch_collections(),
        }
    }

    /// Open the destination a saved location names (design D9): the presented
    /// destination whose identity matches, or the first presented one while
    /// the GUI presents only some destinations. Returns the opened
    /// destination's identity as the saved file carries it; `None` means the
    /// catalog holds none yet and the caller retries when one lands. The
    /// caller applies the saved Panel focus after the open, because opening a
    /// library takes Library focus.
    pub fn open_gui_launch_destination(&mut self, state: &GuiLaunchState) -> Option<TabIdentity> {
        let position = self.presented_destination_position(&state.destination)?;
        self.set_library_tab(position);
        Some(self.gui_launch_destination())
    }

    /// Apply the saved collection set of the destination the first phase
    /// opened (design D9): a saved group that the library no longer holds is
    /// dropped, and an empty result — a saved set the library emptied, and
    /// a destination with no saved entry, a fresh launch's case — is the
    /// show-all selection: the union of every group the library holds. The
    /// saved entry matches `destination`, the identity phase 1 recorded, and
    /// the active destination must still be that one: a mid-restore tab
    /// switch abandons the restore, so no other destination's saved set
    /// lands under the user. Returns `false` while the destination's group
    /// list has not landed, so the caller retries.
    pub fn restore_gui_launch_collections(
        &mut self,
        state: &GuiLaunchState,
        destination: &TabIdentity,
    ) -> bool {
        let Some(lib_idx) = self.gui_music_destination() else {
            // The active destination paints no collection pills.
            return true;
        };
        if self.gui_launch_destination() != *destination {
            // A different destination is active: the restore gives up rather
            // than applying a saved set the user did not ask for.
            return true;
        }
        let Some(present) = self.gui_music_present_groups(lib_idx) else {
            return false;
        };
        if self.gui_library_groups.contains_key(&lib_idx) {
            // The user already selected a group in the pre-restore window: the
            // restore never clobbers a choice the session made.
            return true;
        }
        // The saved group the library still holds, in saved order, is the
        // one selected. Without one, the destination keeps the default first
        // group (design D6), so a fresh launch opens on the first group.
        let saved_group = state
            .collections
            .iter()
            .find(|entry| entry.destination == *destination)
            .and_then(|entry| {
                entry
                    .group_ids
                    .iter()
                    .find(|group_id| present.contains(group_id))
            });
        if let Some(group_id) = saved_group.cloned() {
            self.select_gui_library_collection(lib_idx, group_id);
        }
        true
    }

    /// The identity of the active destination (design D9), as the saved file
    /// carries it: a stable Service library id, never a tab position.
    fn gui_launch_destination(&self) -> TabIdentity {
        match self.tab {
            TabSelection::Home => TabIdentity::Home,
            TabSelection::Feeds => TabIdentity::Feeds,
            TabSelection::EmbyLibrary(lib_idx) => {
                self.libs.get(lib_idx).map_or(TabIdentity::Home, |tab| {
                    library_identity(ServiceKind::Emby, &tab.library.id)
                })
            }
            TabSelection::AudiobookshelfLibrary(index) => self
                .audiobookshelf_libraries
                .get(index)
                .map_or(TabIdentity::Home, |library| {
                    library_identity(ServiceKind::Audiobookshelf, &library.id)
                }),
        }
    }

    /// The selected collection of the catalog's destinations (design D9), in
    /// catalog order: one entry per destination whose choice the session holds,
    /// so a destination the session never chose opens on its first group. The
    /// file keeps the list shape, and each entry holds the one selected group.
    fn gui_launch_collections(&self) -> Vec<GuiDestinationCollections> {
        (0..self.libs.len())
            .filter_map(|lib_idx| {
                let group_id = self.gui_library_groups.get(&lib_idx)?;
                Some(GuiDestinationCollections {
                    destination: library_identity(
                        ServiceKind::Emby,
                        &self.libs[lib_idx].library.id,
                    ),
                    group_ids: vec![group_id.clone()],
                })
            })
            .collect()
    }

    /// The destinations the GUI presents, in catalog order (design D9, D2):
    /// the Grouped Music libraries, phase 1's presented set. The restore's
    /// fallback and the frame's destination tabs (`gui_library_tabs`) read
    /// this one list, so they cannot disagree about the presented set.
    pub(in crate::app::gui_library) fn presented_destinations(&self) -> Vec<usize> {
        (0..self.libs.len())
            .filter(|lib_idx| self.is_grouped_music_library(*lib_idx))
            .collect()
    }

    /// Whether an open-destination command's `position` names a destination
    /// the GUI presents (design D2): Home, or one of the presented
    /// destinations' tab positions. The command arm reads this one
    /// definition, so a stale frame's position cannot activate a destination
    /// the top bar does not paint — an unpresented destination opens
    /// nothing, as the absent digit key does.
    pub(in crate::app::gui_library) fn presents_open_position(&self, position: usize) -> bool {
        position == 0
            || self.presented_destinations().into_iter().any(|lib_idx| {
                TabSelection::EmbyLibrary(lib_idx)
                    .to_position_with_counts(self.libs.len(), self.feeds_tab_pos())
                    == position
            })
    }

    /// The tab position of the destination a saved location names, resolved
    /// against the destinations the GUI presents (design D9): the matching
    /// presented library, else the first presented one. Phase 1 presents the
    /// Grouped Music libraries only, so every other saved destination falls
    /// back to the first of them.
    fn presented_destination_position(&self, destination: &TabIdentity) -> Option<usize> {
        let saved_library_id = match destination {
            TabIdentity::ServiceLibrary {
                kind: ServiceKind::Emby,
                library_id,
            } => Some(library_id.as_str()),
            TabIdentity::Home | TabIdentity::Feeds | TabIdentity::ServiceLibrary { .. } => None,
        };
        let presented = self.presented_destinations();
        let lib_idx = presented
            .iter()
            .copied()
            .find(|lib_idx| {
                saved_library_id
                    .is_some_and(|library_id| self.libs[*lib_idx].library.id == library_id)
            })
            .or_else(|| presented.first().copied())?;
        Some(
            TabSelection::EmbyLibrary(lib_idx)
                .to_position_with_counts(self.libs.len(), self.feeds_tab_pos()),
        )
    }

    /// The active Grouped Music library's group level: the library's root in
    /// `nav_stack`, whose items are the collection groups the pills paint.
    pub(in crate::app::gui_library) fn gui_music_group_level(
        &self,
        lib_idx: usize,
    ) -> Option<&BrowseLevel> {
        let lib = self.libs.get(lib_idx)?;
        lib.nav_stack
            .iter()
            .find(|level| level.parent_id == lib.library.id)
    }

    /// The groups the library holds, in pill order. `None` while the list has
    /// not landed: the level is absent, or its fetch is still in flight.
    pub(in crate::app::gui_library) fn gui_music_present_groups(
        &self,
        lib_idx: usize,
    ) -> Option<Vec<String>> {
        let level = self.gui_music_group_level(lib_idx)?;
        if level.loading {
            return None;
        }
        Some(level.items.iter().map(|group| group.id.clone()).collect())
    }
}

/// The saved identity of one Service library.
fn library_identity(kind: ServiceKind, library_id: &str) -> TabIdentity {
    TabIdentity::ServiceLibrary {
        kind,
        library_id: library_id.to_string(),
    }
}
