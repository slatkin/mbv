use super::components::book_content::BookContent;
use super::components::library_panel::LibraryKey;
use super::components::msg::{AudiobookshelfBookIntent, AudiobookshelfBookMove, ShellRequest};
use super::components::LibraryKind;
use super::Model;
use super::TabSelection;
use crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseKind;
use mbv_core::config::ServiceKind;

impl Model {
    fn abs_book_key(&self) -> Option<LibraryKey> {
        let TabSelection::AudiobookshelfLibrary(index) = self.app.tab else {
            return None;
        };
        (matches!(
            self.app.audiobookshelf_kind_at(index),
            Some(AudiobookshelfBrowseKind::Book)
        ))
        .then(|| {
            let library = self.app.audiobookshelf_libraries.get(index)?;
            Some(LibraryKey::Service {
                service: ServiceKind::Audiobookshelf,
                library_id: library.id.clone(),
                kind: LibraryKind::AudiobookshelfBook,
            })
        })?
    }

    pub fn abs_book_owner(&self) -> Option<&BookContent> {
        let key = self.abs_book_key()?;
        self.library_owner(&key)
    }

    fn update_abs_book_owner<R>(&mut self, f: impl FnOnce(&mut BookContent) -> R) -> Option<R> {
        let key = self.abs_book_key()?;
        self.update_library_owner(&key, || Box::new(BookContent::new()), f)
    }

    pub(in crate::app) fn push_audiobookshelf_book_content(&mut self) {
        if self.abs_book_key().is_none() {
            return;
        }
        let TabSelection::AudiobookshelfLibrary(index) = self.app.tab else {
            return;
        };
        let Some(snapshot) = self.app.audiobookshelf_book_browse.get(index).cloned() else {
            return;
        };
        let focused = matches!(self.app.effective_panel_focus(), super::PanelFocus::Library);
        let images_enabled = self.app.images_enabled();
        self.update_abs_book_owner(|owner| {
            owner.set_content(&snapshot, images_enabled);
            owner.set_focused(focused);
        });
    }

    pub(in crate::app) fn sync_audiobookshelf_book() {
        // Books are retained as a LibraryPanel owner for the lifetime of the
        // catalog entry; content is pushed by discrete writers/events. The
        // panel sync pass reconciles the active owner and focus separately.
    }

    pub(in crate::app) fn handle_audiobookshelf_book_request(&mut self, request: ShellRequest) {
        // A request can arrive before the Books owner has been registered by
        // its first discrete content push. Do not mutate shell focus on that
        // handled no-op path.
        if self.abs_book_owner().is_none() {
            return;
        }
        self.app.set_panel_focus(crate::app::PanelFocus::Library);
        match request {
            ShellRequest::AudiobookshelfBookMove(movement) => match movement {
                AudiobookshelfBookMove::Book(Some(target)) => {
                    self.app.select_audiobookshelf_book_target(&target);
                }
                AudiobookshelfBookMove::Book(None) => {}
                AudiobookshelfBookMove::Bucket(position) => {
                    self.app.select_audiobookshelf_book_bucket(position);
                }
                AudiobookshelfBookMove::ChapterFocus(selection) => {
                    crate::app::App::set_audiobookshelf_book_chapter_focus(selection);
                }
            },
            ShellRequest::AudiobookshelfBookIntent(intent) => match intent {
                AudiobookshelfBookIntent::Play => {
                    if let Some(index) = self.app.tab.audiobookshelf_index() {
                        self.app.play_selected_audiobookshelf_book(index);
                    }
                }
                AudiobookshelfBookIntent::Activate => {
                    if self.app.is_right_panel_wide() {
                        if let Some(index) = self.app.tab.audiobookshelf_index() {
                            self.app.play_selected_audiobookshelf_book(index);
                        }
                    } else {
                        self.open_library_hero_overlay();
                    }
                }
                AudiobookshelfBookIntent::Enqueue => {
                    if let Some(index) = self.app.tab.audiobookshelf_index() {
                        self.app.enqueue_selected_audiobookshelf_book(index);
                    }
                }
                AudiobookshelfBookIntent::FocusChapters => {
                    if self.app.is_right_panel_wide() {
                        self.update_abs_book_owner(super::super::components::book_content::BookContent::enter_chapter_focus);
                    } else {
                        self.open_library_hero_overlay();
                    }
                }
                AudiobookshelfBookIntent::ActivateChapter(target) => {
                    self.app.activate_audiobookshelf_book_row_target(target);
                }
            },
            _ => unreachable!("non-book request routed to book handler"),
        }
        self.push_audiobookshelf_book_content();
    }

    #[cfg(test)]
    pub(in crate::app) fn test_abs_book_owner(&self) -> &BookContent {
        self.abs_book_owner().expect("book owner")
    }

    #[cfg(test)]
    pub(in crate::app) fn test_abs_book_owner_mut(&mut self) -> &mut BookContent {
        let key = self.abs_book_key().expect("book key");
        self.library_owner_mut(&key).expect("book owner")
    }
}
