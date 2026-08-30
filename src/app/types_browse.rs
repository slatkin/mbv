use mbv_core::api::EmbyItem;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AlbumPathPart {
    pub(super) id: String,
    pub(super) name: String,
}

#[derive(Clone, Debug)]
pub(super) struct AlbumSearchEntry {
    pub(super) album: EmbyItem,
    pub(super) ancestors: Vec<AlbumPathPart>,
    pub(super) display_label: String,
    pub(super) search_text: String,
}

#[derive(Clone, Debug)]
pub(super) enum AlbumIndexState {
    Unavailable,
    Loading { rebuild_pending: bool },
    Ready(Vec<AlbumSearchEntry>),
}

/// TV series detail data for inline rendering.
/// When a Series is selected, we proactively fetch seasons and episodes
/// so the inline detail pane can render without drilling in.
#[derive(Clone, Debug)]
pub(super) struct SeriesDetail {
    pub(super) seasons: Vec<EmbyItem>,
    pub(super) episodes: std::collections::HashMap<String, Vec<EmbyItem>>,
}

/// Where a browse level rests when it is *not* the visible one: the cursor and
/// viewport scroll the shell restores on re-entry and serializes into
/// `LibraryPosition`. This is a distinct fact from the live cursor the mounted
/// component owns while the level is on screen (see `design.md` D1); reading it
/// through this type rather than the raw fields keeps the two uses from being
/// spelled identically.
///
/// The resting values are owned directly by each `BrowseLevel` and are used
/// for persistence and re-entry when the level is not visible.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct BrowseResting {
    cursor: usize,
    scroll: usize,
}

impl BrowseResting {
    pub(super) fn new(cursor: usize, scroll: usize) -> Self {
        Self { cursor, scroll }
    }

    pub(super) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(super) fn scroll(&self) -> usize {
        self.scroll
    }
}

pub(super) struct BrowseLevel {
    pub(super) parent_id: String,
    pub(super) title: String,
    pub(super) items: Vec<EmbyItem>,
    pub(super) total_count: usize,
    pub(super) resting: BrowseResting,
    pub(super) item_types: Option<String>,
    pub(super) unplayed_only: bool,
    pub(super) sort_by: String,
    pub(super) sort_order: String,
    pub(super) loading: bool,
    pub(super) all_items: Option<Vec<EmbyItem>>, // prefetched full list for instant search
    /// Active letter-range pill scope for a large library's top browse level
    /// (`None` = unfiltered). See `render::LetterFilter`.
    pub(super) letter_filter: Option<crate::app::render::LetterFilter>,
    /// Grouping lifecycle state for a music album level (candidate +
    /// settled catalog). `None` for non-music or non-album levels.
    pub(super) music_grouping: Option<super::music_grouping::MusicGroupingState>,
}

impl BrowseLevel {
    /// Whether every item reported by the server for this level has been
    /// fetched into `items` (i.e. pagination is complete).
    pub(super) fn is_fully_loaded(&self) -> bool {
        self.items.len() >= self.total_count
    }

    pub(super) fn from_position_level(
        saved: &crate::config::LibraryPositionLevel,
        items: Vec<EmbyItem>,
        total_count: usize,
        visible_rows: usize,
    ) -> Self {
        let cursor = saved
            .focused_item_id
            .as_ref()
            .and_then(|id| items.iter().position(|item| &item.id == id))
            .unwrap_or_else(|| saved.cursor_index.min(items.len().saturating_sub(1)));
        let scroll = Self::scroll_for_cursor(cursor, visible_rows);
        Self {
            parent_id: saved.parent_id.clone(),
            title: saved.title.clone(),
            items,
            total_count,
            resting: BrowseResting::new(cursor, scroll),
            item_types: saved.item_types.clone(),
            unplayed_only: saved.unplayed_only,
            sort_by: saved.sort_by.clone(),
            sort_order: saved.sort_order.clone(),
            loading: false,
            all_items: None,
            letter_filter: saved
                .letter_filter_index
                .and_then(crate::app::render::LetterFilter::for_index),
            music_grouping: None,
        }
    }

    /// The level's resting cursor/scroll — the persistence-facing view of its
    /// position, distinct from the live component cursor (`design.md` D1).
    pub(super) fn resting(&self) -> BrowseResting {
        self.resting
    }

    pub(super) fn set_resting_cursor(&mut self, cursor: usize) {
        self.resting.cursor = cursor;
    }

    pub(super) fn set_resting_scroll(&mut self, scroll: usize) {
        self.resting.scroll = scroll;
    }

    pub(super) fn to_position_level(&self) -> crate::config::LibraryPositionLevel {
        let resting = self.resting();
        crate::config::LibraryPositionLevel {
            parent_id: self.parent_id.clone(),
            title: self.title.clone(),
            focused_item_id: self.items.get(resting.cursor()).map(|item| item.id.clone()),
            cursor_index: resting.cursor(),
            item_types: self.item_types.clone(),
            unplayed_only: self.unplayed_only,
            sort_by: self.sort_by.clone(),
            sort_order: self.sort_order.clone(),
            letter_filter_index: self.letter_filter.as_ref().map(|f| f.index),
            // Only meaningful for the root level; `library_position_snapshot`
            // (the `LibraryTab` method) fills this in for `levels[0]` from
            // `LibraryTab.library_total` after collecting all levels here.
            library_total: None,
        }
    }

    pub(super) fn scroll_for_cursor(cursor: usize, visible_rows: usize) -> usize {
        if visible_rows == 0 || cursor < visible_rows {
            0
        } else {
            cursor + 1 - visible_rows
        }
    }
}

pub(super) fn restore_library_position<F>(
    saved: &crate::config::LibraryPosition,
    visible_rows: usize,
    mut fetch_level: F,
) -> Result<Option<(crate::config::LibraryPosition, Vec<BrowseLevel>)>, String>
where
    F: FnMut(&crate::config::LibraryPositionLevel) -> Result<(Vec<EmbyItem>, usize), String>,
{
    if saved.levels.is_empty() {
        return Ok(None);
    }

    let mut restored = crate::config::LibraryPosition {
        feed_selected_group: saved.feed_selected_group,
        feed_video_cursor: saved.feed_video_cursor,
        feed_video_scroll: saved.feed_video_scroll,
        ..Default::default()
    };
    let mut nav_stack = Vec::new();

    for (idx, saved_level) in saved.levels.iter().enumerate() {
        let (items, total_count) = fetch_level(saved_level)?;
        let level = BrowseLevel::from_position_level(saved_level, items, total_count, visible_rows);
        let can_descend = saved
            .levels
            .get(idx + 1)
            .is_some_and(|next| level.items.iter().any(|item| item.id == next.parent_id));
        let mut position_level = level.to_position_level();
        if idx == 0 {
            // `library_total` belongs to the root LibraryTab, not BrowseLevel.
            // Preserve it while rebuilding the saved position so restored
            // letter-pill views remain eligible for their selector row.
            position_level.library_total = saved_level.library_total;
        }
        restored.levels.push(position_level);
        nav_stack.push(level);
        if !can_descend {
            break;
        }
    }

    if restored.levels.is_empty() {
        Ok(None)
    } else {
        Ok(Some((restored, nav_stack)))
    }
}
