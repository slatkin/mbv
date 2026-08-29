use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{AudiobookshelfBookIntent, AudiobookshelfBookMove, Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::{
    render_audiobookshelf_book_content, AudiobookshelfBookGeometry, HomeImagePaint,
};
use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;

pub struct AudiobookshelfBookComponent {
    state: AudiobookshelfBookBrowseState,
    /// `false` until the first `set_content`: the initial projection adopts
    /// the shell snapshot wholesale; only later pushes reset stale
    /// component-owned fields (split-audiobookshelf-cursor-ownership D4).
    initialized: bool,
    browser_offset: usize,
    focused: bool,
    images_enabled: bool,
    geometry: AudiobookshelfBookGeometry,
    /// Whether the last rendered presentation actually exposes chapter focus.
    /// Narrow layouts may retain chapter state across a projection, so input
    /// must follow the rendered wide/chapter geometry rather than that state.
    chapters_visible: bool,
    image_paint: Option<HomeImagePaint>,
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
            browser_offset: 0,
            focused: false,
            images_enabled: false,
            geometry: AudiobookshelfBookGeometry::default(),
            chapters_visible: false,
            image_paint: None,
        }
    }

    pub(in crate::app) fn set_content(
        &mut self,
        snapshot: &AudiobookshelfBookBrowseState,
        focused: bool,
        images_enabled: bool,
    ) {
        // The component's own interaction values win over the incoming
        // snapshot unconditionally (split-audiobookshelf-cursor-ownership D4).
        // When the book the component had selected is gone from the new
        // content, its own fields reset to their defaults — the snapshot's
        // `chapter_selection` / `selected_bucket` are never adopted.
        let carried = self.initialized.then(|| {
            (
                self.state.selected_id.clone(),
                self.state.chapter_selection,
                self.browser_offset,
                self.state.selected_bucket,
            )
        });
        self.state = snapshot.clone();
        let survivor = carried.filter(|(id, ..)| {
            id.as_ref().is_some_and(|id| {
                self.state
                    .books
                    .iter()
                    .any(|book| &book.library_item_id == id)
            })
        });
        if let Some((id, chapter_selection, browser_offset, selected_bucket)) = survivor {
            self.state.selected_id = id;
            self.state.chapter_selection = chapter_selection;
            self.browser_offset = browser_offset;
            self.state.selected_bucket = selected_bucket;
        } else if self.initialized {
            self.state.chapter_selection = None;
            self.browser_offset = 0;
        }
        self.state.selected_bucket = self
            .state
            .selected_bucket
            .min(self.state.buckets.len().saturating_sub(1));
        self.initialized = true;
        self.focused = focused;
        self.images_enabled = images_enabled;
    }

    pub(in crate::app) fn take_image_paint(&mut self) -> Option<HomeImagePaint> {
        self.image_paint.take()
    }

    /// The geometry the component computed during its last `view`, exposed so
    /// the shell can anchor the context menu (task 5.3d.13, render ownership).
    pub(in crate::app) fn geometry(&self) -> &AudiobookshelfBookGeometry {
        &self.geometry
    }

    pub(in crate::app) fn chapter_selection(&self) -> Option<usize> {
        self.state.chapter_selection
    }

    #[cfg(test)]
    pub(crate) fn selected_book_id(&self) -> Option<&str> {
        self.state.selected_id.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn selected_bucket(&self) -> usize {
        self.state.selected_bucket
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

    fn book_request(&self) -> Msg {
        Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::Book(self.state.cursor()),
        ))
    }

    fn bucket_request(&self) -> Msg {
        Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::Bucket(self.state.selected_bucket),
        ))
    }

    fn chapter_focus_request(&self) -> Msg {
        Msg::Shell(ShellRequest::AudiobookshelfBookMove(
            AudiobookshelfBookMove::ChapterFocus(self.state.chapter_selection),
        ))
    }

    fn move_book(&mut self, delta: i64) {
        let Some(bucket) = self.state.buckets.get(self.state.selected_bucket) else {
            return;
        };
        if bucket.end <= bucket.start {
            return;
        }
        let cursor = (self.state.cursor() as i64).clamp(bucket.start as i64, bucket.end as i64 - 1);
        self.state
            .select((cursor + delta).clamp(bucket.start as i64, bucket.end as i64 - 1) as usize);
    }

    fn move_chapter(&mut self, delta: i64) {
        let Some(id) = self.state.selected_id.as_deref() else {
            return;
        };
        let count = self.state.visible_rows(id).len();
        if count > 0 {
            self.state.chapter_selection = Some(crate::app::ui_util::move_cursor(
                self.state.chapter_selection.unwrap_or(0),
                delta,
                count,
            ));
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if !self.focused {
            return None;
        }

        let chapters_focused = self.chapters_visible && self.state.chapter_selection.is_some();
        match key.code {
            Key::Char('[') if key.modifiers.is_empty() => {
                self.cycle_bucket(-1);
                Some(self.bucket_request())
            }
            Key::Char(']') if key.modifiers.is_empty() => {
                self.cycle_bucket(1);
                Some(self.bucket_request())
            }
            Key::Up | Key::Char('k') if chapters_focused => {
                self.move_chapter(-1);
                Some(self.chapter_focus_request())
            }
            Key::Down | Key::Char('j') if chapters_focused => {
                self.move_chapter(1);
                Some(self.chapter_focus_request())
            }
            Key::Right if chapters_focused => {
                self.state.chapter_selection = None;
                Some(self.chapter_focus_request())
            }
            Key::Left if self.chapters_visible && !chapters_focused => {
                self.state.chapter_selection = Some(0);
                Some(self.chapter_focus_request())
            }
            Key::Up | Key::Char('k') => {
                self.move_book(-1);
                Some(self.book_request())
            }
            Key::Down | Key::Char('j') => {
                self.move_book(1);
                Some(self.book_request())
            }
            Key::PageUp if !chapters_focused => {
                self.move_book(-(self.page_size() as i64));
                Some(self.book_request())
            }
            Key::PageDown if !chapters_focused => {
                self.move_book(self.page_size() as i64);
                Some(self.book_request())
            }
            Key::Home if !chapters_focused => {
                self.select_bucket_edge(false);
                Some(self.book_request())
            }
            Key::End if !chapters_focused => {
                self.select_bucket_edge(true);
                Some(self.book_request())
            }
            Key::Esc | Key::Backspace if chapters_focused => {
                self.state.chapter_selection = None;
                Some(self.chapter_focus_request())
            }
            Key::Char(' ') if chapters_focused => Some(Msg::Shell(
                ShellRequest::AudiobookshelfBookIntent(AudiobookshelfBookIntent::ActivateChapter),
            )),
            Key::Enter if chapters_focused => Some(Msg::Shell(
                ShellRequest::AudiobookshelfBookIntent(AudiobookshelfBookIntent::ActivateChapter),
            )),
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

    fn cycle_bucket(&mut self, delta: i64) {
        let count = self.state.buckets.len();
        if count > 0 {
            self.state.selected_bucket =
                (self.state.selected_bucket as i64 + delta).rem_euclid(count as i64) as usize;
            if let Some(bucket) = self.state.buckets.get(self.state.selected_bucket).copied() {
                if !(bucket.start..bucket.end).contains(&self.state.cursor()) {
                    self.state.select(bucket.start);
                }
            }
        }
    }

    fn select_bucket_edge(&mut self, end: bool) {
        if let Some(bucket) = self.state.buckets.get(self.state.selected_bucket).copied() {
            if bucket.end > bucket.start {
                self.state
                    .select(if end { bucket.end - 1 } else { bucket.start });
            }
        }
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let position: ratatui::layout::Position = (mouse.column, mouse.row).into();
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            if let Some((_, index)) = self
                .geometry
                .book_rows
                .iter()
                .find(|(rect, _)| rect.contains(position))
            {
                self.state.select(*index);
            }
            if self.chapters_visible {
                if let Some((_, index)) = self
                    .geometry
                    .chapter_rows
                    .iter()
                    .find(|(rect, _)| rect.contains(position))
                {
                    self.state.chapter_selection = Some(*index);
                }
            }
            if let Some((_, bucket)) = self
                .geometry
                .selector_tabs
                .iter()
                .find(|(rect, _)| rect.contains(position))
            {
                if let Some(range) = self.state.buckets.get(*bucket).copied() {
                    self.state.selected_bucket = *bucket;
                    self.state.select(range.start);
                }
            }
        }
        None
    }
}

impl Default for AudiobookshelfBookComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AudiobookshelfBookComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // Chapter focus belongs only to the rendered wide hero-on-left
        // presentation. Clear it before painting a narrow frame so a
        // wide→narrow resize cannot leave keyboard input targeting a hidden
        // chapter pane.
        self.chapters_visible = crate::app::render::shared_hero_presentation(area).is_some();
        if !self.chapters_visible {
            self.state.chapter_selection = None;
        }
        self.image_paint = render_audiobookshelf_book_content(
            frame,
            area,
            self.focused,
            &mut self.state,
            self.images_enabled,
            &mut self.geometry,
            &mut self.browser_offset,
        );
        // A wide frame can still have no painted chapter rows (for example
        // while detail is loading or when the selected book has no chapters).
        // Do not advertise focus for geometry that was not rendered.
        if self.geometry.chapter_rows.is_empty() {
            self.chapters_visible = false;
            self.state.chapter_selection = None;
        }
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }
    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}
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
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
