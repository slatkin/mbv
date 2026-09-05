use super::components::msg::{AudiobookshelfBookIntent, AudiobookshelfBookMove, ShellRequest};
use super::components::{AudiobookshelfBookComponent, BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::TabSelection;
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
        let images_enabled = self.app.images_enabled();
        if let Some(comp) = self.application.get_component_mut(id) {
            if let Some(book) = comp
                .as_any_mut()
                .downcast_mut::<AudiobookshelfBookComponent>()
            {
                book.set_content(snapshot, images_enabled);
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
#[path = "shell_audiobookshelf_book_tests.rs"]
mod tests;
