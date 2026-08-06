use super::search_modal::SearchMode;
use super::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

const SEARCH_PROMOTION_WINDOW: Duration = Duration::from_millis(400);

impl App {
    /// Handle keys while the unified search modal is open. Takes precedence
    /// over the view beneath it (returns `Some(false)` for handled keys,
    /// never `Some(true)` -- no key in this context quits the app). Returns
    /// `None` when the modal is closed so the input resolver falls through
    /// to the next-lower-priority context.
    #[allow(clippy::question_mark)]
    pub(super) fn handle_key_search_modal(&mut self, key: KeyEvent) -> Option<bool> {
        if self.search_modal.is_none() {
            return None;
        }
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl {
            return Some(false);
        }
        if alt && !matches!(key.code, KeyCode::Left | KeyCode::Right) {
            return Some(false);
        }
        match key.code {
            KeyCode::Esc => {
                self.dismiss_search_modal();
            }
            KeyCode::Enter => {
                self.activate_search_result();
            }
            KeyCode::Up => self.move_search_modal_cursor(-1),
            KeyCode::Down => self.move_search_modal_cursor(1),
            KeyCode::Tab => {
                if self.is_search_modal_global() {
                    self.cycle_search_modal_type_filter(1);
                }
            }
            KeyCode::BackTab => {
                if self.is_search_modal_global() {
                    self.cycle_search_modal_type_filter(-1);
                }
            }
            KeyCode::Left if alt => {
                if self.is_search_modal_global() {
                    self.cycle_search_modal_type_filter(-1);
                }
            }
            KeyCode::Right if alt => {
                if self.is_search_modal_global() {
                    self.cycle_search_modal_type_filter(1);
                }
            }
            KeyCode::Backspace => self.search_modal_backspace(),
            KeyCode::Char('/') => {
                if self.search_modal_promote_or_append_slash() {
                    return Some(false);
                }
            }
            KeyCode::Char(c) => self.search_modal_append_char(c),
            _ => {}
        }
        Some(false)
    }

    /// Dispatch a `/` press depending on current modal state. Returns
    /// `true` if the press was consumed as the modal's promotion or
    /// literal-input path; `false` means the caller should open the modal
    /// (this happens on the first press when the modal is closed).
    pub(super) fn search_modal_promote_or_append_slash(&mut self) -> bool {
        let now = std::time::Instant::now();
        let Some(modal) = self.search_modal.as_mut() else {
            return false;
        };
        let within = self
            .last_slash_at
            .map(|t| now.duration_since(t) < SEARCH_PROMOTION_WINDOW)
            .unwrap_or(true);
        if within && modal.query.is_empty() && matches!(modal.mode, SearchMode::Fuzzy) {
            modal.mode = SearchMode::Global;
            modal.corpus.clear();
            modal.loading = true;
            let q = modal.query.clone();
            let client = self.client.lock().unwrap().clone();
            self.spawn_search_modal_query(client, q);
        } else {
            modal.query.push('/');
            modal.on_query_changed();
        }
        self.last_slash_at = Some(now);
        true
    }

    fn is_search_modal_global(&self) -> bool {
        matches!(
            self.search_modal.as_ref().map(|m| m.mode),
            Some(SearchMode::Global)
        )
    }

    fn move_search_modal_cursor(&mut self, delta: i64) {
        let Some(modal) = self.search_modal.as_mut() else {
            return;
        };
        let n = modal.filtered_count();
        if n == 0 {
            modal.cursor = 0;
            modal.scroll = 0;
            return;
        }
        let cur = modal.cursor as i64;
        let max = n as i64 - 1;
        let new = (cur + delta).clamp(0, max) as usize;
        modal.cursor = new;
        if modal.cursor < modal.scroll {
            modal.scroll = modal.cursor;
        } else if modal.cursor >= modal.scroll + n {
            modal.scroll = modal.cursor + 1 - n;
        }
    }

    fn cycle_search_modal_type_filter(&mut self, delta: i64) {
        let Some(modal) = self.search_modal.as_mut() else {
            return;
        };
        let n = modal.available_types().len() + 1;
        if n <= 1 {
            return;
        }
        let cur = modal.type_filter as i64;
        let new = ((cur + delta).rem_euclid(n as i64)) as usize;
        modal.type_filter = new;
        modal.cursor = 0;
        modal.scroll = 0;
    }

    fn search_modal_backspace(&mut self) {
        let Some(modal) = self.search_modal.as_mut() else {
            return;
        };
        if modal.query.is_empty() {
            self.dismiss_search_modal();
            return;
        }
        modal.query.pop();
        modal.on_query_changed();
    }

    fn search_modal_append_char(&mut self, c: char) {
        let Some(modal) = self.search_modal.as_mut() else {
            return;
        };
        modal.query.push(c);
        modal.on_query_changed();
    }
}

impl App {
    /// Close the search modal, clear the double-slash arming, and restore
    /// the panel focus that was active before the modal opened. The
    /// underlying view's navigation position is untouched by construction --
    /// the modal never mutates `libs[].nav_stack` or `library_tab`.
    pub(super) fn dismiss_search_modal(&mut self) {
        self.search_modal = None;
        self.last_slash_at = None;
        if let Some(focus) = self.search_modal_prior_focus.take() {
            self.panel_focus = focus;
        }
    }

    /// Activate the currently selected result, if any, and close the
    /// modal. With no selected result the modal stays open with its
    /// query intact.
    pub(super) fn activate_search_result(&mut self) {
        let selected = {
            let Some(modal) = self.search_modal.as_ref() else {
                return;
            };
            let results = modal.filtered_results();
            if results.is_empty() {
                return;
            }
            let idx = modal.cursor.min(results.len() - 1);
            results[idx].clone()
        };
        let item_id = selected.id.clone();
        let item_type = selected.item_type.clone();
        let libs = self.library_tabs_for_nav();
        self.spawn_navigate_to_item(item_id, item_type, libs);
        self.search_modal = None;
        self.last_slash_at = None;
    }
}
