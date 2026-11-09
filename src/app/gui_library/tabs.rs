//! The GUI Library's destination tabs (add-gui-library-panel design D2): the
//! destinations the GUI presents and the active one among them, as the top
//! bar paints them. Phase 1 presents the Grouped Music libraries, and row
//! 7.1 adds the Shows destinations; the launch restore's fallback reads the
//! same presented list, so the top bar, the restore, and the content
//! projection agree on one set of destinations.

use mbv_ui_model::tab_selection::TabSelection;

use crate::app::App;

/// The destination kind one tab position names (design D2, D12): the one
/// abstraction the top bar, the content gate, and the launch restore read,
/// so a destination joins the presented set through this enum instead of a
/// second hard-coded Music-shaped path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiDestinationKind {
    /// The Home destination: position 0, always openable, never in the
    /// presented libraries' list.
    Home,
    /// A Grouped Music library's Music destination (design D5).
    Music { lib_idx: usize },
    /// An Emby TV (Shows) library's Shows destination (design D12, row
    /// 7.1).
    Shows { lib_idx: usize },
    /// A plain Emby library's destination (design D12, row 8.1): every Emby
    /// library that is neither a Grouped Music library nor a TV library —
    /// Movies, boxsets, music videos, home videos outside the feed view,
    /// mixed, and the open collection types.
    Library { lib_idx: usize },
    /// An Audiobookshelf book library's Audiobooks destination (design D12,
    /// row 9.1). `lib_idx` indexes the Audiobookshelf libraries, not the Emby
    /// ones. The podcast libraries stay unpresented until row 10.1.
    Audiobooks { lib_idx: usize },
}

impl GuiDestinationKind {
    /// The destination's library index; `None` for Home, which names no
    /// library.
    #[must_use]
    pub fn library_index(self) -> Option<usize> {
        match self {
            Self::Home => None,
            Self::Music { lib_idx }
            | Self::Shows { lib_idx }
            | Self::Library { lib_idx }
            | Self::Audiobooks { lib_idx } => Some(lib_idx),
        }
    }
}

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
    /// The active destination's kind (design D2, D12): the one fact the
    /// content gate, the tabs' current mark, and the Shows open step read.
    /// `None` for Feeds and every other destination the GUI does not present
    /// yet.
    #[must_use]
    pub fn gui_active_destination(&self) -> Option<GuiDestinationKind> {
        match self.tab {
            TabSelection::Home => Some(GuiDestinationKind::Home),
            TabSelection::EmbyLibrary(lib_idx) if lib_idx < self.libs.len() => {
                if self.is_grouped_music_library(lib_idx) {
                    Some(GuiDestinationKind::Music { lib_idx })
                } else if self.libs[lib_idx].library.collection_type == "tvshows" {
                    Some(GuiDestinationKind::Shows { lib_idx })
                } else {
                    Some(GuiDestinationKind::Library { lib_idx })
                }
            }
            TabSelection::AudiobookshelfLibrary(index)
                if index < self.audiobookshelf_libraries.len()
                    && self.audiobookshelf_kind_at(index)
                        == Some(
                            mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Book,
                        ) =>
            {
                Some(GuiDestinationKind::Audiobooks { lib_idx: index })
            }
            _ => None,
        }
    }

    /// The tab position one presented destination's tab takes (design D2):
    /// the position the open-destination command takes. `None` for Home,
    /// which names no library tab.
    pub(in crate::app::gui_library) fn gui_destination_position(
        &self,
        kind: GuiDestinationKind,
    ) -> Option<usize> {
        if let GuiDestinationKind::Audiobooks { lib_idx } = kind {
            return Some(
                TabSelection::AudiobookshelfLibrary(lib_idx)
                    .to_position_with_counts(self.libs.len(), self.feeds_tab_pos()),
            );
        }
        let lib_idx = kind.library_index()?;
        Some(
            TabSelection::EmbyLibrary(lib_idx)
                .to_position_with_counts(self.libs.len(), self.feeds_tab_pos()),
        )
    }

    /// One presented destination's tab label (design D2): the library's
    /// display name. `None` for Home, whose icon paints no label here.
    fn gui_destination_label(&self, kind: GuiDestinationKind) -> Option<String> {
        match kind {
            GuiDestinationKind::Home => None,
            GuiDestinationKind::Audiobooks { lib_idx } => {
                Some(self.audiobookshelf_libraries.get(lib_idx)?.name.clone())
            }
            _ => Some(self.libs[kind.library_index()?].library.name.clone()),
        }
    }

    /// The destination tabs the Library panel's frame carries (design D2):
    /// the presented destinations in catalog order — one tab per Grouped
    /// Music and Shows library — and the active tab's position when the
    /// active destination is one of them. The presented list is the launch
    /// restore's (`presented_destinations`), so the top bar and the restore
    /// cannot disagree.
    #[must_use]
    pub fn gui_library_tabs(&self) -> GuiLibraryTabs {
        let tabs = self
            .presented_destinations()
            .into_iter()
            .filter_map(|kind| {
                let position = self.gui_destination_position(kind)?;
                let label = self.gui_destination_label(kind)?;
                Some(GuiLibraryTab { position, label })
            })
            .collect();
        let current = self
            .gui_active_destination()
            .and_then(|kind| self.gui_destination_position(kind));
        GuiLibraryTabs { tabs, current }
    }
}
