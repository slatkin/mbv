//! Grouped Music's destination-specific tree adapter (task 1.1, design D1/D8).
//!
//! This module is the one boundary between mbv and the locked
//! `tui-treelistview` 0.2.2 dependency. It owns the destination-local node
//! arena (`MusicTreeModel`), the `TreeListViewState` owner, and the tree view
//! painting through the crate's supported model/query/state/renderer/style
//! seams only — no crate keymap, no second painter, no raw colours (every
//! style resolves an existing `palette` role here, inside the owning layer).
//!
//! Task 1.1 scope was the dependency gate: the spike renders one frame at the
//! existing Wide and smallest supported non-Wide Library-panel fixtures and
//! proves the full-row selected bar, group-relative zebra, scrollbar, focused
//! marquee, clipping, latest-render hit testing, and aggregate marks. Task 2.2
//! adds the one state owner over that model: settled-catalog reconciliation
//! (selection, expansion, marks, viewport continuity) and responsive geometry
//! reconciliation, plus the album-selection persistence guard. The tree is not
//! yet wired into `MusicContent` or the `PanelList` surface (tasks 2.3/2.4/
//! 5.2); nothing outside this module and its tests uses it.

// Until tasks 2.x wire the tree into the Grouped Music browser, only the
// spike tests reach this module; the allowance lapses at that integration.
#![cfg_attr(not(test), allow(dead_code))]
//!
//! Layout arithmetic the view relies on (mirroring the crate's
//! `resolve_layout` for this configuration, asserted by the spike tests):
//! borderless block, no header, empty highlight symbol, `column_spacing` 0,
//! horizontal scrolling disabled, one fixed six-column year gutter — so the
//! tree column takes every remaining column and the vertical scrollbar takes
//! exactly one column when the projection overflows.

use std::collections::HashMap;
use std::time::Instant;

use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, StatefulWidget};
use tui_treelistview::{
    tree_label_line, ColumnDef, ColumnWidth, ProjectedNode, TreeChildren, TreeColumnSet,
    TreeGlyphs, TreeHit, TreeLabelPrefix, TreeLabelRenderer, TreeListView, TreeListViewState,
    TreeMarkState, TreeModel, TreeQuery, TreeRevision, TreeRowContext,
};

use crate::app::music_grouping::ArtistKey;
use crate::app::palette::{self, Surface};
use crate::app::render::components::marquee::marquee_spans;
use crate::app::ui_util::trunc_str;

/// The album year's fixed right-aligned gutter width (design D8: this
/// change's pinned metadata-gutter contract, painted through the crate's
/// column interface).
pub(in crate::app) const YEAR_GUTTER_WIDTH: u16 = 6;

/// The stable semantic identity one arena node is interned by (design D2).
/// Artist roots intern by the settled catalog's `ArtistKey` — the resolved
/// `ArtistItems` identity or the deterministic fallback grouping key — so
/// equal display names with distinct Service identities stay separate roots.
/// Album leaves intern by the stable album target the shell already keys
/// albums by. Neither key is ever a projection row position.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(in crate::app) enum MusicNodeKey {
    Artist(ArtistKey),
    Album(String),
}

/// One settled album leaf's domain projection: the stable target the shell
/// already keys albums by, plus its settled artist identity and the display
/// text the row paints.
#[derive(Clone)]
pub(in crate::app) struct MusicTreeEntry {
    pub(in crate::app) artist: String,
    pub(in crate::app) artist_key: ArtistKey,
    pub(in crate::app) title: String,
    pub(in crate::app) year: Option<String>,
    pub(in crate::app) target: String,
}

enum MusicNode {
    Artist {
        /// The node's own settled identity (node-to-domain translation, D2).
        key: ArtistKey,
        name: String,
    },
    Album {
        title: String,
        year: Option<String>,
        target: String,
    },
}

/// The destination-local node arena behind the crate's `TreeModel` (design
/// D2). Node ids are arena indexes into a monotonic, append-only `Vec`:
/// interned by semantic key, stable across ordinary settled-catalog
/// replacement, and never reused for a different key during the owner's
/// lifetime. Entries removed by a replacement leave the root/child
/// projection (tombstoned) while their interned mapping is retained; the
/// arena resets only when the retained Music destination changes identity
/// (`reset`).
pub(in crate::app) struct MusicTreeModel {
    nodes: Vec<MusicNode>,
    intern: HashMap<MusicNodeKey, usize>,
    roots: Vec<usize>,
    /// Per node id: its artist root's settled child leaves. Album nodes and
    /// tombstoned artist roots carry an empty child list.
    children: Vec<Vec<usize>>,
    /// Per album node: its index within its artist group's settled order
    /// (the group-relative zebra phase). `usize::MAX` for artist roots.
    leaf_position: Vec<usize>,
    revision: TreeRevision,
}

impl MusicTreeModel {
    pub(in crate::app) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            intern: HashMap::new(),
            roots: Vec::new(),
            children: Vec::new(),
            leaf_position: Vec::new(),
            revision: TreeRevision::INITIAL,
        }
    }

    /// A model over one settled entry set, for callers that do not keep an
    /// incremental owner.
    pub(in crate::app) fn from_entries(entries: &[MusicTreeEntry]) -> Self {
        let mut model = Self::new();
        model.reconcile(entries);
        model
    }

    /// Reconciles the arena with one settled catalog (design D2/D3): intern
    /// artist roots and album leaves in settled order, then atomically
    /// replace the root/child projection, bumping the model revision only
    /// when the settled content actually changed. Settled entries sharing
    /// one `ArtistKey` form one artist root, with roots in first-occurrence
    /// order and leaves in settled order.
    pub(in crate::app) fn reconcile(&mut self, entries: &[MusicTreeEntry]) {
        let mut next_roots = Vec::new();
        let mut next_children: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut next_leaf_position: HashMap<usize, usize> = HashMap::new();
        let mut display_changed = false;
        let mut root_of_key: HashMap<ArtistKey, usize> = HashMap::new();

        for entry in entries {
            let root = match root_of_key.get(&entry.artist_key) {
                Some(&root) => root,
                None => {
                    let root =
                        self.intern_artist(&entry.artist_key, &entry.artist, &mut display_changed);
                    root_of_key.insert(entry.artist_key.clone(), root);
                    next_roots.push(root);
                    root
                }
            };
            let leaf = self.intern_album(
                &entry.target,
                &entry.title,
                entry.year.as_deref(),
                &mut display_changed,
            );
            let siblings = next_children.entry(root).or_default();
            siblings.push(leaf);
            next_leaf_position.insert(leaf, siblings.len() - 1);
        }

        let mut children = vec![Vec::new(); self.nodes.len()];
        for (root, leaves) in next_children {
            children[root] = leaves;
        }
        let mut leaf_position = vec![usize::MAX; self.nodes.len()];
        for (leaf, position) in next_leaf_position {
            leaf_position[leaf] = position;
        }

        let changed = display_changed || self.roots != next_roots || self.children != children;
        self.roots = next_roots;
        self.children = children;
        self.leaf_position = leaf_position;
        if changed {
            self.revision.advance();
        }
    }

    /// Clears the arena for a new destination identity (design D2): the
    /// intern space restarts, so no stale mapping survives a destination
    /// change.
    pub(in crate::app) fn reset(&mut self) {
        self.nodes.clear();
        self.intern.clear();
        self.roots.clear();
        self.children.clear();
        self.leaf_position.clear();
        self.revision = TreeRevision::INITIAL;
    }

    /// Interns (or refreshes the display name of) one artist root.
    fn intern_artist(&mut self, key: &ArtistKey, name: &str, display_changed: &mut bool) -> usize {
        let node_key = MusicNodeKey::Artist(key.clone());
        if let Some(id) = self.intern.get(&node_key).copied() {
            if let Some(MusicNode::Artist { name: existing, .. }) = self.nodes.get_mut(id) {
                if existing != name {
                    *existing = name.to_string();
                    *display_changed = true;
                }
            }
            return id;
        }
        let id = self.nodes.len();
        self.nodes.push(MusicNode::Artist {
            key: key.clone(),
            name: name.to_string(),
        });
        self.intern.insert(node_key, id);
        id
    }

    /// Interns (or refreshes the display data of) one album leaf.
    fn intern_album(
        &mut self,
        target: &str,
        title: &str,
        year: Option<&str>,
        display_changed: &mut bool,
    ) -> usize {
        let node_key = MusicNodeKey::Album(target.to_string());
        if let Some(id) = self.intern.get(&node_key).copied() {
            if let Some(MusicNode::Album {
                title: existing_title,
                year: existing_year,
                ..
            }) = self.nodes.get_mut(id)
            {
                if existing_title != title || existing_year.as_deref() != year {
                    *existing_title = title.to_string();
                    *existing_year = year.map(str::to_string);
                    *display_changed = true;
                }
            }
            return id;
        }
        let id = self.nodes.len();
        self.nodes.push(MusicNode::Album {
            title: title.to_string(),
            year: year.map(str::to_string),
            target: target.to_string(),
        });
        self.intern.insert(node_key, id);
        id
    }

    /// The interned node id for a semantic key, if any.
    pub(in crate::app) fn node_id(&self, key: &MusicNodeKey) -> Option<usize> {
        self.intern.get(key).copied()
    }

    /// Whether the node is an artist root.
    pub(in crate::app) fn is_artist(&self, id: usize) -> bool {
        matches!(self.nodes.get(id), Some(MusicNode::Artist { .. }))
    }

    /// The artist root's settled identity (node-to-domain translation, D2);
    /// album leaves have none.
    pub(in crate::app) fn artist_key_of(&self, id: usize) -> Option<&ArtistKey> {
        match self.nodes.get(id) {
            Some(MusicNode::Artist { key, .. }) => Some(key),
            _ => None,
        }
    }

    /// The model's current revision value.
    #[cfg(test)]
    pub(in crate::app) fn revision_value(&self) -> u64 {
        self.revision.get()
    }

    /// The settled child album ids of an artist root.
    #[cfg(test)]
    pub(in crate::app) fn children_of(&self, id: usize) -> Vec<usize> {
        self.children[id].clone()
    }

    /// The projected artist root ids in settled order.
    #[cfg(test)]
    pub(in crate::app) fn root_ids(&self) -> Vec<usize> {
        self.roots.clone()
    }

    pub(in crate::app) fn title_of(&self, id: usize) -> &str {
        match &self.nodes[id] {
            MusicNode::Artist { name, .. } => name,
            MusicNode::Album { title, .. } => title,
        }
    }

    pub(in crate::app) fn year_of(&self, id: usize) -> Option<&str> {
        match &self.nodes[id] {
            MusicNode::Album { year, .. } => year.as_deref().filter(|year| !year.is_empty()),
            MusicNode::Artist { .. } => None,
        }
    }

    pub(in crate::app) fn target_of(&self, id: usize) -> Option<&str> {
        match self.nodes.get(id) {
            Some(MusicNode::Album { target, .. }) => Some(target),
            _ => None,
        }
    }

    /// The node's group-relative zebra phase: album leaves alternate from the
    /// secondary fill at their group's first member (the canonical grouped
    /// list's `grouped_member_striped` rule); artist roots never stripe.
    pub(in crate::app) fn is_striped(&self, id: usize) -> bool {
        self.leaf_position[id] != usize::MAX && self.leaf_position[id].is_multiple_of(2)
    }
}

impl TreeModel for MusicTreeModel {
    type Id = usize;

    fn roots(&self) -> impl Iterator<Item = usize> + '_ {
        self.roots.iter().copied()
    }

    fn children(&self, id: usize) -> TreeChildren<'_, usize> {
        match &self.nodes[id] {
            MusicNode::Artist { .. } => TreeChildren::Loaded(&self.children[id]),
            MusicNode::Album { .. } => TreeChildren::Leaf,
        }
    }

    fn revision(&self) -> TreeRevision {
        self.revision
    }

    fn size_hint(&self) -> usize {
        self.nodes.len()
    }
}

/// Width of the hierarchy guides + expansion glyph prefix `tree_label_line`
/// paints before a row's name: `level` three-column guides, then one
/// separator, the one-column state glyph, and one separator.
fn glyph_prefix_width(level: usize) -> usize {
    if level == 0 {
        2
    } else {
        3 * level + 3
    }
}

/// The tree's primary-cell renderer. It receives the crate's own row context
/// (level, tail stack, expansion glyph selection, mark state, selected bit)
/// and composes the label line through `tree_label_line`, then applies mbv's
/// semantic roles: hierarchy glyphs in the muted role, artist roots in the
/// metadata role, album leaves in the emphasis role, marks in the positive
/// status role, the group-relative zebra fill on the whole cell, and the
/// focused selected row's marquee window computed for this frame.
struct MusicTreeLabelRenderer<'a> {
    tree_col_width: u16,
    zebra_fill: ratatui::style::Color,
    /// The focused selected row's marquee spans for this frame (`None` when
    /// unfocused or nothing is selected). Computed in `view` so the renderer
    /// itself can stay `&self` inside the crate's trait signature.
    selected_title_spans: Option<Vec<Span<'static>>>,
    selected_title_budget: usize,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl TreeLabelRenderer<MusicTreeModel> for MusicTreeLabelRenderer<'_> {
    fn cell<'a>(
        &'a self,
        model: &'a MusicTreeModel,
        id: usize,
        context: &TreeRowContext<'_>,
        glyphs: &TreeGlyphs<'a>,
    ) -> Cell<'a> {
        let title = model.title_of(id);
        let budget = if context.render.is_selected {
            self.selected_title_budget
        } else {
            (self.tree_col_width as usize).saturating_sub(glyph_prefix_width(context.level))
        };

        // The crate composes the hierarchy guides and expansion glyph; the
        // renderer fills the name slot itself so the focused selected row can
        // carry this frame's marquee window.
        let mut line = tree_label_line(context, TreeLabelPrefix::borrowed(""), glyphs);
        line.spans.pop(); // the empty borrowed name span; its separator stays
        let composed = line.spans.len();

        if context.render.is_selected {
            if let Some(spans) = &self.selected_title_spans {
                line.spans.extend(spans.iter().cloned());
            }
        }
        if line.spans.len() == composed {
            // Ordinary truncation: whole-title ellipsis cut to the budget.
            line.spans.push(Span::styled(
                trunc_str(title, budget).to_string(),
                Style::default().fg(name_role(model, context)),
            ));
        }

        // Everything the crate composed raw (guides, state glyph, separators)
        // takes the muted hierarchy role; the name spans above keep theirs.
        for span in line.spans.iter_mut().take(composed) {
            span.style = span.style.fg(palette::TEXT_MUTED);
        }

        let mut style = Style::default();
        if model.is_striped(id) {
            style = style.bg(self.zebra_fill);
        }
        Cell::from(line).style(style)
    }
}

/// The row's name role: marks override the semantic base (artist roots in the
/// metadata role, album leaves in the emphasis role); the exact mark visual
/// treatment is task 3.1/4.2 scope — the gate proves the renderer can see and
/// paint the aggregated mark state.
fn name_role(model: &MusicTreeModel, context: &TreeRowContext<'_>) -> ratatui::style::Color {
    if context.node.mark != TreeMarkState::Unmarked {
        palette::STATUS_AVAILABLE
    } else if matches!(model.nodes_at_context(context), Some(true)) {
        palette::TEXT_METADATA
    } else {
        palette::TEXT_EMPHASIS
    }
}

impl MusicTreeModel {
    /// Whether the row is an artist root, derived from the row's own context
    /// position: roots sit at level 0.
    fn nodes_at_context(&self, context: &TreeRowContext<'_>) -> Option<bool> {
        Some(context.level == 0)
    }
}

/// The Grouped Music tree browser: one `TreeListViewState` owner over the
/// destination-local model, painted through the crate's widget. The query
/// stays unfiltered and unsorted (settled order) until the filter session
/// lands; `TreeQuery` remains that seam. Task 2.4 maps the destination's local
/// chords (visible-node movement, Home/End, paging, parent/child, root
/// expansion) onto this owner's operations; the crate keymap stays disabled.
pub(in crate::app) struct MusicTreeBrowser {
    model: MusicTreeModel,
    query: TreeQuery,
    state: TreeListViewState<usize>,
    focused: bool,
    marquee_key: String,
    marquee_started: Instant,
    /// The rect of the latest frame, so a geometry change re-applies the
    /// viewport visibility rule to this same owner (design D3/D4) instead of
    /// leaving the selection outside a newly clamped viewport.
    last_area: Option<Rect>,
    /// The last album target reported for the shell's destination-position
    /// persistence (design D3 step 5). An artist-root focus resolves to no
    /// album and never clears or overwrites it.
    last_reported_album: Option<String>,
    /// Whether the latest `view` completed and retained hit geometry (the
    /// panel's `set_paint_policy`/`set_geometry`/`clamp_viewport` invalidate
    /// it before the next view, so a skipped frame claims no point).
    paint_complete: bool,
}

impl MusicTreeBrowser {
    pub(in crate::app) fn new(model: MusicTreeModel) -> Self {
        let query = TreeQuery::new();
        let mut state = TreeListViewState::with_capacity(model.size_hint());
        state.ensure_projection(&model, &query);
        state.select_index((!state.is_empty()).then_some(0));
        Self {
            model,
            query,
            state,
            focused: true,
            marquee_key: String::new(),
            marquee_started: Instant::now(),
            last_area: None,
            last_reported_album: None,
            paint_complete: false,
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
        self.model.reconcile(entries);
        let rebuilt = self.state.ensure_projection(&self.model, &self.query);
        self.state.ensure_mark_states(&self.model);
        rebuilt
    }

    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(in crate::app) fn expand_root(&mut self, root: usize) {
        self.state.set_expanded(root, None, true);
        self.state.ensure_projection(&self.model, &self.query);
    }

    /// Collapses an artist root (task 2.4 Left): its leaves leave the visible
    /// projection while the selection stays on the root, and the crate's
    /// projection rebuild re-arms the viewport visibility rule.
    pub(in crate::app) fn collapse_root(&mut self, root: usize) {
        self.state.set_expanded(root, None, false);
        self.state.ensure_projection(&self.model, &self.query);
    }

    /// Toggles an artist root's persistent expansion (task 2.4 Enter). This is
    /// the tree's own expansion, never the filter-forced projection state.
    pub(in crate::app) fn toggle_root(&mut self, root: usize) {
        if self.state.node_is_expanded(root, None) {
            self.collapse_root(root);
        } else {
            self.expand_root(root);
        }
    }

    /// Moves the selection to the visible parent: an album leaf's artist root
    /// (task 2.4 Left). Returns false for an artist root, which has no parent
    /// to move to. The crate's selection change re-arms the viewport
    /// visibility rule.
    pub(in crate::app) fn move_to_parent(&mut self) -> bool {
        self.state.select_parent()
    }

    /// Whether an artist root is persistently expanded.
    pub(in crate::app) fn root_is_expanded(&self, root: usize) -> bool {
        self.state.node_is_expanded(root, None)
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

    /// Anchors the shell's persisted album position: selects the album leaf,
    /// translates the persisted flat-flow offset into the current projection,
    /// and re-arms the visibility rule so the next view keeps the selection
    /// visible (design D3). The offset must never be applied as a raw
    /// projection row: the tree interleaves artist roots with album leaves.
    pub(in crate::app) fn anchor_album_target(&mut self, target: &str, row: usize) -> bool {
        if !self.select_album_target(target) {
            return false;
        }
        if let Some(translated) = self.projection_row_for_flow_offset(row) {
            self.state.set_offset(translated);
        }
        self.rearm_selection_visibility();
        true
    }

    /// Selects a node by stable identity, loading its ancestor path, and arms
    /// the viewport to keep it visible (design D3 step 4).
    pub(in crate::app) fn select_id(&mut self, id: usize) -> bool {
        self.state.select_by_id(&self.model, &self.query, id)
    }

    pub(in crate::app) fn select_index(&mut self, index: usize) {
        self.state.select_index(Some(index));
    }

    /// The selected node's stable arena id.
    pub(in crate::app) fn selected_id(&self) -> Option<usize> {
        self.state.selected_id()
    }

    /// The selected node's album target; `None` when an artist root (or
    /// nothing) is selected.
    pub(in crate::app) fn selected_album_target(&self) -> Option<&str> {
        self.state
            .selected_id()
            .and_then(|id| self.model.target_of(id))
    }

    /// The album-selection persistence request for the shell (design D3 step
    /// 5): `Some(target)` only when the resolved selected album differs from
    /// the last one reported, and `None` while an artist root is focused — an
    /// artist focus never overwrites (or clears) the retained album identity.
    pub(in crate::app) fn take_album_selection_change(&mut self) -> Option<String> {
        let resolved = self.selected_album_target().map(str::to_owned);
        if let Some(target) = resolved {
            if self.last_reported_album.as_deref() != Some(target.as_str()) {
                self.last_reported_album = Some(target.clone());
                return Some(target);
            }
        }
        None
    }

    /// The first visible projection row (the viewport offset the latest
    /// render settled on).
    pub(in crate::app) fn offset(&self) -> usize {
        self.state.offset()
    }

    /// Stores a mark on an album leaf. Artist roots derive their aggregate
    /// state from child leaves and are never stored as mark targets (design
    /// D6), so marking one is a no-op.
    pub(in crate::app) fn set_marked(&mut self, id: usize, marked: bool) -> bool {
        if self.model.target_of(id).is_none() {
            return false;
        }
        let changed = self.state.set_marked(id, marked);
        if changed {
            self.state.ensure_mark_states(&self.model);
        }
        changed
    }

    pub(in crate::app) fn mark_state(&self, id: usize) -> TreeMarkState {
        self.state.mark_state(id)
    }

    /// The node's painted title (the spike tests locate rows by title rather
    /// than by hard-coded settled sort positions).
    #[cfg(test)]
    pub(in crate::app) fn title_of(&self, id: usize) -> &str {
        self.model.title_of(id)
    }

    /// The album leaf's stable target (the identity that crosses to the
    /// shell); artist roots have none.
    #[cfg(test)]
    pub(in crate::app) fn target_of(&self, id: usize) -> Option<&str> {
        self.model.target_of(id)
    }

    /// Scrolls the viewport without changing selection (the crate's own
    /// offset seam); the panel's paging maps here in later tasks.
    #[cfg(test)]
    pub(in crate::app) fn scroll_to(&mut self, offset: usize) {
        self.state.set_offset(offset);
    }

    /// Expands every artist root (component-test fixture for the tree's
    /// settled visible album order).
    #[cfg(test)]
    pub(in crate::app) fn expand_all_roots(&mut self) {
        let _ = self.state.expand_all(&self.model);
        self.state.ensure_projection(&self.model, &self.query);
    }

    /// Whether the selected node is an artist root (task 2.3: an artist focus
    /// resolves to no album, so callers never treat it as a selectable album).
    pub(in crate::app) fn selected_is_artist(&self) -> bool {
        self.state
            .selected_id()
            .is_some_and(|id| self.model.is_artist(id))
    }

    /// Whether the owner has any selected node (the shell projection adopts
    /// its album position only into a selection-less tree, task 2.3).
    pub(in crate::app) fn has_selection(&self) -> bool {
        self.state.selected_id().is_some()
    }

    /// Selects the album leaf with `target`, loading its ancestor path so it
    /// becomes visible, and arms the viewport visibility rule (task 2.3: the
    /// tree replaces the album carrier for the shell's selected-album
    /// projection).
    pub(in crate::app) fn select_album_target(&mut self, target: &str) -> bool {
        let Some(id) = self.model.node_id(&MusicNodeKey::Album(target.to_string())) else {
            return false;
        };
        self.state.select_by_id(&self.model, &self.query, id)
    }

    /// The current visible projection's album targets: `Some(target)` per album
    /// leaf, `None` per artist root (the tree's row flow, replacing the removed
    /// flat album carrier's `album_flow_targets`).
    pub(in crate::app) fn projected_targets(&self) -> Vec<Option<String>> {
        self.state
            .projection()
            .nodes()
            .iter()
            .map(|node| self.model.target_of(node.id()).map(str::to_owned))
            .collect()
    }

    /// Invalidates the retained paint geometry: until the next view completes
    /// the owner claims no point (the canonical latest-render contract).
    pub(in crate::app) fn invalidate(&mut self) {
        self.paint_complete = false;
    }

    /// Clamps the viewport to a painted height without transferring owner state,
    /// then re-arms the selected node's visibility: the panel's per-frame
    /// `PanelList` viewport clamp for this owner. The crate re-applies the
    /// minimum scroll during the next render.
    pub(in crate::app) fn clamp_viewport_to(&mut self, viewport_height: usize) {
        self.invalidate();
        let max = self
            .state
            .visible_len()
            .saturating_sub(viewport_height.max(1));
        self.state.set_offset(self.state.offset().min(max));
        self.rearm_selection_visibility();
    }

    /// Clears every stored album mark and refreshes the derived aggregate
    /// state (destination-switch selection clear).
    pub(in crate::app) fn clear_marks(&mut self) {
        self.state.clear_marks();
        self.state.ensure_mark_states(&self.model);
    }

    /// Latest-completed-render hit resolution: the node id and its projection
    /// row for the row under `at`, if any.
    pub(in crate::app) fn hit_node(&self, at: Position) -> Option<(usize, usize)> {
        match self.state.hit_test(at)? {
            TreeHit::Row { id, index, .. } => Some((id, index)),
            TreeHit::Header { .. } | TreeHit::VerticalScrollbar | TreeHit::HorizontalScrollbar => {
                None
            }
        }
    }

    /// Moves the selection `delta` visible rows (the tree's own visible-node
    /// movement), clamped at the projection bounds by the crate.
    pub(in crate::app) fn move_selection(&mut self, delta: i64) {
        for _ in 0..delta.unsigned_abs() {
            let _ = if delta < 0 {
                self.state.select_prev()
            } else {
                self.state.select_next()
            };
        }
    }

    /// Moves the selection one viewport page (the shared media list's five-row
    /// page stride), keeping the tree's own visible-node movement.
    pub(in crate::app) fn page_selection(&mut self, delta: i64) {
        self.move_selection(delta.saturating_mul(5));
    }

    pub(in crate::app) fn select_first_visible(&mut self) {
        let _ = self.state.select_first();
    }

    pub(in crate::app) fn select_last_visible(&mut self) {
        let _ = self.state.select_last();
    }

    /// The selected row's one-line rect from the latest completed view, when
    /// the node is visible (the panel's retained selected-row geometry).
    pub(in crate::app) fn selected_row_rect(&self) -> Option<Rect> {
        if !self.paint_complete {
            return None;
        }
        let area = self.last_area?;
        let row = self
            .state
            .selected_index()?
            .checked_sub(self.state.offset())?;
        if row as u16 >= area.height {
            return None;
        }
        Some(Rect {
            x: area.x,
            y: area.y.saturating_add(row as u16),
            width: area.width,
            height: 1,
        })
    }

    /// Whether the latest completed view's retained geometry claims `at`.
    pub(in crate::app) fn claims_point(&self, at: Position) -> bool {
        self.paint_complete && self.state.hit_test(at).is_some()
    }

    /// Arms the crate's `KeepInView` rule for the current selection without
    /// touching expansion (design D3/D4: a geometry or viewport change re-arms
    /// the same owner's visibility rule). Deliberately not `select_by_id`,
    /// whose `expand_to` would promote filter-forced expansion into persistent
    /// expansion on every resize (D5).
    fn rearm_selection_visibility(&mut self) {
        rearm_selection_visibility_for(&mut self.state);
    }

    pub(in crate::app) fn hit_test(&self, position: Position) -> Option<TreeHit<usize>> {
        self.state.hit_test(position)
    }

    pub(in crate::app) fn projection_len(&self) -> usize {
        self.state.visible_len()
    }

    pub(in crate::app) fn projected_nodes(&self) -> &[ProjectedNode<usize>] {
        self.state.projection().nodes()
    }

    /// Injects the marquee clock directly (no sleeps): `key` must be the
    /// title the selected row marquees, `elapsed_ms` the age of its start
    /// time. The spike's no-sleep marquee proof and the later title-clock
    /// reset coverage both go through here.
    #[cfg(test)]
    pub(in crate::app) fn set_marquee_clock_for_test(&mut self, key: &str, elapsed_ms: u64) {
        self.marquee_key.clear();
        self.marquee_key.push_str(key);
        self.marquee_started = Instant::now() - std::time::Duration::from_millis(elapsed_ms);
    }

    /// One frame of tree painting into `area` through the crate's widget.
    /// Everything below the widget call is the adapter's own supported-seam
    /// configuration; the widget owns projection refresh, mark aggregation,
    /// viewport scrolling, row painting, scrollbars, and the latest-render
    /// hit map.
    pub(in crate::app) fn view(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let Self {
            model,
            query,
            state,
            focused,
            marquee_key,
            marquee_started,
            last_area,
            ..
        } = self;

        state.ensure_projection(model, query);
        // A geometry change re-applies the viewport visibility rule to this
        // same owner (design D3/D4): re-arm the selected node's visibility so
        // the crate's `KeepInView` policy scrolls the minimum needed at the
        // new height instead of leaving the selection outside a clamped
        // viewport. A settled-content change already re-arms through
        // `reconcile`, and a no-op frame does not touch the offset.
        if *last_area != Some(area) {
            // Re-arm the selected row's visibility for the new height without
            // touching expansion. The crate only arms its `KeepInView` rule
            // when the selection actually changes, so clear and restore the
            // current projection row; `select_id`/`select_by_id` must not be
            // used here because their `expand_to` would promote filter-forced
            // expansion into persistent expansion on every resize (D5).
            rearm_selection_visibility_for(state);
            *last_area = Some(area);
        }
        // Mirror the crate's resolved layout for this fixed configuration so
        // the renderer can budget titles before the widget renders: the
        // vertical scrollbar takes one column when the projection overflows,
        // and the year gutter takes its fixed six columns.
        let overflow = usize::from(state.visible_len() > area.height as usize);
        let table_width = area.width.saturating_sub(overflow as u16);
        let tree_col_width = table_width.saturating_sub(YEAR_GUTTER_WIDTH);

        // The focused selected row's marquee window, computed once per frame
        // through the shared marquee primitive (design D8). The clock keys on
        // the marqueed title text, exactly like the media-list painter.
        let selected = state
            .selected_index()
            .and_then(|index| state.projection().nodes().get(index).copied());
        let selected_title_spans = selected.filter(|_| *focused).map(|node| {
            let title = model.title_of(node.id());
            let budget = (tree_col_width as usize).saturating_sub(glyph_prefix_width(node.level()));
            let parts = vec![(title.to_string(), palette::TEXT_EMPHASIS)];
            marquee_spans(title, &parts, budget, marquee_key, marquee_started, false)
        });
        let selected_title_budget = selected
            .map(|node| (tree_col_width as usize).saturating_sub(glyph_prefix_width(node.level())))
            .unwrap_or(0);

        let zebra_fill = palette::surface_colors(Surface::SidebarBody, *focused).fill;
        let label = MusicTreeLabelRenderer {
            tree_col_width,
            zebra_fill,
            selected_title_spans,
            selected_title_budget,
            _marker: std::marker::PhantomData,
        };

        // The year gutter: one right-aligned fixed six-column cell in the
        // `STATUS_AVAILABLE` role, zebra-filled with its row, no gutter
        // reserved on artist roots or yearless leaves.
        let year_renderer =
            move |model: &MusicTreeModel, id: usize, _context: &TreeRowContext<'_>| match model
                .year_of(id)
            {
                Some(year) => {
                    let mut style = Style::default().fg(palette::STATUS_AVAILABLE);
                    if model.is_striped(id) {
                        style = style.bg(zebra_fill);
                    }
                    Cell::from(
                        Line::from(Span::styled(year.to_string(), style))
                            .alignment(ratatui::layout::Alignment::Right),
                    )
                }
                None => Cell::default(),
            };
        let columns = TreeColumnSet::new(vec![
            ColumnDef::tree(
                "",
                ColumnWidth::flexible(1, u16::MAX)
                    .expect("the tree column's width range is always valid"),
            ),
            ColumnDef::data_owned("Year", ColumnWidth::fixed(YEAR_GUTTER_WIDTH), year_renderer),
        ])
        .expect("the tree adapter's column set is always valid")
        .without_header();

        let widget = TreeListView::new(model, query, &label, &columns, tree_style(*focused));
        StatefulWidget::render(widget, area, frame.buffer_mut(), state);
        self.paint_complete = true;
    }
}

/// Arms the crate's `KeepInView` rule for the current selection without
/// touching expansion: the crate only arms when the selection changes, so
/// clear and restore the current projection row.
fn rearm_selection_visibility_for(state: &mut TreeListViewState<usize>) {
    if let Some(index) = state.selected_index() {
        state.select_index(None);
        state.select_index(Some(index));
    }
}

/// The tree's visual configuration through `TreeListViewStyle`: no border,
/// no header, no highlight symbol, no horizontal scroll; the focused
/// selected row paints the canonical `SELECTED_ROW_BG` bar across the whole
/// row (the crate applies it after row cells, so it overrides the zebra);
/// hierarchy guides take the muted role via `line_style`.
fn tree_style(focused: bool) -> tui_treelistview::TreeListViewStyle<'static> {
    use tui_treelistview::{TreeHorizontalScroll, TreeListViewStyle, TreeRowRendering};
    TreeListViewStyle {
        highlight_style: if focused {
            Style::default().bg(palette::SELECTED_ROW_BG)
        } else {
            Style::default()
        },
        line_style: Style::default().fg(palette::TEXT_MUTED),
        highlight_symbol: "",
        borders: ratatui::widgets::Borders::NONE,
        column_spacing: 0,
        row_rendering: TreeRowRendering::Virtualized,
        horizontal_scroll: TreeHorizontalScroll::Disabled,
        ..tui_treelistview::TreeListViewStyle::default()
    }
}

#[cfg(test)]
#[path = "music_tree_tests.rs"]
mod tests;
