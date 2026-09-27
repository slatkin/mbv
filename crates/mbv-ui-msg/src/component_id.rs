//! `ComponentId` and its identity-key payloads (design D3).

/// Flat-registry identity for an interactive component. Stable across re-renders
/// so an inactive destination keeps its private state (design D3).
///
/// `Application<ComponentId, Msg, UserEvent>` requires `ComponentId` to be
/// `Eq`, `Hash`, and `Clone`; the convenience `Debug`/`PartialEq` derives aid
/// diagnostics.
/// Not `Copy` -- `LibraryKey` carries a `String` library id (matching the
/// plain-`String` identifiers already used throughout this app, e.g.
/// `BrowseLevel::parent_id`), so callers `.clone()` a `ComponentId` where
/// they used to copy it.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum ComponentId {
    UiRoot,
    /// The tab bar panel (`TabPanel`, task 2.1): paints the `RootFrame.tab`
    /// placement and resolves tab clicks against its own painted hit
    /// regions.
    TabPanel,
    /// The status row panel (`StatusBarPanel`, task 2.2): paints the
    /// `RootFrame.status_bar` placement and owns the volume/mute/remote
    /// pill regions.
    StatusBarPanel,
    /// The right-column playback strip (`LibraryPlaybackPanel`, task 4.1):
    /// paints the `RootFrame.library_playback` placement — mounted only when
    /// the queue column is hidden (D1's mount rule), where it is the frame's
    /// one transport — and resolves transport clicks against its own
    /// retained hit geometry.
    LibraryPlaybackPanel,
    /// The Queue playback panel (`QueuePlaybackPanel`, task 3.5): paints the
    /// `RootFrame.queue_playback` placement — the always-painted header row,
    /// the visual slot's region and the queue-column transport — and resolves
    /// transport clicks against its own retained hit geometry (task 3.7).
    QueuePlaybackPanel,
    Queue,
    QueueBoundary,
    /// The Library panel (`LibraryPanel`, task 5.9, design D2): the
    /// library area's one event boundary.
    Library,
    Overlay(OverlayId),
    Modal(ModalId),
    Popup(PopupId),
}

/// Top-level overlay identity (design D3 names: Search, Settings, Sessions,
/// Playlists, Help, and `ContextMenu`).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum OverlayId {
    Search,
    Settings,
    Sessions,
    Playlists,
    Help,
    ContextMenu,
}

/// Blocking modal identity (design D3 names: Confirm, `DaemonLost`, `SavePlaylist`).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ModalId {
    Confirm,
    DaemonLost,
    SavePlaylist,
}

/// Nested Settings popup identity (design D3 names: Multiselect,
/// `LibraryRoutes`, `FeedManage`).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum PopupId {
    Multiselect,
    LibraryRoutes,
    FeedManage,
}
