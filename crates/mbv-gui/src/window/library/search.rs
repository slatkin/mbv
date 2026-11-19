//! The Search field's front-end state (design D12, row 12.1, the port of
//! `lib/library/search.ts`): the query text the field types, whether the field
//! holds the keyboard, and the one result row the keys highlight. The results
//! themselves are the core's — the frame carries them — so this state holds
//! only the facts the keys and the field own.
//!
//! The typing sends the debounced `searchQuery` command: the debounce keeps a
//! keystroke burst from sending one request per key, and it runs over the
//! window's [`Clock`] seam (D10), so a test injects a recording clock and no
//! test sleeps. The armed send's timer rides the state, and the timer the
//! clock passed consumes its own id — one settled query sends one command. The
//! close cancels the pending send with it, and Reset UI State resets this
//! state beside the interaction stores it resets; a pending send the reset
//! leaves armed sends the emptied query the reset held, as the TypeScript
//! reset does.

use std::time::Duration;

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiSearchFrame, GuiSearchRowFrame};
use crate::bridge::library::GuiLibraryCommand;
use crate::window::clock::TimerId;

use super::state::LibraryState;

/// The query typing waits this long before the command sends (row 12.1).
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(200);

/// The Search field's front-end state (`search.ts`'s `LibrarySearchState`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarySearchState {
    /// Whether a mounted Search control paints the field (design D3, step 3):
    /// the top bar's `LibrarySearchControl`, inline or inside its collapsed
    /// panel. The key router arms and routes the field only on this fact, so a
    /// chord never arms a field whose element no window paints.
    pub mounted: bool,
    /// Whether the Search field holds the keyboard (design D3, step 2): the
    /// key router's gate.
    pub focused: bool,
    /// The field's text, as the keys typed it.
    pub query: String,
    /// The highlighted result's row id, or `None` while none is. The id
    /// survives a new results frame that still carries it.
    pub highlight: Option<String>,
}

/// The state a fresh session starts with (`search.ts`'s
/// `initialLibrarySearch`).
#[must_use]
pub fn initial_library_search() -> LibrarySearchState {
    LibrarySearchState {
        mounted: false,
        focused: false,
        query: String::new(),
        highlight: None,
    }
}

impl LibraryState {
    /// The top bar's Search control paints the field (design D3, step 3):
    /// `LibrarySearchControl` in its inline form or inside its collapsed
    /// panel, in every screen but Settings. The mount sets this fact, and
    /// teardown retracts it with the focus fact — an arrangement change that
    /// unmounts the control under an armed field must not leave the keyboard
    /// armed where no element holds it.
    pub fn mount_search_field(&self) {
        let mut search = self.search;
        if search.peek().mounted {
            return;
        }
        search.with_mut(|s| s.mounted = true);
    }

    /// Retract the mount fact with the focus fact. The query text stays: the
    /// core keeps its live query, so a remounted field paints the same search.
    pub fn unmount_search_field(&self) {
        let held = self.search.peek().clone();
        if !held.mounted && !held.focused {
            return;
        }
        let mut search = self.search;
        search.with_mut(|s| {
            s.mounted = false;
            s.focused = false;
        });
    }

    /// Whether a mounted Search control paints the field (design D3, step 3):
    /// the key router's second gate, beside the keyboard hold.
    #[must_use]
    pub fn search_field_mounted(&self) -> bool {
        self.search.peek().mounted
    }

    /// Focus the Search field (design D3, step 3): the `Ctrl+K` chord. The
    /// Search control's input follows this fact into focus, and its focus
    /// event confirms the same fact back.
    pub fn focus_search_field(&self) {
        let mut search = self.search;
        if search.peek().focused {
            return;
        }
        search.with_mut(|s| s.focused = true);
    }

    /// Whether the Search field holds the keyboard (design D3, step 2): the
    /// key router asks this before the panel's other keys route.
    #[must_use]
    pub fn search_field_focused(&self) -> bool {
        self.search.peek().focused
    }

    /// The field lost the keyboard: a press took the focus elsewhere (the
    /// component's blur). The Esc close lands on the same fact.
    pub fn blur_search_field(&self) {
        let mut search = self.search;
        if !search.peek().focused {
            return;
        }
        search.with_mut(|s| s.focused = false);
    }

    /// The field's text changed: the typing keeps the text locally and
    /// schedules the debounced query send. An emptied field clears at once —
    /// there is no query left to wait for. A fresh keystroke replaces the
    /// pending send, never stacks one.
    pub fn set_search_query(&self, query: String) {
        self.cancel_search_send();
        let mut search = self.search;
        if search.peek().query != query {
            search.with_mut(|s| s.query.clone_from(&query));
        }
        if query.is_empty() {
            self.link.library(GuiLibraryCommand::SearchQuery { query });
            return;
        }
        let timer = self.clock.after(SEARCH_DEBOUNCE);
        let mut send = self.search_send;
        send.set(Some(timer));
    }

    /// Close the Search field with Esc (design D3, step 2): the field clears
    /// and gives the keyboard back, and the core closes the results section.
    pub fn close_library_search(&self) {
        self.cancel_search_send();
        let mut search = self.search;
        search.set(initial_library_search());
        self.link.library(GuiLibraryCommand::SearchClose);
    }

    /// Hold the highlighted result's row id, or `None` while none is. An id
    /// the state already holds writes nothing, so the Search control's
    /// keep-the-highlight effect never loops on itself.
    pub fn set_search_highlight(&self, highlight: Option<String>) {
        let mut search = self.search;
        if search.peek().highlight == highlight {
            return;
        }
        search.with_mut(|s| s.highlight = highlight);
    }

    /// Move the highlighted result `step` rows (design D12, row 12.1): the
    /// rows compare by id, never object identity, so a new results frame
    /// keeps the held highlight whenever its id survives. An arrow with no
    /// results or no held id takes the end it walks from.
    pub fn move_search_highlight(&self, step: i32, search: Option<&GuiSearchFrame>) {
        let Some(search) = search else {
            return;
        };
        let results = &search.results;
        if results.is_empty() {
            return;
        }
        let held = self.search.peek().highlight.clone();
        let at = results
            .iter()
            .position(|row| Some(&row.id) == held.as_ref());
        let next = match at {
            None => {
                if step > 0 {
                    0
                } else {
                    results.len() - 1
                }
            }
            Some(at) => {
                let moved = if step > 0 {
                    at.saturating_add(step.unsigned_abs() as usize)
                } else {
                    at.saturating_sub(step.unsigned_abs() as usize)
                };
                moved.min(results.len() - 1)
            }
        };
        let row = &results[next];
        self.set_search_highlight(Some(row.id.clone()));
    }

    /// Activate the found row (design D12, row 12.1): Enter on the highlight,
    /// or the row's press. The command carries the row's stable id and item
    /// type as the frame painted them, and the core opens it. The pending
    /// query send goes with it — an activation answers the query — and the
    /// collapsed control's panel closes with it: the press lands inside the
    /// panel, so no outside-press rule can close it there (user request
    /// 2026-11-19).
    pub fn activate_search_row(&self, row: &GuiSearchRowFrame) {
        self.cancel_search_send();
        self.close_search_panel();
        self.link.library(GuiLibraryCommand::SearchActivate {
            id: row.id.clone(),
            item_type: row.item_type.clone(),
        });
    }

    /// Activate the highlighted result, if the carried results hold one
    /// (design D12, row 12.1). No highlight sends nothing.
    pub fn activate_search_highlight(&self, search: Option<&GuiSearchFrame>) {
        let Some(search) = search else {
            return;
        };
        let held = self.search.peek().highlight.clone();
        if let Some(row) = search
            .results
            .iter()
            .find(|row| Some(&row.id) == held.as_ref())
        {
            self.activate_search_row(row);
        }
    }

    /// Drop the pending query send, if one is (D10): a pending send cancels
    /// before the next one arms.
    fn cancel_search_send(&self) {
        let pending = *self.search_send.peek();
        if let Some(timer) = pending {
            self.clock.cancel(timer);
            let mut send = self.search_send;
            send.set(None);
        }
    }

    /// The query send the debounce armed (D10): the timer the clock passed
    /// consumes its own id, and one settled query sends one command — the
    /// field's text as the state holds it now, not the text the keystroke
    /// scheduled.
    pub fn on_search_timer(&self, timer: TimerId) {
        if *self.search_send.peek() != Some(timer) {
            return;
        }
        let mut send = self.search_send;
        send.set(None);
        let query = self.search.peek().query.clone();
        self.link.library(GuiLibraryCommand::SearchQuery { query });
    }
}

#[cfg(test)]
mod tests;
