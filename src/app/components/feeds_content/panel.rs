use super::super::library_panel::content::{
    HeroContent, LibraryPanelContent, ListSlot, SelectorRow,
};
use super::super::library_panel::hero::hero_content_feed;
use super::super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::super::library_panel::HeroContentData;
use super::super::media_list::MediaListOperation;
use super::*;

impl FeedsContent {
    fn on_selector_picked(&mut self, index: usize) -> Option<Msg> {
        if index == 0 {
            self.latest_selected = true;
            self.rebuild_visible_entries();
            self.reset_selection();
            Some(Msg::Shell(Box::new(ShellRequest::FeedsLatestSelected)))
        } else if index <= WatchedFilter::COUNT {
            self.latest_selected = false;
            self.select_watched_filter(index - 1);
            None
        } else {
            self.latest_selected = false;
            self.select_group(index - 1 - WatchedFilter::COUNT);
            None
        }
    }

    fn on_list_event(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        match input {
            MediaListSurfaceInput::Wheel { at, delta } => {
                // The claim gate mirrors the mounted component: a wheel
                // outside the painted active list is unclaimed.
                if !self.claims_current_point(at) {
                    return None;
                }
                self.delegate_row_local_input(MediaListSurfaceInput::Wheel { at, delta }, None);
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            MediaListSurfaceInput::Click(at)
            | MediaListSurfaceInput::ToggleClick(at)
            | MediaListSurfaceInput::RangeClick(at) => self.on_row_click(input, at),
            MediaListSurfaceInput::ContextClick(at) => self.on_context_click(input, at),
            MediaListSurfaceInput::DoubleClick(at) => self.on_double_click(at),
            _ => None,
        }
    }

    fn on_row_click(
        &mut self,
        input: MediaListSurfaceInput,
        at: ratatui::layout::Position,
    ) -> Option<Msg> {
        let target = self.resolve_row_id(at)?;
        self.delegate_row_local_input(input, Some(target));
        Some(Msg::Shell(Box::new(ShellRequest::FeedsRowClick)))
    }

    fn on_context_click(
        &mut self,
        input: MediaListSurfaceInput,
        at: ratatui::layout::Position,
    ) -> Option<Msg> {
        let target = self.resolve_row_id(at)?;
        let outcome = self.delegate_row_local_input(input, Some(target.clone()));
        let entries = match outcome.external_intent {
            Some(RowIntent::ContextSelection(targets)) => targets
                .into_iter()
                .filter_map(|target| self.entry_for_target(&target).cloned())
                .collect(),
            _ => vec![self.entry_for_target(&target)?.clone()],
        };
        Some(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            crate::app::state::types::context_menu::ContextMenuTargets::Feeds(entries),
            Some((at.x, at.y)),
        ))))
    }

    fn on_double_click(&mut self, at: ratatui::layout::Position) -> Option<Msg> {
        // Resolve once, then delegate the target-bearing activation.
        let target = self.resolve_row_id(at)?;
        self.carrier
            .delegate_operation(MediaListOperation::Activate(target.clone()));
        let entry = self.entry_for_target(&target)?.clone();
        Some(Msg::Shell(Box::new(ShellRequest::FeedsPlay(vec![entry]))))
    }
}

impl LibraryContentOwner for FeedsContent {
    fn reanchor_launch_state(&mut self, state: &mbv_core::config::TuiLaunchState) -> bool {
        if self.loading && self.visible_entries.is_empty() && !self.subscriptions.is_empty() {
            return false;
        }
        if self.subscriptions.is_empty() {
            self.latest_selected = false;
            self.rebuild_visible_entries();
        } else {
            match state.selector.as_ref() {
                Some(SelectorIdentity::Feeds {
                    key: FeedsSelectorKey::Latest,
                }) => {
                    self.latest_selected = true;
                }
                Some(SelectorIdentity::Feeds {
                    key: FeedsSelectorKey::Filter(filter),
                }) => {
                    self.latest_selected = false;
                    self.selected_group = 0;
                    self.watched_filter = match filter {
                        FeedsFilter::All => WatchedFilter::All,
                        FeedsFilter::Played => WatchedFilter::Watched,
                        FeedsFilter::Unplayed => WatchedFilter::Unwatched,
                    };
                }
                Some(SelectorIdentity::Feeds {
                    key: FeedsSelectorKey::Group(FeedGroupKey::Feed(url)),
                }) => {
                    self.latest_selected = false;
                    self.selected_group = self
                        .subscriptions
                        .iter()
                        .position(|subscription| &subscription.url == url)
                        .map(|index| index + 1)
                        .unwrap_or(0);
                    self.watched_filter = WatchedFilter::All;
                }
                _ => {
                    self.latest_selected = false;
                    self.selected_group = 0;
                    self.watched_filter = WatchedFilter::All;
                }
            }
            self.rebuild_visible_entries();
        }
        let selected = match state.item.as_ref() {
            Some(LibraryItemIdentity::Feeds { id }) => self.carrier.select_target(id),
            _ => false,
        };
        if !selected {
            self.carrier.select_first();
        }
        true
    }

    fn launch_snapshot(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        let selector = if self.subscriptions.is_empty() {
            None
        } else if self.latest_selected {
            Some(SelectorIdentity::Feeds {
                key: FeedsSelectorKey::Latest,
            })
        } else if self.selected_group == 0 && self.watched_filter != WatchedFilter::All {
            // The combined row's active pill is the filter block or the
            // selected Feed group. A non-default filter is the only stable
            // identity for the filter state in the current group.
            Some(SelectorIdentity::Feeds {
                key: FeedsSelectorKey::Filter(match self.watched_filter {
                    WatchedFilter::All => FeedsFilter::All,
                    WatchedFilter::Watched => FeedsFilter::Played,
                    WatchedFilter::Unwatched => FeedsFilter::Unplayed,
                }),
            })
        } else if self.selected_group == 0 {
            Some(SelectorIdentity::Feeds {
                key: FeedsSelectorKey::Group(FeedGroupKey::All),
            })
        } else {
            self.subscriptions
                .get(self.selected_group - 1)
                .map(|subscription| SelectorIdentity::Feeds {
                    key: FeedsSelectorKey::Group(FeedGroupKey::Feed(subscription.url.clone())),
                })
        };
        let item = self
            .carrier
            .selected_target()
            .cloned()
            .map(|id| LibraryItemIdentity::Feeds { id });
        (selector, item)
    }

    fn clear_selection(&mut self) {
        self.carrier.clear_owner_selection();
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
        // Hero first: it only reads the projected snapshot, while the list
        // slot borrows the shared carrier mutably for the rest of the frame.
        let hero = self.selected_entry().map(|entry| {
            let data = hero_content_feed(entry);
            let mut facts = data.facts;
            facts.artwork.image = self.hero_image.clone();
            HeroContent {
                facts,
                overview: data.overview,
                credits: data.credits,
                workspace: None,
            }
        });
        let has_subs = !self.subscriptions.is_empty();
        // One Selector bar carries Latest, watched filters, then the existing
        // feed-group pills. Group/filter selection remains owner-local.
        let selector = has_subs.then(|| SelectorRow {
            pills: std::iter::once("Latest".to_string())
                .chain(
                    [
                        WatchedFilter::All,
                        WatchedFilter::Watched,
                        WatchedFilter::Unwatched,
                    ]
                    .iter()
                    .map(|filter| filter.label().to_string()),
                )
                .chain(std::iter::once("All".to_string()))
                .chain(
                    self.subscriptions
                        .iter()
                        .map(|subscription| trunc_str(&subscription.name, MAX_GROUP_LABEL)),
                )
                .collect(),
            markers: crate::app::components::selector_markers(
                1 + WatchedFilter::COUNT + 1 + self.subscriptions.len(),
                self.latest_marker,
            ),
            // `[`/`]` move the feed-group selection, so the active pill and
            // overflow window follow that group within the combined row.
            active: Some(if self.latest_selected {
                0
            } else {
                1 + WatchedFilter::COUNT + self.selected_group
            }),
        });
        let list = if !has_subs {
            ListSlot::Empty {
                loading: false,
                text: " No feed subscriptions configured".into(),
            }
        } else if self.visible_entries.is_empty() {
            ListSlot::Empty {
                loading: self.loading,
                text: " Press r to load feeds".into(),
            }
        } else {
            ListSlot::Media(&mut self.carrier)
        };
        LibraryPanelContent {
            selector,
            list,
            hero,
        }
    }

    /// Translate one resolved slot event into Feeds' existing typed `Msg`s
    /// (design D2): the panel resolves pointer geometry against its own
    /// painted slots, and the owner resolves the row's stable target through
    /// its own carrier.
    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => self.on_selector_picked(index),
            LibrarySlotEvent::List(input) => self.on_list_event(input),
            // Feeds has no Workspace and no hero-pane input of its own.
            LibrarySlotEvent::WorkspaceSelectorPicked(_) | LibrarySlotEvent::HeroPane(_) => None,
            LibrarySlotEvent::HeroActivate => self
                .selected_entry()
                .cloned()
                .map(|entry| Msg::Shell(Box::new(ShellRequest::FeedsPlay(vec![entry])))),
        }
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        self.handle_key(key)
    }

    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        match self.handle_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))),
            None if matches!(
                key.code,
                Key::Up
                    | Key::Down
                    | Key::PageUp
                    | Key::PageDown
                    | Key::Home
                    | Key::End
                    | Key::Left
                    | Key::Right
            ) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
    }

    // Feed entries are leaf Heroes: the first Enter opens the Library Hero
    // overlay and the overlay's subsequent activation uses the same typed
    // playback request as the browser row.
    fn hero_overlay_available(&mut self) -> bool {
        self.selected_entry().is_some()
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        self.selected_entry().map(hero_content_feed)
    }

    fn set_hero_image(&mut self, state: HeroImageState) {
        self.hero_image = state;
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
