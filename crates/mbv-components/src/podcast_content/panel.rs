use super::{
    AudiobookshelfEpisodeFilter, AudiobookshelfSelectorKey, LaunchSelector, LibraryContentOwner,
    LibraryItemIdentity, LibraryPanelContent, LibrarySlotEvent, MediaListOperation,
    MediaListSurfaceInput, Msg, PillSelection, PodcastContent, PodcastEpisodeIntent,
    STATE_PILL_COUNT, SelectorIdentity, ShellRequest, TerminalObserverEvent,
};
use crate::library_panel::HeroContentData;
use crate::media_list::RowIntent;
use mbv_render::components::tv_wide::HeroImageState;
use tuirealm::event::{Key, KeyEvent};

impl LibraryContentOwner for PodcastContent {
    /// Forwards to the inherent content-preserving reset (design D3).
    fn reset_presentation(&mut self) {
        self.reset_presentation();
    }

    fn launch_selector(&self, state: &mbv_config::TuiLaunchState) -> Option<LaunchSelector> {
        // Only the persisted scope applies: a podcast library restarts on
        // Latest. Legacy filter/show selectors in an old snapshot decode but
        // resolve to the destination default.
        let is_latest = matches!(
            state.selector.as_ref(),
            Some(SelectorIdentity::Audiobookshelf {
                key: AudiobookshelfSelectorKey::Latest,
            })
        );
        (is_latest && self.pill != PillSelection::Latest)
            .then_some(LaunchSelector::AudiobookshelfLatest)
    }

    fn reanchor_launch_state(&mut self, _state: &mbv_config::TuiLaunchState) -> bool {
        // The shell applies the selector through App before this item-level
        // re-anchor. Restore the persisted scope here so the component scopes
        // its rows before selecting the first row: every restart lands on
        // Latest whether or not the podcast library was the exit tab (the
        // snapshot records the exit tab's scope only), and a legacy
        // filter/show key in an old snapshot decodes but resolves to Latest
        // too.
        self.set_pill(PillSelection::Latest);
        // A saved episode never restores (spec: every restart lands on the
        // first row), so a legacy snapshot's item is ignored here too.
        self.episodes.select_first();
        self.sync_hero_scroll();
        true
    }

    fn launch_snapshot(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        // The persisted pill scope is always Latest; a state/show pill is
        // session memory. The selected episode is never recorded, so
        // restoration always lands on the first row.
        let selector = Some(SelectorIdentity::Audiobookshelf {
            key: AudiobookshelfSelectorKey::Latest,
        });
        (selector, None)
    }

    fn clear_selection(&mut self) {
        self.episodes.clear_owner_selection();
    }

    fn set_selection_origin(&mut self, origin: mbv_ui_model::media_list::SelectionOrigin) {
        self.episodes.set_selection_origin(origin);
    }

    fn selection_summary(&self) -> Option<mbv_ui_msg::SelectionSummary> {
        Some(self.episodes.selection_summary())
    }

    fn hero_scroll_offset(&self) -> usize {
        self.hero_scroll
    }

    /// One wheel step of the Wide hero's overview box: the panel gates the
    /// pointer against the box it painted and supplies that box's scroll
    /// range, so the offset only clamps here.
    fn hero_scroll(&mut self, delta: i16, max_offset: usize) -> bool {
        let next = if delta < 0 {
            self.hero_scroll
                .saturating_sub(usize::from(delta.unsigned_abs()))
        } else {
            self.hero_scroll
                .saturating_add(usize::from(delta.unsigned_abs()))
        }
        .min(max_offset);
        let changed = next != self.hero_scroll;
        self.hero_scroll = next;
        changed
    }

    fn content(&mut self) -> LibraryPanelContent<'_> {
        self.content()
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => {
                // One selector resolved in the owner (design D3): Latest,
                // then state pills, then show pills in painted order.
                let pill = if index == 0 {
                    PillSelection::Latest
                } else if let Some(filter) = AudiobookshelfEpisodeFilter::ALL.get(index - 1) {
                    PillSelection::State(*filter)
                } else {
                    PillSelection::Show(
                        self.state
                            .shows
                            .get(index - 1 - STATE_PILL_COUNT)?
                            .library_item_id
                            .clone(),
                    )
                };
                let changed = self.pill != pill;
                self.set_pill(pill);
                let effect = self.pill_effect_msg(changed);
                // A resolved pill pick claims the pointer gesture that
                // delivered it.
                effect.or(Some(Msg::TerminalEvent(
                    TerminalObserverEvent::MouseClaimed,
                )))
            }
            LibrarySlotEvent::List(input) => match input {
                MediaListSurfaceInput::Wheel { at, delta } => {
                    // The claim gate mirrors the mounted component: a wheel
                    // outside the painted active list is unclaimed.
                    if !self.episodes.claims_current_point(at) {
                        return None;
                    }
                    self.delegate_episodes(
                        MediaListSurfaceInput::Wheel { at, delta }
                            .into_operation(None)
                            .expect("resolved media-list pointer target"),
                    );
                    Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
                }
                MediaListSurfaceInput::Click(at)
                | MediaListSurfaceInput::ToggleClick(at)
                | MediaListSurfaceInput::RangeClick(at) => {
                    let target = self.episodes.resolve_current_point(at)?.clone();
                    self.delegate_episodes(
                        input
                            .into_operation(Some(target))
                            .expect("resolved media-list pointer target"),
                    );
                    // Click-to-focus (task 4.5): pull panel focus to the
                    // Library and persist the tab's slot, as the Feeds row
                    // click does.
                    self.move_effect()
                }
                MediaListSurfaceInput::ContextClick(at) => {
                    // Right-click resolves through the shared owner the way
                    // the `.` keyboard gesture does: a Visual multi-selection
                    // supplies every marked row in display order, otherwise
                    // the clicked row. Selector pills never reach this arm —
                    // the panel resolves them to `SelectorPicked` first, so
                    // a show pill opens no menu.
                    let target = self.episodes.resolve_current_point(at)?.clone();
                    let outcome =
                        self.delegate_episodes(MediaListOperation::Context(target.clone()));
                    let targets = match outcome.external_intent {
                        Some(RowIntent::ContextSelection(targets)) => targets,
                        Some(RowIntent::Context(target)) => vec![target],
                        _ => vec![target],
                    };
                    Some(Self::episode_context_msg(targets, Some((at.x, at.y))))
                }
                MediaListSurfaceInput::DoubleClick(at) => {
                    // Resolve once, then delegate the target-bearing activation.
                    let target = self.episodes.resolve_current_point(at)?.clone();
                    self.delegate_episodes(MediaListOperation::Activate(target.clone()));
                    Some(Msg::Shell(Box::new(
                        ShellRequest::AudiobookshelfPodcastEpisodeIntent(
                            PodcastEpisodeIntent::OpenOrPlay(Some(target)),
                        ),
                    )))
                }
                _ => {
                    self.delegate_episodes(
                        input
                            .into_operation(None)
                            .expect("resolved media-list pointer target"),
                    );
                    None
                }
            },
            // No Workspace and no hero-pane input of its own (design D1).
            LibrarySlotEvent::WorkspaceSelectorPicked(_) | LibrarySlotEvent::HeroPane(_) => None,
            LibrarySlotEvent::HeroActivate => Some(Msg::Shell(Box::new(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    self.episodes.selected_target().cloned(),
                )),
            ))),
        }
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.episodes.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(Box::new(ShellRequest::SelectionProjection(
                self.episodes.selection_summary(),
            ))));
        }
        if !self.focused {
            return None;
        }
        match key.code {
            Key::Up | Key::Char('k') => {
                self.delegate_episodes(
                    MediaListSurfaceInput::Move(-1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                self.move_effect()
            }
            Key::Down | Key::Char('j') => {
                self.delegate_episodes(
                    MediaListSurfaceInput::Move(1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                self.move_effect()
            }
            Key::PageUp => {
                self.delegate_episodes(
                    MediaListSurfaceInput::Page(-1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                self.move_effect()
            }
            Key::PageDown => {
                self.delegate_episodes(
                    MediaListSurfaceInput::Page(1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                self.move_effect()
            }
            Key::Home => {
                self.delegate_episodes(
                    MediaListSurfaceInput::First
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                self.move_effect()
            }
            Key::End => {
                self.delegate_episodes(
                    MediaListSurfaceInput::Last
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                self.move_effect()
            }
            Key::Char('[') if key.modifiers.is_empty() => self.cycle_pill(-1),
            Key::Char(']') if key.modifiers.is_empty() => self.cycle_pill(1),
            Key::Enter => Some(Msg::Shell(Box::new(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    self.episodes.selected_target().cloned(),
                )),
            ))),
            Key::Char(' ') => Some(Msg::Shell(Box::new(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(
                    PodcastEpisodeIntent::FocusOrPlay(self.episodes.selected_target().cloned()),
                ),
            ))),
            Key::Char('.') if key.modifiers.is_empty() => self.open_selected_episode_context(),
            // Ctrl+A multi-selects the list via `handle_visual_key`; enqueue
            // stays on the context menu here.
            _ => None,
        }
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        self.hero_data()
    }
    fn set_hero_image(&mut self, image: HeroImageState) {
        self.set_hero_image(image);
    }
    // Podcast episodes are not hero-bearing browser rows (design D6, rows
    // 3.4): Enter plays immediately in every geometry and no Library Hero
    // overlay ever opens for the tab.
    fn hero_overlay_available(&mut self) -> bool {
        false
    }
    fn browser_rows_are_hero_bearing(&mut self) -> bool {
        false
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
