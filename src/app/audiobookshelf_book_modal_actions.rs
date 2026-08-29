use super::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
use super::types_selection_modal::{
    SelectionModalItem, SelectionModalListState, SelectionModalRow, SelectionModalSource,
};
use super::App;

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
    /// Narrow parent activation opens the chapter modal while its inline hero
    /// is admitted; a cannot-fit hero restores ordinary book activation.
    /// The narrow/wide discriminator and hero presence come from the
    /// component-reported geometry mirror (2.1j), not the removed legacy
    /// `inline_hero_area`.
    pub(super) fn activate_audiobookshelf_book_parent(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let narrow_with_hero = !self.layout.main.is_wide_book_active()
            && self.layout.main.hero_area.width > 0
            && self.layout.main.hero_area.height > 0;
        if narrow_with_hero {
            self.open_audiobookshelf_book_selection_modal();
        } else {
            self.play_selected_audiobookshelf_book(index);
        }
    }

    /// Opens the narrow book chapter modal. Rows reuse the same chapter or
    /// audio-file fallback data and duration metadata as the persistent book
    /// list; the modal only changes the interaction surface.
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
