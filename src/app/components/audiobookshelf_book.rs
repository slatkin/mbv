use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{
    AudiobookshelfBookIntent, AudiobookshelfBookMove, LegacyTerminalEvent, Msg, ShellRequest,
};
use super::user_event::UserEvent;
use crate::app::render::{
    render_audiobookshelf_book_content, AudiobookshelfBookGeometry, HomeImagePaint,
};
use crate::app::types_audiobookshelf_browse::AudiobookshelfBookBrowseState;

pub struct AudiobookshelfBookComponent {
    state: AudiobookshelfBookBrowseState,
    focused: bool,
    images_enabled: bool,
    geometry: AudiobookshelfBookGeometry,
    /// Shell-provided page stride matching `App::lib_page_size`; the painted
    /// book rows are not a reliable source when an inline hero replaces rows.
    page_size: usize,
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
            focused: false,
            images_enabled: false,
            geometry: AudiobookshelfBookGeometry::default(),
            page_size: 1,
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
        let selected_id = self.state.selected_id.clone();
        let chapter_selection = self.state.chapter_selection;
        let scroll = self.state.scroll;
        let selected_bucket = self.state.selected_bucket;
        self.state = snapshot.clone();
        if selected_id.as_ref().is_some_and(|id| {
            self.state
                .books
                .iter()
                .any(|book| &book.library_item_id == id)
        }) {
            self.state.selected_id = selected_id;
            self.state.chapter_selection = chapter_selection;
            self.state.scroll = scroll;
            self.state.selected_bucket =
                selected_bucket.min(self.state.buckets.len().saturating_sub(1));
        }
        self.focused = focused;
        self.images_enabled = images_enabled;
    }

    pub(in crate::app) fn take_image_paint(&mut self) -> Option<HomeImagePaint> {
        self.image_paint.take()
    }

    /// Shell projection of the existing App page-size contract. The component
    /// cannot derive this from `book_rows`: inline replacement heroes can make
    /// that painted-row list empty or partial.
    pub(in crate::app) fn set_page_size(&mut self, page_size: usize) {
        self.page_size = page_size.max(1);
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

    fn page_size(&self) -> usize {
        self.page_size
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
            return Some(Msg::Legacy(LegacyTerminalEvent::Key(
                to_crossterm_key_event(key),
            )));
        }

        let chapters_focused = self.chapters_visible && self.state.chapter_selection.is_some();
        match key.code {
            Key::Char('[') if key.modifiers.is_empty() => {
                self.cycle_bucket(-1);
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::PreviousBucket,
                )))
            }
            Key::Char(']') if key.modifiers.is_empty() => {
                self.cycle_bucket(1);
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::NextBucket,
                )))
            }
            Key::Up | Key::Char('k') if chapters_focused => {
                self.move_chapter(-1);
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::PreviousChapter,
                )))
            }
            Key::Down | Key::Char('j') if chapters_focused => {
                self.move_chapter(1);
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::NextChapter,
                )))
            }
            Key::Right if chapters_focused => {
                self.state.chapter_selection = None;
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::FocusBrowser,
                )))
            }
            Key::Left if self.chapters_visible && !chapters_focused => {
                self.state.chapter_selection = Some(0);
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::FocusChapters,
                )))
            }
            Key::Up | Key::Char('k') => {
                self.move_book(-1);
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::PreviousBookRow,
                )))
            }
            Key::Down | Key::Char('j') => {
                self.move_book(1);
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::NextBookRow,
                )))
            }
            Key::PageUp if !chapters_focused => {
                self.move_book(-(self.page_size() as i64));
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::PreviousBookPage,
                )))
            }
            Key::PageDown if !chapters_focused => {
                self.move_book(self.page_size() as i64);
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::NextBookPage,
                )))
            }
            Key::Home if !chapters_focused => {
                self.select_bucket_edge(false);
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::FirstBook,
                )))
            }
            Key::End if !chapters_focused => {
                self.select_bucket_edge(true);
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::LastBook,
                )))
            }
            Key::Esc | Key::Backspace if chapters_focused => {
                self.state.chapter_selection = None;
                Some(Msg::Shell(ShellRequest::AudiobookshelfBookMove(
                    AudiobookshelfBookMove::FocusBrowser,
                )))
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
            _ => Some(Msg::Legacy(LegacyTerminalEvent::Key(
                to_crossterm_key_event(key),
            ))),
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
        let mouse = to_crossterm_mouse_event(mouse);
        let position: ratatui::layout::Position = (mouse.column, mouse.row).into();
        if matches!(
            mouse.kind,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        ) {
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
        Some(Msg::Legacy(LegacyTerminalEvent::Mouse(mouse)))
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
