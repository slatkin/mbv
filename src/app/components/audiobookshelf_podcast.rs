//! Interactive Component for one Audiobookshelf podcast library.
//!
//! The shell projects validated browse content into this stable browser
//! instance. One shared canonical show owner (a [`MediaListCarrier`]) moves
//! between the Wide and Inline presentations and is authoritative for the show
//! cursor, scroll, and selected target; a second canonical episode owner is
//! authoritative for the selected episode. The component keeps only the
//! parent-owned episode-pane focus, the bucket chrome, and typed shell intents.
//! Row-local input reaches the owners through the common delegation seam
//! (design.md D3/D5).

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
    Msg, PodcastEpisodeIntent, PodcastEpisodeTarget, PodcastEpisodeTransition, ShellRequest,
    TerminalObserverEvent,
};
use super::user_event::UserEvent;
use crate::app::render::{
    podcast_show_rows, render_audiobookshelf_podcast_content, wide_hero_presentation,
    AudiobookshelfPodcastGeometry, HomeImagePaint, PodcastEpisodePresentation, PodcastInteraction,
    PodcastShowPresentation,
};
use crate::app::types_audiobookshelf_browse::{
    AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter,
};
use mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode;

pub struct AudiobookshelfPodcastComponent {
    state: AudiobookshelfBrowseState,
    /// Component-owned interaction state, never present on the projected
    /// content type (split-browse-state-interaction-fields task 3.2).
    episode_filter: AudiobookshelfEpisodeFilter,
    /// Parent-owned episode-pane focus, separate from the episode owner's
    /// selected row (design.md D5). Entering/leaving focus never moves the
    /// episode owner's selection; the owner re-parks it only at discrete
    /// show-identity/filter re-projection boundaries.
    episode_focused: bool,
    initialized: bool,
    focused: bool,
    images_enabled: bool,
    geometry: AudiobookshelfPodcastGeometry,
    image_paint: Option<HomeImagePaint>,
    /// One shared canonical show owner, carried by exactly one of the Wide and
    /// Inline presentations (design.md D1/D2). It owns the show cursor, scroll,
    /// and selected target across a breakpoint change; the two adapters are
    /// never synchronized.
    carrier: MediaListCarrier<String>,
    /// The persistent canonical episode owner (design.md D5): authoritative for
    /// the selected episode, scrolling, painting, and retained hits. Its rows
    /// are projected from the selected, filtered episode snapshot before view.
    episode_list: WideMediaList<String>,
    /// The presentation the last `view` painted; `None` before the first paint.
    wide: bool,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3): owns
    /// the double-click window and wheel throttle. Not a shared clock.
    mouse_gestures: MouseGestureState,
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
            episode_filter: AudiobookshelfEpisodeFilter::All,
            episode_focused: false,
            initialized: false,
            focused: false,
            images_enabled: false,
            geometry: AudiobookshelfPodcastGeometry::default(),
            image_paint: None,
            carrier: MediaListCarrier::new(Presentation::Inline),
            episode_list: WideMediaList::new(),
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

    /// Move the shared show owner into the active presentation when they
    /// diverge. A responsive change reads the same owner and preserves only the
    /// outgoing selected-row viewport offset (design.md D1); no cursor, scroll,
    /// or selection is copied between presentations.
    fn ensure_carrier(&mut self) {
        let viewport_height = self.geometry.list_area.height.max(1) as usize;
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
        snapshot: &AudiobookshelfBrowseState,
        images_enabled: bool,
    ) {
        // Content and interaction are separate types now: the projected
        // snapshot carries no `episode_filter` / `episode_focused` / `scroll`,
        // so adopting it wholesale cannot clobber them and there is nothing to
        // save and restore (split-browse-state-interaction-fields task 3.4).
        // Whether the show the component had selected survived the new content
        // decides if its derived local state still means anything.
        let survived = self.initialized
            && self.state.selected_id.as_ref().is_some_and(|prior| {
                snapshot
                    .shows
                    .iter()
                    .any(|show| &show.library_item_id == prior)
            });
        self.state = snapshot.clone();
        let show_rows = podcast_show_rows(&self.state.shows);
        // An unchanged projection does not invalidate the retained frame
        // (design.md D6).
        if self.carrier.rows() != show_rows.as_slice() {
            self.carrier.set_content(show_rows);
        }
        if !self.initialized {
            if let Some(target) = self.state.selected_id.clone() {
                self.carrier.select_target(&target);
            }
        }
        let identity_changed = self.initialized && !survived;
        if identity_changed {
            // The selected show dropped out of the new content: reset the
            // component-owned interaction state and re-park the show owner at
            // its first row, then re-project the episode rows (design.md D5).
            self.episode_filter = AudiobookshelfEpisodeFilter::All;
            self.episode_focused = false;
            self.carrier.select_first();
            self.state.select(0);
        }
        self.initialized = true;
        self.images_enabled = images_enabled;
        self.project_episode_rows();
        if identity_changed {
            self.episode_list.select_first();
        }
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.selected_show_index().unwrap_or(0)
    }

    /// The active show owner's stable target index in the projected catalog.
    fn selected_show_index(&self) -> Option<usize> {
        let target = self.carrier.selected_target()?;
        self.state
            .shows
            .iter()
            .position(|show| &show.library_item_id == target)
    }

    /// Re-home accessors (task 5.3d.11 U0): owned/copy views of the shared
    /// `AudiobookshelfBrowseState` members the App-level readers read. The
    /// show owner is authoritative; `state.selected_id` mirrors it for the
    /// projected snapshot.
    pub(in crate::app) fn selected_id(&self) -> Option<String> {
        self.carrier.selected_target().cloned()
    }

    /// Whether the parent-owned episode pane currently has focus (design.md
    /// D5). Independent of the episode owner's selected row.
    pub(in crate::app) fn episode_focused(&self) -> bool {
        self.episode_focused
    }

    pub(in crate::app) fn episode_filter(&self) -> AudiobookshelfEpisodeFilter {
        self.episode_filter
    }

    /// The episode owner's stable, show-qualified target, resolved only while
    /// the episode pane holds focus (design.md D4/D5). Activation never
    /// re-derives it from a numeric index.
    pub(in crate::app) fn episode_target(&self) -> Option<PodcastEpisodeTarget> {
        if !self.episode_focused {
            return None;
        }
        let show = self.state.selected_show()?;
        let episode_id = self.episode_list.selected_target()?;
        Some(PodcastEpisodeTarget::new(
            show.library_item_id.clone(),
            episode_id.clone(),
        ))
    }

    /// Enter the parent-owned episode pane without moving the episode owner's
    /// selection (design.md D5): focus is not selection.
    pub(in crate::app) fn enter_episode_focus(&mut self) {
        self.episode_focused = true;
    }

    /// Leave the parent-owned episode pane without moving the episode owner's
    /// selection or scroll (design.md D5).
    pub(in crate::app) fn clear_episode_focus(&mut self) {
        self.episode_focused = false;
    }

    /// Sets the component-owned episode filter. A filter change re-projects the
    /// filtered episode rows and explicitly re-parks the episode owner at its
    /// first row -- the discrete re-projection boundary design.md D5 allows.
    pub(in crate::app) fn set_episode_filter(&mut self, filter: AudiobookshelfEpisodeFilter) {
        self.episode_filter = filter;
        self.project_episode_rows();
        self.episode_list.select_first();
    }

    /// The canonical episode rows for the selected show under the active
    /// filter, keyed by the episode's stable id. Episode identity is only
    /// unique per show, so the typed target pairs it with the selected show's
    /// `library_item_id` (design.md D4).
    fn episode_rows(&self) -> Vec<MediaListRow<String>> {
        self.state
            .visible_episodes(self.episode_filter)
            .into_iter()
            .map(|episode| MediaListRow::Item {
                target: episode.episode_id.clone(),
                primary: episode.title.clone(),
                trailing: None,
                duration: episode.duration_seconds.and_then(|seconds| {
                    crate::app::ui_util::list_duration_secs(seconds.round() as i64)
                }),
                kind: MediaKind::Media,
                semantic_state: self.episode_semantic_state(episode),
            })
            .collect()
    }

    fn episode_semantic_state(
        &self,
        episode: &AudiobookshelfDownloadedEpisode,
    ) -> MediaSemanticState {
        self.state
            .progress
            .get(&(episode.library_item_id.clone(), episode.episode_id.clone()))
            .map(|progress| {
                if progress.is_finished {
                    MediaSemanticState::Played
                } else {
                    MediaSemanticState::Ordinary
                }
            })
            .unwrap_or(MediaSemanticState::Ordinary)
    }

    /// Project the selected show's filtered episode rows into the episode owner
    /// before view (design.md D6). An unchanged projection keeps the retained
    /// frame valid.
    fn project_episode_rows(&mut self) {
        let rows = self.episode_rows();
        if self.episode_list.rows() != rows.as_slice() {
            self.episode_list.set_content(rows);
        }
    }

    /// The image-paint plan this component computed during its last `view`
    /// (task 5.3d.10b): `Some` only when images are enabled, a selected show
    /// hero was actually admitted/painted, and the hero reserved an image
    /// rect. Replaced on every `view`, taken once by the shell after paint.
    pub(in crate::app) fn take_image_paint(&mut self) -> Option<HomeImagePaint> {
        self.image_paint.take()
    }

    /// The geometry the component computed during its last `view`, exposed so
    /// the shell can anchor overlays / read painted areas. Immutable: the
    /// component owns painting; callers do not write back.
    pub(in crate::app) fn geometry(&self) -> &AudiobookshelfPodcastGeometry {
        &self.geometry
    }

    /// The one seam through which the component offers an already-normalized
    /// row-local key or pointer gesture to the shared show owner. The owner
    /// applies the local state transition and returns the closed
    /// provider-neutral outcome; the component then mirrors the owner's
    /// selection into the projected snapshot and returns the typed show-move
    /// request (design.md D3).
    fn delegate_show_input(&mut self, input: RowLocalInput) -> Option<Msg> {
        self.carrier.delegate(input, None);
        self.sync_show_selection();
        self.show_move_request()
    }

    /// Mirror the show owner's stable selection into the projected snapshot,
    /// resetting the derived filter/episode-pane state and re-projecting the
    /// episode rows only at a discrete show-identity boundary (design.md D5).
    fn sync_show_selection(&mut self) {
        let Some(target) = self.carrier.selected_target().cloned() else {
            return;
        };
        let Some(index) = self
            .state
            .shows
            .iter()
            .position(|show| show.library_item_id == target)
        else {
            return;
        };
        let identity_changed = self.state.select_changed_identity(index);
        if identity_changed {
            self.episode_filter = AudiobookshelfEpisodeFilter::All;
            self.episode_focused = false;
        }
        self.state.select(index);
        if identity_changed {
            self.project_episode_rows();
            self.episode_list.select_first();
        }
    }

    /// Discrete show re-anchor (bucket jump, Home/End, pointer click): the
    /// stable target is explicitly selected on the owner before any derived
    /// state is reset (design.md D5).
    fn select_show_index(&mut self, index: usize) {
        let Some(target) = self
            .state
            .shows
            .get(index)
            .map(|show| show.library_item_id.clone())
        else {
            return;
        };
        if self.carrier.select_target(&target) {
            self.sync_show_selection();
        }
    }

    fn move_episode(&mut self, delta: i64) {
        self.episode_list.delegate(RowLocalInput::Move(delta), None);
    }

    fn cycle_show_bucket(&mut self, delta: i64) -> Option<Msg> {
        let buckets =
            crate::app::types_audiobookshelf_browse::build_show_title_buckets(&self.state.shows);
        if buckets.is_empty() {
            return self.show_move_request();
        }
        let cursor = self.cursor();
        let current = buckets
            .iter()
            .position(|bucket| cursor >= bucket.start && cursor < bucket.end)
            .unwrap_or(0);
        let next = (current as i64 + delta).rem_euclid(buckets.len() as i64) as usize;
        if let Some(bucket) = buckets.get(next) {
            self.select_show_index(bucket.start);
        }
        self.show_move_request()
    }

    /// The resolved-index show-move request for the target the show owner just
    /// landed on (split-audiobookshelf-cursor-ownership D1). The owner stays
    /// authoritative; the shell applies the carried stable target.
    fn show_move_request(&self) -> Option<Msg> {
        Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id: self.carrier.selected_target().cloned(),
        }))
    }

    fn cycle_filter(&mut self, delta: i64) {
        let current = AudiobookshelfEpisodeFilter::ALL
            .iter()
            .position(|filter| *filter == self.episode_filter)
            .unwrap_or(0);
        let next = crate::app::ui_util::move_cursor(
            current,
            delta,
            AudiobookshelfEpisodeFilter::ALL.len(),
        );
        self.set_episode_filter(AudiobookshelfEpisodeFilter::ALL[next]);
    }

    fn page_rows(&self) -> i64 {
        self.geometry.list_area.height.saturating_sub(1).max(1) as i64
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if !self.focused {
            return None;
        }
        match key.code {
            Key::Up | Key::Char('k') if !self.episode_focused => {
                self.delegate_show_input(RowLocalInput::Move(-1))
            }
            Key::Down | Key::Char('j') if !self.episode_focused => {
                self.delegate_show_input(RowLocalInput::Move(1))
            }
            Key::Left | Key::Char('h') if !self.episode_focused => {
                self.delegate_show_input(RowLocalInput::Move(-1))
            }
            Key::Right | Key::Char('l') if !self.episode_focused => {
                self.delegate_show_input(RowLocalInput::Move(1))
            }
            Key::PageUp if !self.episode_focused => {
                let rows = self.page_rows();
                self.delegate_show_input(RowLocalInput::Move(-rows))
            }
            Key::PageDown if !self.episode_focused => {
                let rows = self.page_rows();
                self.delegate_show_input(RowLocalInput::Move(rows))
            }
            Key::Home if !self.episode_focused => self.delegate_show_input(RowLocalInput::First),
            Key::End if !self.episode_focused => self.delegate_show_input(RowLocalInput::Last),
            Key::Char('[') if !self.episode_focused && key.modifiers.is_empty() => {
                self.cycle_show_bucket(-1)
            }
            Key::Char(']') if !self.episode_focused && key.modifiers.is_empty() => {
                self.cycle_show_bucket(1)
            }
            Key::Up | Key::Char('k') => {
                self.move_episode(-1);
                Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                        PodcastEpisodeTransition::PreviousEpisode,
                    ),
                ))
            }
            Key::Down | Key::Char('j') => {
                self.move_episode(1);
                Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                        PodcastEpisodeTransition::NextEpisode,
                    ),
                ))
            }
            Key::Char('[') if key.modifiers.is_empty() => {
                self.cycle_filter(-1);
                Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                        PodcastEpisodeTransition::PreviousFilter,
                    ),
                ))
            }
            Key::Char(']') if key.modifiers.is_empty() => {
                self.cycle_filter(1);
                Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                        PodcastEpisodeTransition::NextFilter,
                    ),
                ))
            }
            Key::Esc | Key::Backspace if self.episode_focused => {
                self.clear_episode_focus();
                Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                        PodcastEpisodeTransition::Exit,
                    ),
                ))
            }
            // Space/Enter/Ctrl+A action intents (task 5.3d.7): the component
            // only reports the matched intent; the shell resolves the
            // episode-focus and wide/narrow conditions from App state at the
            // Model boundary and runs the existing App effect (D17). The
            // target is the episode owner's stable selection.
            Key::Char(' ') => Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(
                    PodcastEpisodeIntent::FocusOrPlay(self.episode_target()),
                ),
            )),
            Key::Enter => Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    self.episode_target(),
                )),
            )),
            Key::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::Enqueue(
                    self.episode_target(),
                )),
            )),
            _ => None,
        }
    }

    /// Handle pointer input using the active persistent show owner's retained
    /// current-frame geometry.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        // The podcast surface does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Scroll { at, delta } => {
                if self.episode_focused {
                    // The focused episode pane owns its rows' wheel input
                    // (design.md D3/D6): offer the normalized wheel delta to
                    // the episode owner's retained current-frame geometry.
                    if !self.episode_list.claims_current_point(at) {
                        return None;
                    }
                    self.move_episode(delta);
                    return Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed));
                }
                if !self.carrier.claims_current_point(at) {
                    None
                } else {
                    self.delegate_show_input(RowLocalInput::Move(delta))
                }
            }
            MouseGesture::Click(at) => {
                if let Some(target) = self.carrier.resolve_current_point(at).cloned() {
                    self.carrier
                        .delegate(RowLocalInput::Click(at), Some(target));
                    self.sync_show_selection();
                    return self.show_move_request();
                }
                // After a show-owner miss, a focused episode pane resolves
                // the point against the episode owner's retained geometry and
                // applies the row-local click there (design.md D4/D6).
                if self.episode_focused {
                    if let Some(target) = self.episode_list.resolve_current_point(at).cloned() {
                        self.episode_list
                            .delegate(RowLocalInput::Click(at), Some(target));
                        // Selection is local to the episode owner; claim the
                        // event so the framework keeps this mutation.
                        return Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed));
                    }
                }
                if let Some((_, bucket)) = self
                    .geometry
                    .selector_tabs
                    .iter()
                    .find(|(rect, _)| rect.contains(at))
                {
                    crate::app::types_audiobookshelf_browse::build_show_title_buckets(
                        &self.state.shows,
                    )
                    .get(*bucket)
                    .and_then(|range| {
                        self.select_show_index(range.start);
                        self.show_move_request()
                    })
                } else {
                    None
                }
            }
            MouseGesture::DoubleClick(at) => {
                if let Some(target) = self.carrier.resolve_current_point(at).cloned() {
                    self.carrier
                        .delegate(RowLocalInput::Click(at), Some(target));
                    self.sync_show_selection();
                    return Some(Msg::Shell(
                        ShellRequest::AudiobookshelfPodcastEpisodeIntent(
                            PodcastEpisodeIntent::OpenOrPlay(self.episode_target()),
                        ),
                    ));
                }
                // Double-click on a focused episode row selects it through
                // the episode owner, then activates the owner-resolved
                // show-qualified target (design.md D4/D6).
                if self.episode_focused {
                    if let Some(target) = self.episode_list.resolve_current_point(at).cloned() {
                        self.episode_list
                            .delegate(RowLocalInput::Click(at), Some(target));
                        return Some(Msg::Shell(
                            ShellRequest::AudiobookshelfPodcastEpisodeIntent(
                                PodcastEpisodeIntent::OpenOrPlay(self.episode_target()),
                            ),
                        ));
                    }
                }
                None
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

    #[cfg(test)]
    pub(crate) fn selected_row_offset_for_test(&self) -> Option<usize> {
        let height = self.geometry.list_area.height.max(1) as usize;
        if self.carrier.active() == Presentation::Wide {
            self.carrier.wide().selected_row_offset(height)
        } else {
            self.carrier.inline().selected_row_offset(height)
        }
    }

    /// Test-only: the episode owner's selectable cursor index.
    #[cfg(test)]
    pub(crate) fn episode_cursor(&self) -> usize {
        self.episode_list.cursor()
    }

    /// Test-only: the episode owner's retained current-frame content rect, the
    /// shared geometry the episode painting uses (design.md D6).
    #[cfg(test)]
    pub(crate) fn episode_content_rect_for_test(&self) -> Option<Rect> {
        self.episode_list.current_content_rect()
    }
}

impl Default for AudiobookshelfPodcastComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AudiobookshelfPodcastComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // One shared show owner per logical flow (design.md D1): a breakpoint
        // change reconfigures the same owner and preserves only the outgoing
        // selected-row viewport offset.
        self.wide = wide_hero_presentation(area).is_some();
        self.ensure_carrier();

        let show_presentation = if self.wide {
            PodcastShowPresentation::Wide(self.carrier.wide_mut())
        } else {
            PodcastShowPresentation::Inline(self.carrier.inline_mut())
        };
        self.image_paint = render_audiobookshelf_podcast_content(
            frame,
            area,
            self.focused,
            self.images_enabled,
            &self.state,
            PodcastInteraction {
                episode_filter: self.episode_filter,
                episode_focused: self.episode_focused,
            },
            show_presentation,
            PodcastEpisodePresentation::Wide(&mut self.episode_list),
            &mut self.geometry,
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

impl AppComponent<Msg, UserEvent> for AudiobookshelfPodcastComponent {
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
