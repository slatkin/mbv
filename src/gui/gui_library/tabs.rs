//! The GUI Library's destination tabs (add-gui-library-panel design D2): the
//! destinations the GUI presents and the active one among them, as the top
//! bar paints them. Phase 1 presents the Grouped Music libraries, and row
//! 7.1 adds the Shows destinations; the launch restore resolves against the
//! same presented list, so the top bar, the restore, and the content
//! projection agree on one set of destinations.

use mbv_ui_model::tab_selection::TabSelection;

use crate::app::App;
use crate::gui::GuiCore;

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

impl GuiCore {
    /// The active destination's kind (design D2, D12): the one fact the
    /// content gate, the tabs' current mark, and the Shows open step read.
    /// `None` for every destination the GUI does not present yet.
    #[must_use]
    pub fn gui_active_destination(&self) -> Option<GuiDestinationKind> {
        active_destination(&self.app)
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
        library_tabs(&self.app)
    }

    /// Step the active destination by `step` presented tabs, wrapping (design
    /// D2): the GUI Client's `Tab` and `Shift+Tab`, which the window content
    /// does not take. The step opens the destination it lands on through the
    /// core's one open step, so a router step opens as a press on the tab
    /// does, and it visits only the presented set — a destination the top bar
    /// does not paint is never opened. A tab state outside that set (no
    /// catalog yet) starts the step at Home.
    ///
    /// The step runs on this core's own state: the open step it calls keeps
    /// the state it leaves (a destination's `Loading` mark among it), exactly
    /// as the window's open-destination command does.
    pub(crate) fn step_gui_library_tab(&mut self, step: isize) {
        let positions = presented_tab_positions(&self.app);
        let current = self
            .app
            .active_tab()
            .to_position_with_counts(self.app.emby_library_count(), self.app.feeds_tab_pos());
        let index = positions
            .iter()
            .position(|position| *position == current)
            .unwrap_or(0);
        let len = isize::try_from(positions.len()).unwrap_or(1);
        let index = isize::try_from(index).unwrap_or(0);
        let next = usize::try_from((index + step).rem_euclid(len)).unwrap_or(0);
        self.open_gui_library_position(positions[next]);
    }
}

/// The active destination's kind over the `App` the GUI core owns (design D2,
/// D12): the one fact the content gate, the tabs' current mark, and the Shows
/// open step read. `None` for every destination the GUI does not present yet.
#[must_use]
pub(crate) fn active_destination(app: &App) -> Option<GuiDestinationKind> {
    match app.active_tab() {
        TabSelection::Home => Some(GuiDestinationKind::Home),
        // The feed tab presents while subscriptions are configured, so a
        // subscription set the session emptied presents no destination.
        TabSelection::Feeds if app.has_feeds_subscriptions() => Some(GuiDestinationKind::Feeds),
        TabSelection::EmbyLibrary(lib_idx) if lib_idx < app.emby_library_count() => {
            if app.is_grouped_music_library(lib_idx) {
                Some(GuiDestinationKind::Music { lib_idx })
            } else if app.emby_library_collection_type(lib_idx) == Some("tvshows") {
                Some(GuiDestinationKind::Shows { lib_idx })
            } else {
                Some(GuiDestinationKind::Library { lib_idx })
            }
        }
        TabSelection::AudiobookshelfLibrary(index)
            if index < app.audiobookshelf_libraries().len() =>
        {
            match app.audiobookshelf_kind_at(index) {
                Some(mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Book) => {
                    Some(GuiDestinationKind::Audiobooks { lib_idx: index })
                }
                Some(mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Podcast) => {
                    Some(GuiDestinationKind::Podcasts { lib_idx: index })
                }
                None => None,
            }
        }
        _ => None,
    }
}

/// The tab position one presented destination's tab takes (design D2): the
/// position the open-destination command takes. `None` for Home, which names
/// no library tab, and for Feeds while no subscription makes its tab. Every
/// destination kind is named, so a new kind is a compile error here rather
/// than a tab that silently takes another kind's position.
#[must_use]
pub(crate) fn destination_position(app: &App, kind: GuiDestinationKind) -> Option<usize> {
    let emby_position = |lib_idx: usize| {
        TabSelection::EmbyLibrary(lib_idx)
            .to_position_with_counts(app.emby_library_count(), app.feeds_tab_pos())
    };
    match kind {
        GuiDestinationKind::Home => None,
        GuiDestinationKind::Feeds => app.feeds_tab_pos(),
        GuiDestinationKind::Music { lib_idx }
        | GuiDestinationKind::Shows { lib_idx }
        | GuiDestinationKind::Library { lib_idx } => Some(emby_position(lib_idx)),
        GuiDestinationKind::Audiobooks { lib_idx } | GuiDestinationKind::Podcasts { lib_idx } => {
            Some(
                TabSelection::AudiobookshelfLibrary(lib_idx)
                    .to_position_with_counts(app.emby_library_count(), app.feeds_tab_pos()),
            )
        }
    }
}

/// The tab positions the GUI presents, Home first (design D2): Home's position
/// 0, then every presented destination's position in tab order. The
/// destination steps read this one list, so a step never lands on a tab
/// position that names no presented destination.
fn presented_tab_positions(app: &App) -> Vec<usize> {
    let mut positions = vec![0];
    positions.extend(
        super::launch::presented_destinations(app)
            .into_iter()
            .filter_map(|kind| destination_position(app, kind)),
    );
    positions.sort_unstable();
    positions.dedup();
    positions
}

/// One presented destination's tab label (design D2): the library's display
/// name, or the Feeds tab's fixed label, as the TUI's tab strip paints it.
/// Every destination kind is named, so a new kind is a compile error here
/// rather than a tab that silently takes another kind's label.
fn destination_label(app: &App, kind: GuiDestinationKind) -> Option<String> {
    match kind {
        GuiDestinationKind::Home => None,
        GuiDestinationKind::Feeds => Some("Feeds".to_string()),
        GuiDestinationKind::Audiobooks { lib_idx } | GuiDestinationKind::Podcasts { lib_idx } => {
            Some(app.audiobookshelf_library(lib_idx)?.name.clone())
        }
        GuiDestinationKind::Music { lib_idx }
        | GuiDestinationKind::Shows { lib_idx }
        | GuiDestinationKind::Library { lib_idx } => {
            Some(app.emby_library_name(lib_idx)?.to_string())
        }
    }
}

/// The destination tabs the Library panel's frame carries over the `App` the
/// GUI core owns (design D2): the presented destinations in catalog order —
/// one tab per Grouped Music and Shows library, per plain Emby library, per
/// Audiobookshelf library of either kind, and the Feeds tab — and the active
/// tab's position when the active destination is one of them. The presented
/// list is the launch restore's (`presented_destinations`), so the top bar and
/// the restore cannot disagree.
#[must_use]
pub(crate) fn library_tabs(app: &App) -> GuiLibraryTabs {
    let tabs = super::launch::presented_destinations(app)
        .into_iter()
        .filter_map(|kind| {
            let position = destination_position(app, kind)?;
            let label = destination_label(app, kind)?;
            Some(GuiLibraryTab { position, label })
        })
        .collect();
    let current = active_destination(app).and_then(|kind| destination_position(app, kind));
    GuiLibraryTabs { tabs, current }
}
