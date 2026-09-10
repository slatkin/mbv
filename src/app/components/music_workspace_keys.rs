//! Keyboard handling for `MusicWorkspaceComponent`, split out of
//! `music_workspace.rs` to keep that file under the repository's file-size
//! ceiling. Pure code relocation plus the shared-owner delegation seam: every
//! eligible row-local key goes through `delegate_row_local_input` (design.md
//! D3) and effect targets are resolved from the owner's stable selection.

use tuirealm::event::{Key, KeyModifiers};

use super::inline_search::InlineSearchAction;
use super::media_list::RowLocalInput;
use super::msg::{AlbumCursorKind, Msg, ShellRequest};
use super::music_workspace::MusicWorkspaceComponent;

impl MusicWorkspaceComponent {
    fn can_emit_album_cursor(&self) -> bool {
        !self.track_focused
    }

    /// Move the track owner through the common delegation seam (design.md
    /// D3); the owner stays authoritative for the selected track and scroll.
    fn move_track(&mut self, delta: i64) {
        self.track_list.delegate(RowLocalInput::Move(delta), None);
    }

    /// Move the album owner through the common delegation seam and report the
    /// resulting App resting-cursor index. The owner (not a destination
    /// cursor) remains authoritative.
    fn move_album(&mut self, input: RowLocalInput, kind: AlbumCursorKind) -> Option<Msg> {
        self.delegate_row_local_input(input, None);
        self.album_cursor_msg(kind)
    }

    /// Ctrl+P/S/A on the selected Inline Search result reuse the ordinary
    /// library result-row effects, resolved against the search cursor (result-
    /// row shortcut actions stay available while search is open).
    fn inline_search_result_action(&mut self, key: &tuirealm::event::KeyEvent) -> Option<Msg> {
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

    pub(super) fn handle_key(&mut self, key: &tuirealm::event::KeyEvent) -> Option<Msg> {
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
        if !self.context.focused {
            return None;
        }
        match key.code {
            // Activation while the track pane is focused: play the track the
            // track owner selected through the album queue path.
            Key::Enter if self.track_focused => self.music_track_activate_msg(),
            // Ctrl+P keeps its "play current" meaning: with a focused track
            // pane that is the owner-selected track, exactly like Enter.
            Key::Char('p')
                if key.modifiers.contains(KeyModifiers::CONTROL) && self.track_focused =>
            {
                self.music_track_activate_msg()
            }
            // Enter on an album row (Library panel): enter inline track focus
            // when wide with cached tracks; otherwise request the narrow album
            // activation effect from the shell, carrying the owner-resolved
            // album.
            Key::Enter if !self.track_focused => {
                if self.can_enter_track_focus() {
                    self.track_focused = true;
                    self.track_list.select_first();
                    return None;
                }
                self.selected_album_item()
                    .map(|item| Msg::Shell(ShellRequest::MusicAlbumActivate { item }))
            }
            // Exit inline track focus locally; the key must not reach the
            // unprefixed panel's Esc/Stop semantics.
            Key::Esc | Key::Backspace if self.track_focused => {
                self.track_focused = false;
                self.track_list.select_first();
                None
            }
            // Track moves are local to the component while the track pane is
            // focused and the Library panel owns the keys; with the Queue
            // panel focused the keys are left unclaimed for the central
            // router.
            Key::Up | Key::Char('k') if self.track_focused && self.context.focused => {
                self.move_track(-1);
                None
            }
            Key::Down | Key::Char('j') if self.track_focused && self.context.focused => {
                self.move_track(1);
                None
            }
            // Enqueue / context menu target the owner-selected track while the
            // track pane is focused (Library panel); otherwise leave the key
            // unhandled.
            Key::Char('a')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.track_focused
                    && self.context.focused =>
            {
                self.selected_track_item()
                    .map(|track| Msg::Shell(ShellRequest::MusicTrackEnqueue { track }))
            }
            Key::Char('.') if self.track_focused && self.context.focused => self
                .selected_track_item()
                .map(|track| Msg::Shell(ShellRequest::MusicTrackContextMenu { track })),
            // Album-level library actions apply only when the track pane is
            // unfocused; track Ctrl+P/Ctrl+A above retain precedence.
            Key::Char('p')
                if key.modifiers.contains(KeyModifiers::CONTROL) && !self.track_focused =>
            {
                self.selected_item()
                    .map(|item| Msg::Shell(ShellRequest::EmbyLibraryPlay { item }))
            }
            Key::Char('a')
                if key.modifiers.contains(KeyModifiers::CONTROL) && !self.track_focused =>
            {
                self.selected_item()
                    .map(|item| Msg::Shell(ShellRequest::EmbyLibraryEnqueue { item }))
            }
            Key::Char('w')
                if key.modifiers.contains(KeyModifiers::CONTROL) && !self.track_focused =>
            {
                self.selected_item()
                    .map(|item| Msg::Shell(ShellRequest::EmbyLibraryToggleWatched { item }))
            }
            Key::Char('s')
                if key.modifiers.contains(KeyModifiers::CONTROL) && !self.track_focused =>
            {
                self.selected_item()
                    .map(|item| Msg::Shell(ShellRequest::EmbyLibraryShuffle { item }))
            }
            Key::Char('r')
                if key.modifiers.contains(KeyModifiers::CONTROL) && !self.track_focused =>
            {
                Some(Msg::Shell(ShellRequest::EmbyLibraryRescan))
            }
            Key::Char('r')
                if !self.track_focused
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                Some(Msg::Shell(ShellRequest::EmbyLibraryRefresh))
            }
            Key::Char('.') if !self.track_focused => {
                Some(Msg::Shell(ShellRequest::EmbyLibraryContextMenu {
                    item: self.selected_item()?,
                }))
            }
            Key::Char('/') if !self.track_focused => {
                self.inline_search.open();
                Some(Msg::Shell(ShellRequest::OpenInlineSearch))
            }
            // `[`/`]` at the album-list level cycle the App-owned group pill.
            // A focused track pane is a track-level context, so guard on
            // `!track_focused`; the focus gate is the early return.
            Key::Char('[')
                if !self.track_focused
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                Some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta: -1 }))
            }
            Key::Char(']')
                if !self.track_focused
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                Some(Msg::Shell(ShellRequest::MusicGroupSwitch { delta: 1 }))
            }
            Key::Up | Key::Char('k') if self.can_emit_album_cursor() => self.move_album(
                RowLocalInput::Move(-(self.album_columns as i64)),
                AlbumCursorKind::Move,
            ),
            Key::Down | Key::Char('j') if self.can_emit_album_cursor() => self.move_album(
                RowLocalInput::Move(self.album_columns as i64),
                AlbumCursorKind::Move,
            ),
            Key::Home if self.can_emit_album_cursor() => {
                self.move_album(RowLocalInput::First, AlbumCursorKind::Jump)
            }
            Key::End if self.can_emit_album_cursor() => {
                self.move_album(RowLocalInput::Last, AlbumCursorKind::Jump)
            }
            Key::PageUp if self.can_emit_album_cursor() => self.move_album(
                RowLocalInput::Move(-((self.page_rows * self.album_columns) as i64)),
                AlbumCursorKind::Page,
            ),
            Key::PageDown if self.can_emit_album_cursor() => self.move_album(
                RowLocalInput::Move((self.page_rows * self.album_columns) as i64),
                AlbumCursorKind::Page,
            ),
            _ => None,
        }
    }
}
