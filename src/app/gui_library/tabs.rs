//! The GUI Library's destination tabs (add-gui-library-panel design D2): the
//! destinations the GUI presents and the active one among them, as the top
//! bar paints them. Phase 1 presents the Grouped Music libraries only, and
//! the launch restore's fallback reads the same presented list, so the top
//! bar, the restore, and the content projection agree on one set of
//! destinations.

use mbv_ui_model::tab_selection::TabSelection;

use crate::app::App;

/// One destination tab the Library panel's top bar paints (design D2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiLibraryTab {
    /// The tab position the open-destination command takes, where 0 is Home.
    pub position: usize,
    /// The tab's label: the library's display name.
    pub label: String,
}

/// The destination tabs the Library panel's frame carries (design D2): the
/// presented destinations in catalog order, and the active tab among them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiLibraryTabs {
    /// One tab per presented destination, in catalog order.
    pub tabs: Vec<GuiLibraryTab>,
    /// The active tab's position while the active destination is one of the
    /// presented ones; `None` for Home, Feeds, and every destination the GUI
    /// does not present yet.
    pub current: Option<usize>,
}

impl App {
    /// The destination tabs the Library panel's frame carries (design D2):
    /// the presented destinations in catalog order — one tab per Grouped
    /// Music library, phase 1's presented set — and the active tab's
    /// position when the active destination is one of them. The presented
    /// list is the launch restore's (`presented_destination_position`), so
    /// the top bar and the restore cannot disagree.
    #[must_use]
    pub fn gui_library_tabs(&self) -> GuiLibraryTabs {
        let tabs = self
            .presented_destinations()
            .into_iter()
            .map(|lib_idx| {
                let position = TabSelection::EmbyLibrary(lib_idx)
                    .to_position_with_counts(self.libs.len(), self.feeds_tab_pos());
                GuiLibraryTab {
                    position,
                    label: self.libs[lib_idx].library.name.clone(),
                }
            })
            .collect();
        let current = self.gui_music_destination().map(|lib_idx| {
            TabSelection::EmbyLibrary(lib_idx)
                .to_position_with_counts(self.libs.len(), self.feeds_tab_pos())
        });
        GuiLibraryTabs { tabs, current }
    }
}
