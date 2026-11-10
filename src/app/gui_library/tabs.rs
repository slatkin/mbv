//! The GUI Library's destination tabs (add-gui-library-panel design D2): the
//! destinations the GUI presents and the active one among them, as the top
//! bar paints them. Phase 1 presents the Grouped Music libraries, and row
//! 7.1 adds the Shows destinations; the launch restore resolves against the
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
    /// ones.
    Audiobooks { lib_idx: usize },
    /// An Audiobookshelf podcast library's Podcasts destination (design D12,
    /// row 10.1). `lib_idx` indexes the Audiobookshelf libraries, as the
    /// Audiobooks kind does; a podcast library never presents the Audiobooks
    /// destination and a book library never presents this one.
    Podcasts { lib_idx: usize },
    /// The Feeds destination (design D12, row 11.1): the one synthetic tab
    /// after every Service library, present while feed subscriptions are
    /// configured, as the TUI's own tab-strip rule has it. It names no
    /// library.
    Feeds,
}

impl GuiDestinationKind {
    /// The destination's library index; `None` for Home and Feeds, which name
    /// no library.
    #[must_use]
    pub fn library_index(self) -> Option<usize> {
        match self {
            Self::Home | Self::Feeds => None,
            Self::Music { lib_idx }
            | Self::Shows { lib_idx }
            | Self::Library { lib_idx }
            | Self::Audiobooks { lib_idx }
            | Self::Podcasts { lib_idx } => Some(lib_idx),
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
    /// `None` for every destination the GUI does not present yet.
    #[must_use]
    pub fn gui_active_destination(&self) -> Option<GuiDestinationKind> {
        match self.tab {
            TabSelection::Home => Some(GuiDestinationKind::Home),
            // The feed tab presents while subscriptions are configured, so a
            // subscription set the session emptied presents no destination.
            TabSelection::Feeds if self.has_feeds_subscriptions() => {
                Some(GuiDestinationKind::Feeds)
            }
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
                if index < self.audiobookshelf_libraries.len() =>
            {
                match self.audiobookshelf_kind_at(index) {
                    Some(mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Book) => {
                        Some(GuiDestinationKind::Audiobooks { lib_idx: index })
                    }
                    Some(
                        mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Podcast,
                    ) => Some(GuiDestinationKind::Podcasts { lib_idx: index }),
                    None => None,
                }
            }
            _ => None,
        }
    }

    /// The tab position one presented destination's tab takes (design D2):
    /// the position the open-destination command takes. `None` for Home,
    /// which names no library tab, and for Feeds while no subscription makes
    /// its tab. Every destination kind is named, so a new kind is a compile
    /// error here rather than a tab that silently takes another kind's
    /// position.
    pub(in crate::app::gui_library) fn gui_destination_position(
        &self,
        kind: GuiDestinationKind,
    ) -> Option<usize> {
        let emby_position = |lib_idx: usize| {
            TabSelection::EmbyLibrary(lib_idx)
                .to_position_with_counts(self.libs.len(), self.feeds_tab_pos())
        };
        match kind {
            GuiDestinationKind::Home => None,
            GuiDestinationKind::Feeds => self.feeds_tab_pos(),
            GuiDestinationKind::Music { lib_idx }
            | GuiDestinationKind::Shows { lib_idx }
            | GuiDestinationKind::Library { lib_idx } => Some(emby_position(lib_idx)),
            GuiDestinationKind::Audiobooks { lib_idx }
            | GuiDestinationKind::Podcasts { lib_idx } => Some(
                TabSelection::AudiobookshelfLibrary(lib_idx)
                    .to_position_with_counts(self.libs.len(), self.feeds_tab_pos()),
            ),
        }
    }

    /// One presented destination's tab label (design D2): the library's
    /// display name, or the Feeds tab's fixed label, as the TUI's tab strip
    /// paints it. Every destination kind is named, so a new kind is a compile
    /// error here rather than a tab that silently takes another kind's label.
    fn gui_destination_label(&self, kind: GuiDestinationKind) -> Option<String> {
        match kind {
            GuiDestinationKind::Home => None,
            GuiDestinationKind::Feeds => Some("Feeds".to_string()),
            GuiDestinationKind::Audiobooks { lib_idx }
            | GuiDestinationKind::Podcasts { lib_idx } => {
                Some(self.audiobookshelf_libraries.get(lib_idx)?.name.clone())
            }
            GuiDestinationKind::Music { lib_idx }
            | GuiDestinationKind::Shows { lib_idx }
            | GuiDestinationKind::Library { lib_idx } => {
                Some(self.libs.get(lib_idx)?.library.name.clone())
            }
        }
    }

    /// The destination tabs the Library panel's frame carries (design D2):
    /// the presented destinations in catalog order — one tab per Grouped
    /// Music and Shows library, per plain Emby library, per Audiobookshelf
    /// library of either kind, and the Feeds tab — and the active tab's
    /// position when the active destination is one of them. The presented
    /// list is the launch restore's (`presented_destinations`), so the top bar
    /// and the restore cannot disagree.
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
