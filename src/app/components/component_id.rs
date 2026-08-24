//! `ComponentId` and its identity-key payloads (design D3).

use mbv_core::config::ServiceKind;

/// Flat-registry identity for an interactive component. Stable across re-renders
/// so an inactive destination keeps its private state (design D3).
///
/// `Application<ComponentId, Msg, UserEvent>` requires `ComponentId` to be
/// `Eq`, `Hash`, and `Clone`; the convenience `Debug`/`PartialEq` derives aid
/// diagnostics.
/// Not `Copy` -- `BrowserKey` carries a `String` library id (matching the
/// plain-`String` identifiers already used throughout this app, e.g.
/// `BrowseLevel::parent_id`), so callers `.clone()` a `ComponentId` where
/// they used to copy it.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum ComponentId {
    UiRoot,
    Playback,
    Queue,
    Library,
    Home,
    Browser(BrowserKey),
    Feeds,
    InlineSearch(BrowserKey),
    Overlay(OverlayId),
    Modal(ModalId),
    Popup(PopupId),
}

/// Composite Service-library browser key (design D3):
/// `{ service, library_id, kind }`. Identifies one browser instance across
/// re-renders and Service reconnects; two libraries with the same name on
/// different Services (or an Emby library id colliding with an
/// Audiobookshelf one) still key distinctly.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BrowserKey {
    pub service: ServiceKind,
    pub library_id: String,
    pub kind: BrowserKind,
}

/// Behavioural category of one Emby library, mirroring the branches already
/// keyed off `collection_type` throughout `src/app` (e.g.
/// `library_browse_actions.rs`, `music_actions.rs`, `feed_actions.rs`) --
/// this enum does not introduce a new taxonomy, it names the one the app
/// already runtime-dispatches on. Tasks 3.5 (this one) owns `Generic`,
/// `Movies`, `HomeVideos`; `TvShows` and `Music` are owned by tasks 4.2 and
/// 4.3 respectively and are named here only so `BrowserKey` has one stable
/// key shape across all of them.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BrowserKind {
    Generic,
    Movies,
    TvShows,
    Music,
    HomeVideos,
}

impl BrowserKind {
    /// Maps an Emby `collection_type` string (e.g. `EmbyItem.collection_type`)
    /// to its `BrowserKind`, matching the string comparisons already used at
    /// each call site above. Unrecognized/empty values (e.g. `"boxsets"`,
    /// `"mixed"`, `"playlists"`) fall back to `Generic`, matching those call
    /// sites' existing behaviour of treating anything not explicitly listed
    /// as a generic library.
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
mod browser_kind_tests {
    use super::BrowserKind;

    #[test]
    fn maps_known_collection_types() {
        assert_eq!(
            BrowserKind::from_collection_type("movies"),
            BrowserKind::Movies
        );
        assert_eq!(
            BrowserKind::from_collection_type("tvshows"),
            BrowserKind::TvShows
        );
        assert_eq!(
            BrowserKind::from_collection_type("music"),
            BrowserKind::Music
        );
        assert_eq!(
            BrowserKind::from_collection_type("homevideos"),
            BrowserKind::HomeVideos
        );
    }

    #[test]
    fn unrecognized_collection_types_fall_back_to_generic() {
        assert_eq!(
            BrowserKind::from_collection_type("boxsets"),
            BrowserKind::Generic
        );
        assert_eq!(
            BrowserKind::from_collection_type("mixed"),
            BrowserKind::Generic
        );
        assert_eq!(BrowserKind::from_collection_type(""), BrowserKind::Generic);
    }
}

/// Top-level overlay identity (design D3 names: Search, Settings, Sessions,
/// Playlists, Help, ContextMenu, SelectionModal).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum OverlayId {
    Search,
    Settings,
    Sessions,
    Playlists,
    Help,
    ContextMenu,
    SelectionModal,
}

/// Blocking modal identity (design D3 names: Confirm, DaemonLost,
/// RemoteReanchor, SavePlaylist).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ModalId {
    Confirm,
    DaemonLost,
    RemoteReanchor,
    SavePlaylist,
}

/// Nested Settings popup identity (design D3 names: Multiselect,
/// LibraryRoutes, FeedManage).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum PopupId {
    Multiselect,
    LibraryRoutes,
    FeedManage,
}
