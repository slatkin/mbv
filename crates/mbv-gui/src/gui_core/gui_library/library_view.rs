//! The GUI Library's destination-neutral single-choice view (emby-favourites
//! design D5): All, New, Upcoming, or Favourites for the Shows destination,
//! All and Favourites for every other Emby destination. This is a view of
//! what the destination shows, not a collection set or filter, so it has its
//! own type, command, core state, frame field, and launch field. The TUI's
//! content modes stay unchanged.

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
}

impl GuiLibraryView {
    /// The choices `destination` offers, in the order the dropdown paints
    /// them (design D5): the Shows destination offers its three content
    /// views and Favourites, and every other Emby destination offers All and
    /// Favourites. No non-Emby destination paints a view dropdown, so its
    /// arm names the Emby pair.
    #[must_use]
    pub fn choices(destination: GuiDestinationKind) -> &'static [Self] {
        match destination {
            GuiDestinationKind::Shows { .. } => {
                &[Self::All, Self::New, Self::Upcoming, Self::Favourites]
            }
            _ => &[Self::All, Self::Favourites],
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
            _ => None,
        }
    }
}
