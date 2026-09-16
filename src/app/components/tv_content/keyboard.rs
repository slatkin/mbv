use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::super::inline_search::InlineSearchAction;
use super::{Msg, Pane, ShellRequest, TvContent};
use crate::app::components::media_list::MediaListSurfaceInput;

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
        Some(Msg::Shell(request))
    }

    pub(super) fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        // Inline Search gets first refusal while active (design.md D4): the
        // component returns immediately after delegating, even when search
        // consumes the key without producing a message.
        if self.inline_search.is_active() {
            return match self.inline_search.handle_key(key) {
                Some(InlineSearchAction::Activate { id, item_type }) => {
                    Some(Msg::Shell(ShellRequest::InlineSearchActivate {
                        id,
                        item_type,
                    }))
                }
                Some(InlineSearchAction::Dismiss) => {
                    // Escape/empty-query Backspace dismiss locally
                    // (design.md D4); no shell effect.
                    self.inline_search.close();
                    None
                }
                // Ctrl+P/S/A that the shared control does not consume act on
                // the selected result row via the ordinary result-row effects.
                None => self.inline_search_result_action(key),
            };
        }
        if self.carrier.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            )));
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
        if !self.is_wide && self.hero_overlay_open && self.pane == Pane::Episodes {
            return self.handle_key_overlay_workspace(key);
        }
        if self.is_wide {
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
        request.map(Msg::Shell)
    }

    /// Wide pane-based keyboard handling (unchanged from before the merge).
    fn handle_key_wide(&mut self, key: &KeyEvent) -> Option<Msg> {
        let request = match key.code {
            Key::Enter if self.pane == Pane::Series => {
                self.episodes.select_first();
                self.pane = Pane::Episodes;
                // Resolve the selected Series from the component's own cursor
                // and carry it in the typed request; if nothing is resolvable
                // (defensive), do not emit the request.
                self.selected_item()
                    .map(|item| ShellRequest::TvActivate { item })
            }
            Key::Enter => self
                .selected_episode_item()
                .map(|episode| ShellRequest::TvEpisodeActivate { episode }),
            Key::Esc | Key::Backspace => {
                self.pane = Pane::Series;
                Some(ShellRequest::TvBack)
            }
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
            Key::Up | Key::Char('k') => {
                self.move_rows(-1);
                Some(ShellRequest::TvMoveRows { rows: -1 })
            }
            Key::Down | Key::Char('j') => {
                self.move_rows(1);
                Some(ShellRequest::TvMoveRows { rows: 1 })
            }
            Key::PageUp => {
                let rows = -(self.painted_viewport_height().saturating_sub(1).max(1) as i64);
                self.move_rows(rows);
                Some(ShellRequest::TvMoveRows { rows })
            }
            Key::PageDown => {
                let rows = self.painted_viewport_height().saturating_sub(1).max(1) as i64;
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
            // Library effects use the component's selected item. TV keeps
            // the series-list selection authoritative even while the local
            // Episodes pane is focused, matching the legacy stack target.
            Key::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => self
                .selected_item()
                .map(|item| ShellRequest::EmbyLibraryPlay { item }),
            Key::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => self
                .selected_item()
                .map(|item| ShellRequest::EmbyLibraryEnqueue { item }),
            Key::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => self
                .selected_item()
                .map(|item| ShellRequest::EmbyLibraryToggleWatched { item }),
            Key::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => self
                .selected_item()
                .map(|item| ShellRequest::EmbyLibraryShuffle { item }),
            Key::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(ShellRequest::EmbyLibraryRescan)
            }
            Key::Char('r')
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                Some(ShellRequest::EmbyLibraryRefresh)
            }
            Key::Char('.') => self.context_menu_request(),
            Key::Char('/') => {
                self.inline_search.open();
                Some(ShellRequest::OpenInlineSearch)
            }
            Key::Char(c @ ('[' | ']'))
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                Some(ShellRequest::TvCycleLetterPill {
                    delta: if c == '[' { -1 } else { 1 },
                })
            }
            _ => None,
        };
        request.map(Msg::Shell)
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
        let request = match key.code {
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
            Key::Enter => self
                .selected_item()
                .map(|item| ShellRequest::TvActivate { item }),
            Key::Esc | Key::Backspace => Some(ShellRequest::TvBack),
            Key::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => self
                .selected_item()
                .map(|item| ShellRequest::EmbyLibraryPlay { item }),
            Key::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => self
                .selected_item()
                .map(|item| ShellRequest::EmbyLibraryEnqueue { item }),
            Key::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => self
                .selected_item()
                .map(|item| ShellRequest::EmbyLibraryToggleWatched { item }),
            Key::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => self
                .selected_item()
                .map(|item| ShellRequest::EmbyLibraryShuffle { item }),
            Key::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(ShellRequest::EmbyLibraryRescan)
            }
            Key::Char('r')
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                Some(ShellRequest::EmbyLibraryRefresh)
            }
            Key::Char('.') => self.context_menu_request(),
            Key::Char('/') => {
                self.inline_search.open();
                Some(ShellRequest::OpenInlineSearch)
            }
            Key::Char(c @ ('[' | ']'))
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                Some(ShellRequest::TvCycleLetterPill {
                    delta: if c == '[' { -1 } else { 1 },
                })
            }
            _ => None,
        };
        request.map(Msg::Shell)
    }
}
