use super::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};

const SEARCH_DEBOUNCE_MS: u64 = 300;

impl App {
    /// Handle keys while the global search sidebar is open. Takes precedence
    /// over the view beneath it (returns `Some(false)` for handled keys,
    /// never `Some(true)` -- no key in this context quits the app). Returns
    /// `None` when the sidebar is closed so the input resolver falls through
    /// to the next-lower-priority context.
    #[allow(clippy::question_mark)]
    pub(super) fn handle_key_search_sidebar(&mut self, key: KeyEvent) -> Option<bool> {
        if self.search_sidebar.is_none() {
            return None;
        }
        if key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return Some(false);
        }
        match key.code {
            KeyCode::Esc => {
                self.dismiss_search_sidebar();
            }
            KeyCode::Enter => {
                self.activate_search_result();
            }
            KeyCode::Up => self.move_search_sidebar_cursor(-1),
            KeyCode::Down => self.move_search_sidebar_cursor(1),
            KeyCode::Tab => self.cycle_search_sidebar_type_filter(1),
            KeyCode::BackTab => self.cycle_search_sidebar_type_filter(-1),
            KeyCode::Backspace => self.search_sidebar_backspace(),
            KeyCode::Char(c) => self.search_sidebar_append_char(c),
            _ => {}
        }
        Some(false)
    }

    fn move_search_sidebar_cursor(&mut self, delta: i64) {
        let Some(sidebar) = self.search_sidebar.as_mut() else {
            return;
        };
        let n = sidebar.filtered_count();
        if n == 0 {
            sidebar.cursor = 0;
            sidebar.scroll = 0;
            return;
        }
        let cur = sidebar.cursor as i64;
        let max = n as i64 - 1;
        let new = (cur + delta).clamp(0, max) as usize;
        sidebar.cursor = new;
        let page = sidebar.list_height.max(1);
        if sidebar.cursor < sidebar.scroll {
            sidebar.scroll = sidebar.cursor;
        } else if sidebar.cursor >= sidebar.scroll + page {
            sidebar.scroll = sidebar.cursor + 1 - page;
        }
    }

    fn cycle_search_sidebar_type_filter(&mut self, delta: i64) {
        let Some(sidebar) = self.search_sidebar.as_mut() else {
            return;
        };
        let n = sidebar.available_types().len() + 1;
        if n <= 1 {
            return;
        }
        let cur = sidebar.type_filter as i64;
        let new = ((cur + delta).rem_euclid(n as i64)) as usize;
        sidebar.type_filter = new;
        sidebar.cursor = 0;
        sidebar.scroll = 0;
    }

    fn search_sidebar_backspace(&mut self) {
        let Some(sidebar) = self.search_sidebar.as_mut() else {
            return;
        };
        if sidebar.query.is_empty() {
            self.dismiss_search_sidebar();
            return;
        }
        sidebar.query.pop();
        sidebar.on_query_changed();
        self.dispatch_search_sidebar_query();
    }

    fn search_sidebar_append_char(&mut self, c: char) {
        let Some(sidebar) = self.search_sidebar.as_mut() else {
            return;
        };
        sidebar.query.push(c);
        sidebar.on_query_changed();
        self.dispatch_search_sidebar_query();
    }

    /// Re-issue the server query as the user types. `on_query_changed` only
    /// resets local state -- the sidebar has no local corpus, so without
    /// this it would sit in its post-`on_query_changed` loading state
    /// forever. Queries are only dispatched once the query reaches 2
    /// characters and are debounced by SEARCH_DEBOUNCE_MS; the actual send
    /// is handled by `maybe_flush_search_debounce` in the run loop.
    fn dispatch_search_sidebar_query(&mut self) {
        let Some(sidebar) = self.search_sidebar.as_ref() else {
            return;
        };
        if sidebar.query.len() < 2 {
            return;
        }
        self.search_debounce_pending = Some(sidebar.query.clone());
        self.search_debounce_deadline =
            Some(Instant::now() + Duration::from_millis(SEARCH_DEBOUNCE_MS));
    }
}

impl App {
    /// Activate the currently selected result, if any, and close the
    /// sidebar. With no selected result the sidebar stays open with its
    /// query intact.
    pub(super) fn activate_search_result(&mut self) {
        let selected = {
            let Some(sidebar) = self.search_sidebar.as_ref() else {
                return;
            };
            let results = sidebar.filtered_results();
            if results.is_empty() {
                return;
            }
            let idx = sidebar.cursor.min(results.len() - 1);
            results[idx].clone()
        };
        let item_id = selected.id.clone();
        let item_type = selected.item_type.clone();
        let libs = self.library_tabs_for_nav();
        self.spawn_navigate_to_item(item_id, item_type, libs);
        self.search_sidebar = None;
    }
}

#[cfg(test)]
mod tests {
    use crate::app::search_sidebar::SearchSidebar;
    use crate::app::tests::make_app_stub;

    #[test]
    fn no_debounce_armed_below_two_characters() {
        let mut app = make_app_stub();
        let mut sidebar = SearchSidebar::new();
        sidebar.query = "a".into();
        app.search_sidebar = Some(sidebar);

        app.dispatch_search_sidebar_query();

        assert!(app.search_debounce_pending.is_none());
        assert!(app.search_debounce_deadline.is_none());
    }

    #[test]
    fn debounce_armed_at_two_characters() {
        let mut app = make_app_stub();
        let mut sidebar = SearchSidebar::new();
        sidebar.query = "ab".into();
        app.search_sidebar = Some(sidebar);

        app.dispatch_search_sidebar_query();

        assert_eq!(app.search_debounce_pending.as_deref(), Some("ab"));
        assert!(app.search_debounce_deadline.is_some());
    }
}
