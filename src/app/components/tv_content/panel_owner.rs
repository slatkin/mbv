use super::*;

impl TvContent {
    /// This frame's typed Library panel content (design D3, task 8.2): the
    /// letter pills are the one Selector row, the hero comes from the shared
    /// `EmbyItem` producer with the shell-projected image state, and the
    /// Workspace is the season pills plus the episode list. The Inline Search
    /// session takes the list slot while active (the panel places its box in
    /// the Selector row and its results in the list box).
    pub(super) fn panel_content(&mut self) -> LibraryPanelContent<'_> {
        let searching = self.inline_search.is_active();
        // Hero facts first: reading the projected snapshot and image state
        // ends before the Workspace borrows the episode carrier mutably.
        let flat_episode_mode = self.flat_episode_mode();
        let hero_item = if flat_episode_mode && !self.is_wide && self.hero_overlay_open {
            self.selected_episode_item()
        } else {
            None
        };
        let hero_data = if flat_episode_mode {
            hero_item.map(|episode| hero_content_emby(&episode))
        } else {
            self.context
                .selected_series
                .clone()
                .map(|series| hero_content_emby(&series))
        };
        let hero_data = hero_data.map(|mut data| {
            data.facts.artwork.image = self.context.hero_image.clone();
            data
        });
        // Season pills are the Workspace selector; without a resolvable
        // season there is no selector row and the episode box takes the
        // whole workspace.
        let workspace_selector = self
            .context
            .series_detail
            .as_ref()
            .filter(|detail| !detail.seasons.is_empty())
            .map(|detail| SelectorRow {
                pills: detail
                    .seasons
                    .iter()
                    .map(|season| season.display_name())
                    .collect(),
                markers: vec![],
                active: Some(self.season_cursor.min(detail.seasons.len() - 1)),
            });
        let workspace_focused = self.context.focused && self.pane == Pane::Episodes;
        let selector = if !searching && self.context.show_letter_pills {
            let large = self
                .context
                .list
                .library_total
                .is_some_and(|total| total > crate::app::render::LIBRARY_PILL_THRESHOLD);
            let mut pills = vec!["Latest".to_string(), "Upcoming".to_string()];
            let latest_marker = self.latest_marker;
            if large {
                pills.extend(LetterFilter::labels_for_kind(LetterFilterKind::Tv));
            } else {
                pills.push("All".to_string());
            }
            let active = match self.context.tv_content_mode.as_ref() {
                Some(mbv_core::config::TvContentMode::Latest) => 0,
                Some(mbv_core::config::TvContentMode::Upcoming) => 1,
                Some(mbv_core::config::TvContentMode::All) => 2,
                Some(mbv_core::config::TvContentMode::Range(index)) => index + 2,
                None => {
                    if large {
                        0
                    } else {
                        2
                    }
                }
            };
            Some(SelectorRow {
                markers: std::iter::once(latest_marker)
                    .chain(std::iter::repeat_n(false, pills.len().saturating_sub(1)))
                    .collect(),
                pills,
                active: Some(active),
            })
        } else {
            None
        };
        let hero = hero_data.map(|data| HeroContent {
            facts: data.facts,
            overview: data.overview,
            credits: data.credits,
            workspace: (!flat_episode_mode).then_some(Workspace {
                header: None,
                selector: workspace_selector,
                list: &mut self.episodes,
                focused: workspace_focused,
            }),
        });
        let list = if searching {
            ListSlot::Search(&mut self.inline_search)
        } else if flat_episode_mode {
            ListSlot::Media(&mut self.carrier)
        } else {
            ListSlot::Media(&mut self.browser)
        };
        LibraryPanelContent {
            selector,
            list,
            hero,
        }
    }
}

impl InlineSearchHost for TvContent {
    fn inline_search(&self) -> &InlineSearch {
        &self.inline_search
    }
    fn inline_search_mut(&mut self) -> &mut InlineSearch {
        &mut self.inline_search
    }
}
impl LibraryContentOwner for TvContent {
    fn clear_selection(&mut self) {
        self.carrier.clear_owner_selection();
    }

    fn hero_overlay_target_available(&mut self) -> bool {
        !self.flat_episode_mode() && self.selected_item().is_some()
    }

    fn hero_overlay_enter_available(&mut self) -> bool {
        !self.flat_episode_mode()
            && matches!(self.browser.selected_target(), Some(TvTreeTarget::Show(_)))
    }

    fn mini_view_hero_available(&mut self) -> bool {
        !self.is_wide && self.flat_episode_mode() && self.selected_episode_item().is_some()
    }

    fn browser_rows_are_hero_bearing(&mut self) -> bool {
        !self.flat_episode_mode()
    }

    fn inline_search_session(&mut self) -> Option<&mut dyn InlineSearchHost> {
        Some(self)
    }

    fn inline_search_session_ref(&self) -> Option<&dyn InlineSearchHost> {
        Some(self)
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
        self.panel_content()
    }
    /// Translate one resolved slot event from the panel into TV's existing
    /// typed `Msg`s (design D2/D12). The panel hands over the pills, list
    /// and hero-pane input it painted; this owner resolves the row-local
    /// target through its own carriers.
    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        self.handle_slot_event(event)
    }
    /// This owner's local key interpretation, forwarded by the focused panel
    /// (the merged component's `handle_key` contract, unchanged: the router
    /// owns every global chord and keeps precedence).
    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        self.handle_key(key)
    }
    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        match self.on_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(message)),
            None if self.inline_search.is_active()
                && matches!(
                    key.code,
                    tuirealm::event::Key::Esc
                        | tuirealm::event::Key::Enter
                        | tuirealm::event::Key::Backspace
                        | tuirealm::event::Key::Up
                        | tuirealm::event::Key::Down
                        | tuirealm::event::Key::Left
                        | tuirealm::event::Key::Right
                        | tuirealm::event::Key::Char(_)
                ) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
    }
    fn inline_search_active(&self) -> bool {
        self.inline_search.is_active()
    }

    fn launch_selector(
        &self,
        state: &mbv_core::config::TuiLaunchState,
    ) -> Option<super::super::library_panel::owner::LaunchSelector> {
        if !self.context.show_letter_pills {
            return None;
        }
        let current = self
            .context
            .list
            .letter_filter
            .as_ref()
            .map(|filter| filter.index);
        match state.selector.as_ref() {
            Some(SelectorIdentity::Emby {
                key: EmbySelectorKey::Letter(bucket),
            }) => {
                let target = bucket.to_index();
                (current != Some(target)).then_some(
                    super::super::library_panel::owner::LaunchSelector::Emby { index: target },
                )
            }
            // No letter pill is represented by an index. The shell uses
            // this out-of-band value for the distinct clear intent.
            _ if current.is_some() => {
                Some(super::super::library_panel::owner::LaunchSelector::Emby { index: usize::MAX })
            }
            _ => None,
        }
    }

    fn reanchor_launch_state(&mut self, state: &mbv_core::config::TuiLaunchState) -> bool {
        if self.context.list.loading && self.context.list.items.is_empty() {
            return false;
        }
        // The shell applies the selector through App and pushes the resulting
        // content before this item-level re-anchor. Keep selector state
        // owned by that projection rather than mirroring it here.
        let selected = match state.item.as_ref() {
            Some(LibraryItemIdentity::Emby { id }) => self.carrier.select_target(id),
            _ => false,
        };
        if !selected {
            self.carrier.select_first();
        }
        true
    }

    fn launch_snapshot(&self) -> (Option<SelectorIdentity>, Option<LibraryItemIdentity>) {
        // TV's season pills live in the Hero Workspace and are deliberately
        // excluded from the bounded launch snapshot. Only the main letter
        // Selector and the selected series belong here.
        let selector = self.context.show_letter_pills.then(|| {
            let key = self
                .context
                .list
                .letter_filter
                .as_ref()
                .and_then(|filter| EmbyLetterBucket::from_index(filter.index))
                .map(EmbySelectorKey::Letter)
                .unwrap_or(EmbySelectorKey::Unfiltered);
            SelectorIdentity::Emby { key }
        });
        let item = self
            .carrier
            .selected_target()
            .cloned()
            .map(|id| LibraryItemIdentity::Emby { id });
        (selector, item)
    }

    fn focus_hero_workspace(&mut self) -> bool {
        if self.flat_episode_mode() {
            // The compact flat-episode hero is passive: its browser carrier
            // remains focused so movement and mode cycling keep reaching the
            // visible list rather than the hidden season workspace.
            return false;
        }
        self.pane = Pane::Episodes;
        true
    }

    fn clear_hero_workspace_focus(&mut self) {
        self.pane = Pane::Series;
    }

    fn set_hero_overlay_open(&mut self, open: bool) {
        self.set_hero_overlay_open(open);
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        if self.flat_episode_mode() {
            (self.hero_overlay_open && !self.is_wide)
                .then(|| self.selected_episode_item())
                .flatten()
                .map(|episode| hero_content_emby(&episode))
        } else {
            self.context.selected_series.as_ref().map(hero_content_emby)
        }
    }
    fn set_hero_image(&mut self, state: HeroImageState) {
        self.context.hero_image = state;
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
