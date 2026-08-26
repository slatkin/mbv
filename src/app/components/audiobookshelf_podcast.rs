//! Interactive Component for one Audiobookshelf podcast library.
//!
//! The shell mirrors validated browse content into this stable browser
//! instance. Show, episode, filter, and scroll state stays local here; the
//! legacy App handler remains the shell-owned effect path during group 5's
//! teardown.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::legacy_input::{to_crossterm_key_event, to_crossterm_mouse_event};
use super::msg::{LegacyTerminalEvent, Msg, PodcastShowMove, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::{render_audiobookshelf_podcast_content, AudiobookshelfPodcastGeometry};
use crate::app::types_audiobookshelf_browse::{
    AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter,
};

pub struct AudiobookshelfPodcastComponent {
    state: AudiobookshelfBrowseState,
    initialized: bool,
    focused: bool,
    geometry: AudiobookshelfPodcastGeometry,
}

impl AudiobookshelfPodcastComponent {
    pub fn new() -> Self {
        Self {
            state: AudiobookshelfBrowseState::new(
                mbv_core::audiobookshelf::AudiobookshelfLibrary {
                    id: String::new(),
                    name: String::new(),
                    media_type: "podcast".into(),
                },
            ),
            initialized: false,
            focused: false,
            geometry: AudiobookshelfPodcastGeometry::default(),
        }
    }

    pub(in crate::app) fn set_content(
        &mut self,
        snapshot: &AudiobookshelfBrowseState,
        focused: bool,
        _images_enabled: bool,
    ) {
        let selected_id = self
            .initialized
            .then(|| self.state.selected_id.clone())
            .flatten();
        let episode_filter = self.state.episode_filter;
        let episode_selection = self.state.episode_selection;
        let scroll = self.state.scroll;
        self.state = snapshot.clone();
        if let Some(id) = selected_id.filter(|id| {
            self.state
                .shows
                .iter()
                .any(|show| &show.library_item_id == id)
        }) {
            self.state.selected_id = Some(id);
            self.state.episodes = self
                .state
                .selected_id
                .as_ref()
                .and_then(|id| self.state.detail_cache.get(id).cloned())
                .or_else(|| snapshot.episodes.clone());
            self.state.episode_filter = episode_filter;
            self.state.episode_selection = episode_selection;
            self.state.scroll = scroll;
        }
        self.initialized = true;
        self.focused = focused;
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.state.cursor()
    }

    fn move_cursor(&mut self, delta: i64) {
        let cursor = self.state.cursor();
        let count = self.state.shows.len();
        if count == 0 {
            return;
        }
        let next = crate::app::ui_util::move_cursor(cursor, delta, count);
        self.state.select(next);
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Up | Key::Char('k') if self.state.episode_selection.is_none() => {
                self.move_cursor(-1);
                return Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(
                    PodcastShowMove::PreviousRow,
                )));
            }
            Key::Down | Key::Char('j') if self.state.episode_selection.is_none() => {
                self.move_cursor(1);
                return Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(
                    PodcastShowMove::NextRow,
                )));
            }
            Key::Left | Key::Char('h') if self.state.episode_selection.is_none() => {
                self.move_cursor(-1);
                return Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(
                    PodcastShowMove::PreviousItem,
                )));
            }
            Key::Right | Key::Char('l') if self.state.episode_selection.is_none() => {
                self.move_cursor(1);
                return Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(
                    PodcastShowMove::NextItem,
                )));
            }
            Key::PageUp if self.state.episode_selection.is_none() => {
                self.move_cursor(-(self.page_size() as i64));
                return Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(
                    PodcastShowMove::PreviousPage,
                )));
            }
            Key::PageDown if self.state.episode_selection.is_none() => {
                self.move_cursor(self.page_size() as i64);
                return Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(
                    PodcastShowMove::NextPage,
                )));
            }
            Key::Home if self.state.episode_selection.is_none() => {
                self.state.select(0);
                return Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(
                    PodcastShowMove::First,
                )));
            }
            Key::End if self.state.episode_selection.is_none() => {
                self.state.select(self.state.shows.len().saturating_sub(1));
                return Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(
                    PodcastShowMove::Last,
                )));
            }
            Key::Up | Key::Char('k') => self.move_episode(-1),
            Key::Down | Key::Char('j') => self.move_episode(1),
            Key::Char('[') if self.state.episode_selection.is_some() => self.cycle_filter(-1),
            Key::Char(']') if self.state.episode_selection.is_some() => self.cycle_filter(1),
            Key::Esc | Key::Backspace if self.state.episode_selection.is_some() => {
                self.state.episode_selection = None;
            }
            _ => {}
        }
        Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastKey(
            to_crossterm_key_event(key),
        )))
    }

    fn page_size(&self) -> usize {
        self.geometry.show_rows.len().max(1)
    }

    fn move_episode(&mut self, delta: i64) {
        let count = self.state.visible_episodes().len();
        if count == 0 {
            return;
        }
        let current = self.state.episode_selection.unwrap_or(0);
        self.state.episode_selection =
            Some(crate::app::ui_util::move_cursor(current, delta, count));
    }

    fn cycle_filter(&mut self, delta: i64) {
        let current = AudiobookshelfEpisodeFilter::ALL
            .iter()
            .position(|filter| *filter == self.state.episode_filter)
            .unwrap_or(0);
        let next = crate::app::ui_util::move_cursor(
            current,
            delta,
            AudiobookshelfEpisodeFilter::ALL.len(),
        );
        self.state
            .set_episode_filter(AudiobookshelfEpisodeFilter::ALL[next]);
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        let mouse = to_crossterm_mouse_event(mouse);
        if matches!(
            mouse.kind,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        ) {
            let pos: ratatui::layout::Position = (mouse.column, mouse.row).into();
            if let Some((_, index)) = self
                .geometry
                .show_rows
                .iter()
                .find(|(rect, _)| rect.contains(pos))
            {
                self.state.select(*index);
            }
            if let Some((_, bucket)) = self
                .geometry
                .selector_tabs
                .iter()
                .find(|(rect, _)| rect.contains(pos))
            {
                if let Some(range) =
                    crate::app::types_audiobookshelf_browse::build_show_title_buckets(
                        &self.state.shows,
                    )
                    .get(*bucket)
                {
                    self.state.select(range.start);
                }
            }
        }
        Some(Msg::Legacy(LegacyTerminalEvent::Mouse(mouse)))
    }
}

impl Default for AudiobookshelfPodcastComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AudiobookshelfPodcastComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        render_audiobookshelf_podcast_content(
            frame,
            area,
            self.focused,
            &mut self.state,
            &mut self.geometry,
        );
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

impl AppComponent<Msg, UserEvent> for AudiobookshelfPodcastComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
