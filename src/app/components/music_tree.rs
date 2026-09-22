//! Grouped Music's destination-specific tree adapter (design D1/D8).
//!
//! This module is the one boundary between mbv and the locked
//! `tui-treelistview` 0.2.2 dependency. It owns the destination-local node
//! arena (`MusicTreeModel`), the `TreeListViewState` owner, and the tree view
//! painting through the crate's supported model/query/state/renderer/style
//! seams only — no crate keymap, no second painter, no raw colours (every
//! style resolves an existing `palette` role here, inside the owning layer).
//!
//! The tree render tests (`render::tests_music_characterization` and
//! `render::tests_music_groups`) paint one frame each at the existing Wide and
//! smallest supported non-Wide Library-panel fixtures and cover the full-row
//! selected bar, group-relative zebra, scrollbar, focused marquee, clipping,
//! latest-render hit testing, and aggregate marks. Task 2.2 added the one state
//! owner over that model: settled-catalog reconciliation (selection,
//! expansion, marks, viewport continuity) and responsive geometry
//! reconciliation, plus the album-selection persistence guard. `MusicContent`
//! drives that owner as the Grouped Music browser.

//!
//! Layout arithmetic the view relies on (mirroring the crate's
//! `resolve_layout` for this configuration, asserted by the tree render tests):
//! borderless block, no header, empty highlight symbol, `column_spacing` 0,
//! horizontal scrolling disabled, one primary tree column — so the tree
//! column takes every remaining column and the vertical scrollbar takes
//! exactly one column when the projection overflows. The pinned six-column
//! year gutter (design D8) is painted inside that tree cell: one right-aligned
//! fixed-width cell per row, reserved only on rows that carry a year, so a
//! yearless row's title keeps the full width.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use fuzzy_matcher::skim::SkimMatcherV2;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Cell, StatefulWidget};
use tui_treelistview::{
    tree_label_line, ColumnDef, ColumnWidth, ProjectedNode, TreeChildren, TreeColumnSet,
    TreeFilter, TreeFilterConfig, TreeGlyphs, TreeHit, TreeLabelPrefix, TreeLabelRenderer,
    TreeListView, TreeListViewState, TreeMarkState, TreeModel, TreeQuery, TreeRevision,
    TreeRowContext,
};

use unicode_width::UnicodeWidthStr;

use crate::app::components::list::{
    AggregateMarkState, Cursored, Expandable, MarkSelection, MarkSelectionState, PagingPolicy,
    PaintRetained, PaintRetainedState, Row, RowFlow, Viewported,
};
use crate::app::components::media_list::{
    queue_row_background, queue_row_zebra, MediaSemanticState,
};
use crate::app::music_grouping::ArtistKey;
use crate::app::palette;
use crate::app::render::components::marquee::marquee_spans;
use crate::app::ui_util::trunc_str;

/// The album year's fixed right-aligned date width (design D8: this
/// change's pinned metadata-gutter contract, painted through the crate's
/// column interface).
pub(in crate::app) const YEAR_GUTTER_WIDTH: u16 = 6;

/// The spacing after a rendered year before the row's trailing edge.
const YEAR_GUTTER_TRAILING_SPACE: usize = 2;

/// The neighbour album-artwork window (task 6.5, design D4): the shell
/// prefetches up to one visible album leaf behind the selected leaf and up to
/// three ahead.
const NEIGHBOUR_PREFETCH_BEHIND: usize = 1;
const NEIGHBOUR_PREFETCH_AHEAD: usize = 3;

/// The stable, destination-facing identity of one tree row (design D2). This
/// is the only row identity the seam accepts: arena indexes and `MusicNodeKey`
/// stay private to this module. Every arm mirrors the arena's interning key
/// one-for-one, so a target is stable across ordinary settled-catalog
/// replacement and never a projection row position.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(in crate::app) enum MusicTreeTarget {
    Artist(ArtistKey),
    Album(String),
    Track { album: String, track: String },
}

impl MusicTreeTarget {
    /// The stable album target when this target is an album leaf; artist roots
    /// and cached tracks have none.
    pub(in crate::app) fn album_leaf_target(&self) -> Option<&str> {
        match self {
            Self::Album(target) => Some(target),
            Self::Artist(_) | Self::Track { .. } => None,
        }
    }

    /// Whether this target is an artist root.
    pub(in crate::app) fn is_artist(&self) -> bool {
        matches!(self, Self::Artist(_))
    }
}

/// The stable semantic identity one arena node is interned by (design D2).
/// Artist roots intern by the settled catalog's `ArtistKey` — the resolved
/// `ArtistItems` identity or the deterministic fallback grouping key — so
/// equal display names with distinct Service identities stay separate roots.
/// Album leaves intern by the stable album target the shell already keys
/// albums by. Neither key is ever a projection row position. The key stays
/// private: `MusicTreeTarget` is the only identity that crosses a module
/// boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum MusicNodeKey {
    Artist(ArtistKey),
    Album(String),
    Track { album: String, track: String },
}

impl From<&MusicTreeTarget> for MusicNodeKey {
    fn from(target: &MusicTreeTarget) -> Self {
        match target {
            MusicTreeTarget::Artist(key) => MusicNodeKey::Artist(key.clone()),
            MusicTreeTarget::Album(target) => MusicNodeKey::Album(target.clone()),
            MusicTreeTarget::Track { album, track } => MusicNodeKey::Track {
                album: album.clone(),
                track: track.clone(),
            },
        }
    }
}

/// One settled album leaf's domain projection: the stable target the shell
/// The Grouped Music tree browser: one `TreeListViewState` owner over the
/// destination-local model, painted through the crate's widget. The query
/// remains unsorted (settled order); its matching predicate is disabled until
/// the filter session feeds the owner-local seam. Task 2.4 maps the destination's local
/// chords (visible-node movement, Home/End, paging, parent/child, root
/// expansion) onto this owner's operations; the crate keymap stays disabled.
pub(in crate::app) struct MusicTreeBrowser {
    model: MusicTreeModel,
    query: TreeQuery<MusicTreeFilter>,
    state: TreeListViewState<usize>,
    /// Cached track projections already settled by the shell. The tree never
    /// fetches; retaining this projection lets a local move from an artist
    /// root onto a child keep the rows visible while the shell switches its
    /// album snapshot.
    track_items: HashMap<String, Vec<MusicTreeTrack>>,
    focused: bool,
    marquee_key: String,
    marquee_started: Instant,
    /// Whether this retained owner has an open local Grouped Music filter
    /// session. This is distinct from the tree query: an empty query shows
    /// the complete tree but remains a filter session for input/prefetch.
    filter_active: bool,
    filter_anchor: Option<usize>,
    filter_query: String,
    search_bar_query: String,
    /// The parent-owned claim and row-flow rectangles. The canonical media
    /// list uses the claim width for rows and the scrollbar while retaining
    /// the content height for viewport metrics; the tree follows that same
    /// geometry without creating a second presentation owner.
    configured_geometry: Option<(Rect, Rect)>,
    /// The rect of the latest frame, so a geometry change re-applies the
    /// viewport visibility rule to this same owner (design D3/D4) instead of
    /// leaving the selection outside a newly clamped viewport.
    last_area: Option<Rect>,
    /// The last album target reported for the shell's destination-position
    /// persistence (design D3 step 5). An artist-root focus resolves to no
    /// album and never clears or overwrites it.
    last_reported_album: Option<String>,
    /// Ordered album-leaf marks in the order they were added. A stable-target
    /// carrier owned here; the crate stores marks as a set, so this order is
    /// the source of truth for ordered multi-selection membership and hidden
    /// marks stay here until a filter is dismissed.
    marks: MarkSelectionState<MusicTreeTarget>,
    /// Retained target-bearing geometry from the latest completed `view`. The
    /// panel's `set_geometry`/`clamp_viewport` and settled-content pushes
    /// invalidate it before the next view, so a skipped frame claims no point.
    paint: PaintRetainedState<MusicTreeTarget>,
}

impl MusicTreeBrowser {
    pub(in crate::app) fn new(model: MusicTreeModel) -> Self {
        let query = TreeQuery::new().with_filter(
            MusicTreeFilter::default(),
            TreeFilterConfig::Disabled,
            TreeRevision::INITIAL,
        );
        let mut state = TreeListViewState::with_capacity(model.size_hint());
        state.set_draw_lines(false);
        state.ensure_projection(&model, &query);
        state.select_index((!state.is_empty()).then_some(0));
        Self {
            model,
            query,
            state,
            track_items: HashMap::new(),
            focused: true,
            filter_active: false,
            filter_anchor: None,
            filter_query: String::new(),
            search_bar_query: String::new(),
            marquee_key: String::new(),
            marquee_started: Instant::now(),
            configured_geometry: None,
            last_area: None,
            last_reported_album: None,
            marks: MarkSelectionState::new(),
            paint: PaintRetainedState::new(),
        }
    }

    /// Reconciles this one owner with a settled catalog (design D3): re-intern
    /// the arena, rebuild the cached projection only when the model revision
    /// changed, and refresh the derived mark aggregate. The crate restores the
    /// selected node and surviving expansion/marks by stable identity; the
    /// viewport keeps its prior offset when bounds permit, otherwise the next
    /// frame applies only the minimum scroll that keeps the selection visible.
    ///
    /// Returns whether the projection was rebuilt: a no-op reconciliation
    /// leaves the cached projection, offset, and arming untouched.
    pub(in crate::app) fn reconcile(&mut self, entries: &[MusicTreeEntry]) -> bool {
        self.model.reconcile_with_tracks(entries, &self.track_items);
        self.marks.retain(|target| {
            let MusicTreeTarget::Album(album) = target else {
                return false;
            };
            self.model
                .roots
                .iter()
                .flat_map(|root| self.model.children[*root].iter())
                .any(|id| self.model.target_of(*id) == Some(album.as_str()))
        });
        let rebuilt = self.state.ensure_projection(&self.model, &self.query);
        self.sync_manual_marks();
        if self.filter_active {
            let query = self.filter_query.clone();
            self.apply_filter_query(&query);
        }
        if rebuilt {
            // The crate's hit map belongs to the completed frame, not to the
            // newly reconciled projection. Do not let a settled content push
            // claim a point until the next view has completed.
            self.invalidate();
        }
        rebuilt
    }

    /// Replaces the cached track projection for the focused artist. This is
    /// called only with shell-projected cache data; no request originates in
    /// the tree owner.
    pub(in crate::app) fn set_track_items(
        &mut self,
        tracks_by_album: HashMap<String, Vec<MusicTreeTrack>>,
    ) {
        self.track_items = tracks_by_album;
    }

    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(in crate::app) fn set_search_bar(&mut self, query: &str) {
        self.search_bar_query.clear();
        self.search_bar_query.push_str(query);
    }

    pub(in crate::app) fn search_bar(&self) -> Option<(String, bool)> {
        self.filter_active
            .then(|| (self.search_bar_query.clone(), false))
    }

    pub(in crate::app) fn open_filter(&mut self) {
        if !self.filter_active {
            self.filter_anchor = self.state.selected_id();
            self.filter_active = true;
        }
        self.filter_query.clear();
        self.apply_filter_match_ids(None);
    }

    pub(in crate::app) fn close_filter(&mut self) {
        let anchor = self.filter_anchor.take();
        self.filter_active = false;
        self.filter_query.clear();
        self.apply_filter_match_ids(None);
        if let Some(anchor) = anchor {
            let _ = self.state.select_by_id(&self.model, &self.query, anchor);
            self.rearm_selection_visibility();
        }
        self.invalidate();
    }

    pub(in crate::app) fn filter_active(&self) -> bool {
        self.filter_active
    }

    pub(in crate::app) fn apply_filter_query(&mut self, query: &str) {
        if !self.filter_active {
            return;
        }
        self.filter_query.clear();
        self.filter_query.push_str(query);
        if query.is_empty() {
            if matches!(self.query.filter_config(), TreeFilterConfig::Disabled) {
                return;
            }
            self.apply_filter_match_ids(None);
            return;
        }
        let matcher = SkimMatcherV2::default().ignore_case();
        // Every level is searched against its own text: an artist name
        // surfaces the artist root, an album title (or year) its album leaf,
        // and a track title its cached track — each with only the ancestors
        // the projection needs to reach it. The shared word-local rule keeps
        // each query word inside one word of the row's text, so a query can no
        // longer be spelled out of letters taken from different words.
        let matching: Vec<usize> = (0..self.model.size_hint())
            .filter(|id| {
                self.model.search_text_of(*id).is_some_and(|text| {
                    crate::app::fuzzy_match::word_match_score(&matcher, &text, query).is_some()
                })
            })
            .collect();
        // Every settled content push re-applies the active filter, and the
        // crate advances its filter revision on every write. An unchanged
        // match set therefore keeps the current projection (and the
        // current-frame hit rows a filtered pointer gesture resolves against)
        // instead of rebuilding and invalidating it for nothing.
        let unchanged = !matches!(self.query.filter_config(), TreeFilterConfig::Disabled)
            && self.query.filter().matching.len() == matching.len()
            && matching
                .iter()
                .all(|id| self.query.filter().matching.contains(id));
        if unchanged {
            return;
        }
        self.apply_filter_match_ids(Some(&matching));
    }

    pub(in crate::app) fn set_geometry(&mut self, claim_rect: Rect, content_rect: Rect) {
        // Only a real geometry change re-arms the selected-row visibility
        // rule (design D3: re-anchor on discrete transitions only). Nulling
        // `last_area` on every push would re-arm KeepInView every frame and
        // snap any offset-only viewport scroll back to the selection.
        if self.configured_geometry != Some((claim_rect, content_rect)) {
            self.configured_geometry = Some((claim_rect, content_rect));
            self.last_area = None;
        }
        self.invalidate();
    }

    /// Expands one artist root or cached-track album node by stable target,
    /// without changing selection. Track children are loaded from the existing
    /// projection; this operation never starts a fetch. A target the arena has
    /// not interned is an explicit absent result.
    pub(in crate::app) fn expand_root(&mut self, root: &MusicTreeTarget) -> bool {
        self.expand_node(root)
    }

    /// The projected parent of a node, from the cached projection.
    fn projected_parent_of(&self, id: usize) -> Option<usize> {
        self.state
            .projection()
            .nodes()
            .iter()
            .find(|node| node.id() == id)
            .and_then(|node| node.parent())
    }

    /// Expands an artist or cached-track album node without changing
    /// selection. Track children are loaded from the existing projection;
    /// this operation never starts a fetch. A target the arena has not
    /// interned is an explicit absent result.
    pub(in crate::app) fn expand_node(&mut self, target: &MusicTreeTarget) -> bool {
        let Some(id) = self.model.id_of(target) else {
            return false;
        };
        let parent = self.projected_parent_of(id);
        self.state.set_expanded(id, parent, true);
        self.state.ensure_projection(&self.model, &self.query);
        self.invalidate();
        true
    }

    pub(in crate::app) fn node_is_expanded(&self, target: &MusicTreeTarget) -> bool {
        let Some(id) = self.model.id_of(target) else {
            return false;
        };
        let parent = self.projected_parent_of(id);
        self.state.node_is_expanded(id, parent)
    }

    /// Whether the settled projection has children for this node. This is a
    /// local cache fact used by pointer expansion; it never starts a fetch.
    pub(in crate::app) fn node_has_children(&self, target: &MusicTreeTarget) -> bool {
        self.model
            .id_of(target)
            .and_then(|id| self.model.children.get(id))
            .is_some_and(|children| !children.is_empty())
    }

    /// Whether the node is a cached track. A track double-click claims the
    /// gesture and emits `MusicTreeTrackActivate` with the node's stable
    /// identity, which the shell plays through the grouped-track resolver.
    pub(in crate::app) fn model_is_track(&self, target: &MusicTreeTarget) -> bool {
        self.model
            .id_of(target)
            .is_some_and(|id| self.model.track_identity_of(id).is_some())
    }

    /// Toggle an artist or cached-track album node without changing selection.
    /// A target the arena has not interned is an explicit absent result.
    pub(in crate::app) fn toggle_node(&mut self, target: &MusicTreeTarget) -> bool {
        let Some(id) = self.model.id_of(target) else {
            return false;
        };
        let parent = self.projected_parent_of(id);
        let expanded = self.state.node_is_expanded(id, parent);
        self.state.set_expanded(id, parent, !expanded);
        self.state.ensure_projection(&self.model, &self.query);
        self.invalidate();
        true
    }

    /// Collapses an artist root (task 2.4 Left): its leaves leave the visible
    /// projection while the selection stays on the root, and the crate's
    /// projection rebuild re-arms the viewport visibility rule. A target the
    /// arena has not interned is an explicit absent result.
    pub(in crate::app) fn collapse_root(&mut self, root: &MusicTreeTarget) -> bool {
        let Some(id) = self.model.id_of(root) else {
            return false;
        };
        self.state.set_expanded(id, None, false);
        self.state.ensure_projection(&self.model, &self.query);
        self.invalidate();
        true
    }

    /// Toggles an artist root's persistent expansion (task 2.4 Enter). This is
    /// the tree's own expansion, never the filter-forced projection state.
    pub(in crate::app) fn toggle_root(&mut self, root: &MusicTreeTarget) -> bool {
        if self.root_is_expanded(root) {
            self.collapse_root(root)
        } else {
            self.expand_root(root)
        }
    }

    /// Moves the selection to the visible parent: an album leaf's artist root
    /// (task 2.4 Left). Returns false for an artist root, which has no parent
    /// to move to. The movement is the shared `Expandable` arithmetic over the
    /// visible row flow, and the crate's selection change re-arms the viewport
    /// visibility rule.
    pub(in crate::app) fn move_to_parent(&mut self) -> bool {
        let flow = self.row_flow();
        Expandable::select_parent(self, &flow).is_some()
    }

    /// Whether an artist root is persistently expanded.
    pub(in crate::app) fn root_is_expanded(&self, root: &MusicTreeTarget) -> bool {
        self.model
            .id_of(root)
            .is_some_and(|id| self.state.node_is_expanded(id, None))
    }

    /// The projection row for a persisted expanded-flow offset. The persisted
    /// offset is a row in the settled flat flow (one artist row per group
    /// followed by its leaves in settled order). The tree's projection
    /// interleaves those same rows but hides collapsed leaves, so walk the
    /// fully-expanded settled order and return the first node at or after
    /// `offset` that is currently visible: a hidden leaf rounds forward to the
    /// next visible node instead of anchoring the viewport to an unrelated row.
    fn projection_row_for_flow_offset(&self, offset: usize) -> Option<usize> {
        let mut position = 0;
        for &root in &self.model.roots {
            if position >= offset {
                if let Some(row) = self.state.visible_index_of(root) {
                    return Some(row);
                }
            }
            position += 1;
            for &leaf in &self.model.children[root] {
                if position >= offset {
                    if let Some(row) = self.state.visible_index_of(leaf) {
                        return Some(row);
                    }
                }
                position += 1;
            }
        }
        None
    }
}

include!("music_tree_model.rs");
include!("music_tree_label.rs");
include!("music_tree_selection.rs");
include!("music_tree_view.rs");

#[cfg(test)]
#[path = "music_tree_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "music_tree_browser_tests.rs"]
mod browser_tests;
