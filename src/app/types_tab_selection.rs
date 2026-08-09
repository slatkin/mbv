/// Which left-panel tab is selected.
///
/// `Home` is the Continue Watching / home view. `Library(usize)` holds the
/// 0-based index into `App::libs`. `Feeds` is the synthetic tab appended
/// after libraries when feed subscriptions exist (#471).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TabSelection {
    Home,
    Library(usize),
    Feeds,
}

impl TabSelection {
    /// `true` when the Home / Continue Watching tab is selected.
    pub(super) fn is_home(self) -> bool {
        matches!(self, Self::Home)
    }

    /// `true` when the Feeds tab is selected.
    pub(super) fn is_feeds(self) -> bool {
        matches!(self, Self::Feeds)
    }

    /// The 0-based library index, or `None` on Home and Feeds.
    pub(super) fn library_index(self) -> Option<usize> {
        match self {
            Self::Home | Self::Feeds => None,
            Self::Library(i) => Some(i),
        }
    }

    /// Strip position: `0 = Home`, `1.. = Library(pos - 1)`, with `feeds_tab_pos`
    /// marking the Feeds tab when `Some`.
    pub(super) fn from_position(pos: usize, feeds_tab_pos: Option<usize>) -> Self {
        if Some(pos) == feeds_tab_pos {
            Self::Feeds
        } else if pos == 0 {
            Self::Home
        } else {
            Self::Library(pos - 1)
        }
    }

    /// Strip position: `0 = Home`, `1.. = Library(pos - 1)`, with `feeds_tab_pos`
    /// marking the Feeds tab when `Some`.
    pub(super) fn to_position(self, feeds_tab_pos: Option<usize>) -> usize {
        match self {
            Self::Home => 0,
            Self::Library(i) => i + 1,
            Self::Feeds => feeds_tab_pos.unwrap_or(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_no_feeds() {
        let cases: &[TabSelection] = &[
            TabSelection::Home,
            TabSelection::Library(0),
            TabSelection::Library(1),
            TabSelection::Library(5),
            TabSelection::Library(100),
        ];
        for &t in cases {
            assert_eq!(
                TabSelection::from_position(t.to_position(None), None),
                t,
                "round-trip failed for {t:?}"
            );
        }
    }

    #[test]
    fn position_mapping_no_feeds() {
        assert_eq!(TabSelection::Home.to_position(None), 0);
        assert_eq!(TabSelection::Library(0).to_position(None), 1);
        assert_eq!(TabSelection::Library(3).to_position(None), 4);

        assert!(TabSelection::from_position(0, None).is_home());
        assert_eq!(
            TabSelection::from_position(1, None),
            TabSelection::Library(0)
        );
        assert_eq!(
            TabSelection::from_position(4, None),
            TabSelection::Library(3)
        );
    }

    #[test]
    fn feeds_position_round_trip() {
        let fp = Some(3); // e.g. Home(0) + 2 libs(1,2) → Feeds at 3
        assert_eq!(TabSelection::Feeds.to_position(fp), 3);
        assert_eq!(TabSelection::from_position(3, fp), TabSelection::Feeds);
        assert!(TabSelection::from_position(3, fp).is_feeds());
    }

    #[test]
    fn feeds_does_not_steal_library_positions() {
        let fp = Some(3);
        // Position 2 is still Library(1), not Feeds.
        assert_eq!(TabSelection::from_position(2, fp), TabSelection::Library(1));
        assert!(!TabSelection::from_position(2, fp).is_feeds());
    }

    #[test]
    fn no_feeds_yields_library_for_nonzero_positions() {
        // Without feeds, position 3 is Library(2).
        assert_eq!(
            TabSelection::from_position(3, None),
            TabSelection::Library(2)
        );
    }

    #[test]
    fn feeds_library_index_is_none() {
        assert_eq!(TabSelection::Feeds.library_index(), None);
    }
}
