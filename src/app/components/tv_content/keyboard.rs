use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::super::inline_search::InlineSearchAction;
use super::super::list::tree_browser::TreeOperation;
use super::{
    Msg, Pane, ShellRequest, TerminalObserverEvent, TvContent, TvDisplayMode, TvTreeTarget,
};
use crate::app::components::media_list::MediaListSurfaceInput;
use mbv_core::api::EmbyItem;

impl TvContent {
    /// Ctrl+P/S/A on the selected Inline Search result reuse the ordinary
    /// library result-row effects, resolved against the search cursor (result-
    /// row shortcut actions stay available while search is open).
    fn inline_search_result_action(&mut self, key: &KeyEvent) -> Option<Msg> {
        if !self.context.focused || !key.modifiers.contains(KeyModifiers::CONTROL) {
            return None;
        }
        let item = self.inline_search.selected_item()?;
        let request = match key.code {
            Key::Char('p') => ShellRequest::EmbyLibraryPlay { item },
            Key::Char('s') => ShellRequest::EmbyLibraryShuffle { item },
            Key::Char('a') => ShellRequest::EmbyLibraryEnqueue { item },
            _ => return None,
        };
        // A launched result exits search like Enter activation does; leaving it
        // open traps focus in the text entry so no other key reaches the shell.
        self.inline_search.close();
        Some(Msg::Shell(Box::new(request)))
    }

    pub(super) fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        // Inline Search gets first refusal while active (design.md D4): the
        // component returns immediately after delegating, even when search
        // consumes the key without producing a message.
        if self.inline_search.is_active() {
            return match self.inline_search.handle_key(key) {
                Some(InlineSearchAction::Activate { id, item_type }) => {
                    Some(Msg::Shell(Box::new(ShellRequest::InlineSearchActivate {
                        id,
                        item_type,
                    })))
                }
                Some(InlineSearchAction::Dismiss) => {
                    // Escape/empty-query Backspace dismiss locally
                    // (design.md D4); no shell effect.
                    self.inline_search.close();
                    None
                }
                Some(InlineSearchAction::QueryStarted) => {
                    Some(Msg::Shell(Box::new(ShellRequest::InlineSearchQueryStarted)))
                }
                // Ctrl+P/S/A that the shared control does not consume act on
                // the selected result row via the ordinary result-row effects.
                None => self.inline_search_result_action(key),
            };
        }
        if self.flat_episode_mode() && self.carrier.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(Box::new(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            ))));
        }
        if !self.context.focused {
            return None;
        }
        // The Library Hero overlay's focused Workspace keeps its local chords
        // in Narrow geometry: the overlay is the only Narrow surface that
        // focuses the Episodes pane, and the covered browser list must never
        // receive its movement keys. The overlay exists only in non-Wide
        // geometry, so the actual breakpoint — not the pushed bit alone —
        // gates these arms: a stale bit in Wide must never shadow the Wide
        // workspace's own handling.
        if self.display_mode == TvDisplayMode::Narrow
            && self.hero_overlay_open
            && self.pane == Pane::Episodes
            && !self.flat_episode_mode()
        {
            return self.handle_key_overlay_workspace(key);
        }
        if !self.flat_episode_mode() && self.pane == Pane::Series {
            return self.handle_show_tree_key(key);
        }
        if self.display_mode == TvDisplayMode::Wide {
            self.handle_key_wide(key)
        } else {
            self.handle_key_narrow(key)
        }
    }

    /// The Library Hero overlay's focused Workspace chords (Narrow
    /// geometry): the same translation the Wide Episodes pane uses for
    /// movement and activation, extended with the pager/jump chords the
    /// covered browser must never receive. The panel claims unhandled chords
    /// while the overlay is open, so `None` here stays overlay-local.
    fn handle_key_overlay_workspace(&mut self, key: &KeyEvent) -> Option<Msg> {
        let request = match key.code {
            Key::Enter => self
                .selected_episode_item()
                .map(|episode| ShellRequest::TvEpisodeActivate { episode }),
            Key::Up | Key::Char('k') => {
                self.move_episode(-1);
                Some(ShellRequest::TvEpisodeMove { delta: -1 })
            }
            Key::Down | Key::Char('j') => {
                self.move_episode(1);
                Some(ShellRequest::TvEpisodeMove { delta: 1 })
            }
            Key::Char('[')
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.move_season(-1);
                Some(ShellRequest::TvSeasonMove { delta: -1 })
            }
            Key::Char(']')
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.move_season(1);
                Some(ShellRequest::TvSeasonMove { delta: 1 })
            }
            Key::PageUp => Some(self.move_episode_by(MediaListSurfaceInput::Page(-1))),
            Key::PageDown => Some(self.move_episode_by(MediaListSurfaceInput::Page(1))),
            Key::Home => Some(self.move_episode_by(MediaListSurfaceInput::First)),
            Key::End => Some(self.move_episode_by(MediaListSurfaceInput::Last)),
            _ => None,
        };
        request.map(|request| Msg::Shell(Box::new(request)))
    }

    /// Show-mode navigation changes the tree's selected stable target. Moving
    /// between show roots also projects the resolved show position to the
    /// shell so its existing series-detail request remains synchronized.
    ///
    /// The shared library effects (play/enqueue/watch/shuffle, rescan/
    /// refresh, search, letter-pill cycle) live in
    /// [`Self::shared_library_effect`]; this entry point resolves the tree's
    /// item and delegates those chords before its own navigation/activation/
    /// context arms. `.` stays caller-specific: show-tree resolves TreeBrowser
    /// context targets while Wide uses `context_menu_request`.
    fn shared_library_effect(&mut self, key: &KeyEvent, item: Option<EmbyItem>) -> Option<Msg> {
        if let Some(request) = Self::shared_library_item_effect(key, item) {
            return Some(Msg::Shell(Box::new(request)));
        }
        let request = match key.code {
            Key::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                ShellRequest::EmbyLibraryRescan
            }
            Key::Char('r')
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                ShellRequest::EmbyLibraryRefresh
            }
            Key::Char('/') => {
                self.inline_search.open();
                ShellRequest::OpenInlineSearch
            }
            Key::Char(c @ ('[' | ']'))
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                ShellRequest::TvCycleLetterPill {
                    delta: if c == '[' { -1 } else { 1 },
                }
            }
            _ => return None,
        };
        Some(Msg::Shell(Box::new(request)))
    }

    /// Shared row actions resolve against the selected library item in each TV layout.
    fn shared_library_item_effect(key: &KeyEvent, item: Option<EmbyItem>) -> Option<ShellRequest> {
        match key.code {
            Key::Char('p' | 'a' | 'w' | 's') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let item = item?;
                Some(match key.code {
                    Key::Char('p') => ShellRequest::EmbyLibraryPlay { item },
                    Key::Char('a') => ShellRequest::EmbyLibraryEnqueue { item },
                    Key::Char('w') => ShellRequest::EmbyLibraryToggleWatched { item },
                    Key::Char('s') => ShellRequest::EmbyLibraryShuffle { item },
                    _ => return None,
                })
            }
            _ => None,
        }
    }

    /// Turn a resolved show activation into the shell's series-activation
    /// request. Wide layout also narrows focus to the episode pane first,
    /// which is why the `Enter` and `Right` arms share it.
    fn activate_show_tree_selection(&mut self, item: Option<EmbyItem>) -> Option<ShellRequest> {
        let item = item?;
        if self.display_mode == TvDisplayMode::Wide {
            self.episodes.select_first();
            self.pane = Pane::Episodes;
        }
        Some(ShellRequest::TvActivate { item })
    }

    fn handle_show_tree_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if let Some(message) = self.handle_show_tree_movement(key) {
            return Some(message);
        }
        if matches!(key.code, Key::Esc | Key::Backspace) {
            return Some(Msg::Shell(Box::new(ShellRequest::TvBack)));
        }

        let target = self.browser.selected_target().cloned();
        let item = target
            .as_ref()
            .and_then(|target| self.show_item_for_tree_target(target));
        // Shared library effects resolve against the tree's item; the match
        // below keeps only branch-specific navigation/activation/context.
        if let Some(message) = self.shared_library_effect(key, item.clone()) {
            return Some(message);
        }
        self.handle_show_tree_action(key, target, item)
    }

    fn handle_show_tree_movement(&mut self, key: &KeyEvent) -> Option<Msg> {
        let selected = self.browser.selected_target().cloned();
        let transition = match key.code {
            Key::Up | Key::Char('k') => Some(self.browser.apply(TreeOperation::Move(-1))),
            Key::Down | Key::Char('j') => Some(self.browser.apply(TreeOperation::Move(1))),
            Key::PageUp => Some(self.browser.apply(TreeOperation::Page(-1))),
            Key::PageDown => Some(self.browser.apply(TreeOperation::Page(1))),
            Key::Home => Some(self.browser.apply(TreeOperation::First)),
            Key::End => Some(self.browser.apply(TreeOperation::Last)),
            _ => None,
        }?;
        let changed = transition.selected_target != selected;
        let selected = transition.selected_target;
        Some(if changed {
            selected
                .as_ref()
                .and_then(|target| self.tree_selection_request(target))
                .unwrap_or(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        } else {
            Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed)
        })
    }

    fn handle_show_tree_action(
        &mut self,
        key: &KeyEvent,
        target: Option<TvTreeTarget>,
        item: Option<EmbyItem>,
    ) -> Option<Msg> {
        let request = match key.code {
            Key::Enter => return Some(self.activate_show_tree_key(item)),
            Key::Right => return Some(self.expand_show_tree_right(target, item)),
            Key::Left => return self.collapse_show_tree_left(target),
            Key::Char('.') => self.show_tree_context_request(),
            _ => None,
        };
        request.map(|request| Msg::Shell(Box::new(request)))
    }

    fn activate_show_tree_key(&mut self, item: Option<EmbyItem>) -> Msg {
        let activated = match self.browser.apply(TreeOperation::Activate).external_intent {
            Some(crate::app::components::list::tree_browser::TreeExternalIntent::Activate(
                target,
            )) => Some(target),
            _ => None,
        };
        match activated {
            Some(TvTreeTarget::Show(_)) => self.activate_show_tree_selection(item).map_or(
                Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed),
                |request| Msg::Shell(Box::new(request)),
            ),
            Some(target @ TvTreeTarget::Season { .. }) => self
                .toggle_tree_expansion(target)
                .unwrap_or(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed)),
            Some(TvTreeTarget::Episode { .. }) => item.map_or(
                Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed),
                |episode| Msg::Shell(Box::new(ShellRequest::TvEpisodeActivate { episode })),
            ),
            None => Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed),
        }
    }

    fn expand_show_tree_right(
        &mut self,
        target: Option<TvTreeTarget>,
        item: Option<EmbyItem>,
    ) -> Msg {
        if let Some(target) = target {
            if !self.browser.is_expanded(&target) {
                if let Some(message) = self.toggle_tree_expansion(target) {
                    return message;
                }
            }
        }
        let intent = self.browser.apply(TreeOperation::Right).external_intent;
        let activation = match intent {
            Some(crate::app::components::list::tree_browser::TreeExternalIntent::Activate(
                TvTreeTarget::Show(_),
            )) => self.activate_show_tree_selection(item),
            _ => None,
        };
        activation.map_or(
            Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed),
            |request| Msg::Shell(Box::new(request)),
        )
    }

    fn collapse_show_tree_left(&mut self, target: Option<TvTreeTarget>) -> Option<Msg> {
        if let Some(target) = target {
            if self.browser.is_expanded(&target) {
                self.browser
                    .apply(TreeOperation::ToggleExpansionTarget(target));
            } else {
                self.browser.apply(TreeOperation::Parent);
            }
            return Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed));
        }
        None
    }

    fn show_tree_context_request(&mut self) -> Option<ShellRequest> {
        let intent = self.browser.apply(TreeOperation::Context).external_intent;
        let targets = match intent {
            Some(crate::app::components::list::tree_browser::TreeExternalIntent::Context(
                target,
            )) => vec![target],
            Some(
                crate::app::components::list::tree_browser::TreeExternalIntent::ContextSelection(
                    targets,
                ),
            ) => targets,
            _ => Vec::new(),
        };
        let items: Vec<_> = targets
            .iter()
            .filter_map(|target| self.show_item_for_tree_target(target))
            .collect();
        (!items.is_empty()).then_some(ShellRequest::RowContextMenu(
            crate::app::state::types::context_menu::ContextMenuTargets::Emby(items),
            None,
        ))
    }

    /// Wide pane-based keyboard handling: keep activation/context here and
    /// delegate movement to its focused-pane helpers. Shared library effects
    /// still resolve against the selected series item, including in Episodes.
    fn handle_key_wide(&mut self, key: &KeyEvent) -> Option<Msg> {
        let request = match key.code {
            Key::Enter => self.wide_activation_request(),
            Key::Esc | Key::Backspace => {
                self.pane = Pane::Series;
                Some(ShellRequest::TvBack)
            }
            Key::Char('.') => self.context_menu_request(),
            _ => self.wide_movement_request(key),
        };
        if let Some(request) = request {
            return Some(Msg::Shell(Box::new(request)));
        }
        // The series-list selection remains authoritative even while the
        // local Episodes pane is focused, matching the legacy stack target.
        self.shared_library_effect(key, self.selected_item())
    }

    fn wide_activation_request(&mut self) -> Option<ShellRequest> {
        if self.pane == Pane::Series && !self.flat_episode_mode() {
            self.episodes.select_first();
            self.pane = Pane::Episodes;
            // Resolve the Series from its own cursor; emit nothing if absent.
            self.selected_item()
                .map(|item| ShellRequest::TvActivate { item })
        } else {
            self.selected_episode_item()
                .map(|episode| ShellRequest::TvEpisodeActivate { episode })
        }
    }

    fn wide_movement_request(&mut self, key: &KeyEvent) -> Option<ShellRequest> {
        self.wide_episode_movement_request(key)
            .or_else(|| self.wide_row_movement_request(key))
    }

    fn wide_episode_movement_request(&mut self, key: &KeyEvent) -> Option<ShellRequest> {
        match key.code {
            Key::Up | Key::Char('k') if self.pane == Pane::Episodes => {
                self.move_episode(-1);
                Some(ShellRequest::TvEpisodeMove { delta: -1 })
            }
            Key::Down | Key::Char('j') if self.pane == Pane::Episodes => {
                self.move_episode(1);
                Some(ShellRequest::TvEpisodeMove { delta: 1 })
            }
            Key::Char('[')
                if self.pane == Pane::Episodes
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.move_season(-1);
                Some(ShellRequest::TvSeasonMove { delta: -1 })
            }
            Key::Char(']')
                if self.pane == Pane::Episodes
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.move_season(1);
                Some(ShellRequest::TvSeasonMove { delta: 1 })
            }
            _ => None,
        }
    }

    fn wide_row_movement_request(&mut self, key: &KeyEvent) -> Option<ShellRequest> {
        match key.code {
            Key::Up | Key::Char('k') => {
                self.move_rows(-1);
                Some(ShellRequest::TvMoveRows { rows: -1 })
            }
            Key::Down | Key::Char('j') => {
                self.move_rows(1);
                Some(ShellRequest::TvMoveRows { rows: 1 })
            }
            Key::PageUp => {
                let height = i64::try_from(self.painted_viewport_height()).unwrap_or(i64::MAX);
                let rows = -(height.saturating_sub(1).max(1));
                self.move_rows(rows);
                Some(ShellRequest::TvMoveRows { rows })
            }
            Key::PageDown => {
                let height = i64::try_from(self.painted_viewport_height()).unwrap_or(i64::MAX);
                let rows = height.saturating_sub(1).max(1);
                self.move_rows(rows);
                Some(ShellRequest::TvMoveRows { rows })
            }
            Key::Home => {
                self.jump_cursor(false);
                Some(ShellRequest::TvJumpCursor { to_end: false })
            }
            Key::End => {
                self.jump_cursor(true);
                Some(ShellRequest::TvJumpCursor { to_end: true })
            }
            _ => None,
        }
    }

    /// Narrow flat-list keyboard handling (mirrors the prior TV browse
    /// key handling before the merge, task 8.1): movement persists the
    /// resting `BrowseLevel` cursor via `EmbyLibraryCursorIndex` (pagination and
    /// position restore keep working exactly as before), while activation,
    /// effects, refresh/rescan, context menu, search and letter-pill cycling
    /// reuse the same requests Wide already emits.
    fn handle_key_narrow(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, Key::Left | Key::Right | Key::Up | Key::Down)
        {
            return None;
        }
        if let Some(request) = self.narrow_movement_request(key) {
            return Some(Msg::Shell(Box::new(request)));
        }
        let request = match key.code {
            Key::Enter => self.selected_item().map(|item| {
                if self.flat_episode_mode() {
                    ShellRequest::TvEpisodeActivate { episode: item }
                } else {
                    ShellRequest::TvActivate { item }
                }
            }),
            Key::Esc | Key::Backspace => Some(ShellRequest::TvBack),
            Key::Char('.') => self.context_menu_request(),
            _ => return self.shared_library_effect(key, self.selected_item()),
        };
        request.map(|request| Msg::Shell(Box::new(request)))
    }

    fn narrow_movement_request(&mut self, key: &KeyEvent) -> Option<ShellRequest> {
        match key.code {
            Key::Up | Key::Char('k') => {
                let index = self.move_by_item_rows_narrow(-1);
                Some(ShellRequest::EmbyLibraryCursorIndex { index })
            }
            Key::Down | Key::Char('j') => {
                let index = self.move_by_item_rows_narrow(1);
                Some(ShellRequest::EmbyLibraryCursorIndex { index })
            }
            Key::PageUp => {
                let rows = -self.narrow_page_rows();
                let index = self.move_by_item_rows_narrow(rows);
                Some(ShellRequest::EmbyLibraryCursorIndex { index })
            }
            Key::PageDown => {
                let rows = self.narrow_page_rows();
                let index = self.move_by_item_rows_narrow(rows);
                Some(ShellRequest::EmbyLibraryCursorIndex { index })
            }
            Key::Home => {
                let index = self.jump_cursor_narrow(false);
                Some(ShellRequest::EmbyLibraryCursorIndex { index })
            }
            Key::End => {
                let index = self.jump_cursor_narrow(true);
                Some(ShellRequest::EmbyLibraryCursorIndex { index })
            }
            _ => None,
        }
    }
}
