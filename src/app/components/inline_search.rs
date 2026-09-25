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

    /// `(original_index, score)` for every corpus entry that matches `query`
    /// against its match text (display name, or indexed `search_text` for
    /// albums). Every match goes through the shared word-local rule, so a
    /// query word can never be spelled out of letters taken from different
    /// words of the label.
    fn match_scores(
        &self,
        matcher: &fuzzy_matcher::skim::SkimMatcherV2,
        query: &str,
    ) -> Vec<(usize, i64)> {
        use crate::app::infra::fuzzy_match::word_match_score;
        match self {
            Self::Items(items) => items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| {
                    word_match_score(matcher, &item.display_name(), query).map(|score| (i, score))
                })
                .collect(),
            Self::Albums(entries) => entries
                .iter()
                .enumerate()
                .filter_map(|(i, entry)| {
                    // The label is the ancestor chain ("Artist / Album"); the
                    // shared rule already keeps each query word inside one of
                    // its words, so the whole label is the candidate.
                    word_match_score(matcher, &entry.search_text, query).map(|score| (i, score))
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
/// secondary/duration. A played result paints the one played-row colour.
fn search_result_row(item: &mbv_core::api::EmbyItem) -> MediaListRow<String> {
    MediaListRow::Item {
        target: item.id.clone(),
        primary: search_row_label(item),
        secondary: None,
        trailing: (!item.is_folder && item.production_year > 0)
            .then(|| MediaListTrailing::Gutter(item.production_year.to_string())),
        duration: None,
        kind: if item.is_folder {
            MediaKind::Collection
        } else {
            MediaKind::Media
        },
        semantic_state: MediaSemanticState::from_emby(item),
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

    #[cfg(test)]
    pub(in crate::app) fn restore_query(&mut self, query: String) {
        self.query = query;
        self.deadline = None;
        self.recompute_order();
        self.publish_rows(true);
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

    /// The embedded canonical row-flow carrier: the panel drives it through
    /// the [`super::library_panel::content::PanelList`] surface and the owners
    /// resolve pointer inputs against it (design.md D1/D4).
    pub(in crate::app) fn results(&self) -> &MediaListCarrier<String> {
        &self.results
    }

    pub(in crate::app) fn results_mut(&mut self) -> &mut MediaListCarrier<String> {
        &mut self.results
    }

    #[cfg(test)]
    pub(in crate::app) fn selected_target(&self) -> Option<(String, String)> {
        self.selected_item().map(|item| (item.id, item.item_type))
    }

    pub(in crate::app) fn results_len(&self) -> usize {
        self.results.rows().len()
    }

    /// Whether a host explicitly supplied a flat corpus. Grouped Music keeps
    /// this false in production; the compatibility distinction lets focused
    /// harnesses exercise the old activation path without changing painting.
    pub(in crate::app) fn has_pool_entries(&self) -> bool {
        match &self.pool {
            SearchPool::Items(items) => !items.is_empty(),
            SearchPool::Albums(entries) => !entries.is_empty(),
        }
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

    /// The pool item for a resolved row target — the row a delegated
    /// double-click or context gesture resolved (design.md D4) — independent
    /// of the carrier's current selection.
    pub(in crate::app) fn item_for_target(&self, target: &str) -> Option<mbv_core::api::EmbyItem> {
        self.order
            .iter()
            .filter_map(|&(idx, _)| self.pool.resolved_item_at(idx))
            .find(|item| item.id == target)
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
            // The resting viewport returns to the top with the reset
            // selection; an ordinary refresh keeps the parked offset and
            // clamps at paint (design.md D2/D3).
            self.results.set_scroll(0);
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

    /// Grouped Music keeps its tree as the browser owner. Such a host uses
    /// this shared control only for query editing, debounce, and the bar; it
    /// must not receive a flat corpus or start a library fetch.
    fn uses_local_filter(&self) -> bool {
        false
    }

    /// Apply a query after the shared debounce fires. The host owns only its
    /// destination-specific projection; the query editor remains this type.
    fn inline_search_debounced(&mut self) {}
    fn selected_inline_search_item(&self) -> Option<mbv_core::api::EmbyItem> {
        self.inline_search().selected_item()
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

    /// The reported case: an album whose own name is the whole phrase. No
    /// separator is involved, so the match can only be a scatter through the
    /// name's words ("devil" out of "The Velvet Underground Live With Lou
    /// Reed") and the anchored rule has to reject it.
    #[test]
    fn album_search_rejects_a_scatter_through_one_name() {
        let entry = crate::app::AlbumSearchEntry {
            album: make_item("The Velvet Underground Live With Lou Reed", "MusicAlbum"),
            ancestors: Vec::new(),
            display_label: "The Velvet Underground Live With Lou Reed".into(),
            search_text: "The Velvet Underground Live With Lou Reed".into(),
        };
        let mut search = InlineSearch::new();
        search.open();
        search.set_pool(SearchPool::Albums(vec![entry]));

        for (query, expected) in [("devil", 0), ("velvet", 1), ("lou reed", 1)] {
            search.restore_query(query.into());
            assert_eq!(search.results_len(), expected, "query {query:?}");
        }
    }

    /// An album's search text is its ancestor chain (`Artist / Album`). The
    /// shared word-local rule keeps each query word inside one word of that
    /// label, so a query word can no longer be spelled out of single letters
    /// taken from different names or different words.
    #[test]
    fn album_search_matches_a_word_within_one_word_of_the_chain() {
        let entry = crate::app::AlbumSearchEntry {
            album: make_item("Live With Lou Reed", "MusicAlbum"),
            ancestors: vec![crate::app::AlbumPathPart {
                id: "artist-1".into(),
                name: "The Velvet Underground".into(),
            }],
            display_label: "The Velvet Underground / Live With Lou Reed".into(),
            search_text: "The Velvet Underground / Live With Lou Reed".into(),
        };
        let mut search = InlineSearch::new();
        search.open();
        search.set_pool(SearchPool::Albums(vec![entry]));

        // "devil" would have to come from single letters spread over four of
        // the label's words (d in "Underground", e after it, v in "Live",
        // i in "With", l in "Lou") and must not hit; the real words still do.
        for (query, expected) in [("devil", 0), ("velvet", 1), ("lou reed", 1)] {
            search.restore_query(query.into());
            assert_eq!(search.results_len(), expected, "query {query:?}");
        }
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
        // Park the resting viewport at the bottom the way the panel does at
        // paint time, so the reset has a stale offset to fall from.
        search.results_mut().clamp_viewport(1);
        assert_eq!(search.results().scroll(), 2);

        // A changed query re-scores: the selection resets to the first row
        // and the resting viewport rests at the top with it.
        search.handle_key(&key(Key::Char('s')));
        fire_debounce(&mut search);
        assert_eq!(search.test_cursor(), 0, "a re-score resets to the first");
        assert_eq!(
            search.results().scroll(),
            0,
            "the resting viewport rests at the top after a re-score"
        );
    }

    #[test]
    fn pool_refresh_preserves_the_selected_target() {
        let mut search = InlineSearch::new();
        search.open();
        search.set_pool(pool(&["a", "b", "c"]));
        search.restore_query("Result".into());
        assert_eq!(search.test_cursor(), 0);
        search.delegate_movement(MediaListSurfaceInput::Last);
        // Park the resting viewport the way the panel does at paint time.
        search.results_mut().clamp_viewport(1);
        let selected = search.selected_target().clone();
        assert_eq!(selected.map(|(id, _)| id), Some("c".into()));

        // A pool refresh with the query unchanged keeps the stable target
        // and leaves the resting viewport alone.
        search.set_pool(pool(&["a", "b", "c", "d"]));
        assert_eq!(
            search.selected_target().map(|(id, _)| id),
            Some("c".into()),
            "the carrier's stable-target preservation survives the refresh"
        );
        assert_eq!(
            search.results().scroll(),
            2,
            "an unchanged-query refresh leaves the resting viewport alone"
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

    /// A played search result projects the shared `Played` state, so the one
    /// played-row colour is used in the search list too.
    #[test]
    fn played_search_results_project_the_shared_played_state() {
        let mut played = make_item("Watched", "Movie");
        played.played = true;
        let fresh = make_item("Fresh", "Movie");
        let state = |item| match search_result_row(&item) {
            MediaListRow::Item { semantic_state, .. } => semantic_state,
            _ => panic!("search rows are items"),
        };
        assert_eq!(state(played), MediaSemanticState::Played);
        assert_eq!(state(fresh), MediaSemanticState::Ordinary);
    }
}
