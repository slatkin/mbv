//! Local event interpretation for the Movies/HomeVideos/Generic Emby content owner:
//! keyboard chords and pointer gestures translated into typed `Msg`s.
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::EmbyLibraryContent;
use crate::app::components::inline_search::InlineSearchAction;
use crate::app::components::media_list::{MediaListOperation, MediaListSurfaceInput, RowIntent};
use crate::app::components::msg::{Msg, ShellRequest, TerminalObserverEvent};
use crate::app::render::LetterFilter;
use mbv_core::api::EmbyItem;

impl EmbyLibraryContent {
    /// Ctrl+P/S/A on the selected Inline Search result (mirrors
    /// the former BrowserComponent's inline-search result action): reuses the ordinary
    /// result-row shell effects, resolved against the search cursor rather
    /// than the ordinary browse cursor.
    fn inline_search_result_action(&mut self, key: &KeyEvent) -> Option<Msg> {
        if !key.modifiers.contains(KeyModifiers::CONTROL) {
            return None;
        }
        let item = self.inline_search.selected_item()?;
        let request = match key.code {
            Key::Char('p') => ShellRequest::EmbyLibraryPlay { item },
            Key::Char('s') => ShellRequest::EmbyLibraryShuffle { item },
            Key::Char('a') => ShellRequest::EmbyLibraryEnqueue { item },
            _ => return None,
        };
        self.inline_search.close();
        Some(Msg::Shell(Box::new(request)))
    }

    /// Pointer input against the active Inline Search session (design.md D4):
    /// the panel-normalized `MediaListSurfaceInput` delegates to the session's
    /// embedded carrier like every other list — a click selects, a double-click
    /// activates, a right-click resolves the row's ordinary item-based
    /// context-menu intent, and a wheel over the painted rows is claimed.
    pub(super) fn handle_search_pointer(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        let search = &mut self.inline_search;
        match input {
            MediaListSurfaceInput::Wheel { at, delta } => {
                if !search.results_mut().claims_current_point(at) {
                    return None;
                }
                search.results_mut().delegate_operation(
                    MediaListSurfaceInput::Wheel { at, delta }
                        .into_operation(None)
                        .expect("wheel converts without a target"),
                );
                Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
            }
            MediaListSurfaceInput::Click(at)
            | MediaListSurfaceInput::ToggleClick(at)
            | MediaListSurfaceInput::RangeClick(at) => {
                let target = search.results_mut().resolve_current_point(at)?.clone();
                // Design D4 non-goal: a search session has no multi-selection
                // UI, so a modifier click selects exactly the clicked row and
                // never toggles or extends a range into `multi_selection`.
                search
                    .results_mut()
                    .delegate_operation(MediaListOperation::Select(target));
                None
            }
            MediaListSurfaceInput::DoubleClick(at) => {
                let target = search.results_mut().resolve_current_point(at)?.clone();
                let outcome = search.results_mut().delegate_operation(
                    MediaListSurfaceInput::DoubleClick(at)
                        .into_operation(Some(target))
                        .expect("resolved media-list pointer target"),
                );
                // The delegated transition's resolved intent is the authority
                // for which row the gesture activated.
                match outcome.external_intent {
                    Some(RowIntent::Activate(target)) => {
                        search.item_for_target(&target).map(|item| {
                            Msg::Shell(Box::new(ShellRequest::InlineSearchActivate {
                                id: item.id,
                                item_type: item.item_type,
                            }))
                        })
                    }
                    // A double-click never resolves a context intent, and no
                    // row resolved when the intent is `None`.
                    Some(RowIntent::Context(_) | RowIntent::ContextSelection(_)) | None => None,
                }
            }
            MediaListSurfaceInput::ContextClick(at) => {
                let target = search.results_mut().resolve_current_point(at)?.clone();
                let outcome = search.results_mut().delegate_operation(
                    MediaListSurfaceInput::ContextClick(at)
                        .into_operation(Some(target))
                        .expect("resolved media-list pointer target"),
                );
                match outcome.external_intent {
                    Some(RowIntent::Context(target)) => {
                        Some(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
                            crate::app::state::types::context_menu::ContextMenuTargets::Browser(
                                vec![target],
                            ),
                            None,
                        ))))
                    }
                    // A context click never resolves an activate intent, a
                    // search session has no Visual-mode multi-selection so a
                    // `ContextSelection` cannot arise (D4 non-goal), and no
                    // row resolved when the intent is `None`.
                    Some(RowIntent::Activate(_) | RowIntent::ContextSelection(_)) | None => None,
                }
            }
            _ => None,
        }
    }

    fn handle_active_search_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match self.inline_search.handle_key(key) {
            Some(InlineSearchAction::Activate { id, item_type }) => {
                Some(Msg::Shell(Box::new(ShellRequest::InlineSearchActivate {
                    id,
                    item_type,
                })))
            }
            Some(InlineSearchAction::Dismiss) => {
                self.inline_search.close();
                None
            }
            Some(InlineSearchAction::QueryStarted) => {
                Some(Msg::Shell(Box::new(ShellRequest::InlineSearchQueryStarted)))
            }
            None => self.inline_search_result_action(key),
        }
    }

    fn handle_navigation_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        let operation = match key.code {
            Key::Up | Key::Char('k') => MediaListOperation::Move(-1),
            Key::Down | Key::Char('j') => MediaListOperation::Move(1),
            Key::PageUp => MediaListOperation::Page(-1),
            Key::PageDown => MediaListOperation::Page(1),
            Key::Home => MediaListOperation::First,
            Key::End => MediaListOperation::Last,
            _ => return None,
        };
        self.carrier.delegate_operation(operation);
        Some(Msg::Shell(Box::new(ShellRequest::EmbyLibraryCursorIndex {
            index: self.cursor(),
        })))
    }

    fn handle_selector_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        let Key::Char(c @ ('[' | ']')) = key.code else {
            return None;
        };
        let (current, count) = if self.selector_mode == super::EmbySelectorMode::FeedGroups {
            (
                if self.latest_mode {
                    0
                } else {
                    self.feed_group_cursor + 1
                },
                self.feed_groups.len() + 2,
            )
        } else if self.selector_mode == super::EmbySelectorMode::Letters {
            (
                if self.latest_mode {
                    0
                } else {
                    self.letter_filter
                        .as_ref()
                        .map_or(1, |filter| filter.index + 1)
                },
                LetterFilter::labels().len() + 1,
            )
        } else {
            return None;
        };
        let delta = if c == '[' { -1 } else { 1 };
        let count = i64::try_from(count).unwrap_or(i64::MAX);
        let current = i64::try_from(current).unwrap_or(i64::MAX);
        let next = usize::try_from((current + delta).rem_euclid(count)).unwrap_or(usize::MAX);
        Some(self.pick_selector(next))
    }

    fn selected_item_action(item: EmbyItem, key: &KeyEvent) -> Option<ShellRequest> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            Key::Enter => Some(ShellRequest::EmbyLibraryActivate { item }),
            Key::Char('p') if ctrl => Some(ShellRequest::EmbyLibraryPlay { item }),
            Key::Char('a') if ctrl => Some(ShellRequest::EmbyLibraryEnqueue { item }),
            Key::Char('w') if ctrl => Some(ShellRequest::EmbyLibraryToggleWatched { item }),
            Key::Char('s') if ctrl => Some(ShellRequest::EmbyLibraryShuffle { item }),
            _ => None,
        }
    }

    fn selected_item_request(&mut self, key: &KeyEvent) -> Option<ShellRequest> {
        if let Some(request) = self
            .selected_effect_item()
            .and_then(|item| Self::selected_item_action(item, key))
        {
            return Some(request);
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            Key::Char('.') if key.modifiers.is_empty() => {
                let targets = match self
                    .carrier
                    .delegate_operation(MediaListOperation::ContextCurrent)
                    .external_intent
                {
                    Some(RowIntent::ContextSelection(targets)) => targets,
                    Some(RowIntent::Context(target)) => vec![target],
                    _ => return None,
                };
                Some(ShellRequest::RowContextMenu(
                    crate::app::state::types::context_menu::ContextMenuTargets::Browser(targets),
                    None,
                ))
            }
            Key::Char('r') if ctrl => Some(ShellRequest::EmbyLibraryRescan),
            Key::Char('r') => Some(ShellRequest::EmbyLibraryRefresh),
            Key::Esc | Key::Backspace => Some(ShellRequest::EmbyLibraryBack),
            _ => None,
        }
    }

    /// This owner's local key interpretation, forwarded by the focused panel
    /// (the embedded owner's local key-handling contract, unchanged —
    /// the router owns every global chord and keeps precedence).
    pub(super) fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.inline_search.is_active() {
            return self.handle_active_search_key(key);
        }
        if key.modifiers.is_empty() && matches!(key.code, Key::Char('/')) {
            self.inline_search.open();
            return Some(Msg::Shell(Box::new(ShellRequest::OpenInlineSearch)));
        }
        if self.carrier.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(Box::new(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            ))));
        }
        if key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, Key::Left | Key::Right | Key::Up | Key::Down)
        {
            return None;
        }
        // Local keyboard navigation returns the resolved index so the shell
        // drives persistence/pagination through the ordinary cursor request.
        if let Some(message) = self.handle_navigation_key(key) {
            return Some(message);
        }
        if matches!(key.code, Key::Char('[' | ']'))
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
        {
            return self.handle_selector_key(key);
        }
        self.selected_item_request(key)
            .map(|request| Msg::Shell(Box::new(request)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::library_panel::LibraryKind;

    fn key(code: Key) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn refresh_and_inline_search_keys_emit_their_intents() {
        let mut owner = super::super::EmbyLibraryContent::new(LibraryKind::Movies);
        assert_eq!(
            owner.handle_key(&key(Key::Char('r'))),
            Some(Msg::Shell(Box::new(ShellRequest::EmbyLibraryRefresh)))
        );
        assert_eq!(
            owner.handle_key(&key(Key::Char('/'))),
            Some(Msg::Shell(Box::new(ShellRequest::OpenInlineSearch)))
        );
        assert!(owner.inline_search.is_active());
    }
}
