use tuirealm::event::Key;

use super::{Msg, Pane, ShellRequest, TvWorkspaceComponent};

impl TvWorkspaceComponent {
    pub(super) fn handle_key(&mut self, key: &tuirealm::event::KeyEvent) -> Option<Msg> {
        if !self.context.focused {
            return None;
        }
        let request = match key.code {
            Key::Left | Key::Char('h') => {
                self.pane = Pane::Series;
                Some(ShellRequest::TvMoveColumn { delta: -1 })
            }
            Key::Right | Key::Char('l') => {
                self.ensure_episode_cursor();
                self.pane = Pane::Episodes;
                Some(ShellRequest::TvMoveColumn { delta: 1 })
            }
            Key::Enter if self.pane == Pane::Series => {
                self.episode_cursor = Some(0);
                self.pane = Pane::Episodes;
                // Resolve the selected Series from the component's own cursor
                // and carry it in the typed request; if nothing is resolvable
                // (defensive), do not emit the request.
                self.selected_item()
                    .map(|item| ShellRequest::TvActivate { item })
            }
            Key::Enter => Some(ShellRequest::TvEpisodeActivate),
            Key::Esc | Key::Backspace => {
                if self.episode_cursor.is_some() {
                    self.episode_cursor = None;
                    self.pane = Pane::Series;
                }
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
                    && !key
                        .modifiers
                        .contains(tuirealm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) =>
            {
                self.move_season(-1);
                Some(ShellRequest::TvSeasonMove { delta: -1 })
            }
            Key::Char(']')
                if self.pane == Pane::Episodes
                    && !key
                        .modifiers
                        .contains(tuirealm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) =>
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
                let rows = -(self
                    .layout
                    .tv_wide_list_area
                    .height
                    .saturating_sub(1)
                    .max(1) as i64);
                self.move_rows(rows);
                Some(ShellRequest::TvMoveRows { rows })
            }
            Key::PageDown => {
                let rows = self
                    .layout
                    .tv_wide_list_area
                    .height
                    .saturating_sub(1)
                    .max(1) as i64;
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
            Key::Char('p')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                self.selected_item()
                    .map(|item| ShellRequest::EmbyLibraryPlay { item })
            }
            Key::Char('a')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                self.selected_item()
                    .map(|item| ShellRequest::EmbyLibraryEnqueue { item })
            }
            Key::Char('w')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                self.selected_item()
                    .map(|item| ShellRequest::EmbyLibraryToggleWatched { item })
            }
            Key::Char('s')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                self.selected_item()
                    .map(|item| ShellRequest::EmbyLibraryShuffle { item })
            }
            Key::Char('r')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                Some(ShellRequest::EmbyLibraryRescan)
            }
            Key::Char('r')
                if !key.modifiers.intersects(
                    tuirealm::event::KeyModifiers::CONTROL | tuirealm::event::KeyModifiers::ALT,
                ) =>
            {
                Some(ShellRequest::EmbyLibraryRefresh)
            }
            Key::Char('.') => Some(ShellRequest::EmbyLibraryContextMenu {
                item: self.selected_item()?,
            }),
            Key::Char('/') => Some(ShellRequest::OpenInlineSearch),
            Key::Char(c @ ('[' | ']'))
                if !key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) =>
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
