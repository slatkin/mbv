use mbv_config::TabIdentity;
use mbv_queue::ServiceKind;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LibraryKind {
    Generic,
    Movies,
    TvShows,
    Music,
    HomeVideos,
    AudiobookshelfPodcast,
    AudiobookshelfBook,
}

impl LibraryKind {
    pub fn from_collection_type(collection_type: &str) -> Self {
        match collection_type {
            "movies" => Self::Movies,
            "tvshows" => Self::TvShows,
            "music" => Self::Music,
            "homevideos" => Self::HomeVideos,
            _ => Self::Generic,
        }
    }
}

#[cfg(test)]
mod library_kind_tests {
    use super::LibraryKind;

    #[test]
    fn maps_known_collection_types() {
        assert_eq!(
            LibraryKind::from_collection_type("movies"),
            LibraryKind::Movies
        );
        assert_eq!(
            LibraryKind::from_collection_type("tvshows"),
            LibraryKind::TvShows
        );
        assert_eq!(
            LibraryKind::from_collection_type("music"),
            LibraryKind::Music
        );
        assert_eq!(
            LibraryKind::from_collection_type("homevideos"),
            LibraryKind::HomeVideos
        );
    }

    #[test]
    fn unrecognized_collection_types_fall_back_to_generic() {
        assert_eq!(
            LibraryKind::from_collection_type("boxsets"),
            LibraryKind::Generic
        );
        assert_eq!(
            LibraryKind::from_collection_type("mixed"),
            LibraryKind::Generic
        );
        assert_eq!(LibraryKind::from_collection_type(""), LibraryKind::Generic);
    }

    /// Task 2.1: every `LibraryKey` maps to a stable launch-state tab
    /// identity. Fixed tabs map to themselves; every `LibraryKind`
    /// maps through with its Service kind plus library ID intact (the
    /// browse kind selects the pill/item interpretation, never the tab).
    /// Uses `super::*` so the mapping test reads the same names as the
    /// contract it pins.
    #[test]
    fn tab_identity_covers_every_key_shape() {
        use super::*;

        assert_eq!(LibraryKey::Home.tab_identity(), TabIdentity::Home);
        assert_eq!(LibraryKey::Feeds.tab_identity(), TabIdentity::Feeds);
        for kind in [
            LibraryKind::Generic,
            LibraryKind::Movies,
            LibraryKind::TvShows,
            LibraryKind::Music,
            LibraryKind::HomeVideos,
            LibraryKind::AudiobookshelfPodcast,
            LibraryKind::AudiobookshelfBook,
        ] {
            assert_eq!(
                LibraryKey::Service {
                    service: ServiceKind::Emby,
                    library_id: "lib-1".to_string(),
                    kind,
                }
                .tab_identity(),
                TabIdentity::ServiceLibrary {
                    kind: ServiceKind::Emby,
                    library_id: "lib-1".to_string(),
                },
                "kind {kind:?} must map through"
            );
        }
        assert_eq!(
            LibraryKey::Service {
                service: ServiceKind::Audiobookshelf,
                library_id: "abs-lib".to_string(),
                kind: LibraryKind::AudiobookshelfPodcast,
            }
            .tab_identity(),
            TabIdentity::ServiceLibrary {
                kind: ServiceKind::Audiobookshelf,
                library_id: "abs-lib".to_string(),
            }
        );
    }
}

/// The identity of one library destination, keying the panel's owner map.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum LibraryKey {
    Home,
    Feeds,
    Service {
        service: ServiceKind,
        library_id: String,
        kind: LibraryKind,
    },
}

impl LibraryKey {
    /// The stable launch-state tab identity for this destination (task 2.1).
    /// Every [`LibraryKind`] maps through — the tab identity carries only
    /// the Service kind plus library ID, so the browse kind selects the
    /// destination-tagged pill/item interpretation, never the tab.
    // Consumed by selected-tab teardown assembly in task 2.3.
    pub fn tab_identity(&self) -> TabIdentity {
        match self {
            Self::Home => TabIdentity::Home,
            Self::Feeds => TabIdentity::Feeds,
            Self::Service {
                service,
                library_id,
                ..
            } => TabIdentity::ServiceLibrary {
                kind: *service,
                library_id: library_id.clone(),
            },
        }
    }
}
