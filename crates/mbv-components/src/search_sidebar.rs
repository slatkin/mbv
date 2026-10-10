//! Interactive Component for the global Search sidebar overlay (design D3–D9).
//!
//! Owns the `SearchSidebar` query/result state (query, `type_filter`,
//! loading, results) and the 300 ms debounce deadline. The row flow itself
//! (cursor, scroll, selected stable target, viewport clamping) lives in the
//! shared `MediaListCarrier` (`results`), as in Inline Search (design D6).
//! The component handles keyboard input locally (query editing, cursor,
//! `type_filter`) and emits `Msg` for cross-boundary work:
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

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyModifiers, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::inline_search::search_result_row;
use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use crate::media_list::{MediaListCarrier, MediaListOperation};
use mbv_render::components::media_list::MediaListRow;
use mbv_ui_model::search_sidebar::SearchSidebar;
use mbv_ui_msg::UserEvent;
use mbv_ui_msg::{LeafKeyResult, Msg, ServiceRequest, ShellRequest};

const SEARCH_DEBOUNCE_MS: u64 = 300;

/// The Interactive Component for the global Search sidebar.
///
/// Owns the `SearchSidebar` state and the debounce deadline. The shell sets
/// the panel area via downcast before each render; the component renders via
/// the existing `render_search_sidebar` free function (design D9).
#[derive(Debug)]
pub struct SearchSidebarComponent {
    pub sidebar: SearchSidebar,
    /// The one canonical owner of the filtered-result row flow
    /// (wheel-scrolls-viewport 5.4a, design D6): the carrier keeps cursor,
    /// scroll, stable-target selection, viewport clamping, and retained
    /// painted geometry. Its rows are always the current `filtered_results()`
    /// through `search_result_row`.
    results: MediaListCarrier<String>,
    /// 300 ms deadline past which `tick_clock` dispatches `SearchQuery`.
    /// `pub` matches the visibility of the rest of the
    /// shell/component seam so test harnesses can assert clear-on-dispatch.
    pub debounce_deadline: Option<Instant>,
    /// Query string the deadline was armed with. `pub`
    /// for the same reason as `debounce_deadline`.
    pub debounce_pending: Option<String>,
    panel_area: Option<Rect>,
    /// The painted panel-shell rect (last frame) — the outside-click
    /// boundary. Empty while no frame has painted.
    frame: Rect,
    /// Irregular painted chrome (task 5.1, design.md D6): result rows and
    /// type-filter chips, repopulated in `view()` from the geometry the
    /// painter just produced.
    hit_results: HitRegions<usize>,
    hit_chips: HitRegions<usize>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3).
    mouse_gestures: MouseGestureState,
}

impl SearchSidebarComponent {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sidebar: SearchSidebar::new(),
            results: MediaListCarrier::new(),
            debounce_deadline: None,
            debounce_pending: None,
            panel_area: None,
            frame: Rect::default(),
            hit_results: HitRegions::new(),
            hit_chips: HitRegions::new(),
            mouse_gestures: MouseGestureState::new(),
        }
    }

    /// Set the panel area for rendering. Called by the shell via downcast.
    pub fn set_panel_area(&mut self, area: Option<Rect>) {
        self.panel_area = area;
    }

    /// Apply a search result drain. Called by the shell via downcast after
    /// draining `search_rx`; the stale-query guard is in `SearchSidebar::
    /// apply_drain`. An applied drain published the new filtered view with
    /// the selection reset to the first row (design D6).
    pub fn apply_drain(
        &mut self,
        query: &str,
        result: Result<Vec<mbv_emby_model::EmbyItem>, mbv_ui_model::UiModelError>,
    ) {
        if self.sidebar.apply_drain(query, result) {
            self.publish_results();
        }
    }

    /// Handle a keyboard event. Local state changes return `None`; the root
    /// terminal observer supplies the redraw signal (design D12).
    /// Cross-boundary requests return the appropriate `Msg`.
    fn handle_key(&mut self, key: &tuirealm::event::KeyEvent) -> Option<Msg> {
        // Ctrl/Alt: swallow (matching legacy `handle_key_search_sidebar`'s
        // modifier guard).
        if key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return None;
        }
        match key.code {
            Key::Esc => Some(Msg::Shell(Box::new(ShellRequest::DismissSearch))),
            Key::Enter => self.handle_activate(),
            Key::Up => {
                self.results
                    .delegate_operation(MediaListOperation::Move(-1));
                None
            }
            Key::Down => {
                self.results.delegate_operation(MediaListOperation::Move(1));
                None
            }
            Key::Tab => {
                self.cycle_type_filter(1);
                None
            }
            Key::BackTab => {
                self.cycle_type_filter(-1);
                None
            }
            Key::Backspace => self.handle_backspace(),
            Key::Char(c) => {
                self.sidebar.query.push(c);
                self.sidebar.on_query_changed();
                self.publish_results();
                self.dispatch_query();
                None
            }
            // Unbound key: swallow (matching legacy `Some(false)` return).
            _ => None,
        }
    }

    /// Activate the currently selected result: resolve the carrier's
    /// selected stable target in the current filtered view and emit
    /// `SearchActivate` with its id and type; the shell owns the library
    /// tabs and navigation spawn (design D4).
    fn handle_activate(&mut self) -> Option<Msg> {
        let target = self.results.selected_target()?.clone();
        let item = self
            .sidebar
            .filtered_results()
            .into_iter()
            .find(|item| item.id == target)?;
        Some(Msg::Shell(Box::new(ShellRequest::SearchActivate {
            id: item.id.clone(),
            item_type: item.item_type.clone(),
        })))
    }

    /// Backspace: pop the last char or dismiss if empty.
    fn handle_backspace(&mut self) -> Option<Msg> {
        if self.sidebar.query.is_empty() {
            return Some(Msg::Shell(Box::new(ShellRequest::DismissSearch)));
        }
        self.sidebar.query.pop();
        self.sidebar.on_query_changed();
        self.publish_results();
        self.dispatch_query();
        None
    }

    /// Publish the current filtered view onto the shared carrier (design
    /// D6, wheel-scrolls-viewport 5.4a): one row per filtered result, the
    /// selection reset to the first row at the top of the viewport — the
    /// same reset Inline Search performs for a changed query. This is the
    /// only writer of the carrier's rows, so its cursor is always an index
    /// into the current filtered view.
    fn publish_results(&mut self) {
        let rows: Vec<MediaListRow<String>> = self
            .sidebar
            .filtered_results()
            .into_iter()
            .map(search_result_row)
            .collect();
        self.results.set_content(rows);
        self.results.select_first();
        self.results.set_scroll(0);
    }

    /// Cycle the type filter by `delta` (matching
    /// `cycle_search_sidebar_type_filter` exactly). The filter change
    /// publishes a new filtered view with the selection reset (design D6).
    fn cycle_type_filter(&mut self, delta: i64) {
        let n = self.sidebar.available_types().len() + 1;
        if n <= 1 {
            return;
        }
        let cur =
            i64::try_from(self.sidebar.type_filter).expect("type_filter is a small list index");
        let n = i64::try_from(n).expect("type-filter count is a small list length");
        let new = (cur + delta).rem_euclid(n);
        let new = usize::try_from(new).expect("rem_euclid result is non-negative");
        self.sidebar.type_filter = new;
        self.publish_results();
    }

    /// Arm the debounce if the query is ≥ 2 characters (matching
    /// `dispatch_search_sidebar_query` exactly). The actual search dispatch
    /// is emitted when `UserEvent::Clock` fires and the deadline has passed.
    /// If the query drops below two characters we clear any armed debounce
    /// so a stale longer query doesn't dispatch after the user shortened it.
    fn dispatch_query(&mut self) {
        if self.sidebar.query.len() < 2 {
            self.debounce_pending = None;
            self.debounce_deadline = None;
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

    /// Mouse handling (task 5.1): only actions with a keyboard equivalent.
    /// A result-row click selects the row's stable target in the shared
    /// carrier (Up/Down equivalent), a double-click activates (Enter
    /// equivalent), a type-filter chip click sets that filter (Tab/BackTab
    /// cycle equivalent — every chip is reachable by cycling), and an
    /// outside click dismisses (Esc equivalent). The query row has no
    /// cursor-positioning keyboard path, so clicking it is a
    /// no-op. Until 5.4b, a wheel over a painted result row moves the
    /// selection by one result; wheel over any other region is ignored.
    /// Right-click has no keyboard equivalent here and is ignored.
    fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Click { at, .. } => {
                if let Some(&chip) = self.hit_chips.resolve(at) {
                    if chip < self.sidebar.available_types().len() + 1
                        && chip != self.sidebar.type_filter
                    {
                        self.sidebar.type_filter = chip;
                        self.publish_results();
                    }
                    return None;
                }
                if let Some(target) = self.resolve_gesture_row(at) {
                    self.results
                        .delegate_operation(MediaListOperation::Select(target));
                    return None;
                }
                if !self.frame.contains(at) {
                    return Some(Msg::Shell(Box::new(ShellRequest::DismissSearch)));
                }
                None
            }
            MouseGesture::Scroll { at, delta } => {
                // Until 5.4b: one row per notch, through the shared owner.
                if self.hit_results.resolve(at).is_some() {
                    self.results
                        .delegate_operation(MediaListOperation::Move(delta.signum()));
                }
                None
            }
            MouseGesture::DoubleClick(at) => {
                let Some(target) = self.resolve_gesture_row(at) else {
                    // Outside double-click: the first click already dismissed.
                    return None;
                };
                self.results
                    .delegate_operation(MediaListOperation::Select(target));
                self.handle_activate()
            }
            _ => None,
        }
    }

    /// The stable target of the painted row a gesture points at
    /// (`wheel-scrolls-viewport` 5.4a, design D6). The carrier carries one
    /// row per filtered result in `filtered_results()` order, so the painted
    /// row index maps to that result's item id.
    fn resolve_gesture_row(&self, at: Position) -> Option<String> {
        let &index = self.hit_results.resolve(at)?;
        let filtered = self.sidebar.filtered_results();
        let item = filtered.get(index)?;
        Some(item.id.clone())
    }

    /// Sweep the debounce deadline using the shell's wall clock. Called by
    /// the shell once per main-loop iteration next to `drain_search_results`
    /// (#609): production never wired a `UserEvent::Clock` publisher, so the
    /// shell fires the clock directly via this method and routes any
    /// emitted `Msg` through `handle_terminal_message` like the keyboard
    /// path. `pub` matches the component/shell boundary used
    /// by every other shell-side adapter.
    pub fn tick_clock(&mut self, now: Instant) -> Option<Msg> {
        self.handle_clock(now)
    }

    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn test_results(&self) -> &HitRegions<usize> {
        &self.hit_results
    }

    /// Test-only: the carrier's selectable cursor, so tests read the row
    /// flow without depending on target identities (the shared fixtures
    /// reuse one item id), mirroring `inline_search::InlineSearch`.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn test_cursor(&self) -> usize {
        self.results.cursor()
    }

    /// Test-only: the carrier's viewport offset.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn test_scroll(&self) -> usize {
        self.results.scroll()
    }
}

impl Default for SearchSidebarComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SearchSidebarComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let _ = area;
        let selected = self.results.cursor();
        let results = &mut self.results;
        // Painted list height → viewport offset: the component wires this to
        // the shared carrier (`clamp_viewport(height)`, then `scroll()`), so
        // the painter holds no scroll offset of its own (design D6).
        let mut resolve_offset = move |height: usize| -> usize {
            results.clamp_viewport(height);
            results.scroll()
        };
        let geometry = mbv_render::render_search_sidebar(
            frame,
            self.panel_area,
            &self.sidebar,
            selected,
            &mut resolve_offset,
        );
        // Adopt the rects the painter just produced into the irregular-
        // chrome registries (task 5.1, design.md D6).
        self.frame = geometry.frame;
        self.hit_results.clear();
        for (rect, index) in geometry.result_rows {
            self.hit_results.push(rect, index);
        }
        self.hit_chips.clear();
        for (rect, chip) in geometry.chips {
            self.hit_chips.push(rect, chip);
        }
    }

    fn query(&self, _attr: Attribute) -> Option<QueryResult<'_>> {
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
            Event::Keyboard(key) => match self.handle_key(key) {
                Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))).into_option(),
                None if matches!(
                    key.code,
                    Key::Esc
                        | Key::Enter
                        | Key::Up
                        | Key::Down
                        | Key::Tab
                        | Key::BackTab
                        | Key::Backspace
                        | Key::Char(_)
                ) =>
                {
                    LeafKeyResult::Consumed(None).into_option()
                }
                None => LeafKeyResult::Unhandled.into_option(),
            },
            Event::Mouse(mouse) => self.handle_mouse(*mouse),
            #[cfg(any(test, feature = "test"))]
            #[cfg(any(test, feature = "test"))]
            Event::User(UserEvent::Clock(now)) => self.handle_clock(*now),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_emby_model::test_support::make_item;
    use rstest::rstest;
    use tuirealm::event::{Key, KeyModifiers};

    fn make_key(code: Key, modifiers: KeyModifiers) -> tuirealm::event::KeyEvent {
        tuirealm::event::KeyEvent { code, modifiers }
    }

    #[rstest]
    #[case::esc_emits_dismiss_search(
        Key::Esc,
        KeyModifiers::NONE,
        Some(Msg::Shell(Box::new(ShellRequest::DismissSearch)))
    )]
    #[case::unbound_key_is_swallowed(Key::Function(1), KeyModifiers::NONE, None)]
    fn key_handling(
        #[case] key: Key,
        #[case] modifiers: KeyModifiers,
        #[case] expected: Option<Msg>,
    ) {
        let mut comp = SearchSidebarComponent::new();
        let msg = comp.handle_key(&make_key(key, modifiers));
        assert_eq!(msg, expected);
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
    fn enter_on_result_emits_search_activate() {
        let mut comp = SearchSidebarComponent::new();
        comp.sidebar.query = "movie".into();
        comp.apply_drain(
            "movie",
            Ok::<_, mbv_ui_model::UiModelError>(vec![make_item("Movie 1", "Movie")]),
        );
        let msg = comp.handle_key(&make_key(Key::Enter, KeyModifiers::NONE));
        assert!(matches!(
            msg,
            Some(Msg::Shell(ref shell_boxed))
                 if matches!(shell_boxed.as_ref(), ShellRequest::SearchActivate { id, item_type } if id == "id" && item_type == "Movie")));
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
    fn backspace_below_two_chars_clears_armed_debounce() {
        let mut comp = SearchSidebarComponent::new();
        comp.handle_key(&make_key(Key::Char('a'), KeyModifiers::NONE));
        comp.handle_key(&make_key(Key::Char('b'), KeyModifiers::NONE));
        assert_eq!(comp.sidebar.query, "ab");
        assert_eq!(comp.debounce_pending.as_deref(), Some("ab"));
        assert!(comp.debounce_deadline.is_some());

        comp.handle_key(&make_key(Key::Backspace, KeyModifiers::NONE));
        assert_eq!(comp.sidebar.query, "a");
        assert!(comp.debounce_pending.is_none());
        assert!(comp.debounce_deadline.is_none());

        let msg = comp.handle_clock(Instant::now() + Duration::from_secs(1));
        assert_eq!(msg, None);
    }

    #[test]
    fn apply_drain_discards_stale_query() {
        let mut comp = SearchSidebarComponent::new();
        comp.sidebar.query = "ab".into();
        let mut one = make_item("One", "Movie");
        one.id = "id-one".into();
        let mut two = make_item("Two", "Movie");
        two.id = "id-two".into();
        comp.apply_drain("ab", Ok::<_, mbv_ui_model::UiModelError>(vec![one, two]));
        // A non-zero selected row so the stale discard must not reset it.
        comp.handle_key(&make_key(Key::Down, KeyModifiers::NONE));
        comp.handle_key(&make_key(Key::Down, KeyModifiers::NONE));
        assert_eq!(comp.test_cursor(), 1);

        comp.apply_drain("a", Ok(vec![make_item("Stale", "Movie")]));

        assert_eq!(comp.test_cursor(), 1, "stale discard leaves the flow alone");
        let names: Vec<&str> = comp
            .sidebar
            .results
            .iter()
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(names, ["One", "Two"], "a stale discard keeps the results");
    }
}
