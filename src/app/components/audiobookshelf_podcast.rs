//! Interactive Component for one Audiobookshelf podcast library.
//!
//! The shell mirrors validated browse content into this stable browser
//! instance. Show, episode, filter, and scroll state stays local here; typed
//! shell requests remain the shell-owned effect path.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{
    Msg, PodcastEpisodeIntent, PodcastEpisodeTransition, PodcastShowMove, ShellRequest,
};
use super::typed_key::to_crossterm_key_event;
use super::user_event::UserEvent;
use crate::app::render::{
    render_audiobookshelf_podcast_content, AudiobookshelfPodcastGeometry, HomeImagePaint,
};
use crate::app::types_audiobookshelf_browse::{
    AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter,
};

pub struct AudiobookshelfPodcastComponent {
    state: AudiobookshelfBrowseState,
    initialized: bool,
    focused: bool,
    images_enabled: bool,
    geometry: AudiobookshelfPodcastGeometry,
    image_paint: Option<HomeImagePaint>,
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
            images_enabled: false,
            geometry: AudiobookshelfPodcastGeometry::default(),
            image_paint: None,
        }
    }

    pub(in crate::app) fn set_content(
        &mut self,
        snapshot: &AudiobookshelfBrowseState,
        focused: bool,
        images_enabled: bool,
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
        self.images_enabled = images_enabled;
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.state.cursor()
    }

    /// Re-home accessors (task 5.3d.11 U0): owned/copy views of the shared
    /// `AudiobookshelfBrowseState` members the App-level readers read. The
    /// state struct is shared with `App.audiobookshelf_browse`, so these let
    /// the shell read the component's authoritative selection without touching
    /// the legacy App readers.
    pub(in crate::app) fn selected_id(&self) -> Option<String> {
        self.state.selected_id.clone()
    }

    pub(in crate::app) fn episode_selection(&self) -> Option<usize> {
        self.state.episode_selection
    }

    pub(in crate::app) fn episode_filter(&self) -> AudiobookshelfEpisodeFilter {
        self.state.episode_filter
    }

    /// Re-home mutators (task 5.3d.11 U0): write the shared browse state the
    /// legacy handlers currently touch via `App.audiobookshelf_browse`.
    /// `set_episode_filter` delegates to the state's existing reset semantics
    /// (drops any in-progress episode selection to `0`).
    pub(in crate::app) fn set_episode_filter(&mut self, filter: AudiobookshelfEpisodeFilter) {
        self.state.set_episode_filter(filter);
    }

    pub(in crate::app) fn set_episode_selection(&mut self, selection: Option<usize>) {
        self.state.episode_selection = selection;
    }

    /// The image-paint plan this component computed during its last `view`
    /// (task 5.3d.10b): `Some` only when images are enabled, a selected show
    /// hero was actually admitted/painted, and the hero reserved an image
    /// rect. Replaced on every `view`, taken once by the shell after paint.
    pub(in crate::app) fn take_image_paint(&mut self) -> Option<HomeImagePaint> {
        self.image_paint.take()
    }

    /// The geometry the component computed during its last `view`, exposed so
    /// the shell can anchor overlays / read painted areas (task 5.3d.10c,
    /// render ownership). Immutable: the component owns painting; callers do
    /// not write back.
    pub(in crate::app) fn geometry(&self) -> &AudiobookshelfPodcastGeometry {
        &self.geometry
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
            Key::Up | Key::Char('k') => {
                self.move_episode(-1);
                return Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                        PodcastEpisodeTransition::PreviousEpisode,
                    ),
                ));
            }
            Key::Down | Key::Char('j') => {
                self.move_episode(1);
                return Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                        PodcastEpisodeTransition::NextEpisode,
                    ),
                ));
            }
            Key::Char('[') if self.state.episode_selection.is_some() => {
                self.cycle_filter(-1);
                return Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                        PodcastEpisodeTransition::PreviousFilter,
                    ),
                ));
            }
            Key::Char(']') if self.state.episode_selection.is_some() => {
                self.cycle_filter(1);
                return Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                        PodcastEpisodeTransition::NextFilter,
                    ),
                ));
            }
            Key::Esc | Key::Backspace if self.state.episode_selection.is_some() => {
                self.state.episode_selection = None;
                return Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                        PodcastEpisodeTransition::Exit,
                    ),
                ));
            }
            // Space/Enter/Ctrl+A action intents (task 5.3d.7): the component
            // only reports the matched intent; the shell resolves the
            // episode-selection and wide/narrow conditions from App state at
            // the Model boundary and runs the existing App effect (D17).
            Key::Char(' ') => {
                return Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeIntent(
                        PodcastEpisodeIntent::FocusOrPlay,
                    ),
                ));
            }
            Key::Enter => {
                return Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeIntent(
                        PodcastEpisodeIntent::OpenOrPlay,
                    ),
                ));
            }
            Key::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::Enqueue),
                ));
            }
            _ => Some(Msg::Shell(ShellRequest::GlobalViewKey(
                to_crossterm_key_event(key),
            ))),
        }
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
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
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
        None
    }
}

impl Default for AudiobookshelfPodcastComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AudiobookshelfPodcastComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.image_paint = render_audiobookshelf_podcast_content(
            frame,
            area,
            self.focused,
            self.images_enabled,
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
