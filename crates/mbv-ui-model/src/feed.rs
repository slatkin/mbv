use mbv_emby_model::EmbyItem;
use mbv_feed::IdleFeedItem;
use std::sync::mpsc;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct FeedHomeVideoGroup {
    pub folder: EmbyItem,
    pub items: Vec<EmbyItem>,
}

#[derive(Clone, Debug, Default)]
pub struct FeedHomeVideoState {
    pub all_items: Vec<EmbyItem>,
    pub groups: Vec<FeedHomeVideoGroup>,
    pub loading: bool,
    pub selected_group: usize,
    pub video_cursor: usize,
    pub video_scroll: usize,
}

impl FeedHomeVideoState {
    /// Clamped selected-group index: 0 means "all items", 1-based otherwise.
    /// Centralizes the `selected_group.min(groups.len())` clamp so it can't
    /// drift between the several call sites that need it.
    #[must_use]
    pub fn selected_group_index(&self) -> usize {
        self.selected_group.min(self.groups.len())
    }

    /// Length of the currently selected item list, without cloning it.
    #[must_use]
    pub fn selected_len(&self) -> usize {
        let group = self.selected_group_index();
        if group == 0 {
            self.all_items.len()
        } else {
            self.groups.get(group - 1).map_or(0, |g| g.items.len())
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SavePlaylistStage {
    EnterName,
    RenamePlaylist { id: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct SavePlaylistDialog {
    pub input: String,
    pub stage: SavePlaylistStage,
}

#[derive(Debug)]
pub struct IdleFeed {
    pub items: Vec<IdleFeedItem>,
    pub current_index: usize,
    pub last_rotation: Instant,
    pub last_fetch: Instant,
    pub items_tx: mpsc::Sender<Vec<IdleFeedItem>>,
    pub items_rx: mpsc::Receiver<Vec<IdleFeedItem>>,
}
