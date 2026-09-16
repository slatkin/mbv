use super::{ShellRequest, TvContent};
use crate::app::components::media_list::MediaListSurfaceInput;

impl TvContent {
    pub(super) fn move_episode(&mut self, delta: i64) {
        self.episodes.delegate_operation(
            MediaListSurfaceInput::Move(delta)
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
    }

    /// Delegate one pager/jump surface input to the episode owner's own list
    /// -- its page stride and its ends, the same seam the movement chords
    /// use -- and report the applied cursor movement as the component-
    /// resolved `TvEpisodeMove` delta; the shell never recomputes it.
    pub(super) fn move_episode_by(&mut self, input: MediaListSurfaceInput) -> ShellRequest {
        let from = self.episodes.cursor();
        self.episodes.delegate_operation(
            input
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
        ShellRequest::TvEpisodeMove {
            delta: self.episodes.cursor() as i64 - from as i64,
        }
    }

    pub(super) fn move_season(&mut self, delta: i64) {
        let count = self
            .context
            .series_detail
            .as_ref()
            .map_or(0, |detail| detail.seasons.len());
        if count > 0 {
            self.season_cursor =
                (self.season_cursor as i64 + delta).rem_euclid(count as i64) as usize;
            self.refresh_episode_rows();
            self.episodes.select_first();
        }
    }

    /// Move the series cursor by `rows` selectable rows through the common
    /// delegation seam (Wide only). The shared owner is the sole source of
    /// truth for the one-column rail -- the legacy layout row map holds
    /// display-row indices (headings included), not selectable cursor
    /// indices, so consulting it made Down jump across grouped rows.
    pub(super) fn move_rows(&mut self, rows: i64) {
        self.carrier.delegate_operation(
            MediaListSurfaceInput::Move(rows)
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
    }

    pub(super) fn jump_cursor(&mut self, to_end: bool) {
        let input = if to_end {
            MediaListSurfaceInput::Last
        } else {
            MediaListSurfaceInput::First
        };
        self.carrier.delegate_operation(
            input
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
    }

    /// Painted item rows the Narrow pager moves per PageUp/PageDown: the
    /// fixed-row list strides one selectable row per painted row.
    pub(super) fn narrow_page_rows(&self) -> i64 {
        self.painted_viewport_height().saturating_sub(1).max(1) as i64
    }

    /// Move the shared owner by `item_rows` painted item rows (Narrow only)
    /// and report the resulting selection as a `context.list.items` index,
    /// the position the shell's `EmbyLibraryCursorIndex` effect persists into
    /// the resting `BrowseLevel` cursor.
    pub(super) fn move_by_item_rows_narrow(&mut self, item_rows: i64) -> usize {
        self.carrier.move_selection(item_rows);
        self.carrier.sync_viewport(self.painted_viewport_height());
        self.browse_cursor()
    }

    /// Home/End select the first/last target in the shared owner (Narrow
    /// only), reporting the same raw index `move_by_item_rows_narrow` does.
    pub(super) fn jump_cursor_narrow(&mut self, to_end: bool) -> usize {
        if to_end {
            self.carrier.select_last();
        } else {
            self.carrier.select_first();
        }
        if self.carrier.selected_target().is_some() {
            self.carrier.sync_viewport(self.painted_viewport_height());
        }
        self.browse_cursor()
    }
}
