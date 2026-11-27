//! The GUI Library's destination-neutral single-choice view (emby-favourites
//! design D5): All, New, Upcoming, or Favourites for the Shows destination,
//! All and Favourites for every other Emby destination. This is a view of
//! what the destination shows, not a collection set or filter, so it has its
//! own type, command, core state, frame field, and launch field. The TUI's
//! content modes stay unchanged.

use crate::gui_core::gui_client_state::GuiClientState;

use super::tabs::GuiDestinationKind;

/// The Emby destination's one active view (design D5): exactly one entry is
/// active. All is the default and is the destination's own list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuiLibraryView {
    /// The destination's own list: the series list with its letter rail,
    /// Hero, and Workspace on Shows, the library's flat list elsewhere.
    #[default]
    All,
    /// The merged latest-episodes list (the Shows destination alone).
    New,
    /// The merged upcoming-premieres list (the Shows destination alone),
    /// whose rows are display-only.
    Upcoming,
    /// The library's favourite items (design D5). The choice does not
    /// survive a restart: the launch saves it as All.
    Favourites,
    /// The TV library's favourite series (design D9): one of the two views
    /// the Shows destination separates its favourites into, as Emby's TV
    /// Favorites tab keeps Favorite Shows and Favorite Episodes apart.
    FavouriteShows,
    /// The TV library's favourite episodes (design D9).
    FavouriteEpisodes,
}

impl GuiLibraryView {
    /// The choices `destination` offers, in the order the dropdown paints
    /// them (design D5): the Shows destination offers its three content
    /// views and the two favourite views, every non-music Emby destination
    /// offers All and Favourites, and a Grouped Music destination offers
    /// nothing, because it is excluded from favourites everywhere (design
    /// D11) and its header paints no view dropdown. No non-Emby destination
    /// paints a view dropdown either, so its arm names the empty set.
    #[must_use]
    pub fn choices(destination: GuiDestinationKind) -> &'static [Self] {
        match destination {
            GuiDestinationKind::Shows { .. } => &[
                Self::All,
                Self::New,
                Self::Upcoming,
                Self::FavouriteShows,
                Self::FavouriteEpisodes,
            ],
            GuiDestinationKind::Library { .. } => &[Self::All, Self::Favourites],
            _ => &[],
        }
    }

    /// The frame key the dropdown row press sends.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::New => "new",
            Self::Upcoming => "upcoming",
            Self::Favourites => "favourites",
            Self::FavouriteShows => "favourite-shows",
            Self::FavouriteEpisodes => "favourite-episodes",
        }
    }

    /// The label the trigger and the dropdown row paint.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::New => "Recent",
            Self::Upcoming => "Upcoming",
            Self::Favourites => "Favourites",
            Self::FavouriteShows => "Favourite Shows",
            Self::FavouriteEpisodes => "Favourite Episodes",
        }
    }

    /// The view a frame key names, or `None` for a stale key.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "all" => Some(Self::All),
            "new" => Some(Self::New),
            "upcoming" => Some(Self::Upcoming),
            "favourites" => Some(Self::Favourites),
            "favourite-shows" => Some(Self::FavouriteShows),
            "favourite-episodes" => Some(Self::FavouriteEpisodes),
            _ => None,
        }
    }
}

/// The stored view of one library (design D5): a library with no stored
/// choice shows All, the default. It reads only the core's library-view
/// store, so it is a core function the projection, the row art, and the
/// launch reach through the `App` they hold.
pub(crate) fn library_view(gui: &GuiClientState, lib_idx: usize) -> GuiLibraryView {
    gui.library_views.get(&lib_idx).copied().unwrap_or_default()
}

impl GuiLibraryView {
    /// Whether the view is one of the favourites family (design D5, D9):
    /// Favourites, Favourite Shows, and Favourite Episodes share the
    /// Favourites list component, the Favourites Hero, the pick, the scroll,
    /// and every header rule, so every fork but the projection's type filter
    /// reads this one predicate.
    #[must_use]
    pub const fn is_favourites_view(self) -> bool {
        matches!(
            self,
            Self::Favourites | Self::FavouriteShows | Self::FavouriteEpisodes
        )
    }
}

/// Whether `lib_idx`'s destination paints a favourites view (emby-favourites
/// design D5, D6, D9): the one predicate every step that forks on the active
/// view reads, so the Music, Shows, and plain Emby destinations answer it the
/// same way.
#[must_use]
pub(crate) fn paints_favourites(gui: &GuiClientState, lib_idx: usize) -> bool {
    library_view(gui, lib_idx).is_favourites_view()
}
