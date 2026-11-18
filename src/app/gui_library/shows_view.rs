//! The GUI Shows destination's single-choice view (gui-collection-dropdown
//! design D8): All, New, or Upcoming. This is a view of what the destination
//! shows, not a collection set or filter, so it has its own type, command,
//! core state, frame field, and launch field. The TUI's TV content modes stay
//! unchanged.

/// The Shows destination's one active view (design D8): exactly one entry is
/// active. All is the default and is the series list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuiShowsView {
    /// The series list with its letter rail, Hero, and Workspace.
    #[default]
    All,
    /// The merged latest-episodes list.
    New,
    /// The merged upcoming-premieres list, whose rows are display-only.
    Upcoming,
}

impl GuiShowsView {
    /// The three entries, in the order the dropdown paints them.
    pub const ALL_VIEWS: [Self; 3] = [Self::All, Self::New, Self::Upcoming];

    /// The frame key the dropdown row press sends.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::New => "new",
            Self::Upcoming => "upcoming",
        }
    }

    /// The label the trigger and the dropdown row paint.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::New => "New",
            Self::Upcoming => "Upcoming",
        }
    }

    /// The view a frame key names, or `None` for a stale key.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "all" => Some(Self::All),
            "new" => Some(Self::New),
            "upcoming" => Some(Self::Upcoming),
            _ => None,
        }
    }
}
