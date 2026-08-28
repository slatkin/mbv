use super::components::msg::{AudiobookshelfBookIntent, AudiobookshelfBookMove, ShellRequest};
use super::components::{AudiobookshelfBookComponent, BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::{PanelFocus, TabSelection};
use mbv_core::config::ServiceKind;

impl Model {
    /// Applies a typed book request to the existing App operations. The
    /// component has already updated its local cursor/focus, so the App call
    /// preserves the legacy persistence, detail-fetch, and playback effects;
    /// the push reconciles the mounted component after that write.
    pub(super) fn handle_audiobookshelf_book_request(&mut self, request: ShellRequest) {
        match request {
            ShellRequest::AudiobookshelfBookMove(movement) => match movement {
                AudiobookshelfBookMove::PreviousBucket => {
                    self.app.cycle_audiobookshelf_book_bucket(-1)
                }
                AudiobookshelfBookMove::NextBucket => self.app.cycle_audiobookshelf_book_bucket(1),
                AudiobookshelfBookMove::PreviousChapter => {
                    self.app.move_audiobookshelf_book_row(-1)
                }
                AudiobookshelfBookMove::NextChapter => self.app.move_audiobookshelf_book_row(1),
                AudiobookshelfBookMove::FocusChapters => {
                    self.app.focus_audiobookshelf_book_chapters()
                }
                AudiobookshelfBookMove::FocusBrowser => {
                    self.app.focus_audiobookshelf_book_browser()
                }
                AudiobookshelfBookMove::PreviousBookRow => {
                    self.app.move_audiobookshelf_book_cursor(-1)
                }
                AudiobookshelfBookMove::NextBookRow => self.app.move_audiobookshelf_book_cursor(1),
                AudiobookshelfBookMove::PreviousBookPage => {
                    let page = self.app.lib_page_size() as i64;
                    self.app.move_audiobookshelf_book_cursor(-page);
                }
                AudiobookshelfBookMove::NextBookPage => {
                    let page = self.app.lib_page_size() as i64;
                    self.app.move_audiobookshelf_book_cursor(page);
                }
                AudiobookshelfBookMove::FirstBook => {
                    self.app.jump_audiobookshelf_book_cursor(false)
                }
                AudiobookshelfBookMove::LastBook => self.app.jump_audiobookshelf_book_cursor(true),
            },
            ShellRequest::AudiobookshelfBookIntent(intent) => match intent {
                AudiobookshelfBookIntent::Play => {
                    if let Some(index) = self.app.tab.audiobookshelf_index() {
                        self.app.play_selected_audiobookshelf_book(index);
                    }
                }
                AudiobookshelfBookIntent::Activate => {
                    if self.app.layout.main.is_wide_book_active() {
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
                AudiobookshelfBookIntent::ActivateChapter => {
                    self.app.activate_audiobookshelf_book_row();
                }
            },
            _ => unreachable!("non-book request routed to book handler"),
        }
        self.push_audiobookshelf_book_content();
    }

    fn abs_book_component_id(&self, index: usize) -> Option<ComponentId> {
        let library = self.app.audiobookshelf_libraries.get(index)?;
        Some(ComponentId::Browser(BrowserKey {
            service: ServiceKind::Audiobookshelf,
            library_id: library.id.clone(),
            kind: BrowserKind::AudiobookshelfBook,
        }))
    }

    /// Mounts / unmounts the Audiobookshelf book browser component to follow
    /// the active tab (task 5.3d). This is the mount lifecycle only: content is
    /// no longer mirrored into the component on every tick. The per-frame
    /// `set_content` projection was replaced by the event-scoped
    /// `push_audiobookshelf_book_content` at the writers of its projected
    /// inputs (active-tab, async completion, progress, refresh/reset, and
    /// saved-position restore). Content is pushed right after a fresh mount so
    /// the newly mounted component paints the current browse snapshot.
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
                // Fresh mount: project the active tab's browse state so the
                // component is initialized with the current books/selection
                // before it is painted (the active-tab writer).
                self.push_audiobookshelf_book_content();
            }
        }
    }

    /// Event-scoped projection replacing the per-frame content mirror (task
    /// 5.3d, `sync_audiobookshelf_book` Phase A): runs only when the active tab
    /// is the mounted book browser and mirrors the validated browse snapshot
    /// plus panel focus into `AudiobookshelfBookComponent` via `set_content`
    /// (preserving its selected-book/chapter/bucket semantics exactly).
    /// Called at the writers of the projected inputs, so it is deterministic in
    /// `App` state and duplicate pushes are idempotent. `sync_audiobookshelf_book`
    /// keeps only mount lifecycle management.
    pub(super) fn push_audiobookshelf_book_content(&mut self) {
        let Some(id) = self.abs_book_id.as_ref() else {
            return;
        };
        // Mirror `sync_audiobookshelf_book`'s active-tab guard: only project
        // while this tab's browse kind is still Book, so a stale mounted book
        // component never receives a non-Book snapshot before mount
        // reconciliation (task 5.3d).
        let index = match self.app.tab {
            TabSelection::AudiobookshelfLibrary(index)
                if matches!(
                    self.app.audiobookshelf_kind_at(index),
                    Some(AudiobookshelfBrowseKind::Book)
                ) =>
            {
                index
            }
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
        let page_size = self.app.lib_page_size();
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(book) = comp
                .as_any_mut()
                .downcast_mut::<AudiobookshelfBookComponent>()
            {
                book.set_page_size(page_size);
            }
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
    use crate::app::components::msg::AudiobookshelfBookMove;
    use crate::app::components::{Msg, ShellRequest};
    use crate::app::tests::make_app_stub;
    use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;
    use mbv_core::audiobookshelf::{AudiobookshelfBook, AudiobookshelfLibrary};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;
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
        let mut second = state.books[0].clone();
        second.library_item_id = "book-2".into();
        second.title = "Book 2".into();
        state.books.push(second);
        let mut third = state.books[0].clone();
        third.library_item_id = "book-3".into();
        third.title = "Book 3".into();
        state.books.push(third);
        let mut zed = state.books[0].clone();
        zed.library_item_id = "book-z".into();
        zed.title = "Book Z".into();
        zed.author_sort_key = "Zed".into();
        state.books.push(zed);
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
            Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                AudiobookshelfBookMove::NextBookRow
            )))
        ));
        model.handle_audiobookshelf_book_request(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::NextBookRow,
        ));
        assert_eq!(
            model.app.audiobookshelf_book_browse[0]
                .selected_id
                .as_deref(),
            Some("book-2")
        );

        // Page size is handed from App's layout contract, rather than inferred
        // from the inline replacement's painted book rows. With a three-row
        // library area, both the component and App must move by two books.
        model.app.layout.main.left_area = Rect::new(0, 0, 10, 3);
        model.app.layout.main.audiobookshelf_book_area = Rect::new(0, 0, 60, 20);
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal
            .draw(|frame| model.render_audiobookshelf_book_component(frame))
            .unwrap();
        let page = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code: Key::PageDown,
                modifiers: KeyModifiers::NONE,
            }));
        assert!(matches!(
            page,
            Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                AudiobookshelfBookMove::NextBookPage
            )))
        ));
        model.handle_audiobookshelf_book_request(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::NextBookPage,
        ));
        assert_eq!(
            model.app.audiobookshelf_book_browse[0]
                .selected_id
                .as_deref(),
            Some("book-3")
        );

        // Unmatched component keys stay unclaimed so the central router can
        // resolve global shortcuts without a legacy raw-key fallback.
        let bucket = model.app.audiobookshelf_book_browse[0].selected_bucket;
        let unclaimed = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Char('['),
                modifiers: KeyModifiers::SHIFT,
            }));
        assert_eq!(unclaimed, None);
        assert_eq!(
            model.app.audiobookshelf_book_browse[0].selected_bucket, bucket,
            "Shift+[ must not enter the ABS Book bucket fallback"
        );
    }
}
