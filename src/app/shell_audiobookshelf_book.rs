use super::components::{AudiobookshelfBookComponent, BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::{PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    pub(super) fn handle_audiobookshelf_book_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> bool {
        self.app.handle_key(key)
    }

    pub(super) fn handle_audiobookshelf_book_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        self.app.handle_mouse(mouse);
    }

    fn abs_book_component_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.audiobookshelf_libraries.get(index)?;
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Audiobookshelf,
            library_id: library.id.clone(),
            kind: BrowserKind::AudiobookshelfBook,
        }))
    }

    pub(super) fn sync_audiobookshelf_book(&mut self) {
        let next_id = match self.app.tab {
            TabSelection::AudiobookshelfLibrary(index)
                if matches!(
                    self.app.audiobookshelf_kind_at(index),
                    Some(AudiobookshelfBrowseKind::Book)
                ) =>
            {
                self.abs_book_component_id(index)
            }
            _ => None,
        };
        if self.abs_book_id != next_id {
            if let Some(id) = self.abs_book_id.take() {
                let _ = self.application.umount(&id);
            }
            if let Some(id) = next_id.clone() {
                self.application
                    .mount(
                        id.clone(),
                        Box::new(AudiobookshelfBookComponent::new()),
                        vec![],
                    )
                    .expect("mount Audiobookshelf book browser");
                self.application
                    .active(&id)
                    .expect("activate Audiobookshelf book browser");
                self.abs_book_id = Some(id);
            }
        }
        let Some(id) = self.abs_book_id.as_ref() else {
            return;
        };
        let index = match self.app.tab {
            TabSelection::AudiobookshelfLibrary(index) => index,
            _ => return,
        };
        let Some(snapshot) = self.app.audiobookshelf_book_browse.get(index) else {
            return;
        };
        let focused = matches!(self.app.effective_panel_focus(), PanelFocus::Library);
        let images_enabled = self.app.images_enabled();
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(book) = comp
                .as_any_mut()
                .downcast_mut::<AudiobookshelfBookComponent>()
            {
                book.set_content(snapshot, focused, images_enabled);
            }
        }
    }

    pub(super) fn render_audiobookshelf_book_component(&mut self, frame: &mut ratatui::Frame) {
        let Some(id) = self.abs_book_id.as_ref() else {
            return;
        };
        let area = self.app.layout.main.audiobookshelf_book_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.application.view(id, frame, area);
        let image_paint = self
            .application
            .get_component_mut(id)
            .and_then(|comp| {
                comp.as_any_mut()
                    .downcast_mut::<AudiobookshelfBookComponent>()
            })
            .and_then(AudiobookshelfBookComponent::take_image_paint);
        self.app.paint_home_image(frame, image_paint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{Msg, ShellRequest};
    use crate::app::tests::make_app_stub;
    use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
    use mbv_core::audiobookshelf::{AudiobookshelfBook, AudiobookshelfLibrary};
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn abs_book_shell_mounts_and_routes_component() {
        let mut app = make_app_stub();
        let library = AudiobookshelfLibrary {
            id: "books".into(),
            name: "Books".into(),
            media_type: "book".into(),
        };
        let mut state = AudiobookshelfBookBrowseState::new(library.clone());
        state.books = vec![AudiobookshelfBook {
            library_item_id: "book".into(),
            title: "Book".into(),
            author_display: None,
            author_sort_key: "Book".into(),
            cover_path: None,
            duration_seconds: 0.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: Vec::new(),
            audio_files: Vec::new(),
        }];
        state.selected_id = Some("book".into());
        state.buckets =
            crate::app::types_audiobookshelf_browse::build_surname_buckets(&state.books);
        app.audiobookshelf_libraries.push(library);
        app.audiobookshelf_book_browse.push(state);
        app.tab = TabSelection::AudiobookshelfLibrary(0);
        app.panel_focus = PanelFocus::Library;
        let mut model = Model::new(app);
        model.sync_audiobookshelf_book();
        let id = model.abs_book_id.clone().expect("book component mounted");
        let message = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        assert!(matches!(
            message,
            Some(Msg::Shell(ShellRequest::AudiobookshelfBookKey(_)))
        ));
    }
}
