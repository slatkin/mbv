use mbv_core::api::MediaItem;

#[derive(Clone)]
pub(super) struct FeedHomeVideoGroup {
    pub(super) folder: MediaItem,
    pub(super) items: Vec<MediaItem>,
}

#[derive(Clone, Default)]
pub(super) struct FeedHomeVideoState {
    pub(super) all_items: Vec<MediaItem>,
    pub(super) groups: Vec<FeedHomeVideoGroup>,
    pub(super) loading: bool,
    pub(super) selected_group: usize,
    pub(super) video_cursor: usize,
    pub(super) video_scroll: usize,
}

impl FeedHomeVideoState {
    /// Clamped selected-group index: 0 means "all items", 1-based otherwise.
    /// Centralizes the `selected_group.min(groups.len())` clamp so it can't
    /// drift between the several call sites that need it.
    pub(super) fn selected_group_index(&self) -> usize {
        self.selected_group.min(self.groups.len())
    }

    /// Length of the currently selected item list, without cloning it.
    pub(super) fn selected_len(&self) -> usize {
        let group = self.selected_group_index();
        if group == 0 {
            self.all_items.len()
        } else {
            self.groups
                .get(group - 1)
                .map(|g| g.items.len())
                .unwrap_or(0)
        }
    }
}

pub(super) enum SavePlaylistStage {
    EnterName,
    ConfirmOverwrite { existing_id: String },
}

pub(super) struct SavePlaylistDialog {
    pub(super) input: String,
    pub(super) stage: SavePlaylistStage,
}
