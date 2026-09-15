//! Audiobookshelf Podcasts' embedded Library panel content owner (task 11.1).
//!
//! This owner is deliberately plain: the Library panel supplies the shared
//! skeleton while this type projects podcast-specific content and translates
//! slot events.

use super::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow, Workspace,
};
use super::library_panel::hero::hero_content_abs_show;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::media_list::MediaListSurfaceInput;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation,
};
use super::msg::{
    LeafKeyResult, Msg, PodcastEpisodeIntent, PodcastEpisodeTarget, PodcastEpisodeTransition,
    ShellRequest,
};
use crate::app::types_audiobookshelf_browse::{
    AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter,
};
use crate::app::ui_util::clean_overview;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

pub(in crate::app) fn podcast_show_rows(
    shows: &[mbv_core::audiobookshelf::AudiobookshelfShow],
) -> Vec<MediaListRow<String>> {
    shows
        .iter()
        .map(|show| MediaListRow::Item {
            target: show.library_item_id.clone(),
            primary: show.title.clone(),
            secondary: None,
            trailing: None,
            duration: None,
            kind: MediaKind::Collection,
            semantic_state: MediaSemanticState::Ordinary,
        })
        .collect()
}

/// Plain owner for one Audiobookshelf podcast library. Content is projected by
/// the shell; filter, focus, and list selection remain local interaction state.
pub(in crate::app) struct PodcastContent {
    pub(in crate::app) state: AudiobookshelfBrowseState,
    episode_filter: AudiobookshelfEpisodeFilter,
    episode_focused: bool,
    initialized: bool,
    focused: bool,
    carrier: MediaListCarrier<String>,
    episode_list: MediaListCarrier<String>,
    hero_image: HeroImageState,
}

impl PodcastContent {
    pub(in crate::app) fn new() -> Self {
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
            carrier: MediaListCarrier::new(Presentation::Inline),
            episode_list: MediaListCarrier::new(Presentation::Wide),
            hero_image: HeroImageState::None,
        }
    }

    pub(in crate::app) fn set_content(
        &mut self,
        snapshot: &AudiobookshelfBrowseState,
        _images_enabled: bool,
    ) {
        let survived = self.initialized
            && self.state.selected_id.as_ref().is_some_and(|prior| {
                snapshot
                    .shows
                    .iter()
                    .any(|show| &show.library_item_id == prior)
            });
        self.state = snapshot.clone();
        let rows = self
            .state
            .shows
            .iter()
            .map(|show| MediaListRow::Item {
                target: show.library_item_id.clone(),
                primary: show.title.clone(),
                secondary: None,
                trailing: None,
                duration: None,
                kind: MediaKind::Collection,
                semantic_state: MediaSemanticState::Ordinary,
            })
            .collect::<Vec<_>>();
        if self.carrier.rows() != rows.as_slice() {
            self.carrier.set_content(rows);
        }
        if !self.initialized {
            if let Some(id) = self.state.selected_id.clone() {
                self.carrier.select_target(&id);
            }
        } else if !survived {
            self.episode_filter = AudiobookshelfEpisodeFilter::All;
            self.episode_focused = false;
            self.carrier.select_first();
            self.state.select(0);
            self.episode_list.select_first();
        }
        self.initialized = true;
        self.project_episode_rows();
    }

    fn project_episode_rows(&mut self) {
        let rows = self
            .state
            .visible_episodes(self.episode_filter)
            .into_iter()
            .map(|episode| MediaListRow::Item {
                target: episode.episode_id.clone(),
                primary: episode.title.clone(),
                secondary: None,
                trailing: None,
                duration: episode.duration_seconds.and_then(|seconds| {
                    crate::app::ui_util::list_duration_secs(seconds.round() as i64)
                }),
                kind: MediaKind::Media,
                semantic_state: self
                    .state
                    .progress
                    .get(&(episode.library_item_id.clone(), episode.episode_id.clone()))
                    .map(|p| {
                        if p.is_finished {
                            MediaSemanticState::Played
                        } else {
                            MediaSemanticState::Ordinary
                        }
                    })
                    .unwrap_or(MediaSemanticState::Ordinary),
            })
            .collect::<Vec<_>>();
        if self.episode_list.rows() != rows.as_slice() {
            self.episode_list.set_content(rows);
        }
    }

    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(in crate::app) fn selected_id(&self) -> Option<String> {
        self.carrier.selected_target().cloned()
    }

    pub(in crate::app) fn episode_focused(&self) -> bool {
        self.episode_focused
    }
    pub(in crate::app) fn episode_filter(&self) -> AudiobookshelfEpisodeFilter {
        self.episode_filter
    }
    pub(in crate::app) fn set_episode_filter(&mut self, filter: AudiobookshelfEpisodeFilter) {
        self.episode_filter = filter;
        self.project_episode_rows();
        self.episode_list.select_first();
    }
    pub(in crate::app) fn enter_episode_focus(&mut self) {
        self.episode_focused = true;
    }

    pub(in crate::app) fn episode_target(&self) -> Option<PodcastEpisodeTarget> {
        if !self.episode_focused {
            return None;
        }
        Some(PodcastEpisodeTarget::new(
            self.state.selected_show()?.library_item_id.clone(),
            self.episode_list.selected_target()?.clone(),
        ))
    }

    fn sync_show_selection(&mut self) {
        let Some(target) = self.carrier.selected_target().cloned() else {
            return;
        };
        let Some(index) = self
            .state
            .shows
            .iter()
            .position(|s| s.library_item_id == target)
        else {
            return;
        };
        if self.state.select_changed_identity(index) {
            self.episode_filter = AudiobookshelfEpisodeFilter::All;
            self.episode_focused = false;
            self.project_episode_rows();
            self.episode_list.select_first();
        }
        self.state.select(index);
    }

    fn show_move(&mut self) -> Option<Msg> {
        Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id: self.selected_id(),
        }))
    }

    fn select_bucket(&mut self, position: usize) -> Option<Msg> {
        let bucket =
            crate::app::types_audiobookshelf_browse::build_show_title_buckets(&self.state.shows)
                .get(position)?
                .start;
        let target = self.state.shows.get(bucket)?.library_item_id.clone();
        self.carrier.select_target(&target);
        self.sync_show_selection();
        self.show_move()
    }

    fn cycle_filter(&mut self, delta: i64) {
        let current = AudiobookshelfEpisodeFilter::ALL
            .iter()
            .position(|f| *f == self.episode_filter)
            .unwrap_or(0);
        let next = crate::app::ui_util::move_cursor(
            current,
            delta,
            AudiobookshelfEpisodeFilter::ALL.len(),
        );
        self.episode_filter = AudiobookshelfEpisodeFilter::ALL[next];
        self.project_episode_rows();
        self.episode_list.select_first();
    }

    pub(in crate::app) fn hero_data(&mut self) -> Option<HeroContentData> {
        let show = self.state.selected_show()?;
        let mut data = hero_content_abs_show(show);
        data.facts.artwork.image = self.hero_image.clone();
        Some(data)
    }

    pub(in crate::app) fn set_hero_image(&mut self, image: HeroImageState) {
        self.hero_image = image;
    }

    pub(in crate::app) fn content(&mut self) -> LibraryPanelContent<'_> {
        let hero = self.state.selected_show().map(|show| {
            let mut data = hero_content_abs_show(show);
            data.facts.artwork.image = self.hero_image.clone();
            HeroContent {
                facts: data.facts,
                overview: data
                    .overview
                    .map(|s| clean_overview(&s))
                    .filter(|s| !s.is_empty()),
                credits: data.credits,
                workspace: Some(Workspace {
                    header: None,
                    selector: Some(SelectorRow {
                        pills: AudiobookshelfEpisodeFilter::ALL
                            .iter()
                            .map(|filter| filter.label().to_string())
                            .collect(),
                        active: Some(
                            AudiobookshelfEpisodeFilter::ALL
                                .iter()
                                .position(|filter| *filter == self.episode_filter)
                                .unwrap_or(0),
                        ),
                    }),
                    list: &mut self.episode_list,
                    focused: self.focused && self.episode_focused,
                }),
            }
        });
        let buckets =
            crate::app::types_audiobookshelf_browse::build_show_title_buckets(&self.state.shows);
        let active_bucket = self
            .state
            .selected_id
            .as_ref()
            .and_then(|id| {
                self.state
                    .shows
                    .iter()
                    .position(|show| &show.library_item_id == id)
            })
            .and_then(|cursor| {
                buckets
                    .iter()
                    .position(|bucket| (bucket.start..bucket.end).contains(&cursor))
            })
            .unwrap_or(0);
        let selector = (!buckets.is_empty()).then(|| SelectorRow {
            pills: buckets
                .iter()
                .map(|bucket| bucket.label.to_string())
                .collect(),
            active: Some(active_bucket),
        });
        let list = if self.state.shows.is_empty() {
            ListSlot::Empty {
                loading: !self.state.loading_pages.is_empty(),
                text: self
                    .state
                    .error
                    .clone()
                    .unwrap_or_else(|| "No podcasts".into()),
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
}

impl Default for PodcastContent {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryContentOwner for PodcastContent {
    fn clear_selection(&mut self) {
        self.carrier.clear_selection();
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
        self.content()
    }
    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => self.select_bucket(index),
            LibrarySlotEvent::WorkspaceSelectorPicked(index)
            | LibrarySlotEvent::ControlPicked(index) => {
                let filter = *AudiobookshelfEpisodeFilter::ALL.get(index)?;
                self.episode_filter = filter;
                self.project_episode_rows();
                self.episode_list.select_first();
                Some(Msg::TerminalEvent(
                    crate::app::components::msg::TerminalObserverEvent::MouseClaimed,
                ))
            }
            LibrarySlotEvent::List(input) => match input {
                MediaListSurfaceInput::Wheel { at, delta } => {
                    if !self.carrier.claims_current_point(at) {
                        return None;
                    }
                    self.carrier.delegate_operation(
                        MediaListSurfaceInput::Wheel { at, delta }
                            .into_operation(None)
                            .expect("resolved media-list pointer target"),
                    );
                    self.sync_show_selection();
                    self.show_move()
                }
                MediaListSurfaceInput::Click(at)
                | MediaListSurfaceInput::ToggleClick(at)
                | MediaListSurfaceInput::RangeClick(at)
                | MediaListSurfaceInput::DoubleClick(at)
                | MediaListSurfaceInput::ContextClick(at) => {
                    let target = self.carrier.resolve_current_point(at)?.clone();
                    self.episode_focused = false;
                    self.carrier.delegate_operation(
                        input
                            .into_operation(Some(target))
                            .expect("resolved media-list pointer target"),
                    );
                    let _ = ();
                    self.sync_show_selection();
                    self.show_move()
                }
                _ => {
                    self.episode_focused = false;
                    self.carrier.delegate_operation(
                        input
                            .into_operation(None)
                            .expect("resolved media-list pointer target"),
                    );
                    let _ = ();
                    self.sync_show_selection();
                    self.show_move()
                }
            },
            LibrarySlotEvent::HeroPane(input) => {
                let point = match input {
                    MediaListSurfaceInput::Click(p)
                    | MediaListSurfaceInput::ToggleClick(p)
                    | MediaListSurfaceInput::RangeClick(p)
                    | MediaListSurfaceInput::DoubleClick(p)
                    | MediaListSurfaceInput::ContextClick(p) => p,
                    MediaListSurfaceInput::Wheel { at, .. } => at,
                    _ => return None,
                };
                if let Some(target) = self.episode_list.resolve_current_point(point).cloned() {
                    if matches!(
                        input,
                        MediaListSurfaceInput::Click(_)
                            | MediaListSurfaceInput::ToggleClick(_)
                            | MediaListSurfaceInput::RangeClick(_)
                            | MediaListSurfaceInput::DoubleClick(_)
                    ) {
                        self.episode_focused = true;
                    }
                    self.episode_list.delegate_operation(
                        input
                            .into_operation(Some(target))
                            .expect("resolved media-list pointer target"),
                    );
                    if matches!(input, MediaListSurfaceInput::DoubleClick(_)) {
                        return Some(Msg::Shell(
                            ShellRequest::AudiobookshelfPodcastEpisodeIntent(
                                PodcastEpisodeIntent::OpenOrPlay(self.episode_target()),
                            ),
                        ));
                    }
                    return Some(Msg::TerminalEvent(
                        crate::app::components::msg::TerminalObserverEvent::MouseClaimed,
                    ));
                }
                None
            }
        }
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.carrier.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            )));
        }
        if !self.focused {
            return None;
        }
        let episode = self.episode_focused;
        match key.code {
            Key::Up | Key::Char('k') if !episode => {
                self.carrier.delegate_operation(
                    MediaListSurfaceInput::Move(-1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                self.sync_show_selection();
                self.show_move()
            }
            Key::Down | Key::Char('j') if !episode => {
                self.carrier.delegate_operation(
                    MediaListSurfaceInput::Move(1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                self.sync_show_selection();
                self.show_move()
            }
            Key::Up | Key::Char('k') if episode => {
                self.episode_list.delegate_operation(
                    MediaListSurfaceInput::Move(-1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                        PodcastEpisodeTransition::PreviousEpisode,
                    ),
                ))
            }
            Key::Down | Key::Char('j') if episode => {
                self.episode_list.delegate_operation(
                    MediaListSurfaceInput::Move(1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
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
            Key::Esc | Key::Backspace if episode => {
                self.episode_focused = false;
                Some(Msg::Shell(
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                        PodcastEpisodeTransition::Exit,
                    ),
                ))
            }
            Key::Enter if !episode => Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    None,
                )),
            )),
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
    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        match self.on_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(message)),
            None if self.focused => LeafKeyResult::Consumed(None),
            None => LeafKeyResult::Unhandled,
        }
    }

    fn focus_hero_workspace(&mut self) -> bool {
        self.enter_episode_focus();
        true
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        self.hero_data()
    }
    fn set_hero_image(&mut self, image: HeroImageState) {
        self.set_hero_image(image);
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
    use mbv_core::audiobookshelf::{AudiobookshelfLibrary, AudiobookshelfShow};
    use ratatui::{
        backend::TestBackend,
        layout::{Position, Rect},
        Terminal,
    };
    use tuirealm::component::Component;

    fn state() -> AudiobookshelfBrowseState {
        let mut state = AudiobookshelfBrowseState::new(AudiobookshelfLibrary {
            id: "lib".into(),
            name: "Podcasts".into(),
            media_type: "podcast".into(),
        });
        state.append_page(
            0,
            1,
            1,
            vec![AudiobookshelfShow {
                library_item_id: "show".into(),
                title: "Show".into(),
                author: Some("Author".into()),
                description: Some("Overview".into()),
                cover_path: Some("cover".into()),
            }],
        );
        state.select(0);
        state
    }

    #[test]
    fn empty_content_leaves_hero_to_the_panel_skeleton() {
        let mut owner = PodcastContent::new();
        let empty = AudiobookshelfBrowseState::new(AudiobookshelfLibrary {
            id: "lib".into(),
            name: "Podcasts".into(),
            media_type: "podcast".into(),
        });
        owner.set_content(&empty, false);
        let content = owner.content();
        assert!(content.hero.is_none());
        assert!(matches!(
            content.list,
            ListSlot::Empty { loading: false, .. }
        ));
    }

    #[test]
    fn enter_asks_the_shell_to_focus_episodes_and_arrows_do_not() {
        let mut owner = PodcastContent::new();
        owner.set_content(&state(), false);
        owner.set_focused(true);

        assert!(!owner.episode_focused());
        // Arrows never move focus between panels.
        assert!(owner
            .on_key(&KeyEvent::new(Key::Right, KeyModifiers::NONE))
            .is_none());
        assert!(owner
            .on_key(&KeyEvent::new(Key::Char('l'), KeyModifiers::NONE))
            .is_none());
        assert!(!owner.episode_focused());
        // Enter on the selected show is the only keyboard way into the
        // episode pane; the shell owns the focus transition.
        assert!(matches!(
            owner.on_key(&KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
            Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    None
                ))
            ))
        ));
        assert!(!owner.episode_focused());
        // With the episode pane focused, Esc exits back to the show list.
        owner.enter_episode_focus();
        assert!(matches!(
            owner.on_key(&KeyEvent::new(Key::Esc, KeyModifiers::NONE)),
            Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeTransition(
                    PodcastEpisodeTransition::Exit
                )
            ))
        ));
        assert!(!owner.episode_focused());
        assert_eq!(owner.selected_id().as_deref(), Some("show"));
    }

    #[test]
    fn podcast_pointer_click_resolves_current_frame_and_unknown_point_is_noop() {
        let mut owner = PodcastContent::new();
        owner.set_content(&state(), false);

        let area = Rect::new(0, 0, 30, 1);
        owner.carrier.inline_mut().set_geometry(area, area);
        let mut terminal = Terminal::new(TestBackend::new(30, 1)).unwrap();
        terminal
            .draw(|frame| owner.carrier.inline_mut().view(frame, area))
            .unwrap();

        assert!(matches!(
            owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
                Position { x: 0, y: 0 }
            ))),
            Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
                library_item_id: Some(ref id)
            })) if id == "show"
        ));
        assert_eq!(
            owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
                Position { x: 0, y: 1 }
            ))),
            None
        );
    }

    #[test]
    fn content_projects_podcast_panel_slots() {
        let mut owner = PodcastContent::new();
        owner.set_content(&state(), false);
        let content = owner.content();
        assert_eq!(
            content.selector.as_ref().map(|row| row.pills.len()),
            Some(1)
        );
        let hero = content.hero.expect("podcast hero");
        assert_eq!(
            hero.facts.artwork.shape,
            super::super::library_panel::ArtworkShape::Square
        );
        assert_eq!(hero.overview.as_deref(), Some("Overview"));
        let workspace = hero.workspace.expect("episode workspace");
        assert_eq!(
            workspace.selector.as_ref().map(|row| row.pills.clone()),
            Some(vec!["All".into(), "Played".into(), "Unplayed".into()])
        );
    }
}
