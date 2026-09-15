impl TvContent {
    pub(super) fn context_menu_request(&mut self) -> Option<ShellRequest> {
        let outcome = self.carrier.delegate_operation(MediaListSurfaceInput::Context.into_operation(None).expect("resolved media-list pointer target"));
        let items = match outcome.external_intent {
            Some(RowIntent::ContextSelection(targets)) => targets
                .into_iter()
                .filter_map(|target| self.context.list.items.iter().find(|item| item.id == target).cloned())
                .collect(),
            Some(RowIntent::Context(target)) => self
                .context
                .list
                .items
                .iter()
                .find(|item| item.id == target)
                .cloned()
                .into_iter()
                .collect(),
            _ => return None,
        };
        Some(ShellRequest::RowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            None,
        ))
    }

    fn handle_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        // Inline Search gets first refusal while active (design.md D6): the
        // panel paints the box in the Selector row's rect and the results in
        // the list box, so a list-slot input belongs to the search session.
        if self.inline_search.is_active() {
            if let LibrarySlotEvent::List(input) = event {
                return self.handle_search_pointer(input);
            }
            return None;
        }
        match event {
            // The letter pills are the one Selector row at both breakpoints.
            LibrarySlotEvent::SelectorPicked(index) => Some(Msg::Shell(ShellRequest::TvHitClick {
                hit: TvHit::LetterPill(index),
            })),
            // The season pills ride in the hero pane's Workspace Selector
            // row (Wide only).
            LibrarySlotEvent::WorkspaceSelectorPicked(index) => {
                self.apply_pane_click(
                    TvHit::SeasonTab(index),
                    Position::new(0, 0),
                    MediaListSurfaceInput::Click(Position::new(0, 0)),
                );
                Some(Msg::Shell(ShellRequest::TvHitClick {
                    hit: TvHit::SeasonTab(index),
                }))
            }
            // The Browser pane's series list.
            LibrarySlotEvent::List(input) => self.series_list_event(input),
            LibrarySlotEvent::HeroActivate => match self.pane {
                Pane::Series => self
                    .selected_item()
                    .map(|item| Msg::Shell(ShellRequest::TvActivate { item })),
                Pane::Episodes => self.selected_episode_item().map(|episode| {
                    Msg::Shell(ShellRequest::TvEpisodeActivate { episode })
                }),
            },
            // The hero pane: the episode box's rows, or the pane itself.
            LibrarySlotEvent::HeroPane(input) => self.hero_pane_event(input),
            LibrarySlotEvent::ControlPicked(_) => None,
        }
    }

    /// The series list's row-local input. Wide and Narrow share the one
    /// carrier, so the resolved target and the emitted `TvHit` are the same
    /// at both breakpoints; the shell's `TvHit*` arms do the rest.
    fn series_list_event(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        match input {
            MediaListSurfaceInput::Wheel { at, delta } => {
                // The series rail is the only scrollable TV surface. Its
                // canonical control claims the painted region.
                if !self.carrier.claims_current_point(at) {
                    return None;
                }
                self.move_rows(delta);
                // Return a framework-visible claim after mutating local
                // state; dropping the message would let the framework's
                // mutation be discarded by the mouse fold.
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            MediaListSurfaceInput::Click(at) | MediaListSurfaceInput::ToggleClick(at) | MediaListSurfaceInput::RangeClick(at) => {
                let hit = self.resolve_series_hit(at)?;
                self.apply_pane_click(hit.clone(), at, input);
                let _ = ();
                Some(Msg::Shell(ShellRequest::TvHitClick { hit }))
            }
            MediaListSurfaceInput::DoubleClick(at) => {
                let hit = self.resolve_series_hit(at)?;
                self.apply_pane_click(hit.clone(), at, MediaListSurfaceInput::Click(at));
                Some(Msg::Shell(ShellRequest::TvHitDoubleClick { hit }))
            }
            MediaListSurfaceInput::ContextClick(at) => {
                let target = self.carrier.resolve_current_point(at)?.clone();
                let item = self.context.list.items.iter().find(|item| item.id == target)?.clone();
                let outcome = self.carrier.delegate_operation(input.into_operation(Some(target)).expect("resolved media-list pointer target"));
                let _ = ();
                let items = match outcome.external_intent {
                    Some(RowIntent::ContextSelection(targets)) => targets
                        .into_iter()
                        .filter_map(|target| self.context.list.items.iter().find(|item| item.id == target).cloned())
                        .collect(),
                    _ => vec![item],
                };
                Some(Msg::Shell(ShellRequest::RowContextMenu(
                    crate::app::types_context_menu::ContextMenuTargets::Emby(items),
                    Some((at.x, at.y)),
                )))
            }
            _ => None,
        }
    }

    /// The hero pane's row-local input: the Workspace episode box resolves
    /// its row through its own carrier, and blank pane space is the
    /// `EpisodesPane` hit the deleted `resolve_hit` fallback produced.
    fn hero_pane_event(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        let at = match input {
            MediaListSurfaceInput::Click(at)
            | MediaListSurfaceInput::ToggleClick(at)
            | MediaListSurfaceInput::RangeClick(at)
            | MediaListSurfaceInput::DoubleClick(at)
            | MediaListSurfaceInput::ContextClick(at) => at,
            // The series rail is the only scrollable TV surface; a wheel
            // over the hero pane is unclaimed (legacy `handle_mouse_wide`).
            MediaListSurfaceInput::Wheel { .. } => return None,
            _ => return None,
        };
        let hit = if self.episodes.claims_current_point(at) {
            self.episodes
                .resolve_current_point(at)
                .cloned()
                .map(TvHit::EpisodeRow)?
        } else {
            TvHit::EpisodesPane
        };
        match input {
            MediaListSurfaceInput::Click(_) | MediaListSurfaceInput::ToggleClick(_) | MediaListSurfaceInput::RangeClick(_) => {
                self.apply_pane_click(hit.clone(), at, input);
                let _ = ();
                Some(Msg::Shell(ShellRequest::TvHitClick { hit }))
            }
            MediaListSurfaceInput::DoubleClick(_) => {
                self.apply_pane_click(hit.clone(), at, MediaListSurfaceInput::Click(at));
                Some(Msg::Shell(ShellRequest::TvHitDoubleClick { hit }))
            }
            MediaListSurfaceInput::ContextClick(_) => {
                let item = match &hit {
                    TvHit::EpisodeRow(target) => self.current_season_episodes().iter().find(|item| item.id == *target).cloned(),
                    TvHit::SeriesRow(target) => self.context.list.items.iter().find(|item| item.id == *target).cloned(),
                    _ => None,
                }?;
                let outcome = self.episodes.delegate_operation(input.into_operation(Some(match &hit {
                    TvHit::EpisodeRow(target) => target.clone(),
                    _ => return None,
                })).expect("resolved media-list pointer target"));
                let _ = ();
                let items = match outcome.external_intent {
                    Some(RowIntent::ContextSelection(targets)) => targets
                        .into_iter()
                        .filter_map(|target| self.current_season_episodes().iter().find(|item| item.id == target).cloned())
                        .collect(),
                    _ => vec![item],
                };
                Some(Msg::Shell(ShellRequest::RowContextMenu(
                    crate::app::types_context_menu::ContextMenuTargets::Emby(items),
                    Some((at.x, at.y)),
                )))
            },
            _ => None,
        }
    }

    /// Inline Search pointer handling (mirrors `EmbyLibraryContent::
    /// handle_search_pointer`): the panel's own recognizer already collapsed
    /// the raw event into a normalized `MediaListSurfaceInput`, so click /
    /// double-click / right-click / wheel against a painted result row are
    /// reproduced here.
    fn handle_search_pointer(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        match input {
            MediaListSurfaceInput::Click(at) => {
                self.inline_search.select_row_at_point(at);
                None
            }
            MediaListSurfaceInput::DoubleClick(at) => {
                self.inline_search.select_row_at_point(at);
                self.inline_search.selected_item().map(|item| {
                    Msg::Shell(ShellRequest::InlineSearchActivate {
                        id: item.id,
                        item_type: item.item_type,
                    })
                })
            }
            MediaListSurfaceInput::ContextClick(at) => {
                self.inline_search.select_row_at_point(at);
                self.inline_search
                    .selected_item()
                    .map(|item| Msg::Shell(ShellRequest::RowContextMenu(
                        crate::app::types_context_menu::ContextMenuTargets::Emby(vec![item]), None,
                    )))
            }
            MediaListSurfaceInput::Wheel { delta, .. } => {
                self.inline_search.move_cursor_by(delta);
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            _ => None,
        }
    }

    /// Resolve a click in the Browser pane's list slot to the series row it
    /// landed on from the carrier's own retained frame geometry.
    fn resolve_series_hit(&mut self, at: Position) -> Option<TvHit> {
        self.carrier
            .resolve_current_point(at)
            .cloned()
            .map(TvHit::SeriesRow)
    }

    /// Move the owner's local pane + pane cursor to the clicked `hit` (Wide
    /// only). A click in the unfocused pane moves local focus there; a click
    /// in the already-focused pane keeps it. Clicking a season pill also
    /// selects that season; blank Episodes-pane space is consumed without
    /// changing the pane. Right-clicks never call this.
    fn apply_pane_click(&mut self, hit: TvHit, _at: Position, input: MediaListSurfaceInput) {
        match hit {
            TvHit::SeasonTab(index) => {
                self.pane = Pane::Episodes;
                self.season_cursor = index;
                self.refresh_episode_rows();
                self.episodes.select_first();
            }
            TvHit::EpisodeRow(target) => {
                self.pane = Pane::Episodes;
                self.episodes.delegate_operation(input.into_operation(Some(target)).expect("resolved media-list pointer target"));
            }
            TvHit::SeriesRow(target) => {
                self.pane = Pane::Series;
                self.carrier.delegate_operation(input.into_operation(Some(target)).expect("resolved media-list pointer target"));
            }
            TvHit::EpisodesPane | TvHit::LetterPill(_) => {}
        }
    }

    #[cfg(test)]
    pub(crate) fn test_episode_claim_rect(&self) -> Option<ratatui::layout::Rect> {
        self.episodes.wide().current_claim_rect()
    }

    /// Test-only: the owner's local key interpretation, so shell tests can
    /// drive it without importing `LibraryContentOwner`.
    #[cfg(test)]
    pub(in crate::app) fn test_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        self.handle_key(key)
    }

    /// Test-only: translate one panel slot event, so shell tests can drive
    /// pointer semantics without importing `LibraryContentOwner`.
    #[cfg(test)]
    pub(in crate::app) fn test_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        self.handle_slot_event(event)
    }

    /// Test-only: the embedded Inline Search session, for the component-level
    /// search tests (the panel forwards the index/keyboard to it).
    #[cfg(test)]
    pub(crate) fn inline_search(&self) -> &InlineSearch {
        &self.inline_search
    }

    #[cfg(test)]
    pub(crate) fn inline_search_mut(&mut self) -> &mut InlineSearch {
        &mut self.inline_search
    }

    /// Test-only cursor seed for the embedded TV content owner:
    /// seeds the shared owner's stable target from a raw `context.list.items`
    /// index, for tests driving the merged component directly.
    #[cfg(test)]
    pub(crate) fn set_cursor_for_test(&mut self, cursor: usize) {
        if let Some(item) = self.context.list.items.get(cursor) {
            let target = item.id.clone();
            self.carrier.select_target(&target);
        }
    }

    #[cfg(test)]
    pub(crate) fn select_targets_for_test(&mut self, targets: &[String]) {
        let Some(first) = targets.first() else { return };
        self.carrier.select_target(first);
        self.carrier.enter_visual_mode();
        for target in targets.iter().skip(1) {
            self.carrier.select_target(target);
            self.carrier.extend_selection_to(target);
        }
        let _ = self.carrier.selection_summary();
    }

    #[cfg(test)]
    pub(crate) fn context_click_for_test(&mut self, target: String) -> Option<usize> {
        self.carrier.delegate_operation(MediaListSurfaceInput::ContextClick(Position::new(0, 0)).into_operation(Some(target)).expect("resolved media-list pointer target"));
        Some(self.carrier.multi_selection().len())
    }

    /// Test-only: the shared owner's current rows' semantic states, in
    /// display order.
    #[cfg(test)]
    pub(crate) fn test_row_semantic_states(&self) -> Vec<MediaSemanticState> {
        self.carrier
            .rows()
            .iter()
            .filter_map(|row| match row {
                MediaListRow::Item { semantic_state, .. } => Some(semantic_state.clone()),
                _ => None,
            })
            .collect()
    }
}

