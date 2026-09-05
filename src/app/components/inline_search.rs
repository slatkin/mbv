//! Shared embedded Inline Search control (design.md D1/D2).
//!
//! [`InlineSearch`] is a plain, unmounted control: active/inactive state,
//! query, the plain-or-recursive-album candidate pool, scored result order
//! stored as `(original_index, score)` pairs, result cursor/scroll, loading,
//! its last painted result geometry, and its private mouse gesture state.
//! [`InlineSearchHost`] is the minimal contract that will expose one embedded
//! control per destination to shell adapters; it does not choose a
//! destination or hand out Service/runtime objects.
//!
//! [`InlineSearchComponent`] remains the mounted `AppComponent` wrapper that
//! the shell still mounts/focuses/paints directly (`shell_inline_search.rs`)
//! until destination embedding (group 2) and overlay deletion (group 4)
//! land; it now delegates its mechanics to one embedded [`InlineSearch`].

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyModifiers, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::layout::LayoutMain;
use crate::app::ui_util::move_cursor;

#[derive(Clone)]
pub(in crate::app) enum SearchPool {
    Items(Vec<mbv_core::api::EmbyItem>),
    Albums(Vec<crate::app::AlbumSearchEntry>),
}

impl SearchPool {
    fn len(&self) -> usize {
        match self {
            Self::Items(items) => items.len(),
            Self::Albums(entries) => entries.len(),
        }
    }

    /// The item at a corpus index, with an album's indexed display label
    /// substituted for its bare name (design.md D2).
    fn resolved_item_at(&self, index: usize) -> Option<mbv_core::api::EmbyItem> {
        match self {
            Self::Items(items) => items.get(index).cloned(),
            Self::Albums(entries) => entries.get(index).map(|entry| {
                let mut item = entry.album.clone();
                item.name = entry.display_label.clone();
                item
            }),
        }
    }

    /// `(original_index, score)` for every corpus entry that fuzzy-matches
    /// `query` against its match text (display name, or indexed
    /// `search_text` for albums).
    fn match_scores(
        &self,
        matcher: &fuzzy_matcher::skim::SkimMatcherV2,
        query: &str,
    ) -> Vec<(usize, i64)> {
        use fuzzy_matcher::FuzzyMatcher;
        match self {
            Self::Items(items) => items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| {
                    matcher
                        .fuzzy_match(&item.display_name(), query)
                        .map(|score| (i, score))
                })
                .collect(),
            Self::Albums(entries) => entries
                .iter()
                .enumerate()
                .filter_map(|(i, entry)| {
                    matcher
                        .fuzzy_match(&entry.search_text, query)
                        .map(|score| (i, score))
                })
                .collect(),
        }
    }
}

/// Resolved effect of a key the shared control consumed (design.md D4). The
/// host translates this into its own typed shell request; the control never
/// depends on the shell's `Msg` type.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::app) enum InlineSearchAction {
    Activate { id: String, item_type: String },
    Dismiss,
}

/// The shared embedded Inline Search control (design.md D1). Never mounted,
/// focused, subscribed, or given a `ComponentId`; the host that embeds it
/// paints through `crate::app::render::render_inline_search` and gives it
/// first refusal on keyboard/mouse events while active.
pub(in crate::app) struct InlineSearch {
    active: bool,
    query: String,
    pool: SearchPool,
    /// Stable-sorted (ties keep corpus order) descending by score; an empty
    /// query is every corpus index in corpus order (design.md D2).
    order: Vec<(usize, i64)>,
    cursor: usize,
    scroll: usize,
    loading: bool,
    /// Last painted result geometry, published by the shared render
    /// component for column-aware cursor/mouse resolution.
    layout: LayoutMain,
    /// Private per-host gesture recognition (ADR 0024, design.md D1).
    mouse_gestures: MouseGestureState,
}

impl InlineSearch {
    pub(in crate::app) fn new() -> Self {
        Self {
            active: false,
            query: String::new(),
            pool: SearchPool::Items(Vec::new()),
            order: Vec::new(),
            cursor: 0,
            scroll: 0,
            loading: false,
            layout: LayoutMain::default(),
            mouse_gestures: MouseGestureState::new(),
        }
    }

    pub(in crate::app) fn is_active(&self) -> bool {
        self.active
    }

    /// Starts a session locally with an empty query (design.md D4); reopening
    /// after a dismissal always starts empty.
    pub(in crate::app) fn open(&mut self) {
        self.active = true;
        self.query.clear();
        self.pool = SearchPool::Items(Vec::new());
        self.order.clear();
        self.cursor = 0;
        self.scroll = 0;
        self.loading = false;
    }

    /// Dismisses locally, discarding the query and results.
    pub(in crate::app) fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.order.clear();
        self.cursor = 0;
        self.scroll = 0;
    }

    pub(in crate::app) fn query(&self) -> &str {
        &self.query
    }

    pub(in crate::app) fn loading(&self) -> bool {
        self.loading
    }

    pub(in crate::app) fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(in crate::app) fn scroll(&self) -> usize {
        self.scroll
    }

    pub(in crate::app) fn set_scroll(&mut self, scroll: usize) {
        self.scroll = scroll;
    }

    pub(in crate::app) fn results_len(&self) -> usize {
        self.order.len()
    }

    pub(in crate::app) fn layout(&self) -> &LayoutMain {
        &self.layout
    }

    pub(in crate::app) fn layout_mut(&mut self) -> &mut LayoutMain {
        &mut self.layout
    }

    /// Replaces the candidate pool, preserving the selected stable target
    /// (id + item type) when it is still present and otherwise clamping to
    /// the first valid result (design.md D2).
    pub(in crate::app) fn set_pool(&mut self, pool: SearchPool) {
        let target = self.selected_item().map(|item| (item.id, item.item_type));
        self.pool = pool;
        self.recompute_order();
        self.cursor = target
            .and_then(|(id, item_type)| {
                self.order.iter().position(|&(idx, _)| {
                    self.pool
                        .resolved_item_at(idx)
                        .is_some_and(|item| item.id == id && item.item_type == item_type)
                })
            })
            .unwrap_or(0);
    }

    /// The item under the cursor, resolved from the stored order without
    /// materializing the whole result set (design.md D2).
    pub(in crate::app) fn selected_item(&self) -> Option<mbv_core::api::EmbyItem> {
        let &(idx, _) = self.order.get(self.cursor)?;
        self.pool.resolved_item_at(idx)
    }

    /// Materializes the ordered result set for one paint; not used for
    /// cursor movement or selection (design.md D2).
    pub(in crate::app) fn ordered_items(&self) -> Vec<mbv_core::api::EmbyItem> {
        self.order
            .iter()
            .filter_map(|&(idx, _)| self.pool.resolved_item_at(idx))
            .collect()
    }

    fn recompute_order(&mut self) {
        if self.query.is_empty() {
            self.order = (0..self.pool.len()).map(|i| (i, 0)).collect();
            return;
        }
        use fuzzy_matcher::skim::SkimMatcherV2;
        let matcher = SkimMatcherV2::default();
        let mut scored = self.pool.match_scores(&matcher, &self.query);
        scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
        self.order = scored;
    }

    fn move_cursor(&mut self, delta: i64) {
        self.cursor = move_cursor(self.cursor, delta, self.order.len());
    }

    /// Page size for PageUp/PageDown, derived from the last painted result
    /// area (falls back to one row before the first paint).
    fn page_size(&self) -> i64 {
        self.layout.left_area.height.max(1) as i64
    }

    fn push_char(&mut self, c: char) {
        self.query.push(c);
        self.recompute_order();
        self.cursor = 0;
        self.scroll = 0;
    }

    /// Resolves Up/Down/PageUp/PageDown/Home/End/Enter/Escape/Backspace
    /// (design.md D4). An empty-query Backspace dismisses, matching the
    /// standing dismissal contract.
    pub(in crate::app) fn handle_key(
        &mut self,
        key: &tuirealm::event::KeyEvent,
    ) -> Option<InlineSearchAction> {
        if key
            .modifiers
            .intersects(KeyModifiers::ALT | KeyModifiers::CONTROL)
        {
            return None;
        }
        match key.code {
            Key::Up => self.move_cursor(-1),
            Key::Down => self.move_cursor(1),
            Key::PageUp => {
                let step = self.page_size();
                self.move_cursor(-step);
            }
            Key::PageDown => {
                let step = self.page_size();
                self.move_cursor(step);
            }
            Key::Home => self.cursor = 0,
            Key::End => self.cursor = self.order.len().saturating_sub(1),
            Key::Enter => {
                if let Some(item) = self.selected_item() {
                    return Some(InlineSearchAction::Activate {
                        id: item.id,
                        item_type: item.item_type,
                    });
                }
            }
            Key::Esc => return Some(InlineSearchAction::Dismiss),
            Key::Char(c) => self.push_char(c),
            Key::Backspace => {
                if self.query.is_empty() {
                    return Some(InlineSearchAction::Dismiss);
                }
                self.query.pop();
                self.recompute_order();
                self.cursor = 0;
                self.scroll = 0;
            }
            _ => {}
        }
        None
    }

    /// Mouse handling (ADR 0024, design.md D6): a left click on a result row
    /// moves the cursor to that row; every other gesture is a no-op. Resolved
    /// against the last painted result geometry.
    pub(in crate::app) fn handle_mouse(&mut self, mouse: &MouseEvent) {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return;
        }
        let Some(gesture) = self.mouse_gestures.recognize(mouse) else {
            return;
        };
        if let MouseGesture::Click(at) | MouseGesture::DoubleClick(at) = gesture {
            if self.layout.left_area.contains(at) {
                let row = at.y.saturating_sub(self.layout.left_area.y) as usize;
                self.cursor = move_cursor(row, 0, self.order.len());
            }
        }
    }

    #[cfg(test)]
    pub(in crate::app) fn test_pool_item_ids(&self) -> Vec<String> {
        match &self.pool {
            SearchPool::Items(items) => items.iter().map(|item| item.id.clone()).collect(),
            SearchPool::Albums(entries) => {
                entries.iter().map(|entry| entry.album.id.clone()).collect()
            }
        }
    }
}

impl Default for InlineSearch {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimal contract exposing one embedded [`InlineSearch`] to shell adapters
/// (design.md D1). It does not define another application framework, choose
/// a destination, or expose Service/runtime objects; destinations implement
/// it once they embed the control (group 2).
pub(in crate::app) trait InlineSearchHost {
    fn inline_search(&self) -> &InlineSearch;
    fn inline_search_mut(&mut self) -> &mut InlineSearch;
}

/// Mounted `AppComponent` wrapper kept for the shell's overlay protocol
/// (`shell_inline_search.rs`) until destination embedding and overlay
/// deletion land; delegates all mechanics to one embedded [`InlineSearch`].
pub struct InlineSearchComponent {
    control: InlineSearch,
    focused: bool,
}

impl InlineSearchComponent {
    pub fn new() -> Self {
        let mut control = InlineSearch::new();
        control.open();
        Self {
            control,
            focused: false,
        }
    }

    /// No-op: the shared arrangement admits the input purely from available
    /// height (design.md D3), so the control no longer needs a Wide flag.
    /// Retained until the shell's overlay protocol that calls it is deleted
    /// (group 4).
    pub(in crate::app) fn set_wide(&mut self, _wide: bool) {}

    pub(in crate::app) fn set_content(&mut self, pool: SearchPool, loading: bool, focused: bool) {
        self.control.set_pool(pool);
        if loading {
            self.control.set_loading(true);
        }
        self.focused = focused;
    }

    pub(in crate::app) fn set_loading(&mut self, loading: bool) {
        self.control.set_loading(loading);
    }

    pub(in crate::app) fn search_state(&self) -> (&str, bool) {
        (self.control.query(), self.control.loading())
    }

    #[cfg(test)]
    pub(in crate::app) fn test_loading(&self) -> bool {
        self.control.loading()
    }

    #[cfg(test)]
    pub(in crate::app) fn test_pool_item_ids(&self) -> Vec<String> {
        self.control.test_pool_item_ids()
    }

    #[cfg(test)]
    pub(in crate::app) fn test_layout(&self) -> &LayoutMain {
        self.control.layout()
    }

    pub(in crate::app) fn selected_item(&self) -> Option<mbv_core::api::EmbyItem> {
        self.control.selected_item()
    }
}

impl Default for InlineSearchComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl InlineSearchHost for InlineSearchComponent {
    fn inline_search(&self) -> &InlineSearch {
        &self.control
    }

    fn inline_search_mut(&mut self) -> &mut InlineSearch {
        &mut self.control
    }
}

impl Component for InlineSearchComponent {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        *self.control.layout_mut() = LayoutMain::default();
        let items = self.control.ordered_items();
        let query = self.control.query().to_string();
        let loading = self.control.loading();
        let cursor = self.control.cursor();
        let scroll_in = self.control.scroll();
        let columns = crate::app::library_column_width::library_column_count(area.width);
        let scroll = crate::app::render::render_inline_search(
            frame,
            area,
            &query,
            loading,
            items,
            cursor,
            scroll_in,
            self.focused,
            columns,
            self.control.layout_mut(),
        );
        self.control.set_scroll(scroll);
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

impl AppComponent<Msg, UserEvent> for InlineSearchComponent {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Keyboard(key) => match self.control.handle_key(key)? {
                InlineSearchAction::Activate { id, item_type } => {
                    Some(Msg::Shell(ShellRequest::InlineSearchActivate {
                        id,
                        item_type,
                    }))
                }
                InlineSearchAction::Dismiss => Some(Msg::Shell(ShellRequest::InlineSearchDismiss)),
            },
            Event::Mouse(mouse) => {
                self.control.handle_mouse(mouse);
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_item;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::component::Component;
    use tuirealm::event::{Event, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    #[test]
    fn inline_library_search_orders_results_by_descending_score() {
        let mut component = InlineSearchComponent::new();
        component.set_content(
            SearchPool::Items(vec![
                make_item("Xylophone", "Movie"), // idx 0: scattered, low-score match
                make_item("One", "Movie"),       // idx 1: exact match
            ]),
            false,
            true,
        );
        for c in "one".chars() {
            component.on(&Event::Keyboard(KeyEvent {
                code: Key::Char(c),
                modifiers: KeyModifiers::NONE,
            }));
        }
        let ordered_indices = component
            .control
            .order
            .iter()
            .map(|&(idx, _)| idx)
            .collect::<Vec<_>>();
        assert_eq!(
            ordered_indices,
            vec![1, 0],
            "exact match ranks above the scattered match: {ordered_indices:?}"
        );
    }

    #[test]
    fn inline_library_search_query_and_cursor_survive_shell_mirrors() {
        let mut component = InlineSearchComponent::new();
        let items = vec![make_item("One", "Movie"), make_item("Only", "Movie")];
        component.set_content(SearchPool::Items(items.clone()), false, true);
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Char('x'),
            modifiers: KeyModifiers::NONE,
        }));
        component.control.query = "on".into();
        component.control.recompute_order();
        component.set_content(SearchPool::Items(items), false, true);
        component.on(&Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(component.control.query(), "on");
        assert_eq!(component.control.cursor(), 1);
    }

    #[test]
    fn inline_library_search_page_movement_uses_painted_result_height() {
        let mut component = InlineSearchComponent::new();
        let items = (0..10)
            .map(|i| make_item(&format!("Item {i}"), "Movie"))
            .collect::<Vec<_>>();
        component.set_content(SearchPool::Items(items), false, true);
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        let page = component.control.layout().left_area.height as usize;
        assert!(page > 0);

        component.on(&Event::Keyboard(KeyEvent {
            code: Key::PageDown,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(component.control.cursor(), page);

        component.on(&Event::Keyboard(KeyEvent {
            code: Key::PageUp,
            modifiers: KeyModifiers::NONE,
        }));
        assert_eq!(component.control.cursor(), 0);
    }

    #[test]
    fn inline_library_search_renders_plain_candidates_without_app() {
        let mut component = InlineSearchComponent::new();
        component.control.query = "one".into();
        component.set_content(
            SearchPool::Items(vec![make_item("One", "Movie")]),
            false,
            true,
        );
        let mut terminal = Terminal::new(TestBackend::new(40, 5)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        assert!(terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.symbol() == "O"));
    }

    #[test]
    fn inline_library_search_mouse_uses_tuirealm_event_directly() {
        let mut component = InlineSearchComponent::new();
        component.control.query = "on".into();
        component.set_content(
            SearchPool::Items(vec![make_item("One", "Movie"), make_item("Only", "Movie")]),
            false,
            true,
        );
        component.control.cursor = 1;
        let mut terminal = Terminal::new(TestBackend::new(40, 5)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();

        let area = component.test_layout().left_area;
        component.on(&Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        }));

        assert_eq!(component.control.cursor(), 0);
    }

    #[test]
    fn inline_library_search_enter_emits_activation_message() {
        let mut component = InlineSearchComponent::new();
        let item = make_item("One", "Movie");
        component.control.query = "one".into();
        component.set_content(SearchPool::Items(vec![item.clone()]), false, true);

        let message = component.on(&Event::Keyboard(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }));

        assert_eq!(
            message,
            Some(Msg::Shell(ShellRequest::InlineSearchActivate {
                id: item.id,
                item_type: item.item_type,
            }))
        );
    }

    #[test]
    fn inline_library_search_empty_query_backspace_dismisses() {
        let mut component = InlineSearchComponent::new();
        component.set_content(
            SearchPool::Items(vec![make_item("One", "Movie")]),
            false,
            true,
        );

        let message = component.on(&Event::Keyboard(KeyEvent {
            code: Key::Backspace,
            modifiers: KeyModifiers::NONE,
        }));

        assert_eq!(message, Some(Msg::Shell(ShellRequest::InlineSearchDismiss)));
    }

    #[test]
    fn inline_library_search_set_pool_preserves_stable_selection() {
        let mut component = InlineSearchComponent::new();
        let mut selected = make_item("Selected", "Movie");
        selected.id = "target".into();
        component.set_content(
            SearchPool::Items(vec![make_item("Other", "Movie"), selected.clone()]),
            false,
            true,
        );
        component.control.cursor = 1;
        assert_eq!(component.control.selected_item().unwrap().id, "target");

        // Replace the pool with the same target at a different index plus a
        // new sibling; the selection should follow the stable id/type, not
        // the numeric index.
        component.set_content(
            SearchPool::Items(vec![
                make_item("Brand New", "Movie"),
                selected.clone(),
                make_item("Another", "Movie"),
            ]),
            false,
            true,
        );
        assert_eq!(component.control.selected_item().unwrap().id, "target");

        // Replace again with the target gone entirely; clamp to the first
        // valid result instead of an out-of-range cursor.
        component.set_content(
            SearchPool::Items(vec![make_item("Different", "Movie")]),
            false,
            true,
        );
        assert_eq!(component.control.cursor(), 0);
        assert_eq!(component.control.selected_item().unwrap().name, "Different");
    }
}
