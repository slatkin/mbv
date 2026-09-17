//! Shared embedded Inline Search control (design.md D1/D2).
//!
//! [`InlineSearch`] is a plain, unmounted control: active/inactive state,
//! query, the plain-or-recursive-album candidate pool, scored result order
//! stored as `(original_index, score)` pairs, loading, and the debounce
//! deadline. The row flow itself (cursor, scroll, selected stable target,
//! viewport clamping, retained row geometry) lives in the embedded
//! [`MediaListCarrier`] and is painted by the Library panel through the
//! object-safe [`PanelList`] surface, exactly like every other list.
//! [`InlineSearchHost`] is the minimal contract that will expose one embedded
//! control per destination to shell adapters; it does not choose a
//! destination or hand out Service/runtime objects.
//!
use std::time::{Duration, Instant};

use tuirealm::event::{Key, KeyModifiers};

use super::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaListSurfaceInput, MediaListTrailing,
    MediaSemanticState,
};

/// Quiet period after a query edit before the scored results re-fire (the
/// shell supplies wall-clock ticks; see [`InlineSearch::handle_clock`]).
const SEARCH_DEBOUNCE_MS: u64 = 300;

#[derive(Clone)]
pub(in crate::app) enum SearchPool {
    Items(Vec<mbv_core::api::EmbyItem>),
    Albums(Vec<crate::app::AlbumSearchEntry>),
}

impl SearchPool {
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
    Activate {
        id: String,
        item_type: String,
    },
    Dismiss,
    /// The first query character landed in an open session: the host asks
    /// the shell to start the corpus load (whole-library fetch or recursive
    /// album index).
    QueryStarted,
}

/// The composed row label the search previously rendered (design.md D2:
/// content parity with the legacy plain-rows path). Folders carry their
/// item-count / unplayed suffixes; everything else the display label.
fn search_row_label(item: &mbv_core::api::EmbyItem) -> String {
    if item.is_folder {
        if item.item_type == "Folder" && item.total_count > 0 {
            format!("{} \u{b7} {} items", item.display_name(), item.total_count)
        } else if item.unplayed_item_count > 0 && item.item_type != "Series" {
            format!("{} [{}]", item.display_name(), item.unplayed_item_count)
        } else {
            item.display_name()
        }
    } else {
        item.display_name()
    }
}

/// One canonical row for a scored result (design.md D2): stable item-id
/// target, legacy label parity, a trailing year on playable leaves, no
/// secondary/duration, and the `Ordinary` semantic state the legacy rows
/// never dimmed past.
fn search_result_row(item: &mbv_core::api::EmbyItem) -> MediaListRow<String> {
    MediaListRow::Item {
        target: item.id.clone(),
        primary: search_row_label(item),
        secondary: None,
        trailing: (!item.is_folder && item.production_year > 0)
            .then(|| MediaListTrailing::Year(item.production_year.to_string())),
        duration: None,
        kind: if item.is_folder {
            MediaKind::Collection
        } else {
            MediaKind::Media
        },
        semantic_state: MediaSemanticState::Ordinary,
    }
}

/// The shared embedded Inline Search control (design.md D1). Never mounted,
/// focused, subscribed, or given a `ComponentId`; the host that embeds it
/// gives it first refusal on keyboard events while active, and the Library
/// panel paints its session through the [`super::library_panel::content::PanelList`]
/// surface over the embedded carrier.
pub(in crate::app) struct InlineSearch {
    active: bool,
    query: String,
    pool: SearchPool,
    /// Stable-sorted (ties keep corpus order) descending by score. An empty
    /// query carries no order at all: results appear only once a query is
    /// typed, after the debounce fires.
    order: Vec<(usize, i64)>,
    loading: bool,
    /// Debounce deadline armed by the last query edit; the pending re-score
    /// fires when a shell clock tick passes it. `None` when the current
    /// query is already scored.
    deadline: Option<Instant>,
    /// The one canonical owner of the result row flow (design.md D1): the
    /// carrier keeps cursor, scroll, stable-target selection, viewport
    /// clamping, and retained painted geometry.
    results: MediaListCarrier<String>,
}

impl InlineSearch {
    pub(in crate::app) fn new() -> Self {
        Self {
            active: false,
            query: String::new(),
            pool: SearchPool::Items(Vec::new()),
            order: Vec::new(),
            loading: false,
            deadline: None,
            results: MediaListCarrier::new(),
        }
    }

    pub(in crate::app) fn is_active(&self) -> bool {
        self.active
    }

    /// Starts a session locally with an empty query; reopening after a
    /// dismissal always starts empty. Nothing is searched or loaded until
    /// the first keystroke.
    pub(in crate::app) fn open(&mut self) {
        self.active = true;
        self.query.clear();
        self.pool = SearchPool::Items(Vec::new());
        self.order.clear();
        self.loading = false;
        self.deadline = None;
        self.results.set_content(Vec::new());
    }

    /// Dismisses locally, discarding the query and results.
    pub(in crate::app) fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.order.clear();
        self.results.set_content(Vec::new());
    }

    pub(in crate::app) fn query(&self) -> &str {
        &self.query
    }

    pub(in crate::app) fn restore_query(&mut self, query: String) {
        self.query = query;
        self.deadline = None;
        self.recompute_order();
        self.publish_rows(true);
    }

    pub(in crate::app) fn loading(&self) -> bool {
        self.loading
    }

    pub(in crate::app) fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    /// The embedded canonical row-flow carrier: the panel drives it through
    /// the [`super::library_panel::content::PanelList`] surface and the owners
    /// resolve pointer inputs against it (design.md D1/D4).
    pub(in crate::app) fn results(&self) -> &MediaListCarrier<String> {
        &self.results
    }

    pub(in crate::app) fn results_mut(&mut self) -> &mut MediaListCarrier<String> {
        &mut self.results
    }

    pub(in crate::app) fn selected_target(&self) -> Option<(String, String)> {
        self.selected_item().map(|item| (item.id, item.item_type))
    }

    /// Restores a session's selected result and viewport: the stable target
    /// moves the carrier's selection when present; the row offset parks the
    /// viewport (design.md D5).
    pub(in crate::app) fn restore_target(&mut self, id: Option<String>, row_offset: usize) {
        if let Some(id) = id {
            self.results.select_target(&id);
            self.results.set_scroll(row_offset);
        }
    }

    pub(in crate::app) fn results_len(&self) -> usize {
        self.results.rows().len()
    }

    /// Replaces the candidate pool and re-scores the current query. The
    /// carrier's ordinary refresh rule preserves the selected stable target
    /// when it is still present and clamps otherwise (design.md D2: a pool
    /// refresh never resets the selection).
    pub(in crate::app) fn set_pool(&mut self, pool: SearchPool) {
        self.pool = pool;
        self.deadline = None;
        self.recompute_order();
        self.publish_rows(false);
    }

    /// The item under the carrier's selection, resolved from the stored order
    /// (design.md D2).
    pub(in crate::app) fn selected_item(&self) -> Option<mbv_core::api::EmbyItem> {
        let target = self.results.selected_target()?;
        self.order
            .iter()
            .filter_map(|&(idx, _)| self.pool.resolved_item_at(idx))
            .find(|item| &item.id == target)
    }

    /// Materializes the scored order into canonical rows and hands them to
    /// the carrier (design.md D2). `reset` selects the first result — a
    /// re-score for a changed query; a pool refresh with an unchanged query
    /// passes `false` and relies on the carrier's stable-target preservation.
    fn publish_rows(&mut self, reset: bool) {
        let rows: Vec<MediaListRow<String>> = self
            .order
            .iter()
            .filter_map(|&(idx, _)| self.pool.resolved_item_at(idx))
            .map(|item| search_result_row(&item))
            .collect();
        self.results.set_content(rows);
        if reset {
            self.results.select_first();
        }
    }

    fn recompute_order(&mut self) {
        if self.query.is_empty() {
            // An empty query shows nothing: search starts with the first
            // typed character, never with the whole corpus.
            self.order.clear();
            return;
        }
        use fuzzy_matcher::skim::SkimMatcherV2;
        // Fully case-insensitive: media titles are searched without smart-case
        // (an uppercase query letter must not make the scan case-sensitive).
        let matcher = SkimMatcherV2::default().ignore_case();
        let mut scored = self.pool.match_scores(&matcher, &self.query);
        scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
        self.order = scored;
    }

    /// Arms the debounce for the current query: the scored results re-fire
    /// once a shell clock tick passes the deadline.
    fn arm_search(&mut self) {
        self.deadline = Some(Instant::now() + Duration::from_millis(SEARCH_DEBOUNCE_MS));
    }

    /// Fires the armed re-score once the debounce deadline has passed (the
    /// shell supplies wall-clock ticks; #609). A fired re-score is a changed
    /// query's: the selection resets to the first result (design.md D2).
    /// Returns whether the debounce fired.
    pub(in crate::app) fn handle_clock(&mut self, now: Instant) -> bool {
        match self.deadline {
            Some(deadline) if now >= deadline => {}
            _ => return false,
        }
        self.deadline = None;
        self.recompute_order();
        self.publish_rows(true);
        true
    }

    fn push_char(&mut self, c: char) {
        self.query.push(c);
        self.arm_search();
    }

    /// Offer one normalized movement input to the embedded carrier. Only the
    /// target-free movement inputs reach this: the row-local pointer inputs
    /// are translated by the owning destination against resolved targets
    /// (design.md D4).
    fn delegate_movement(&mut self, input: MediaListSurfaceInput) {
        self.results.delegate_operation(
            input
                .into_operation(None)
                .expect("target-free movement inputs always convert to media-list operations"),
        );
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
            Key::Up => self.delegate_movement(MediaListSurfaceInput::Move(-1)),
            Key::Down => self.delegate_movement(MediaListSurfaceInput::Move(1)),
            Key::PageUp => self.delegate_movement(MediaListSurfaceInput::Page(-1)),
            Key::PageDown => self.delegate_movement(MediaListSurfaceInput::Page(1)),
            Key::Home => self.delegate_movement(MediaListSurfaceInput::First),
            Key::End => self.delegate_movement(MediaListSurfaceInput::Last),
            Key::Enter => {
                if let Some(item) = self.selected_item() {
                    return Some(InlineSearchAction::Activate {
                        id: item.id,
                        item_type: item.item_type,
                    });
                }
            }
            Key::Esc => return Some(InlineSearchAction::Dismiss),
            Key::Char(c) => {
                let started = self.query.is_empty();
                self.push_char(c);
                // The first keystroke starts the search: the host tells the
                // shell to begin the corpus load (deferred from open).
                return started.then_some(InlineSearchAction::QueryStarted);
            }
            Key::Backspace => {
                if self.query.is_empty() {
                    return Some(InlineSearchAction::Dismiss);
                }
                self.query.pop();
                if self.query.is_empty() {
                    // Back to an empty query: no results, no pending score.
                    self.deadline = None;
                    self.order.clear();
                    self.results.set_content(Vec::new());
                } else {
                    self.arm_search();
                }
            }
            _ => {}
        }
        None
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

    /// Test-only: the carrier's selectable cursor, so tests can assert
    /// movement without depending on target identities (the shared fixtures
    /// reuse one item id).
    #[cfg(test)]
    pub(in crate::app) fn test_cursor(&self) -> usize {
        self.results.cursor()
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
    fn selected_inline_search_item(&self) -> Option<mbv_core::api::EmbyItem> {
        self.inline_search().selected_item()
    }
    fn restore_inline_search_query(&mut self, query: String) {
        self.inline_search_mut().restore_query(query);
    }

    fn open_inline_search(&mut self) {
        self.inline_search_mut().open();
    }
    fn close_inline_search(&mut self) {
        self.inline_search_mut().close();
    }
    fn set_inline_search_content(&mut self, pool: SearchPool, loading: bool, focused: bool) {
        let search = self.inline_search_mut();
        search.set_pool(pool);
        search.set_loading(loading);
        let _ = focused;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::app::tests::make_item;
    use std::time::{Duration, Instant};
    use tuirealm::event::KeyEvent;

    fn pool(ids: &[&str]) -> SearchPool {
        SearchPool::Items(
            ids.iter()
                .map(|id| {
                    let mut item = make_item(&format!("Result {id}"), "Movie");
                    item.id = (*id).to_string();
                    item
                })
                .collect(),
        )
    }

    fn key(code: Key) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// Scores the current query immediately (the shell's clock tick past the
    /// debounce deadline).
    fn fire_debounce(search: &mut InlineSearch) {
        assert!(
            search.handle_clock(Instant::now() + Duration::from_millis(301)),
            "the deadline fires the armed re-score"
        );
    }

    #[test]
    fn re_score_resets_selection_to_the_first_result() {
        let mut search = InlineSearch::new();
        search.open();
        search.set_pool(pool(&["a", "b", "c"]));
        assert_eq!(search.results_len(), 0, "an empty query shows no rows");

        search.handle_key(&key(Key::Char('r')));
        fire_debounce(&mut search);
        assert_eq!(search.results_len(), 3);
        search.delegate_movement(MediaListSurfaceInput::Move(2));
        assert_eq!(search.test_cursor(), 2);

        // A changed query re-scores: the selection resets to the first row.
        search.handle_key(&key(Key::Char('s')));
        fire_debounce(&mut search);
        assert_eq!(search.test_cursor(), 0, "a re-score resets to the first");
    }

    #[test]
    fn pool_refresh_preserves_the_selected_target() {
        let mut search = InlineSearch::new();
        search.open();
        search.set_pool(pool(&["a", "b", "c"]));
        search.restore_query("Result".into());
        assert_eq!(search.test_cursor(), 0);
        search.delegate_movement(MediaListSurfaceInput::Last);
        let selected = search.selected_target().clone();
        assert_eq!(selected.map(|(id, _)| id), Some("c".into()));

        // A pool refresh with the query unchanged keeps the stable target.
        search.set_pool(pool(&["a", "b", "c", "d"]));
        assert_eq!(
            search.selected_target().map(|(id, _)| id),
            Some("c".into()),
            "the carrier's stable-target preservation survives the refresh"
        );
        // A refresh that drops the selected target clamps instead.
        search.set_pool(pool(&["a", "b"]));
        assert!(search.selected_target().is_some());
    }

    #[test]
    fn empty_query_projects_zero_rows() {
        let mut search = InlineSearch::new();
        search.open();
        search.set_pool(pool(&["a", "b"]));
        assert_eq!(search.results_len(), 0);

        search.handle_key(&key(Key::Char('r')));
        fire_debounce(&mut search);
        assert_eq!(search.results_len(), 2);

        // Backspace back to the empty query clears the rows again.
        search.handle_key(&key(Key::Backspace));
        assert_eq!(search.results_len(), 0, "an empty query shows no rows");
        assert_eq!(search.test_cursor(), 0);
    }

    #[test]
    fn movement_routes_through_the_carrier() {
        let mut search = InlineSearch::new();
        search.open();
        search.set_pool(pool(&["a", "b", "c"]));
        search.handle_key(&key(Key::Char('r')));
        fire_debounce(&mut search);

        search.handle_key(&key(Key::Down));
        assert_eq!(search.test_cursor(), 1);
        search.handle_key(&key(Key::End));
        assert_eq!(search.test_cursor(), 2);
        search.handle_key(&key(Key::Home));
        assert_eq!(search.test_cursor(), 0);
        search.handle_key(&key(Key::Up));
        assert_eq!(search.test_cursor(), 0, "movement clamps at the ends");
        assert_eq!(search.selected_target().map(|(id, _)| id), Some("a".into()));
    }
}
