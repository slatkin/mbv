use super::*;

impl LibraryContentOwner for PodcastContent {
    fn launch_selector(&self, state: &mbv_core::config::TuiLaunchState) -> Option<LaunchSelector> {
        let target = match state.selector.as_ref() {
            Some(SelectorIdentity::Audiobookshelf {
                key: AudiobookshelfSelectorKey::Latest,
            }) => LaunchSelector::AudiobookshelfLatest,
            Some(SelectorIdentity::Audiobookshelf {
                key: AudiobookshelfSelectorKey::PodcastShow(id),
            }) if self.show_exists(id) => LaunchSelector::AudiobookshelfShow(id.clone()),
            _ => LaunchSelector::AudiobookshelfState,
        };
        let same = match (&self.pill, &target) {
            (PillSelection::Latest, LaunchSelector::AudiobookshelfLatest) => true,
            (PillSelection::Show(current), LaunchSelector::AudiobookshelfShow(target)) => {
                current == target
            }
            (PillSelection::State(_), LaunchSelector::AudiobookshelfState) => true,
            _ => false,
        };
        (!same).then_some(target)
    }

    fn reanchor_launch_state(&mut self, state: &mbv_core::config::TuiLaunchState) -> bool {
        // The shell applies the selector through App before this item-level
        // re-anchor. Restore the saved pill here so the component scopes its
        // rows before selecting the saved item.
        match state.selector.as_ref() {
            Some(SelectorIdentity::Audiobookshelf {
                key: AudiobookshelfSelectorKey::Latest,
            }) => self.set_pill(PillSelection::Latest),
            Some(SelectorIdentity::Audiobookshelf {
                key: AudiobookshelfSelectorKey::PodcastFilter(filter),
            }) => {
                self.set_pill(PillSelection::State(match filter {
                    AudiobookshelfPodcastFilter::All => AudiobookshelfEpisodeFilter::All,
                    AudiobookshelfPodcastFilter::Unplayed => AudiobookshelfEpisodeFilter::Unplayed,
                    AudiobookshelfPodcastFilter::Played => AudiobookshelfEpisodeFilter::Played,
                }));
            }
            Some(SelectorIdentity::Audiobookshelf {
                key: AudiobookshelfSelectorKey::PodcastShow(id),
            }) if self.show_exists(id) => {
                self.set_pill(PillSelection::Show(id.clone()));
            }
            _ => {}
        }
        if let PillSelection::Show(id) = &self.pill {
            if !self.state.detail_cache.contains_key(id) {
                return false;
            }
        }
        let selected = state
            .item
            .as_ref()
            .and_then(|item| match item {
                LibraryItemIdentity::Audiobookshelf { id } => id.split_once('\0'),
                _ => None,
            })
            .map(|(library_item_id, episode_id)| {
                self.episodes.select_target(&PodcastEpisodeTarget::new(
                    library_item_id.to_owned(),
                    episode_id.to_owned(),
                ))
            })
            .unwrap_or(false);
        if !selected {
            self.episodes.select_first();
        }
        self.sync_hero_scroll();
        true
    }

    fn launch_snapshot(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        let selector = {
            let key = match &self.pill {
                PillSelection::Latest => AudiobookshelfSelectorKey::Latest,
                PillSelection::State(filter) => {
                    AudiobookshelfSelectorKey::PodcastFilter(match filter {
                        AudiobookshelfEpisodeFilter::All => AudiobookshelfPodcastFilter::All,
                        AudiobookshelfEpisodeFilter::Unplayed => {
                            AudiobookshelfPodcastFilter::Unplayed
                        }
                        AudiobookshelfEpisodeFilter::Played => AudiobookshelfPodcastFilter::Played,
                    })
                }
                PillSelection::Show(id) => AudiobookshelfSelectorKey::PodcastShow(id.clone()),
            };
            Some(SelectorIdentity::Audiobookshelf { key })
        };
        let item =
            self.episodes
                .selected_target()
                .map(|target| LibraryItemIdentity::Audiobookshelf {
                    id: format!("{}\0{}", target.library_item_id(), target.episode_id()),
                });
        (selector, item)
    }

    fn clear_selection(&mut self) {
        self.episodes.clear_owner_selection();
    }

    fn set_selection_origin(
        &mut self,
        origin: crate::app::components::media_list::SelectionOrigin,
    ) {
        self.episodes.set_selection_origin(origin);
    }

    fn selection_summary(&self) -> Option<crate::app::components::media_list::SelectionSummary> {
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
            self.hero_scroll.saturating_sub((-delta) as usize)
        } else {
            self.hero_scroll.saturating_add(delta as usize)
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
                | MediaListSurfaceInput::RangeClick(at)
                | MediaListSurfaceInput::ContextClick(at) => {
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
                MediaListSurfaceInput::DoubleClick(at) => {
                    // Resolve once, then delegate the target-bearing activation.
                    let target = self.episodes.resolve_current_point(at)?.clone();
                    self.delegate_episodes(MediaListOperation::Activate(target.clone()));
                    Some(Msg::Shell(
                        ShellRequest::AudiobookshelfPodcastEpisodeIntent(
                            PodcastEpisodeIntent::OpenOrPlay(Some(target)),
                        ),
                    ))
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
            LibrarySlotEvent::HeroActivate => Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    self.episodes.selected_target().cloned(),
                )),
            )),
        }
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.episodes.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(ShellRequest::SelectionProjection(
                self.episodes.selection_summary(),
            )));
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
            Key::Enter => Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    self.episodes.selected_target().cloned(),
                )),
            )),
            Key::Char(' ') => Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(
                    PodcastEpisodeIntent::FocusOrPlay(self.episodes.selected_target().cloned()),
                ),
            )),
            Key::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::Enqueue(
                    self.episodes.selected_target().cloned(),
                )),
            )),
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
