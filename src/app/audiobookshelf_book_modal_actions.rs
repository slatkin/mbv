use super::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
use super::types_selection_modal::{
    SelectionModalItem, SelectionModalListState, SelectionModalRow, SelectionModalSource,
};
use super::App;

#[allow(dead_code)]
pub(super) fn book_modal_state(
    state: &AudiobookshelfBookBrowseState,
    book_id: &str,
) -> SelectionModalListState {
    match state.detail_cache.get(book_id) {
        Some((chapters, _)) if !chapters.is_empty() => SelectionModalListState::Ready(
            chapters
                .iter()
                .map(|chapter| {
                    let seconds = (chapter.end - chapter.start).max(0.0) as i64;
                    SelectionModalRow::Item(SelectionModalItem {
                        name: chapter.title.clone(),
                        meta: if seconds > 0 {
                            crate::app::ui_util::fmt_duration_approx(seconds)
                        } else {
                            String::new()
                        },
                        id: format!("chapter:{}", chapter.id),
                    })
                })
                .collect(),
        ),
        Some((_, files)) if !files.is_empty() => SelectionModalListState::Ready(
            files
                .iter()
                .map(|file| {
                    SelectionModalRow::Item(SelectionModalItem {
                        name: format!("Part {}", file.index),
                        meta: crate::app::ui_util::fmt_duration_approx(file.duration as i64),
                        id: format!("audio-file:{}", file.ino),
                    })
                })
                .collect(),
        ),
        Some(_) => SelectionModalListState::Empty,
        None if state.detail_loading_ids.contains(book_id) => SelectionModalListState::Loading,
        None => SelectionModalListState::Empty,
    }
}

impl App {
    /// Wide parent activation plays the selected book directly. Non-Wide
    /// activation is handled by the mounted Books owner and Library panel;
    /// this compatibility entry point intentionally has no modal side effect.
    #[allow(dead_code)]
    pub(super) fn activate_audiobookshelf_book_parent(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        if self.is_right_panel_wide() {
            self.play_selected_audiobookshelf_book(index);
        }
        // The mounted Books owner opens the Library Hero overlay on the
        // non-Wide path; this legacy App entry point must not construct a
        // constituent selection modal.
    }

    /// Opens the narrow book chapter modal. Rows reuse the same chapter or
    /// audio-file fallback data and duration metadata as the persistent book
    /// list; the modal only changes the interaction surface.
    #[allow(dead_code)]
    pub(super) fn open_audiobookshelf_book_selection_modal(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some((book_id, title)) = self
            .audiobookshelf_book_browse
            .get(index)
            .and_then(|state| {
                let id = state.selected_id.as_deref()?;
                let title = state.selected_book()?.title.clone();
                Some((id.to_string(), title))
            })
        else {
            return;
        };
        self.start_audiobookshelf_book_detail(book_id.clone());
        let Some(state) = self
            .audiobookshelf_book_browse
            .get(index)
            .map(|state| book_modal_state(state, &book_id))
        else {
            return;
        };
        self.open_selection_modal(SelectionModalSource::Book { book_id }, title, state, None);
    }
}
