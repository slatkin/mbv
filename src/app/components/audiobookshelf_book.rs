//! Interactive Component for the Audiobookshelf book library.
//!
//! The shell projects validated browse content into this stable browser
//! instance. One shared canonical book owner (a [`MediaListCarrier`]) moves
//! between the Wide and Inline presentations and is authoritative for the book
//! cursor, scroll, and selected target; a second canonical chapter/audio-part
//! owner is authoritative for the focused chapter row. The component keeps only
//! the parent-owned chapter-pane focus, the surname-bucket chrome, and typed
//! shell intents. Row-local input reaches the owners through the common
//! delegation seam (design.md D3/D5).

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation, RowLocalInput,
    WideMediaList,
};
use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::msg::{
    AudiobookshelfBookIntent, AudiobookshelfBookMove, BookChapterTarget, Msg, ShellRequest,
};
use super::user_event::UserEvent;
use crate::app::render::{
    book_rows, render_audiobookshelf_book_content, wide_hero_presentation,
    AudiobookshelfBookGeometry, BookChapterPresentation, BookInteraction, BookPresentation,
    HomeImagePaint,
};
use crate::app::types_audiobookshelf_browse::{AudiobookshelfBookBrowseState, BookRow};

pub struct AudiobookshelfBookComponent {
    state: AudiobookshelfBookBrowseState,
    /// `false` until the first `set_content`: the initial projection adopts
    /// the shell snapshot wholesale; only later pushes reset stale
    /// component-owned fields (split-audiobookshelf-cursor-ownership D4).
    initialized: bool,
    /// Parent-owned chapter-pane focus, separate from the chapter owner's
    /// selected row (design.md D5). Entering/leaving focus never moves the
    /// chapter owner's selection; the owner re-parks it only at discrete
    /// book-identity re-projection boundaries.
    chapter_focused: bool,
    selected_bucket: usize,
    focused: bool,
    images_enabled: bool,
    geometry: AudiobookshelfBookGeometry,
    /// Whether the last rendered presentation actually exposes chapter focus.
    /// Narrow layouts may retain chapter state across a projection, so input
    /// must follow the rendered wide/chapter geometry rather than that state.
    chapters_visible: bool,
    image_paint: Option<HomeImagePaint>,
    /// One shared canonical book owner, carried by exactly one of the Wide and
    /// Inline presentations (design.md D1/D2). It owns the book cursor, scroll,
    /// and selected target across a breakpoint change; the two adapters are
    /// never synchronized.
    carrier: MediaListCarrier<String>,
    /// The persistent canonical chapter/audio-part owner (design.md D5):
    /// authoritative for the focused chapter row, scrolling, painting, and
    /// retained hits. Its rows are projected from the selected book's detail
    /// before view.
    chapter_list: WideMediaList<usize>,
    /// The presentation the last `view` painted; `None` before the first paint.
    wide: bool,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3): owns
    /// the double-click window and wheel throttle. Not a shared clock.
    mouse_gestures: MouseGestureState,
}

impl AudiobookshelfBookComponent {
    pub fn new() -> Self {
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
            geometry: AudiobookshelfBookGeometry::default(),
            chapters_visible: false,
            image_paint: None,
            carrier: MediaListCarrier::new(Presentation::Inline),
            chapter_list: WideMediaList::new(),
            wide: false,
            mouse_gestures: MouseGestureState::new(),
        }
    }

    /// The presentation the painted breakpoint currently selects (design.md
    /// D2).
    fn active_presentation(&self) -> Presentation {
        if self.wide {
            Presentation::Wide
        } else {
            Presentation::Inline
        }
    }

    /// Move the shared book owner into the active presentation when they
    /// diverge. A responsive change reads the same owner and preserves only the
    /// outgoing selected-row viewport offset (design.md D1); no cursor, scroll,
    /// or selection is copied between presentations.
    fn ensure_carrier(&mut self) {
        let viewport_height = self.geometry.left_area.height.max(1) as usize;
        self.carrier
            .ensure_presentation(self.active_presentation(), viewport_height);
    }

    /// Test-only: drive framework focus the way `Component::attr` does.
    #[cfg(test)]
    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
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

    pub(in crate::app) fn take_image_paint(&mut self) -> Option<HomeImagePaint> {
        self.image_paint.take()
    }

    /// The geometry the component computed during its last `view`, exposed so
    /// the shell can anchor the context menu (task 5.3d.13, render ownership).
    pub(in crate::app) fn geometry(&self) -> &AudiobookshelfBookGeometry {
        &self.geometry
    }

    /// Whether the parent-owned chapter pane currently has focus (design.md
    /// D5). Independent of the chapter owner's selected row.
    pub(in crate::app) fn chapter_focused(&self) -> bool {
        self.chapter_focused
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

    /// The chapter owner's book-qualified stable target, resolved only while
    /// the chapter pane holds focus (design.md D4/D5). Activation never
    /// re-derives it from a numeric display position.
    fn chapter_target(&self) -> Option<BookChapterTarget> {
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

    #[cfg(test)]
    pub(crate) fn selected_book_id(&self) -> Option<&str> {
        self.carrier.selected_target().map(String::as_str)
    }

    #[cfg(test)]
    pub(crate) fn selected_row_offset_for_test(&self) -> Option<usize> {
        let height = self.geometry.left_area.height.max(1) as usize;
        if self.carrier.active() == Presentation::Wide {
            self.carrier.wide().selected_row_offset(height)
        } else {
            self.carrier.inline().selected_row_offset(height)
        }
    }

    #[cfg(test)]
    pub(crate) fn book_row_rect_for_test(&self, row: usize) -> Option<Rect> {
        let content = if self.carrier.active() == Presentation::Wide {
            self.carrier.wide().current_content_rect()?
        } else {
            self.carrier.inline().current_content_rect()?
        };
        Some(Rect {
            y: content.y + row as u16,
            height: 1,
            ..content
        })
    }

    #[cfg(test)]
    pub(crate) fn selected_bucket(&self) -> usize {
        self.selected_bucket
    }

    /// Test-only: the chapter owner's retained current-frame content rect, the
    /// shared geometry the chapter hit resolution uses (design.md D6).
    #[cfg(test)]
    pub(crate) fn chapter_content_rect_for_test(&self) -> Option<Rect> {
        self.chapter_list.current_content_rect()
    }

    /// The page stride from the component's own painted geometry
    /// (split-audiobookshelf-cursor-ownership D1): the list/content area's
    /// height minus its header line — the same value `App::lib_page_size()`
    /// derived from the projected `left_area`, now sourced locally so the
    /// shell applies no competing stride.
    fn page_size(&self) -> usize {
        (self.geometry.left_area.height as usize)
            .saturating_sub(1)
            .max(1)
    }

    fn book_request(&self) -> Option<Msg> {
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::Book(self.carrier.selected_target().cloned()),
        )))
    }

    fn bucket_request(&self) -> Option<Msg> {
        Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::Bucket(self.selected_bucket),
        )))
    }

    fn chapter_focus_request(&self) -> Option<Msg> {
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
    fn move_book(&mut self, input: RowLocalInput) -> Option<Msg> {
        self.carrier.delegate(input, None);
        self.sync_book_from_owner();
        self.book_request()
    }

    /// Mirror the book owner's stable selection into the projected snapshot,
    /// resetting the derived chapter-pane focus and re-projecting the chapter
    /// rows only at a discrete book-identity boundary (design.md D5).
    fn sync_book_from_owner(&mut self) {
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
    fn select_book_index(&mut self, index: usize) {
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
        let rows = book_rows(&self.state, self.selected_bucket);
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

    fn select_bucket(&mut self, bucket_pos: usize) {
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

    fn cycle_bucket(&mut self, delta: i64) {
        let count = self.state.buckets.len();
        if count == 0 {
            return;
        }
        let next = (self.selected_bucket as i64 + delta).rem_euclid(count as i64) as usize;
        self.select_bucket(next);
    }

    fn select_bucket_edge(&mut self, end: bool) {
        if let Some(bucket) = self.state.buckets.get(self.selected_bucket).copied() {
            if bucket.end > bucket.start {
                self.select_book_index(if end { bucket.end - 1 } else { bucket.start });
            }
        }
    }

    fn move_chapter(&mut self, delta: i64) {
        self.chapter_list.delegate(RowLocalInput::Move(delta), None);
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if !self.focused {
            return None;
        }

        let chapters_focused = self.chapters_visible && self.chapter_focused;
        match key.code {
            Key::Char('[') if key.modifiers.is_empty() => {
                self.cycle_bucket(-1);
                self.bucket_request()
            }
            Key::Char(']') if key.modifiers.is_empty() => {
                self.cycle_bucket(1);
                self.bucket_request()
            }
            Key::Up | Key::Char('k') if chapters_focused => {
                self.move_chapter(-1);
                self.chapter_focus_request()
            }
            Key::Down | Key::Char('j') if chapters_focused => {
                self.move_chapter(1);
                self.chapter_focus_request()
            }
            Key::Right if chapters_focused => {
                self.clear_chapter_focus();
                self.chapter_focus_request()
            }
            Key::Left if self.chapters_visible && !self.chapter_focused => {
                self.enter_chapter_focus();
                self.chapter_focus_request()
            }
            Key::Up | Key::Char('k') => self.move_book(RowLocalInput::Move(-1)),
            Key::Down | Key::Char('j') => self.move_book(RowLocalInput::Move(1)),
            Key::PageUp if !chapters_focused => {
                let rows = self.page_size() as i64;
                self.move_book(RowLocalInput::Move(-rows))
            }
            Key::PageDown if !chapters_focused => {
                let rows = self.page_size() as i64;
                self.move_book(RowLocalInput::Move(rows))
            }
            Key::Home if !chapters_focused => {
                self.select_bucket_edge(false);
                self.book_request()
            }
            Key::End if !chapters_focused => {
                self.select_bucket_edge(true);
                self.book_request()
            }
            Key::Esc | Key::Backspace if chapters_focused => {
                self.clear_chapter_focus();
                self.chapter_focus_request()
            }
            Key::Char(' ') if chapters_focused => {
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                    AudiobookshelfBookIntent::ActivateChapter(self.chapter_target()),
                )))
            }
            Key::Enter if chapters_focused => {
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                    AudiobookshelfBookIntent::ActivateChapter(self.chapter_target()),
                )))
            }
            Key::Char(' ') => Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                AudiobookshelfBookIntent::Play,
            ))),
            Key::Enter => Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                AudiobookshelfBookIntent::Activate,
            ))),
            Key::Char('a')
                if !chapters_focused && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                    AudiobookshelfBookIntent::Enqueue,
                )))
            }
            _ => None,
        }
    }

    /// Handle pointer input using retained geometry from the active book owner
    /// and the chapter owner (design.md D6).
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        // The book surface does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Click(at) => {
                if let Some((_, bucket)) = self
                    .geometry
                    .selector_tabs
                    .iter()
                    .find(|(rect, _)| rect.contains(at))
                {
                    self.select_bucket(*bucket);
                    return self.bucket_request();
                }
                if let Some(target) = self.carrier.resolve_current_point(at).cloned() {
                    self.carrier
                        .delegate(RowLocalInput::Click(at), Some(target));
                    self.sync_book_from_owner();
                    return self.book_request();
                }
                if self.chapters_visible {
                    if let Some(target) = self.chapter_list.resolve_current_point(at).copied() {
                        self.enter_chapter_focus();
                        self.chapter_list
                            .delegate(RowLocalInput::Click(at), Some(target));
                        return self.chapter_focus_request();
                    }
                }
                None
            }
            MouseGesture::DoubleClick(at) => {
                self.carrier
                    .resolve_current_point(at)
                    .cloned()
                    .map(|target| {
                        self.carrier
                            .delegate(RowLocalInput::Click(at), Some(target));
                        self.sync_book_from_owner();
                        Msg::Shell(ShellRequest::AudiobookshelfBookIntent(
                            AudiobookshelfBookIntent::Activate,
                        ))
                    })
            }
            MouseGesture::Scroll { at, delta } => {
                let chapter_focus = self.chapters_visible && self.chapter_focused;
                if chapter_focus {
                    if !self.chapter_list.claims_current_point(at) {
                        return None;
                    }
                    self.move_chapter(delta);
                    self.chapter_focus_request()
                } else {
                    if !self.carrier.claims_current_point(at) {
                        return None;
                    }
                    self.move_book(RowLocalInput::Move(delta))
                }
            }
            _ => None,
        }
    }

    /// Test seam: reset the private gesture recognizer so a synchronous test
    /// loop can drive successive wheel/click events without the
    /// throttle/double-click window collapsing them.
    #[cfg(test)]
    pub(crate) fn reset_mouse_gestures_for_test(&mut self) {
        self.mouse_gestures.reset_for_test();
    }
}

/// Canonical row projection for one book's chapter/audio-part detail: one
/// selectable `Item` per visible row, keyed by its stable row discriminator.
/// Chapters fall back to audio files when the book exposes no chapters
/// (book-browsing spec: never an empty or broken list state).
fn chapter_rows(state: &AudiobookshelfBookBrowseState, id: &str) -> Vec<MediaListRow<usize>> {
    state
        .visible_rows(id)
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let (primary, duration) = match row {
                BookRow::Chapter { title, start, end } => (
                    title,
                    crate::app::ui_util::list_duration_secs((end - start).max(0.0) as i64),
                ),
                BookRow::AudioFile { index, duration } => (
                    format!("Part {index}"),
                    crate::app::ui_util::list_duration_secs(duration as i64),
                ),
            };
            MediaListRow::Item {
                target: index,
                primary,
                trailing: None,
                duration,
                kind: MediaKind::Media,
                semantic_state: MediaSemanticState::Ordinary,
            }
        })
        .collect()
}

impl Default for AudiobookshelfBookComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AudiobookshelfBookComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // Chapter focus belongs only to the rendered wide Wide hero
        // presentation. Clear it before painting a narrow frame so a
        // wide→narrow resize cannot leave keyboard input targeting a hidden
        // chapter pane.
        self.wide = wide_hero_presentation(area).is_some();
        self.ensure_carrier();
        self.chapters_visible = self.wide && !self.chapter_list.is_empty();
        if !self.chapters_visible {
            self.chapter_focused = false;
        }
        let book_presentation = if self.wide {
            BookPresentation::Wide(self.carrier.wide_mut())
        } else {
            BookPresentation::Inline(self.carrier.inline_mut())
        };
        self.image_paint = render_audiobookshelf_book_content(
            frame,
            area,
            self.focused,
            &self.state,
            BookInteraction {
                chapter_focused: self.chapter_focused,
                selected_bucket: self.selected_bucket,
            },
            self.images_enabled,
            &mut self.geometry,
            book_presentation,
            BookChapterPresentation::Wide(&mut self.chapter_list),
        );
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus {
            self.focused = matches!(value, AttrValue::Flag(true));
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for AudiobookshelfBookComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => {
                self.ensure_carrier();
                self.handle_key(key)
            }
            Event::Mouse(mouse) => {
                self.ensure_carrier();
                self.handle_mouse(mouse)
            }
            _ => None,
        }
    }
}
