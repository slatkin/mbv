/// Which left-panel tab is selected.
///
/// `Home` is the Continue Watching / home view. `EmbyLibrary(usize)` holds
/// the 0-based index into `App::libs`; `AudiobookshelfLibrary(usize)` holds
/// the 0-based index into `App::audiobookshelf_libraries`. `Feeds` is the
/// synthetic tab appended after libraries when feed subscriptions exist
/// (#471).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TabSelection {
    Home,
    EmbyLibrary(usize),
    AudiobookshelfLibrary(usize),
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

    /// The 0-based Emby library index, or `None` on Home,
    /// Audiobookshelf, and Feeds.
    pub(super) fn emby_library_index(self) -> Option<usize> {
        match self {
            Self::Home | Self::Feeds => None,
            Self::EmbyLibrary(i) => Some(i),
            Self::AudiobookshelfLibrary(_) => None,
        }
    }

    pub(super) fn audiobookshelf_index(self) -> Option<usize> {
        match self {
            Self::AudiobookshelfLibrary(i) => Some(i),
            _ => None,
        }
    }

    pub(super) fn from_position_with_counts(
        pos: usize,
        emby: usize,
        audio: usize,
        feeds: bool,
    ) -> Self {
        if pos == 0 {
            Self::Home
        } else if pos <= emby {
            Self::EmbyLibrary(pos - 1)
        } else if pos <= emby + audio {
            Self::AudiobookshelfLibrary(pos - emby - 1)
        } else if feeds {
            Self::Feeds
        } else {
            Self::Home
        }
    }

    pub(super) fn to_position_with_counts(self, emby: usize, feeds_pos: Option<usize>) -> usize {
        match self {
            Self::Home => 0,
            Self::EmbyLibrary(i) => i + 1,
            Self::AudiobookshelfLibrary(i) => emby + i + 1,
            Self::Feeds => feeds_pos.unwrap_or(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feeds_emby_library_index_is_none() {
        assert_eq!(TabSelection::Feeds.emby_library_index(), None);
    }

    /// The count-aware mapping must assign every visible Home / Emby /
    /// Audiobookshelf / Feeds destination a unique position and round-trip it,
    /// for zero, one, and multiple libraries from each Remote Service.
    #[test]
    fn count_aware_mixed_strip_positions_are_unique_and_round_trip() {
        for emby in 0..=2 {
            for audio in 0..=2 {
                for feeds in [false, true] {
                    let total = 1 + emby + audio + usize::from(feeds);
                    let feeds_pos = feeds.then_some(total - 1);
                    let mut seen: Vec<TabSelection> = Vec::new();
                    for pos in 0..total {
                        let sel = TabSelection::from_position_with_counts(pos, emby, audio, feeds);
                        assert!(
                            !seen.contains(&sel),
                            "position {pos} collides for {sel:?} with emby={emby} audio={audio} feeds={feeds}"
                        );
                        seen.push(sel);
                        assert_eq!(
                            sel.to_position_with_counts(emby, feeds_pos),
                            pos,
                            "round trip failed for {sel:?} at {pos} (emby={emby} audio={audio} feeds={feeds})"
                        );
                    }
                }
            }
        }
    }

    /// Documents the displayed strip order: Home, then Emby libraries, then
    /// Audiobookshelf libraries, then Feeds, with no shared positions.
    #[test]
    fn count_aware_strip_orders_emby_then_audiobookshelf_then_feeds() {
        let emby = 2;
        let audio = 2;
        let feeds_pos = Some(5);
        assert_eq!(
            TabSelection::Home.to_position_with_counts(emby, feeds_pos),
            0
        );
        assert_eq!(
            TabSelection::EmbyLibrary(0).to_position_with_counts(emby, feeds_pos),
            1
        );
        assert_eq!(
            TabSelection::EmbyLibrary(1).to_position_with_counts(emby, feeds_pos),
            2
        );
        assert_eq!(
            TabSelection::AudiobookshelfLibrary(0).to_position_with_counts(emby, feeds_pos),
            3
        );
        assert_eq!(
            TabSelection::AudiobookshelfLibrary(1).to_position_with_counts(emby, feeds_pos),
            4
        );
        assert_eq!(
            TabSelection::Feeds.to_position_with_counts(emby, feeds_pos),
            5
        );
        assert_eq!(
            TabSelection::from_position_with_counts(3, emby, audio, true),
            TabSelection::AudiobookshelfLibrary(0)
        );
    }
}
