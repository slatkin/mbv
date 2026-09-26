use super::{
    fmt_duration_gutter, AudiobookshelfBookBrowseState, AudiobookshelfBookMove, BookChapterTarget,
    BookContent, BookRow, MediaKind, MediaListRow, MediaListSurfaceInput, MediaListTrailing,
    MediaSemanticState, Msg, ShellRequest,
};

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
                reason = "seconds↔ticks conversion through f64; no lossless integer-path conversion exists (approved, issue #804)"
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
    pub(in crate::app) fn enter_chapter_focus(&mut self) {
        self.chapter_focused = true;
    }

    /// Leave the parent-owned chapter pane without moving the chapter owner's
    /// selection or scroll (design.md D5).
    pub(in crate::app) fn clear_chapter_focus(&mut self) {
        self.chapter_focused = false;
    }
    /// The chapter owner's book-qualified stable target, resolved only while
    /// the chapter pane holds focus (design.md D4/D5). Activation never
    /// re-derives it from a numeric display position.
    pub(in crate::app) fn chapter_target(&self) -> Option<BookChapterTarget> {
        if !self.chapter_focused {
            return None;
        }
        let index = *self.chapter_list.selected_target()?;
        Some(BookChapterTarget::new(
            self.state.selected_id.clone()?,
            index,
        ))
    }
    pub(in crate::app) fn chapter_focus_request(&self) -> Msg {
        Msg::Shell(Box::new(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::ChapterFocus(self.chapter_target()),
        )))
    }
    /// Project the selected book's canonical chapter/audio-part rows into the
    /// chapter owner before view (design.md D6). The row's stable target is the
    /// chapter/audio-part discriminator; the typed `BookChapterTarget` pairs it
    /// with the selected book identity (design.md D4).
    pub(super) fn project_chapter_rows(&mut self) {
        let rows = self
            .state
            .selected_id
            .as_deref()
            .map(|id| chapter_rows(&self.state, id))
            .unwrap_or_default();
        if self.chapter_list.rows() != rows.as_slice() {
            self.chapter_list.set_content(rows);
        }
    }
    pub(in crate::app) fn move_chapter(&mut self, delta: i64) {
        self.chapter_list.delegate_operation(
            MediaListSurfaceInput::Move(delta)
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
    }
}
