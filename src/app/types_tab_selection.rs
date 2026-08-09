/// Which left-panel tab is selected.
///
/// `Home` is the Continue Watching / home view. `Library(usize)` holds the
/// 0-based index into `App::libs`. No `Feeds` variant yet — #471 adds it,
/// at which point exhaustive matches force every decision site to account
/// for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TabSelection {
    Home,
    Library(usize),
}

impl TabSelection {
    /// `true` when the Home / Continue Watching tab is selected.
    pub(super) fn is_home(self) -> bool {
        matches!(self, Self::Home)
    }

    /// The 0-based library index, or `None` on Home.
    pub(super) fn library_index(self) -> Option<usize> {
        match self {
            Self::Home => None,
            Self::Library(i) => Some(i),
        }
    }

    /// Strip position: `0 = Home`, `1.. = Library(pos - 1)`.
    pub(super) fn from_position(pos: usize) -> Self {
        if pos == 0 {
            Self::Home
        } else {
            Self::Library(pos - 1)
        }
    }

    /// Strip position: `0 = Home`, `1.. = Library(pos - 1)`.
    pub(super) fn to_position(self) -> usize {
        match self {
            Self::Home => 0,
            Self::Library(i) => i + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let cases: &[TabSelection] = &[
            TabSelection::Home,
            TabSelection::Library(0),
            TabSelection::Library(1),
            TabSelection::Library(5),
            TabSelection::Library(100),
        ];
        for &t in cases {
            assert_eq!(
                TabSelection::from_position(t.to_position()),
                t,
                "round-trip failed for {t:?}"
            );
        }
    }

    #[test]
    fn position_mapping() {
        assert_eq!(TabSelection::Home.to_position(), 0);
        assert_eq!(TabSelection::Library(0).to_position(), 1);
        assert_eq!(TabSelection::Library(3).to_position(), 4);

        assert!(TabSelection::from_position(0).is_home());
        assert_eq!(TabSelection::from_position(1), TabSelection::Library(0));
        assert_eq!(TabSelection::from_position(4), TabSelection::Library(3));
    }
}
