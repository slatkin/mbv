use super::components::book_content::BookContent;
use super::components::library_panel::{LibraryKey, LibraryPanel};
use super::components::msg::{AudiobookshelfBookIntent, AudiobookshelfBookMove, ShellRequest};
use super::components::{BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::TabSelection;
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
            Some(LibraryKey::Service(BrowserKey {
                service: ServiceKind::Audiobookshelf,
                library_id: library.id.clone(),
                kind: BrowserKind::AudiobookshelfBook,
            }))
        })?
    }

    pub fn abs_book_owner(&self) -> Option<&BookContent> {
        let key = self.abs_book_key()?;
        self.application
            .get_component(&ComponentId::Library)
            .and_then(|c| c.as_any().downcast_ref::<LibraryPanel>())
            .and_then(|p| p.owner(&key))
            .and_then(|o| o.as_any().downcast_ref::<BookContent>())
    }

    pub fn abs_book_owner_mut(&mut self) -> Option<&mut BookContent> {
        let key = self.abs_book_key()?;
        self.application
            .get_component_mut(&ComponentId::Library)
            .and_then(|c| c.as_any_mut().downcast_mut::<LibraryPanel>())
            .and_then(|p| p.owner_mut(&key))
            .and_then(|o| o.as_any_mut().downcast_mut::<BookContent>())
    }

    fn update_abs_book_owner<R>(&mut self, f: impl FnOnce(&mut BookContent) -> R) -> Option<R> {
        let key = self.abs_book_key()?;
        if !self.library_panel_has_owner(&key) {
            self.push_library_owner(key.clone(), Box::new(BookContent::new()));
        }
        self.application
            .get_component_mut(&ComponentId::Library)
            .and_then(|c| c.as_any_mut().downcast_mut::<LibraryPanel>())
            .and_then(|p| p.owner_mut(&key))
            .and_then(|o| o.as_any_mut().downcast_mut::<BookContent>())
            .map(f)
    }

    pub(super) fn push_audiobookshelf_book_content(&mut self) {
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

    pub(super) fn sync_audiobookshelf_book(&mut self) {
        // Books are retained as a LibraryPanel owner for the lifetime of the
        // catalog entry; content is pushed by discrete writers/events. The
        // panel sync pass reconciles the active owner and focus separately.
    }

    pub(super) fn handle_audiobookshelf_book_request(&mut self, request: ShellRequest) {
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
                    self.app.select_audiobookshelf_book_target(&target)
                }
                AudiobookshelfBookMove::Book(None) => {}
                AudiobookshelfBookMove::Bucket(position) => {
                    self.app.select_audiobookshelf_book_bucket(position)
                }
                AudiobookshelfBookMove::ChapterFocus(selection) => {
                    self.app.set_audiobookshelf_book_chapter_focus(selection)
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
                        self.app.activate_audiobookshelf_book_parent();
                    }
                }
                AudiobookshelfBookIntent::Enqueue => {
                    if let Some(index) = self.app.tab.audiobookshelf_index() {
                        self.app.enqueue_selected_audiobookshelf_book(index);
                    }
                }
                AudiobookshelfBookIntent::ActivateChapter(target) => {
                    self.app.activate_audiobookshelf_book_row_target(target)
                }
            },
            _ => unreachable!("non-book request routed to book handler"),
        }
        self.push_audiobookshelf_book_content();
    }

    #[cfg(test)]
    pub(super) fn test_abs_book_owner(&self) -> &BookContent {
        self.abs_book_owner().expect("book owner")
    }

    #[cfg(test)]
    pub(super) fn test_abs_book_owner_mut(&mut self) -> &mut BookContent {
        self.abs_book_owner_mut().expect("book owner")
    }
}
