use mbv_core::api::EmbyItem;
use std::sync::mpsc;
use std::time::Instant;

#[derive(Clone)]
pub(in crate::app) struct FeedHomeVideoGroup {
    pub(in crate::app) folder: EmbyItem,
    pub(in crate::app) items: Vec<EmbyItem>,
}

#[derive(Clone, Default)]
pub(in crate::app) struct FeedHomeVideoState {
    pub(in crate::app) all_items: Vec<EmbyItem>,
    pub(in crate::app) groups: Vec<FeedHomeVideoGroup>,
    pub(in crate::app) loading: bool,
    pub(in crate::app) selected_group: usize,
    pub(in crate::app) video_cursor: usize,
    pub(in crate::app) video_scroll: usize,
}

impl FeedHomeVideoState {
    /// Clamped selected-group index: 0 means "all items", 1-based otherwise.
    /// Centralizes the `selected_group.min(groups.len())` clamp so it can't
    /// drift between the several call sites that need it.
    pub(in crate::app) fn selected_group_index(&self) -> usize {
        self.selected_group.min(self.groups.len())
    }

    /// Length of the currently selected item list, without cloning it.
    pub(in crate::app) fn selected_len(&self) -> usize {
        let group = self.selected_group_index();
        if group == 0 {
            self.all_items.len()
        } else {
            self.groups.get(group - 1).map_or(0, |g| g.items.len())
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum SavePlaylistStage {
    EnterName,
    RenamePlaylist { id: String },
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct SavePlaylistDialog {
    pub(in crate::app) input: String,
    pub(in crate::app) stage: SavePlaylistStage,
}

pub(in crate::app) struct IdleFeedItem {
    pub(in crate::app) title: String,
    pub(in crate::app) link: Option<String>,
}

pub(in crate::app) struct IdleFeed {
    pub(in crate::app) items: Vec<IdleFeedItem>,
    pub(in crate::app) current_index: usize,
    pub(in crate::app) last_rotation: Instant,
    pub(in crate::app) last_fetch: Instant,
    pub(in crate::app) items_tx: mpsc::Sender<Vec<IdleFeedItem>>,
    pub(in crate::app) items_rx: mpsc::Receiver<Vec<IdleFeedItem>>,
}
