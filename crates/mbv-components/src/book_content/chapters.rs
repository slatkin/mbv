use super::{
    AudiobookshelfBookBrowseState, AudiobookshelfBookMove, BookChapterTarget, BookContent, BookRow,
    MediaKind, MediaListRow, MediaListSurfaceInput, MediaListTrailing, MediaSemanticState, Msg,
    ShellRequest, fmt_duration_gutter,
};
use mbv_queue::QueueItemContentId;
use mbv_render::components::media_list::ActiveProgress;
use mbv_ui_model::now_playing::WorkspaceNowPlaying;

/// The now-playing override (hero-workspace-now-playing D3/D4): the chapter
/// row the projection names becomes `NowPlaying { progress }`; every other
/// row keeps its derived state. The projection applies only when its book is
/// this Workspace's selected book, and the named target must resolve to a
/// projected `BookRow::Chapter` -- an audio-part row never carries the state,
/// and an `Item` or foreign-provider projection overrides nothing.
fn apply_now_playing_override(
    rows: &mut [MediaListRow<usize>],
    state: &AudiobookshelfBookBrowseState,
    now_playing: Option<&WorkspaceNowPlaying>,
) {
    let Some(WorkspaceNowPlaying::Chapter {
        content_id: QueueItemContentId::AudiobookshelfBook { library_item_id },
        chapter_id,
        progress,
    }) = now_playing
    else {
        return;
    };
    let Some(selected) = state
        .selected_id
        .as_deref()
        .filter(|id| id == library_item_id)
    else {
        return;
    };
    // `chapter_rows` projects `visible_rows` one-to-one in order, so the
    // named chapter's row is the position of its `BookRow::Chapter`; an
    // audio-part fallback's indices can never match a chapter id here.
    let Some(position) = state
        .visible_rows(selected)
        .iter()
        .position(|row| matches!(row, BookRow::Chapter { id, .. } if *id == *chapter_id))
    else {
        return;
    };
    let Some(MediaListRow::Item { semantic_state, .. }) = rows.get_mut(position) else {
        return;
    };
    *semantic_state = MediaSemanticState::NowPlaying {
        progress: Some(ActiveProgress::new(*progress)),
    };
}

/// Canonical row projection for one book's chapter/audio-part detail: one
/// selectable `Item` per visible row, keyed by its stable row discriminator.
/// Chapters fall back to audio files when the book exposes no chapters
/// (book-browsing spec: never an empty or broken list state). Rehomed from
/// the Books panel owner (task 10.1).
fn chapter_rows(state: &AudiobookshelfBookBrowseState, id: &str) -> Vec<MediaListRow<usize>> {
    state
        .visible_rows(id)
        .into_iter()
        .map(|row| {
            // The row target is the stable Service discriminator -- the chapter
            // number or the audio-part index -- never the `enumerate()` display
            // position, so a detail refresh that re-composes `visible_rows`
            // cannot resolve a stale target to a different row (design.md D4).
            let (target, primary, duration_seconds) = match row {
                BookRow::Chapter {
                    id,
                    start,
                    end,
                    title,
                } => (id, title, end - start),
                BookRow::AudioFile {
                    index, duration, ..
                } => (index, format!("Part {index}"), duration),
            };
            #[expect(
                clippy::cast_possible_truncation,
                reason = "fractional seconds truncate to whole seconds for the duration gutter; sub-second display is not wanted (approved, issue #804)"
            )]
            let trailing = (duration_seconds > 0.0)
                .then(|| fmt_duration_gutter(duration_seconds as i64))
                .map(MediaListTrailing::Gutter);
            MediaListRow::Item {
                target,
                primary,
                secondary: None,
                trailing,
                duration: None,
                kind: MediaKind::Media,
                semantic_state: MediaSemanticState::Ordinary,
            }
        })
        .collect()
}

impl BookContent {
    /// Enter the parent-owned chapter pane without moving the chapter owner's
    /// selection (design.md D5): focus is not selection.
    pub fn enter_chapter_focus(&mut self) {
        self.chapter_focused = true;
    }

    /// Leave the parent-owned chapter pane without moving the chapter owner's
    /// selection or scroll (design.md D5).
    pub fn clear_chapter_focus(&mut self) {
        self.chapter_focused = false;
    }
    /// The chapter owner's book-qualified stable target, resolved only while
    /// the chapter pane holds focus (design.md D4/D5). Activation never
    /// re-derives it from a numeric display position.
    #[must_use]
    pub fn chapter_target(&self) -> Option<BookChapterTarget> {
        if !self.chapter_focused {
            return None;
        }
        let index = *self.chapter_list.selected_target()?;
        Some(BookChapterTarget::new(
            self.state.selected_id.clone()?,
            index,
        ))
    }
    #[must_use]
    pub fn chapter_focus_request(&self) -> Msg {
        Msg::Shell(Box::new(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::ChapterFocus(self.chapter_target()),
        )))
    }
    /// Project the selected book's canonical chapter/audio-part rows into the
    /// chapter owner before view (design.md D6). The row's stable target is the
    /// chapter/audio-part discriminator; the typed `BookChapterTarget` pairs it
    /// with the selected book identity (design.md D4).
    pub fn project_chapter_rows(&mut self) {
        let mut rows = self
            .state
            .selected_id
            .as_deref()
            .map(|id| chapter_rows(&self.state, id))
            .unwrap_or_default();
        apply_now_playing_override(&mut rows, &self.state, self.now_playing.as_ref());
        if self.chapter_list.rows() != rows.as_slice() {
            self.chapter_list.set_content(rows);
        }
    }
    /// # Panics
    /// Panics if the internally generated movement input cannot be converted to a media-list operation.
    pub fn move_chapter(&mut self, delta: i64) {
        self.chapter_list.delegate_operation(
            MediaListSurfaceInput::Move(delta)
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
    }
}
