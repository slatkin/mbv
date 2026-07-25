use super::types_playback::ArtistHeaderSelection;
use mbv_core::api::MediaItem;

#[derive(Clone, Debug)]
pub(super) enum ContextAction {
    Play,
    PlayFolder(String),
    ShuffleFolder(String),
    PlayArtistHeader(ArtistHeaderSelection),
    ShuffleArtistHeader(ArtistHeaderSelection),
    EnqueueArtistHeader(ArtistHeaderSelection),
    Enqueue,
    EnqueueFolder(Box<MediaItem>),
    MarkPlayed(String),
    MarkItemsPlayed(Vec<String>),
    MarkUnplayed(String),
    MarkItemsUnplayed(Vec<String>),
    RemoveFromContinueWatching,
    RemoveFromQueue(usize),
    GoToLibrary(String, String), // (item_id, item_type)
}

pub(super) struct ContextMenuEntry {
    pub(super) label: &'static str,
    pub(super) action: Option<ContextAction>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MultiSelectKind {
    HiddenLibraries,
    HiddenLatest,
    MyLanguages,
    FeedViewLibraries,
}

pub(super) struct MultiSelectPopup {
    pub(super) kind: MultiSelectKind,
    pub(super) items: Vec<(String, String, bool)>, // (name_lower, display_name, is_hidden)
    pub(super) cursor: usize,
}

#[derive(Clone)]
pub(crate) enum LibraryRouteStage {
    /// (library_name_lower, display_name, current_device_or_none)
    PickLibrary {
        items: Vec<(String, String, Option<String>)>,
    },
    /// index 0 is always the synthetic "Local (no route)" entry.
    /// Each entry pairs a device's display name (UX only -- #256 never
    /// persists it) with its live-resolved endpoint (what actually gets
    /// written to config on commit). `None` means the device is visible
    /// in the live session list but session_direct_endpoint couldn't
    /// resolve it to a connectable address (e.g. no advertised
    /// direct-connect port, or an unparseable host) -- shown greyed out
    /// with a reason rather than silently omitted, and not committable.
    PickDevice {
        library_lower: String,
        library_display: String,
        devices: Vec<(String, Option<mbv_core::remote_player::DaemonEndpoint>)>,
    },
}

pub(crate) struct LibraryRoutePopup {
    pub(super) stage: LibraryRouteStage,
    pub(super) cursor: usize,
}

pub(super) struct ContextMenu {
    pub(super) x: u16,
    pub(super) y: u16,
    pub(super) entries: Vec<ContextMenuEntry>,
    pub(super) cursor: usize,
}

impl ContextMenu {
    pub(super) fn first_selectable(entries: &[ContextMenuEntry]) -> usize {
        entries
            .iter()
            .position(|entry| entry.action.is_some())
            .unwrap_or(0)
    }

    pub(super) fn move_cursor(&mut self, delta: i64) {
        if self.entries.is_empty() {
            return;
        }
        let mut idx = self.cursor as i64;
        loop {
            let next = idx + delta;
            if next < 0 || next >= self.entries.len() as i64 {
                return;
            }
            idx = next;
            if self.entries[idx as usize].action.is_some() {
                self.cursor = idx as usize;
                return;
            }
        }
    }
}
