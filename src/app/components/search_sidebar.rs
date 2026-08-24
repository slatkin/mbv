//! Interactive Component for the global Search sidebar overlay (design D3–D9).
//!
//! Owns the `SearchSidebar` state (query, cursor, scroll, type_filter,
//! loading, results) and the 300 ms debounce deadline. The component handles
//! keyboard input locally (query editing, cursor, scroll, type_filter) and
//! emits `Msg` for cross-boundary work:
//! - `Msg::Shell(DismissSearch)` — Esc or Backspace on empty query
//! - `Msg::Shell(SearchActivate { id, item_type })` — Enter on a result
//! - `Msg::Service(SearchQuery(query))` — debounce deadline passed
//!
//! The debounce is component-owned and driven by `UserEvent::Clock` (design
//! D5/D12): the component receives Clock ticks in `on()`, checks the deadline,
//! and emits the search-dispatch `Msg` when it passes. The shell owns the
//! Emby client and spawns the search thread (design D4: cross-boundary work).
//!
//! Search results are delivered by the shell via `apply_drain` (downcast),
//! preserving the existing stale-query guard (`apply_drain` discards results
//! not matching the current query).

use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyModifiers};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{LegacyTerminalEvent, Msg, ServiceRequest, ShellRequest};
use super::user_event::UserEvent;
use crate::app::search_sidebar::SearchSidebar;
use crate::app::ui_util::move_cursor;

const SEARCH_DEBOUNCE_MS: u64 = 300;

/// The Interactive Component for the global Search sidebar.
///
/// Owns the `SearchSidebar` state and the debounce deadline. The shell sets
/// the panel area via downcast before each render; the component renders via
/// the existing `render_search_sidebar` free function (design D9).
pub struct SearchSidebarComponent {
    pub(in crate::app) sidebar: SearchSidebar,
    debounce_deadline: Option<Instant>,
    debounce_pending: Option<String>,
    panel_area: Option<Rect>,
}

impl SearchSidebarComponent {
    pub fn new() -> Self {
        Self {
            sidebar: SearchSidebar::new(),
            debounce_deadline: None,
            debounce_pending: None,
            panel_area: None,
        }
    }

    /// Set the panel area for rendering. Called by the shell via downcast.
    pub(in crate::app) fn set_panel_area(&mut self, area: Option<Rect>) {
        self.panel_area = area;
    }

    /// Apply a search result drain. Called by the shell via downcast after
    /// draining `search_rx`. The stale-query guard is in `SearchSidebar::
    /// apply_drain` (unchanged from the legacy path).
    pub(in crate::app) fn apply_drain(
        &mut self,
        query: &str,
        result: Result<Vec<mbv_core::api::EmbyItem>, String>,
    ) {
        self.sidebar.apply_drain(query, result);
    }

    /// Handle a keyboard event. Local state changes (query, cursor, scroll,
    /// type_filter) return `Msg::Legacy(NoOp)` (the redraw signal, design
    /// D12). Cross-boundary requests return the appropriate `Msg`.
    fn handle_key(&mut self, key: &tuirealm::event::KeyEvent) -> Option<Msg> {
        // Ctrl/Alt: swallow (matching legacy `handle_key_search_sidebar`'s
        // modifier guard).
        if key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return Some(Msg::Legacy(LegacyTerminalEvent::NoOp));
        }
        match key.code {
            Key::Esc => Some(Msg::Shell(ShellRequest::DismissSearch)),
            Key::Enter => self.handle_activate(),
            Key::Up => {
                self.move_cursor(-1);
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            Key::Down => {
                self.move_cursor(1);
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            Key::Tab => {
                self.cycle_type_filter(1);
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            Key::BackTab => {
                self.cycle_type_filter(-1);
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            Key::Backspace => self.handle_backspace(),
            Key::Char(c) => {
                self.sidebar.query.push(c);
                self.sidebar.on_query_changed();
                self.dispatch_query();
                Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
            }
            // Unbound key: swallow (matching legacy `Some(false)` return).
            _ => Some(Msg::Legacy(LegacyTerminalEvent::NoOp)),
        }
    }

    /// Activate the currently selected result. Emits `SearchActivate` with
    /// the selected item's id and type; the shell owns the library tabs and
    /// navigation spawn (design D4).
    fn handle_activate(&mut self) -> Option<Msg> {
        let results = self.sidebar.filtered_results();
        if results.is_empty() {
            return Some(Msg::Legacy(LegacyTerminalEvent::NoOp));
        }
        let idx = self.sidebar.cursor.min(results.len() - 1);
        let item = &results[idx];
        Some(Msg::Shell(ShellRequest::SearchActivate {
            id: item.id.clone(),
            item_type: item.item_type.clone(),
        }))
    }

    /// Backspace: pop the last char or dismiss if empty.
    fn handle_backspace(&mut self) -> Option<Msg> {
        if self.sidebar.query.is_empty() {
            return Some(Msg::Shell(ShellRequest::DismissSearch));
        }
        self.sidebar.query.pop();
        self.sidebar.on_query_changed();
        self.dispatch_query();
        Some(Msg::Legacy(LegacyTerminalEvent::NoOp))
    }

    /// Move the cursor by `delta`, clamping and adjusting scroll (matching
    /// `move_search_sidebar_cursor` exactly).
    fn move_cursor(&mut self, delta: i64) {
        let n = self.sidebar.filtered_count();
        if n == 0 {
            self.sidebar.cursor = 0;
            self.sidebar.scroll = 0;
            return;
        }
        self.sidebar.cursor = move_cursor(self.sidebar.cursor, delta, n);
        let page = self.sidebar.list_height.max(1);
        if self.sidebar.cursor < self.sidebar.scroll {
            self.sidebar.scroll = self.sidebar.cursor;
        } else if self.sidebar.cursor >= self.sidebar.scroll + page {
            self.sidebar.scroll = self.sidebar.cursor + 1 - page;
        }
    }

    /// Cycle the type filter by `delta` (matching
    /// `cycle_search_sidebar_type_filter` exactly).
    fn cycle_type_filter(&mut self, delta: i64) {
        let n = self.sidebar.available_types().len() + 1;
        if n <= 1 {
            return;
        }
        let cur = self.sidebar.type_filter as i64;
        let new = ((cur + delta).rem_euclid(n as i64)) as usize;
        self.sidebar.type_filter = new;
        self.sidebar.cursor = 0;
        self.sidebar.scroll = 0;
    }

    /// Arm the debounce if the query is ≥ 2 characters (matching
    /// `dispatch_search_sidebar_query` exactly). The actual search dispatch
    /// is emitted when `UserEvent::Clock` fires and the deadline has passed.
    fn dispatch_query(&mut self) {
        if self.sidebar.query.len() < 2 {
            return;
        }
        self.debounce_pending = Some(self.sidebar.query.clone());
        self.debounce_deadline = Some(Instant::now() + Duration::from_millis(SEARCH_DEBOUNCE_MS));
    }

    /// Handle a `UserEvent::Clock` tick: if the debounce deadline has passed,
    /// emit `Msg::Service(SearchQuery(query))` and clear the debounce state.
    fn handle_clock(&mut self, now: Instant) -> Option<Msg> {
        let deadline = self.debounce_deadline?;
        if now < deadline {
            return None;
        }
        let Some(query) = self.debounce_pending.take() else {
            self.debounce_deadline = None;
            return None;
        };
        self.debounce_deadline = None;
        Some(Msg::Service(ServiceRequest::SearchQuery(query)))
    }
}

impl Default for SearchSidebarComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SearchSidebarComponent {
    fn view(&mut self, f: &mut Frame, _area: Rect) {
        crate::app::render::render_search_sidebar(f, self.panel_area, &mut self.sidebar);
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for SearchSidebarComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => self.handle_key(key),
            Event::User(UserEvent::Clock(now)) => self
                .handle_clock(*now)
                .or(Some(Msg::Legacy(LegacyTerminalEvent::NoOp))),
            // Non-key/non-clock events: no-op redraw signal (design D12).
            _ => Some(Msg::Legacy(LegacyTerminalEvent::NoOp)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_item;
    use tuirealm::event::{Key, KeyModifiers};

    fn make_key(code: Key, modifiers: KeyModifiers) -> tuirealm::event::KeyEvent {
        tuirealm::event::KeyEvent { code, modifiers }
    }

    #[test]
    fn esc_emits_dismiss_search() {
        let mut comp = SearchSidebarComponent::new();
        let msg = comp.handle_key(&make_key(Key::Esc, KeyModifiers::NONE));
        assert_eq!(msg, Some(Msg::Shell(ShellRequest::DismissSearch)));
    }

    #[test]
    fn ctrl_key_is_swallowed() {
        let mut comp = SearchSidebarComponent::new();
        let msg = comp.handle_key(&make_key(Key::Char('a'), KeyModifiers::CONTROL));
        assert_eq!(msg, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));
    }

    #[test]
    fn alt_key_is_swallowed() {
        let mut comp = SearchSidebarComponent::new();
        let msg = comp.handle_key(&make_key(Key::Char('a'), KeyModifiers::ALT));
        assert_eq!(msg, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));
    }

    #[test]
    fn char_appends_to_query_and_armss_debounce() {
        let mut comp = SearchSidebarComponent::new();
        comp.handle_key(&make_key(Key::Char('a'), KeyModifiers::NONE));
        assert_eq!(comp.sidebar.query, "a");
        assert!(comp.debounce_pending.is_none()); // < 2 chars

        comp.handle_key(&make_key(Key::Char('b'), KeyModifiers::NONE));
        assert_eq!(comp.sidebar.query, "ab");
        assert_eq!(comp.debounce_pending.as_deref(), Some("ab"));
        assert!(comp.debounce_deadline.is_some());
    }

    #[test]
    fn backspace_pops_query() {
        let mut comp = SearchSidebarComponent::new();
        comp.handle_key(&make_key(Key::Char('a'), KeyModifiers::NONE));
        comp.handle_key(&make_key(Key::Char('b'), KeyModifiers::NONE));
        assert_eq!(comp.sidebar.query, "ab");

        comp.handle_key(&make_key(Key::Backspace, KeyModifiers::NONE));
        assert_eq!(comp.sidebar.query, "a");
    }

    #[test]
    fn backspace_on_empty_emits_dismiss() {
        let mut comp = SearchSidebarComponent::new();
        let msg = comp.handle_key(&make_key(Key::Backspace, KeyModifiers::NONE));
        assert_eq!(msg, Some(Msg::Shell(ShellRequest::DismissSearch)));
    }

    #[test]
    fn enter_on_empty_results_is_noop() {
        let mut comp = SearchSidebarComponent::new();
        let msg = comp.handle_key(&make_key(Key::Enter, KeyModifiers::NONE));
        assert_eq!(msg, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));
    }

    #[test]
    fn enter_on_result_emits_search_activate() {
        let mut comp = SearchSidebarComponent::new();
        comp.sidebar.results = vec![make_item("Movie 1", "Movie")];
        comp.sidebar.cursor = 0;
        let msg = comp.handle_key(&make_key(Key::Enter, KeyModifiers::NONE));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ShellRequest::SearchActivate { id, item_type }))
                if id == "id" && item_type == "Movie"
        ));
    }

    #[test]
    fn up_down_move_cursor() {
        let mut comp = SearchSidebarComponent::new();
        comp.sidebar.results = vec![
            make_item("A", "Movie"),
            make_item("B", "Movie"),
            make_item("C", "Movie"),
        ];
        comp.sidebar.list_height = 10;
        comp.handle_key(&make_key(Key::Down, KeyModifiers::NONE));
        assert_eq!(comp.sidebar.cursor, 1);
        comp.handle_key(&make_key(Key::Down, KeyModifiers::NONE));
        assert_eq!(comp.sidebar.cursor, 2);
        comp.handle_key(&make_key(Key::Up, KeyModifiers::NONE));
        assert_eq!(comp.sidebar.cursor, 1);
    }

    #[test]
    fn tab_cycles_type_filter() {
        let mut comp = SearchSidebarComponent::new();
        comp.sidebar.results = vec![make_item("A", "Movie"), make_item("B", "Series")];
        assert_eq!(comp.sidebar.type_filter, 0);
        comp.handle_key(&make_key(Key::Tab, KeyModifiers::NONE));
        assert_eq!(comp.sidebar.type_filter, 1);
        comp.handle_key(&make_key(Key::Tab, KeyModifiers::NONE));
        assert_eq!(comp.sidebar.type_filter, 2);
        comp.handle_key(&make_key(Key::Tab, KeyModifiers::NONE));
        assert_eq!(comp.sidebar.type_filter, 0); // wraps to All
    }

    #[test]
    fn clock_before_deadline_does_not_dispatch() {
        let mut comp = SearchSidebarComponent::new();
        comp.handle_key(&make_key(Key::Char('a'), KeyModifiers::NONE));
        comp.handle_key(&make_key(Key::Char('b'), KeyModifiers::NONE));
        let now = comp.debounce_deadline.unwrap();
        let msg = comp.handle_clock(now - Duration::from_millis(1));
        assert_eq!(msg, None);
        assert!(comp.debounce_pending.is_some());
    }

    #[test]
    fn clock_after_deadline_dispatches_search() {
        let mut comp = SearchSidebarComponent::new();
        comp.handle_key(&make_key(Key::Char('a'), KeyModifiers::NONE));
        comp.handle_key(&make_key(Key::Char('b'), KeyModifiers::NONE));
        let deadline = comp.debounce_deadline.unwrap();
        let msg = comp.handle_clock(deadline + Duration::from_millis(1));
        assert!(matches!(
            msg,
            Some(Msg::Service(ServiceRequest::SearchQuery(q))) if q == "ab"
        ));
        assert!(comp.debounce_pending.is_none());
        assert!(comp.debounce_deadline.is_none());
    }

    #[test]
    fn unbound_key_is_swallowed() {
        let mut comp = SearchSidebarComponent::new();
        let msg = comp.handle_key(&make_key(Key::Function(1), KeyModifiers::NONE));
        assert_eq!(msg, Some(Msg::Legacy(LegacyTerminalEvent::NoOp)));
    }

    #[test]
    fn apply_drain_sets_results() {
        let mut comp = SearchSidebarComponent::new();
        comp.sidebar.query = "test".into();
        comp.apply_drain("test", Ok(vec![make_item("Found", "Movie")]));
        assert_eq!(comp.sidebar.results.len(), 1);
        assert!(!comp.sidebar.loading);
    }

    #[test]
    fn apply_drain_discards_stale_query() {
        let mut comp = SearchSidebarComponent::new();
        comp.sidebar.query = "ab".into();
        comp.sidebar.cursor = 5;
        comp.apply_drain("a", Ok(vec![make_item("Stale", "Movie")]));
        assert_eq!(comp.sidebar.cursor, 5);
        assert!(comp.sidebar.results.is_empty());
    }

    #[test]
    fn no_debounce_armed_below_two_characters() {
        let mut comp = SearchSidebarComponent::new();
        comp.handle_key(&make_key(Key::Char('a'), KeyModifiers::NONE));
        assert!(comp.debounce_pending.is_none());
        assert!(comp.debounce_deadline.is_none());
    }
}
