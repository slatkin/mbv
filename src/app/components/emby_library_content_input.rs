//! Local event interpretation for the Movies/HomeVideos/Generic Emby content owner:
//! keyboard chords and pointer gestures translated into typed `Msg`s.
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::EmbyLibraryContent;
use crate::app::components::inline_search::InlineSearchAction;
use crate::app::components::media_list::{MediaListOperation, MediaListSurfaceInput, RowIntent};
use crate::app::components::msg::{Msg, ShellRequest, TerminalObserverEvent};
use crate::app::render::LetterFilter;

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
        Some(Msg::Shell(request))
    }

    /// Pointer input against the active Inline Search session (design.md D4):
    /// the panel-normalized `MediaListSurfaceInput` delegates to the session's
    /// embedded carrier like every other list — a click selects, a double-click
    /// activates, a right-click resolves the row's ordinary item-based
    /// context-menu intent, and a wheel over the painted rows is claimed.
    fn handle_search_pointer(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
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
                            Msg::Shell(ShellRequest::InlineSearchActivate {
                                id: item.id,
                                item_type: item.item_type,
                            })
                        })
                    }
                    // A double-click never resolves a context intent, and no
                    // row resolved when the intent is `None`.
                    Some(RowIntent::Context(_)) | Some(RowIntent::ContextSelection(_)) | None => {
                        None
                    }
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
                        Some(Msg::Shell(ShellRequest::RowContextMenu(
                            crate::app::state::types::context_menu::ContextMenuTargets::Browser(
                                vec![target],
                            ),
                            None,
                        )))
                    }
                    // A context click never resolves an activate intent, a
                    // search session has no Visual-mode multi-selection so a
                    // `ContextSelection` cannot arise (D4 non-goal), and no
                    // row resolved when the intent is `None`.
                    Some(RowIntent::Activate(_)) | Some(RowIntent::ContextSelection(_)) | None => {
                        None
                    }
                }
            }
            _ => None,
        }
    }

    /// This owner's local key interpretation, forwarded by the focused panel
    /// (the embedded owner's local key-handling contract, unchanged —
    /// the router owns every global chord and keeps precedence).
    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.inline_search.is_active() {
            return match self.inline_search.handle_key(key) {
                Some(InlineSearchAction::Activate { id, item_type }) => {
                    Some(Msg::Shell(ShellRequest::InlineSearchActivate {
                        id,
                        item_type,
                    }))
                }
                Some(InlineSearchAction::Dismiss) => {
                    self.inline_search.close();
                    None
                }
                Some(InlineSearchAction::QueryStarted) => {
                    Some(Msg::Shell(ShellRequest::InlineSearchQueryStarted))
                }
                None => self.inline_search_result_action(key),
            };
        }
        if key.modifiers.is_empty() && matches!(key.code, Key::Char('/')) {
            self.inline_search.open();
            return Some(Msg::Shell(ShellRequest::OpenInlineSearch));
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if self.carrier.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            )));
        }
        if alt && matches!(key.code, Key::Left | Key::Right | Key::Up | Key::Down) {
            return None;
        }
        // Local keyboard navigation routes through the same typed
        // `ShellRequest` the embedded owner emits
        // (`browser/keyboard.rs`): this owner mutates only its own selection,
        // then returns the resolved index in place of the raw key so the
        // shell drives persistence/pagination through the same arm as TV's
        // still-mounted browser — never by recomputing the App cursor here
        // (design D3: local movement sends the resolved value).
        match key.code {
            Key::Up | Key::Char('k') => {
                self.carrier
                    .delegate_operation(MediaListOperation::Move(-1));
                return Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.cursor(),
                }));
            }
            Key::Down | Key::Char('j') => {
                self.carrier.delegate_operation(MediaListOperation::Move(1));
                return Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.cursor(),
                }));
            }
            Key::PageUp => {
                self.carrier
                    .delegate_operation(MediaListOperation::Page(-1));
                return Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.cursor(),
                }));
            }
            Key::PageDown => {
                self.carrier.delegate_operation(MediaListOperation::Page(1));
                return Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.cursor(),
                }));
            }
            Key::Home => {
                self.carrier.delegate_operation(MediaListOperation::First);
                return Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.cursor(),
                }));
            }
            Key::End => {
                self.carrier.delegate_operation(MediaListOperation::Last);
                return Some(Msg::Shell(ShellRequest::EmbyLibraryCursorIndex {
                    index: self.cursor(),
                }));
            }
            _ => {}
        }
        let selected = self.selected_effect_item();
        let request = match key.code {
            Key::Enter => selected.map(|item| ShellRequest::EmbyLibraryActivate { item }),
            Key::Char('p') if ctrl => selected.map(|item| ShellRequest::EmbyLibraryPlay { item }),
            Key::Char('a') if ctrl => {
                selected.map(|item| ShellRequest::EmbyLibraryEnqueue { item })
            }
            Key::Char('w') if ctrl => {
                selected.map(|item| ShellRequest::EmbyLibraryToggleWatched { item })
            }
            Key::Char('.') if key.modifiers.is_empty() => match self
                .carrier
                .delegate_operation(MediaListOperation::ContextCurrent)
                .external_intent
            {
                Some(RowIntent::ContextSelection(targets)) => Some(ShellRequest::RowContextMenu(
                    crate::app::state::types::context_menu::ContextMenuTargets::Browser(targets),
                    None,
                )),
                Some(RowIntent::Context(target)) => Some(ShellRequest::RowContextMenu(
                    crate::app::state::types::context_menu::ContextMenuTargets::Browser(vec![
                        target,
                    ]),
                    None,
                )),
                _ => None,
            },
            Key::Char('s') if ctrl => {
                selected.map(|item| ShellRequest::EmbyLibraryShuffle { item })
            }
            Key::Char('r') if ctrl => Some(ShellRequest::EmbyLibraryRescan),
            Key::Char('r') => Some(ShellRequest::EmbyLibraryRefresh),
            Key::Esc | Key::Backspace => Some(ShellRequest::EmbyLibraryBack),
            Key::Char(c @ ('[' | ']')) if !ctrl && !alt => {
                let (current, count) = if self.group_pills {
                    (
                        if self.latest_mode {
                            0
                        } else {
                            self.feed_group_cursor + 1
                        },
                        self.feed_groups.len() + 2,
                    )
                } else if self.show_letter_pills {
                    (
                        if self.latest_mode {
                            0
                        } else {
                            self.letter_filter
                                .as_ref()
                                .map(|filter| filter.index + 1)
                                .unwrap_or(1)
                        },
                        LetterFilter::labels().len() + 1,
                    )
                } else {
                    return None;
                };
                let delta = if c == '[' { -1 } else { 1 };
                let next = (current as i64 + delta).rem_euclid(count as i64) as usize;
                return self.pick_selector(next);
            }
            _ => None,
        };
        request.map(Msg::Shell)
    }
}
