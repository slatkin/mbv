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
        // Click-to-focus (task 4.5): a mouse-driven book request (and any
        // keyboard request from the already-focused component) pulls panel
        // focus to the Library.
        self.app.set_panel_focus(crate::app::PanelFocus::Library);
        match request {
            ShellRequest::AudiobookshelfBookMove(movement) => match movement {
                // Resolved-value routing
                // (split-audiobookshelf-cursor-ownership D1/D3): the
                // component carries the landed value; apply it through the
                // existing index-taking entry points, never recomputing the
                // movement from a delta.
                AudiobookshelfBookMove::Book(index) => self.app.select_audiobookshelf_book(index),
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
                AudiobookshelfBookIntent::ActivateChapter => {
                    let chapter_selection = self
                        .abs_book_id
                        .as_ref()
                        .and_then(|id| self.application.get_component(id))
                        .and_then(|comp| {
                            comp.as_any().downcast_ref::<AudiobookshelfBookComponent>()
                        })
                        .and_then(AudiobookshelfBookComponent::chapter_selection);
                    self.app.activate_audiobookshelf_book_row(chapter_selection);
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
            match next_id {
                Some(id) => {
                    if !self.application.mounted(&id) {
                        self.application
                            .mount(
                                id.clone(),
                                Box::new(AudiobookshelfBookComponent::new()),
                                vec![],
                            )
                            .expect("mount Audiobookshelf book browser");
                        self.register_destination(&id);
                    }
                    self.abs_book_id = Some(id);
                    // Re-point: project the active tab's browse state so the
                    // component paints the current books/selection (the
                    // active-tab writer); keep-mounted preserves its private
                    // selection across the switch.
                    self.push_audiobookshelf_book_content();
                }
                None => {
                    self.abs_book_id = None;
                }
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
        self.application.view(id, frame, area);
        // Component owns painting; read back its painted geometry so the
        // still-required legacy `LayoutMain` readers (overlay/menu anchors)
        // stay correct once the legacy underpaint renderer was removed
        // (2.1j). The page stride is now the component's own
        // (split-audiobookshelf-cursor-ownership D1), so the shell projects
        // no page size in.
        let projection = self
            .application
            .get_component_mut(id)
            .and_then(|comp| {
                comp.as_any_mut()
                    .downcast_mut::<AudiobookshelfBookComponent>()
            })
            .map(|component| {
                let image_paint = component.take_image_paint();
                let geometry = component.geometry();
                (
                    image_paint,
                    geometry.left_area,
                    geometry.hero_area,
                    geometry.selected_item_rect,
                    geometry.selector_tabs.clone(),
                )
            });
        if let Some((image_paint, left_area, hero_area, selected_item_rect, selector_tabs)) =
            projection
        {
            self.app.paint_home_image(frame, image_paint);
            self.app.layout.main.left_area = left_area;
            self.app.layout.main.hero_area = hero_area.unwrap_or_default();
            self.app.layout.main.selected_item_rect = selected_item_rect;
            self.app.layout.main.selector_tabs = selector_tabs;
        }
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
    fn abs_book_hero_uses_isolated_book_cover_key() {
        let mut app = make_app_stub();
        app.image_protocol_enabled = true;
        app.config.lock().unwrap().audiobookshelf_setup = Some(
            mbv_core::config::AudiobookshelfSetup::new("https://books.example"),
        );
        mbv_core::config::save_service_secret(
            mbv_core::config::ServiceKind::Audiobookshelf,
            "book-hero-secret",
        )
        .unwrap();

        let library = AudiobookshelfLibrary {
            id: "books".into(),
            name: "Books".into(),
            media_type: "book".into(),
        };
        let mut state = AudiobookshelfBookBrowseState::new(library.clone());
        state.books.push(AudiobookshelfBook {
            library_item_id: "book-hero-isolation".into(),
            title: "Book".into(),
            author_display: None,
            author_sort_key: "Book".into(),
            cover_path: Some("cover.jpg".into()),
            duration_seconds: 60.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: Vec::new(),
            audio_files: Vec::new(),
        });
        state.selected_id = Some("book-hero-isolation".into());
        app.audiobookshelf_libraries.push(library);
        app.audiobookshelf_book_browse.push(state);
        app.tab = TabSelection::AudiobookshelfLibrary(0);
        app.panel_focus = PanelFocus::Library;

        let mut model = Model::new(app);
        model.sync_audiobookshelf_book();
        model.app.layout.main.audiobookshelf_book_area = Rect::new(0, 0, 100, 40);
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        terminal
            .draw(|frame| model.render_audiobookshelf_book_component(frame))
            .unwrap();

        let server = "https://books.example";
        let suffix = model.app.current_protocol_suffix();
        let book_key = crate::app::images::audiobookshelf_book_cover_cache_key(
            server,
            "book-hero-isolation",
            suffix,
        );
        let generic_key = crate::app::images::audiobookshelf_cover_cache_key(
            server,
            "book-hero-isolation",
            suffix,
        );
        assert!(
            model.app.card_image_loading.contains(&book_key)
                || model.app.card_image_states.contains_key(&book_key),
            "book hero must reserve or load the isolated book-cover key"
        );
        assert!(!model.app.card_image_loading.contains(&generic_key));
        assert!(!model.app.card_image_states.contains_key(&generic_key));
    }

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
        let Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(AudiobookshelfBookMove::Book(
            index,
        )))) = message
        else {
            panic!("Down must emit a resolved book index, got {message:?}");
        };
        assert_eq!(index, 1, "component resolved the next book row locally");
        model.handle_audiobookshelf_book_request(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::Book(index),
        ));
        assert_eq!(
            model.app.audiobookshelf_book_browse[0]
                .selected_id
                .as_deref(),
            Some("book-2")
        );

        // The page stride is now the component's own painted geometry
        // (split-audiobookshelf-cursor-ownership D1): the shell applies no
        // competing stride. PageDown resolves a page jump against the painted
        // list and clamps to the selected surname bucket.
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
        let Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(AudiobookshelfBookMove::Book(
            index,
        )))) = page
        else {
            panic!("PageDown must emit a resolved book index, got {page:?}");
        };
        assert!(index > 1, "page jump advanced past a single row");
        model.handle_audiobookshelf_book_request(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::Book(index),
        ));
        assert_eq!(
            model.app.audiobookshelf_book_browse[0]
                .selected_id
                .as_deref(),
            Some("book-3")
        );

        // Unmatched component keys stay unclaimed so the central router can
        // resolve global shortcuts without a legacy raw-key fallback.
        let component_bucket = |model: &Model| {
            model
                .application
                .get_component(&id)
                .and_then(|comp| comp.as_any().downcast_ref::<AudiobookshelfBookComponent>())
                .map(AudiobookshelfBookComponent::selected_bucket)
        };
        let bucket = component_bucket(&model);
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
            component_bucket(&model),
            bucket,
            "Shift+[ must not enter the ABS Book bucket fallback"
        );
    }

    /// keep-destination-components-mounted task 3.3: the ABS book browser
    /// stays mounted across a tab switch and back (keep-mounted, D1).
    /// Switching away must not unmount the book component, and switching
    /// back must re-point the SAME component (not remount), preserving its
    /// private selection.
    #[test]
    fn abs_book_stays_mounted_and_preserves_selection_across_switch() {
        let mut app = make_app_stub();
        // Book library at index 0.
        let book_library = AudiobookshelfLibrary {
            id: "abs-books".into(),
            name: "ABS Books".into(),
            media_type: "book".into(),
        };
        let mut book_state = AudiobookshelfBookBrowseState::new(book_library.clone());
        let book1 = AudiobookshelfBook {
            library_item_id: "book-a".into(),
            title: "Book A".into(),
            author_display: None,
            author_sort_key: "A".into(),
            cover_path: None,
            duration_seconds: 0.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: Vec::new(),
            audio_files: Vec::new(),
        };
        let mut book2 = book1.clone();
        book2.library_item_id = "book-b".into();
        book2.title = "Book B".into();
        book2.author_sort_key = "B".into();
        book_state.books = vec![book1, book2];
        book_state.selected_id = Some("book-a".into());
        book_state.buckets =
            crate::app::types_audiobookshelf_browse::build_surname_buckets(&book_state.books);
        // Podcast library at index 1, so switching changes the destination.
        let podcast_library = AudiobookshelfLibrary {
            id: "abs-podcasts".into(),
            name: "ABS Podcasts".into(),
            media_type: "podcast".into(),
        };
        app.audiobookshelf_libraries.push(book_library);
        app.audiobookshelf_libraries.push(podcast_library.clone());
        app.audiobookshelf_book_browse.push(book_state);
        app.audiobookshelf_browse.push(
            crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState::new(
                podcast_library,
            ),
        );
        app.tab = TabSelection::AudiobookshelfLibrary(0);
        app.panel_focus = PanelFocus::Library;
        let mut model = Model::new(app);
        model.sync_audiobookshelf_book();
        let id = model.abs_book_id.clone().expect("book component mounted");
        let selected_book_id = |model: &Model| {
            model
                .application
                .get_component(&model.abs_book_id.clone().expect("book component mounted"))
                .and_then(|comp| comp.as_any().downcast_ref::<AudiobookshelfBookComponent>())
                .and_then(AudiobookshelfBookComponent::selected_book_id)
                .map(|s| s.to_owned())
        };
        // Drive the selection to a non-default value: move Down (which selects
        // the second book) and apply the resulting request to App, so content
        // and interaction agree on the second book before the switch.
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
                AudiobookshelfBookMove::Book(1)
            )))
        ));
        model.handle_audiobookshelf_book_request(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::Book(1),
        ));
        model.sync_audiobookshelf_book();
        assert_eq!(
            selected_book_id(&model),
            Some("book-b".into()),
            "component selection must have moved to the second book"
        );

        // Switch to the Podcast library: the book component stays mounted.
        model.app.tab = TabSelection::AudiobookshelfLibrary(1);
        model.sync_audiobookshelf_book();
        assert_eq!(model.abs_book_id, None);
        assert!(
            model.application.mounted(&id),
            "the book browser must stay mounted across the switch"
        );

        // Switch back: the SAME component is re-pointed, still mounted, and
        // its selection is preserved.
        model.app.tab = TabSelection::AudiobookshelfLibrary(0);
        model.sync_audiobookshelf_book();
        assert_eq!(
            model.abs_book_id.as_ref(),
            Some(&id),
            "re-point must restore the same book component id"
        );
        assert!(model.application.mounted(&id));
        assert_eq!(
            selected_book_id(&model),
            Some("book-b".into()),
            "the book selection must survive the switch-and-return round trip"
        );
    }

    /// split-audiobookshelf-cursor-ownership D4 / task 5.3: a real shell
    /// content push that drops the component's selected book must not leave
    /// any App-sourced interaction value (here `chapter_selection`) in the
    /// component.
    #[test]
    fn abs_book_shell_push_drops_stale_component_chapter_focus() {
        let mut app = make_app_stub();
        let library = AudiobookshelfLibrary {
            id: "books".into(),
            name: "Books".into(),
            media_type: "book".into(),
        };
        let chapter = mbv_core::audiobookshelf::AudiobookshelfChapter {
            id: 0,
            start: 0.0,
            end: 60.0,
            title: "Chapter 1".into(),
        };
        let book = |id: &str| AudiobookshelfBook {
            library_item_id: id.into(),
            title: format!("Book {id}"),
            author_display: Some("Author".into()),
            author_sort_key: "Author".into(),
            cover_path: None,
            duration_seconds: 60.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: vec![chapter.clone()],
            audio_files: Vec::new(),
        };
        let mut state = AudiobookshelfBookBrowseState::new(library.clone());
        state.books = vec![book("a")];
        state.selected_id = Some("a".into());
        state
            .detail_cache
            .insert("a".into(), (vec![chapter.clone()], Vec::new()));
        state.buckets =
            crate::app::types_audiobookshelf_browse::build_surname_buckets(&state.books);
        app.audiobookshelf_libraries.push(library);
        app.audiobookshelf_book_browse.push(state);
        app.tab = TabSelection::AudiobookshelfLibrary(0);
        app.panel_focus = PanelFocus::Library;
        let mut model = Model::new(app);
        model.sync_audiobookshelf_book();
        let id = model.abs_book_id.clone().expect("book component mounted");

        // Paint wide so the chapter pane exists, then focus it locally.
        model.app.layout.main.audiobookshelf_book_area = Rect::new(0, 0, 120, 24);
        let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();
        terminal
            .draw(|frame| model.render_audiobookshelf_book_component(frame))
            .unwrap();
        let focus = model
            .application
            .get_component_mut(&id)
            .unwrap()
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Left,
                modifiers: KeyModifiers::NONE,
            }));
        assert!(matches!(
            focus,
            Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                AudiobookshelfBookMove::ChapterFocus(Some(0))
            )))
        ));

        // App content changes: book "a" is gone from the projected content.
        let state = &mut model.app.audiobookshelf_book_browse[0];
        state.books = vec![book("z")];
        state.selected_id = Some("z".into());
        state.buckets =
            crate::app::types_audiobookshelf_browse::build_surname_buckets(&state.books);
        model.push_audiobookshelf_book_content();

        let chapter_selection = model
            .application
            .get_component(&id)
            .and_then(|comp| comp.as_any().downcast_ref::<AudiobookshelfBookComponent>())
            .and_then(AudiobookshelfBookComponent::chapter_selection);
        assert_eq!(
            chapter_selection, None,
            "the content push must not adopt App's stale chapter selection"
        );
    }

    /// 2.1j book mirror: after the component paints, the shell projects its
    /// geometry (left area, hero, selected rect, selector tabs) into
    /// `LayoutMain`, and that projected left area matches the component's own
    /// painted content area (the page stride source,
    /// split-audiobookshelf-cursor-ownership D1).
    #[test]
    fn book_mirror_projects_component_geometry_and_page_stride() {
        let mut app = make_app_stub();
        let library = AudiobookshelfLibrary {
            id: "books".into(),
            name: "Books".into(),
            media_type: "book".into(),
        };
        let mut state = AudiobookshelfBookBrowseState::new(library.clone());
        let book = AudiobookshelfBook {
            library_item_id: "book-a".into(),
            title: "Book A".into(),
            author_display: None,
            author_sort_key: "A".into(),
            cover_path: None,
            duration_seconds: 0.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: Vec::new(),
            audio_files: Vec::new(),
        };
        for (i, suffix) in ["B", "C", "D", "E", "F"].iter().enumerate() {
            let mut b = book.clone();
            b.library_item_id = format!("book-{}", suffix.to_lowercase());
            b.title = format!("Book {}", suffix);
            b.author_sort_key = suffix.to_string();
            state.books.push(b);
            let _ = i;
        }
        state.books.insert(0, book.clone());
        state.selected_id = Some("book-a".into());
        state.buckets =
            crate::app::types_audiobookshelf_browse::build_surname_buckets(&state.books);
        app.audiobookshelf_libraries.push(library);
        app.audiobookshelf_book_browse.push(state);
        app.tab = TabSelection::AudiobookshelfLibrary(0);
        app.panel_focus = PanelFocus::Library;
        let mut model = Model::new(app);
        model.sync_audiobookshelf_book();
        // Narrow book area (60x20): the component paints the narrow
        // presentation; the mirror must project the narrow content area.
        model.app.layout.main.audiobookshelf_book_area = Rect::new(0, 0, 60, 20);
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal
            .draw(|frame| model.render_audiobookshelf_book_component(frame))
            .unwrap();

        // Narrow: wide flag false, wide right area zero, and the projected
        // left area mirrors the component's own painted content area (the
        // page stride now comes from that geometry alone,
        // split-audiobookshelf-cursor-ownership D1 — the shell projects no
        // page size in).
        assert!(!model.app.layout.main.is_wide_book_active());
        assert_eq!(
            model.app.layout.main.audiobookshelf_book_wide_right_area,
            Rect::default()
        );
        assert!(model.app.layout.main.left_area.height > 0);
        let component_left_area = model
            .application
            .get_component(&model.abs_book_id.clone().unwrap())
            .and_then(|comp| comp.as_any().downcast_ref::<AudiobookshelfBookComponent>())
            .map(|book| book.geometry().left_area)
            .unwrap();
        assert_eq!(component_left_area, model.app.layout.main.left_area);
        assert!(
            model.app.layout.main.selected_item_rect.is_some()
                || model.app.layout.main.hero_area.width > 0
        );
    }

    // Task 4.5: a book request (mouse or already-focused keyboard) pulls
    // panel focus to the Library.
    #[test]
    fn abs_book_request_pulls_panel_focus_to_library() {
        let mut app = make_app_stub();
        let library = AudiobookshelfLibrary {
            id: "books".into(),
            name: "Books".into(),
            media_type: "book".into(),
        };
        app.audiobookshelf_libraries.push(library);
        app.audiobookshelf_book_browse
            .push(AudiobookshelfBookBrowseState::new(AudiobookshelfLibrary {
                id: "books".into(),
                name: "Books".into(),
                media_type: "book".into(),
            }));
        app.panel_focus = PanelFocus::Queue;
        let mut model = Model::new(app);
        model.handle_audiobookshelf_book_request(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::Book(0),
        ));
        assert_eq!(model.app.panel_focus, PanelFocus::Library);
    }
}
