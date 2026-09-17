use super::super::super::components::msg::TerminalObserverEvent;
use super::super::media_list::{MediaListDisposition, MediaListSurfaceInput};
use super::super::Msg;
use super::{ShellRequest, TvContent};

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
    /// resolved `TvEpisodeMove` delta; the shell never recomputes it. A
    /// window-only viewport step (design D8) reports none: the overlay's
    /// unhandled-chord claim keeps the key overlay-local.
    pub(super) fn move_episode_by(&mut self, input: MediaListSurfaceInput) -> Option<ShellRequest> {
        let from = self.episodes.cursor();
        self.episodes.delegate_operation(
            input
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
        let delta = self.episodes.cursor() as i64 - from as i64;
        (delta != 0).then_some(ShellRequest::TvEpisodeMove { delta })
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

    /// Move the shared owner by `item_rows` painted item rows (Narrow only)
    /// and report the resulting selection as a `context.list.items` index,
    /// the position the shell's `EmbyLibraryCursorIndex` effect persists into
    /// the resting `BrowseLevel` cursor.
    pub(super) fn move_by_item_rows_narrow(&mut self, item_rows: i64) -> usize {
        self.carrier.move_selection(item_rows);
        self.carrier.sync_viewport(self.painted_viewport_height());
        self.browse_cursor()
    }

    /// A keyboard viewport step on the series rail (design D6/D7): the
    /// shared owner's window moves by a page (`Page`) or one display row
    /// (`ScrollViewport`), and the selection rides only when the step would
    /// leave it outside. The echo reports an actual selection move (design
    /// D8): Wide reports the applied `TvMoveRows` delta, Narrow the persisted
    /// `EmbyLibraryCursorIndex`; a window-only step is a consumed key with
    /// no shell effect — its reached position persists through the panel's
    /// deferred resting-scroll update.
    pub(super) fn viewport_step_rows(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        let from = self.carrier.cursor();
        let outcome = self.carrier.delegate_operation(
            input
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
        if outcome.selected_target.is_some() {
            if self.is_wide {
                Some(Msg::Shell(ShellRequest::TvMoveRows {
                    rows: self.carrier.cursor() as i64 - from as i64,
                }))
            } else {
                Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.browse_cursor(),
                }))
            }
        } else if outcome.disposition == MediaListDisposition::Consumed {
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        } else {
            None
        }
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
