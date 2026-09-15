//! Audiobookshelf Books' embedded Library panel content owner (task 10.1).
//!
//! `BookContent` owns the shell-projected book/chapter snapshot, the shared
//! book and chapter list controls, local chapter-pane focus, and the surname
//! bucket selection. It is a plain [`LibraryContentOwner`].

use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow, Workspace,
};
use super::library_panel::hero::hero_content_queue;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaListSurfaceInput, MediaSemanticState,
    Presentation,
};
use super::msg::{
    AudiobookshelfBookIntent, AudiobookshelfBookMove, BookChapterTarget, LeafKeyResult, Msg,
    ShellRequest,
};
use crate::app::audiobookshelf_browse_actions::audiobookshelf_book_queue_item;
use crate::app::types_audiobookshelf_browse::{AudiobookshelfBookBrowseState, BookRow};
use crate::app::ui_util::clean_overview;

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
            let (target, primary, duration) = match row {
                BookRow::Chapter {
                    id,
                    title,
                    start,
                    end,
                } => (
                    id,
                    title,
                    crate::app::ui_util::list_duration_secs((end - start).max(0.0) as i64),
                ),
                BookRow::AudioFile { index, duration } => (
                    index,
                    format!("Part {index}"),
                    crate::app::ui_util::list_duration_secs(duration as i64),
                ),
            };
            MediaListRow::Item {
                target,
                primary,
                secondary: None,
                trailing: None,
                duration,
                kind: MediaKind::Media,
                semantic_state: MediaSemanticState::Ordinary,
            }
        })
        .collect()
}

/// The plain Books content owner. Its list controls retain cursor, scroll,
/// selected targets, and chapter-pane focus locally; shell pushes replace
/// only the content snapshot and never mirror those interaction values.
pub struct BookContent {
    pub(in crate::app) state: AudiobookshelfBookBrowseState,
    /// `false` until the first `set_content`: the initial projection adopts
    /// the shell snapshot wholesale; only later pushes reset stale
    /// component-owned fields (split-audiobookshelf-cursor-ownership D4).
    initialized: bool,
    /// Parent-owned chapter-pane focus, separate from the chapter owner's
    /// selected row (design.md D5). Entering/leaving focus never moves the
    /// chapter owner's selection; the owner re-parks it only at discrete
    /// book-identity re-projection boundaries.
    pub(in crate::app) chapter_focused: bool,
    pub(in crate::app) selected_bucket: usize,
    pub(in crate::app) focused: bool,
    pub(in crate::app) images_enabled: bool,
    /// One shared canonical book owner, carried by exactly one of the Wide and
    /// Inline presentations (design.md D1/D2). It owns the book cursor, scroll,
    /// and selected target across a breakpoint change; the two adapters are
    /// never synchronized.
    pub(in crate::app) carrier: MediaListCarrier<String>,
    /// The persistent canonical chapter/audio-part owner (design.md D5):
    /// authoritative for the focused chapter row, scrolling, painting, and
    /// retained hits. Its rows are projected from the selected book's detail
    /// before view.
    pub(in crate::app) chapter_list: MediaListCarrier<usize>,
    hero_image: HeroImageState,
}

impl BookContent {
    pub(in crate::app) fn new() -> Self {
        Self {
            state: AudiobookshelfBookBrowseState::new(
                mbv_core::audiobookshelf::AudiobookshelfLibrary {
                    id: String::new(),
                    name: String::new(),
                    media_type: "book".into(),
                },
            ),
            initialized: false,
            chapter_focused: false,
            selected_bucket: 0,
            focused: false,
            images_enabled: false,
            carrier: MediaListCarrier::new(Presentation::Inline),
            chapter_list: MediaListCarrier::new(Presentation::Wide),
            hero_image: HeroImageState::None,
        }
    }

    pub(in crate::app) fn set_content(
        &mut self,
        snapshot: &AudiobookshelfBookBrowseState,
        images_enabled: bool,
    ) {
        // Content and interaction are separate types now: the projected
        // snapshot carries no `chapter_focused` / `selected_bucket`, so
        // adopting it wholesale cannot clobber them and there is nothing to
        // save and restore (split-browse-state-interaction-fields task 2.2).
        // Whether the book the component was showing survived the new content
        // decides if its derived local state still means anything.
        let survived = self.initialized
            && self.state.selected_id.as_ref().is_some_and(|prior| {
                snapshot
                    .books
                    .iter()
                    .any(|book| &book.library_item_id == prior)
            });
        self.state = snapshot.clone();
        let identity_changed = self.initialized && !survived;
        if identity_changed {
            self.chapter_focused = false;
            self.state.select(0);
        }
        // Re-anchor the surname-bucket pill onto the selected book
        // (book-browsing spec: refresh/paging preserves the selected book
        // regardless of its new bucket).
        let cursor = self.state.cursor();
        if let Some(pos) = self
            .state
            .buckets
            .iter()
            .position(|bucket| cursor >= bucket.start && cursor < bucket.end)
        {
            self.selected_bucket = pos;
        }
        self.selected_bucket = self
            .selected_bucket
            .min(self.state.buckets.len().saturating_sub(1));
        self.set_book_rows();
        if !self.initialized {
            if let Some(target) = self.state.selected_id.clone() {
                self.carrier.select_target(&target);
            }
        } else if identity_changed {
            self.carrier.select_first();
        }
        self.initialized = true;
        self.images_enabled = images_enabled;
        self.project_chapter_rows();
        if identity_changed {
            self.chapter_list.select_first();
        }
    }

    /// Whether the parent-owned chapter pane currently has focus (design.md
    /// D5). Independent of the chapter owner's selected row.
    pub(in crate::app) fn chapter_focused(&self) -> bool {
        self.chapter_focused
    }

    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

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

    #[cfg(test)]
    pub(crate) fn selected_book_id(&self) -> Option<&str> {
        self.carrier.selected_target().map(String::as_str)
    }

    #[cfg(test)]
    pub(crate) fn selected_bucket(&self) -> usize {
        self.selected_bucket
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

    /// The active book owner's stable target index in the projected catalog.
    fn selected_book_index(&self) -> Option<usize> {
        let target = self.carrier.selected_target()?;
        self.state
            .books
            .iter()
            .position(|book| &book.library_item_id == target)
    }

    pub(in crate::app) fn book_request(&self) -> Option<Msg> {
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::Book(self.carrier.selected_target().cloned()),
        )))
    }

    pub(in crate::app) fn bucket_request(&self) -> Option<Msg> {
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::Bucket(self.selected_bucket),
        )))
    }

    pub(in crate::app) fn chapter_focus_request(&self) -> Option<Msg> {
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::ChapterFocus(self.chapter_target()),
        )))
    }

    /// The one seam through which the component offers an already-normalized
    /// row-local key or pointer gesture to the shared book owner. The owner
    /// applies the local state transition and returns the closed
    /// provider-neutral outcome; the component mirrors the owner's selection
    /// into the projected snapshot and returns the typed book-move request
    /// (design.md D3).
    pub(in crate::app) fn move_book(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        self.carrier.delegate_operation(
            input
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
        self.sync_book_from_owner();
        self.book_request()
    }

    /// Mirror the book owner's stable selection into the projected snapshot,
    /// resetting the derived chapter-pane focus and re-projecting the chapter
    /// rows only at a discrete book-identity boundary (design.md D5).
    pub(in crate::app) fn sync_book_from_owner(&mut self) {
        let Some(target) = self.carrier.selected_target().cloned() else {
            return;
        };
        let Some(index) = self
            .state
            .books
            .iter()
            .position(|book| book.library_item_id == target)
        else {
            return;
        };
        let identity_changed = self.state.selected_id.as_deref() != Some(target.as_str());
        self.state.select(index);
        if identity_changed {
            self.chapter_focused = false;
            self.project_chapter_rows();
            self.chapter_list.select_first();
        }
    }

    /// Discrete book re-anchor (bucket jump, Home/End, pointer click): the
    /// stable target is explicitly selected on the owner before any derived
    /// state is reset (design.md D5).
    pub(in crate::app) fn select_book_index(&mut self, index: usize) {
        let Some(target) = self
            .state
            .books
            .get(index)
            .map(|book| book.library_item_id.clone())
        else {
            return;
        };
        if self.carrier.select_target(&target) {
            self.sync_book_from_owner();
        }
    }

    /// Project the selected surname bucket's canonical book rows into the book
    /// owner before view (design.md D6). An unchanged projection keeps the
    /// retained frame valid.
    fn set_book_rows(&mut self) {
        let rows = crate::app::render::book_rows(&self.state, self.selected_bucket);
        if self.carrier.rows() != rows.as_slice() {
            self.carrier.set_content(rows);
        }
    }

    /// Project the selected book's canonical chapter/audio-part rows into the
    /// chapter owner before view (design.md D6). The row's stable target is the
    /// chapter/audio-part discriminator; the typed `BookChapterTarget` pairs it
    /// with the selected book identity (design.md D4).
    fn project_chapter_rows(&mut self) {
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

    pub(in crate::app) fn select_bucket(&mut self, bucket_pos: usize) {
        let Some(bucket) = self.state.buckets.get(bucket_pos).copied() else {
            return;
        };
        self.selected_bucket = bucket_pos;
        self.set_book_rows();
        let keep = self
            .state
            .selected_id
            .as_deref()
            .and_then(|id| {
                self.state
                    .books
                    .iter()
                    .position(|book| book.library_item_id == id)
            })
            .filter(|index| (bucket.start..bucket.end).contains(index));
        self.select_book_index(keep.unwrap_or(bucket.start));
    }

    pub(in crate::app) fn cycle_bucket(&mut self, delta: i64) {
        let count = self.state.buckets.len();
        if count == 0 {
            return;
        }
        let next = (self.selected_bucket as i64 + delta).rem_euclid(count as i64) as usize;
        self.select_bucket(next);
    }

    pub(in crate::app) fn select_bucket_edge(&mut self, end: bool) {
        if let Some(bucket) = self.state.buckets.get(self.selected_bucket).copied() {
            if bucket.end > bucket.start {
                self.select_book_index(if end { bucket.end - 1 } else { bucket.start });
            }
        }
    }

    pub(in crate::app) fn move_chapter(&mut self, delta: i64) {
        self.chapter_list.delegate_operation(
            MediaListSurfaceInput::Move(delta)
                .into_operation(None)
                .expect("resolved media-list pointer target"),
        );
    }

    /// The selected book's producer facts (design D5): the shared
    /// `hero_content_abs_book` producer's title, duration, and progress meta
    /// rows, with the catalog-only narrator/year meta rows and description
    /// overview restored (the producer carries neither -- hero.rs's
    /// `hero_content_abs_book` doc comment).
    fn resolved_hero_data(&self) -> Option<HeroContentData> {
        let book = self.state.selected_book()?;
        let queue_item = audiobookshelf_book_queue_item(&self.state)?;
        let mut data = hero_content_queue(&queue_item);
        if let Some(narrator) = book.narrator.as_deref().filter(|value| !value.is_empty()) {
            data.facts.meta_rows.push(format!("Read by {narrator}"));
        }
        if let Some(year) = book
            .published_year
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            data.facts.meta_rows.push(year.to_string());
        }
        data.overview = book
            .description
            .as_deref()
            .map(clean_overview)
            .filter(|overview| !overview.is_empty());
        Some(data)
    }

    pub(in crate::app) fn hero_data(&mut self) -> Option<HeroContentData> {
        self.resolved_hero_data().map(|mut data| {
            data.facts.artwork.image = self.hero_image.clone();
            data
        })
    }

    pub(in crate::app) fn set_hero_image(&mut self, state: HeroImageState) {
        self.hero_image = state;
    }

    pub(in crate::app) fn panel_content(&mut self) -> LibraryPanelContent<'_> {
        let focused = self.focused;
        let chapter_focused = self.chapter_focused;
        let hero = self.resolved_hero_data().map(|mut data| {
            data.facts.artwork.image = self.hero_image.clone();
            HeroContent {
                facts: data.facts,
                overview: data.overview,
                credits: data.credits,
                workspace: Some(Workspace {
                    header: None,
                    selector: None,
                    list: &mut self.chapter_list,
                    focused: focused && chapter_focused,
                }),
            }
        });
        let selector = (!self.state.buckets.is_empty()).then(|| SelectorRow {
            pills: self
                .state
                .buckets
                .iter()
                .map(|bucket| bucket.label.to_string())
                .collect(),
            active: Some(self.selected_bucket),
        });
        let list = if self.state.books.is_empty() {
            self.carrier.invalidate_paint();
            let text = self.state.error.clone().unwrap_or_else(|| {
                if self.state.loading_pages.is_empty() {
                    "No audiobooks".into()
                } else {
                    "Loading audiobooks…".into()
                }
            });
            ListSlot::Empty {
                loading: !self.state.loading_pages.is_empty(),
                text,
            }
        } else {
            ListSlot::Media(&mut self.carrier)
        };
        LibraryPanelContent {
            selector,
            controls: None,
            list,
            hero,
        }
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => {
                self.select_bucket(index);
                self.bucket_request()
            }
            LibrarySlotEvent::List(input) => match input {
                MediaListSurfaceInput::Wheel { at, delta } => {
                    if self.carrier.claims_current_point(at) {
                        self.move_book(MediaListSurfaceInput::Wheel { at, delta })
                    } else {
                        None
                    }
                }
                MediaListSurfaceInput::Click(at)
                | MediaListSurfaceInput::ToggleClick(at)
                | MediaListSurfaceInput::RangeClick(at) => {
                    let target = self.carrier.resolve_current_point(at)?.clone();
                    self.carrier.delegate_operation(
                        input
                            .into_operation(Some(target))
                            .expect("resolved media-list pointer target"),
                    );
                    let _ = ();
                    self.sync_book_from_owner();
                    self.book_request()
                }
                MediaListSurfaceInput::DoubleClick(at) => {
                    let target = self.carrier.resolve_current_point(at)?.clone();
                    self.carrier.delegate_operation(
                        MediaListSurfaceInput::Click(at)
                            .into_operation(Some(target))
                            .expect("resolved media-list pointer target"),
                    );
                    self.sync_book_from_owner();
                    Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                        AudiobookshelfBookIntent::Activate,
                    )))
                }
                _ => None,
            },
            LibrarySlotEvent::WorkspaceSelectorPicked(_) | LibrarySlotEvent::ControlPicked(_) => {
                None
            }
            LibrarySlotEvent::HeroPane(input) => match input {
                MediaListSurfaceInput::Wheel { at, delta } => {
                    if self.chapter_list.claims_current_point(at) {
                        self.chapter_list.delegate_operation(
                            MediaListSurfaceInput::Wheel { at, delta }
                                .into_operation(None)
                                .expect("resolved media-list pointer target"),
                        );
                    }
                    None
                }
                MediaListSurfaceInput::Click(at)
                | MediaListSurfaceInput::ToggleClick(at)
                | MediaListSurfaceInput::RangeClick(at) => {
                    let target = self.chapter_list.resolve_current_point(at).copied()?;
                    self.enter_chapter_focus();
                    self.chapter_list.delegate_operation(
                        input
                            .into_operation(Some(target))
                            .expect("resolved media-list pointer target"),
                    );
                    self.chapter_focus_request()
                }
                MediaListSurfaceInput::DoubleClick(at)
                | MediaListSurfaceInput::ContextClick(at) => {
                    let target = self.chapter_list.resolve_current_point(at).copied()?;
                    self.enter_chapter_focus();
                    self.chapter_list.delegate_operation(
                        MediaListSurfaceInput::Click(at)
                            .into_operation(Some(target))
                            .expect("resolved media-list pointer target"),
                    );
                    Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                        AudiobookshelfBookIntent::ActivateChapter(self.chapter_target()),
                    )))
                }
                _ => None,
            },
        }
    }
}

impl Default for BookContent {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryContentOwner for BookContent {
    fn clear_selection(&mut self) {
        self.carrier.clear_selection();
        self.chapter_list.clear_selection();
    }

    fn set_selection_origin(
        &mut self,
        origin: crate::app::components::media_list::SelectionOrigin,
    ) {
        self.carrier.set_selection_origin(origin);
    }

    fn selection_summary(&self) -> Option<crate::app::components::media_list::SelectionSummary> {
        Some(self.carrier.selection_summary())
    }

    fn content(&mut self) -> LibraryPanelContent<'_> {
        self.panel_content()
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        self.on_slot_event(event)
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.carrier.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            )));
        }
        // The LibraryPanel is the framework focus boundary; reaching this
        // method already proves Books is focused.
        match key.code {
            Key::Char('[') if key.modifiers.is_empty() => {
                self.cycle_bucket(-1);
                self.bucket_request()
            }
            Key::Char(']') if key.modifiers.is_empty() => {
                self.cycle_bucket(1);
                self.bucket_request()
            }
            Key::Up | Key::Char('k') if self.chapter_focused => {
                self.move_chapter(-1);
                self.chapter_focus_request()
            }
            Key::Down | Key::Char('j') if self.chapter_focused => {
                self.move_chapter(1);
                self.chapter_focus_request()
            }
            Key::Up | Key::Char('k') => self.move_book(MediaListSurfaceInput::Move(-1)),
            Key::Down | Key::Char('j') => self.move_book(MediaListSurfaceInput::Move(1)),
            Key::PageUp if !self.chapter_focused => self.move_book(MediaListSurfaceInput::Page(-1)),
            Key::PageDown if !self.chapter_focused => {
                self.move_book(MediaListSurfaceInput::Page(1))
            }
            Key::Home if !self.chapter_focused => {
                self.select_bucket_edge(false);
                self.book_request()
            }
            Key::End if !self.chapter_focused => {
                self.select_bucket_edge(true);
                self.book_request()
            }
            Key::Esc | Key::Backspace if self.chapter_focused => {
                self.clear_chapter_focus();
                self.chapter_focus_request()
            }
            Key::Char(' ') if self.chapter_focused => {
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                    AudiobookshelfBookIntent::ActivateChapter(self.chapter_target()),
                )))
            }
            Key::Enter if self.chapter_focused => {
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                    AudiobookshelfBookIntent::ActivateChapter(self.chapter_target()),
                )))
            }
            // Enter on the selected book moves focus into the chapter
            // workspace (the same rule as grouped Music's track pane); the
            // shell decides wide focus vs narrow modal. Arrows never move
            // focus between panels.
            Key::Enter if !self.chapter_list.rows().is_empty() => Some(Msg::Shell(
                ShellRequest::AudiobookshelfBookIntent(AudiobookshelfBookIntent::FocusChapters),
            )),
            Key::Char(' ') => Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                AudiobookshelfBookIntent::Play,
            ))),
            Key::Enter => Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                AudiobookshelfBookIntent::Activate,
            ))),
            Key::Char('a')
                if !self.chapter_focused && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                    AudiobookshelfBookIntent::Enqueue,
                )))
            }
            _ => None,
        }
    }

    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        match self.on_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(message)),
            None if self.chapter_focused
                || matches!(
                    key.code,
                    Key::Up | Key::Down | Key::PageUp | Key::PageDown | Key::Home | Key::End
                ) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
    }

    fn focus_hero_workspace(&mut self) -> bool {
        self.enter_chapter_focus();
        true
    }

    fn clear_hero_workspace_focus(&mut self) {
        self.clear_chapter_focus();
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        self.hero_data()
    }

    fn set_hero_image(&mut self, state: HeroImageState) {
        self.set_hero_image(state)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_core::audiobookshelf::{
        AudiobookshelfAudioFile, AudiobookshelfBook, AudiobookshelfBookProgress,
        AudiobookshelfLibrary,
    };

    fn book(id: &str, author_sort_key: &str) -> AudiobookshelfBook {
        AudiobookshelfBook {
            library_item_id: id.into(),
            title: format!("Book {id}"),
            author_display: Some(format!("Author {id}")),
            author_sort_key: author_sort_key.into(),
            cover_path: Some(format!("{id}-cover")),
            duration_seconds: 3_600.0,
            narrator: None,
            published_year: None,
            genres: Vec::new(),
            description: None,
            series_name: None,
            chapters: Vec::new(),
            audio_files: Vec::new(),
        }
    }

    fn state_with_books(books: Vec<AudiobookshelfBook>) -> AudiobookshelfBookBrowseState {
        let mut state = AudiobookshelfBookBrowseState::new(AudiobookshelfLibrary {
            id: "lib".into(),
            name: "Books".into(),
            media_type: "book".into(),
        });
        state.append_page_books(0, books.len(), books);
        state.select(0);
        state
    }

    #[test]
    fn content_shows_one_pill_bar_for_the_surname_buckets() {
        let mut owner = BookContent::new();
        owner.set_content(&state_with_books(vec![book("book-1", "Adams")]), false);
        let bucket_count = owner.state.buckets.len();
        let content = owner.content();
        let selector = content.selector.expect("expected a selector row");
        assert_eq!(selector.pills.len(), bucket_count);
        assert!(content.controls.is_none());
    }

    #[test]
    fn content_uses_portrait_artwork_for_a_book() {
        let mut owner = BookContent::new();
        owner.set_content(&state_with_books(vec![book("book-1", "Adams")]), false);
        let content = owner.content();
        assert_eq!(
            content.hero.unwrap().facts.artwork.shape,
            super::super::library_panel::ArtworkShape::Portrait
        );
    }

    #[test]
    fn content_shows_progress_as_a_plain_meta_row() {
        let mut owner = BookContent::new();
        let mut state = state_with_books(vec![book("book-1", "Adams")]);
        state.progress.insert(
            "book-1".into(),
            AudiobookshelfBookProgress {
                library_item_id: "book-1".into(),
                current_time_seconds: 0.0,
                is_finished: true,
            },
        );
        owner.set_content(&state, false);
        let content = owner.content();
        assert!(content
            .hero
            .unwrap()
            .facts
            .meta_rows
            .iter()
            .any(|row| row == "Finished"));
    }

    #[test]
    fn content_exposes_chapters_as_the_workspace() {
        let mut owner = BookContent::new();
        let mut state = state_with_books(vec![book("book-1", "Adams")]);
        state.detail_cache.insert(
            "book-1".into(),
            (
                Vec::new(),
                vec![AudiobookshelfAudioFile {
                    index: 0,
                    ino: "ino".into(),
                    duration: 60.0,
                }],
            ),
        );
        owner.set_content(&state, false);
        let has_workspace = owner
            .content()
            .hero
            .as_ref()
            .is_some_and(|hero| hero.workspace.is_some());
        assert!(has_workspace);
        assert_eq!(owner.chapter_list.rows().len(), 1);
    }
}
