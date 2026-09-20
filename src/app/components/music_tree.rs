//! Grouped Music's destination-specific tree adapter (task 1.1, design D1/D8).
//!
//! This module is the one boundary between mbv and the locked
//! `tui-treelistview` 0.2.2 dependency. It owns the destination-local node
//! arena (`MusicTreeModel`), the `TreeListViewState` owner, and the tree view
//! painting through the crate's supported model/query/state/renderer/style
//! seams only — no crate keymap, no second painter, no raw colours (every
//! style resolves an existing `palette` role here, inside the owning layer).
//!
//! Task 1.1 scope is the dependency gate: the spike renders one frame at the
//! existing Wide and smallest supported non-Wide Library-panel fixtures and
//! proves the full-row selected bar, group-relative zebra, scrollbar, focused
//! marquee, clipping, latest-render hit testing, and aggregate marks. The
//! tree is not yet wired into `MusicContent` or the `PanelList` surface
//! (tasks 2.x/5.2); nothing outside this module and its spike tests uses it.

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
        match &self.nodes[id] {
            MusicNode::Album { target, .. } => Some(target),
            MusicNode::Artist { .. } => None,
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
/// destination-local model, painted through the crate's widget. The spike
/// scope keeps the query unfiltered and unsorted (settled order);
/// `TreeQuery` remains the seam the later filter session plugs into.
pub(in crate::app) struct MusicTreeBrowser {
    model: MusicTreeModel,
    query: TreeQuery,
    state: TreeListViewState<usize>,
    focused: bool,
    marquee_key: String,
    marquee_started: Instant,
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
        }
    }

    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(in crate::app) fn expand_root(&mut self, root: usize) {
        self.state.set_expanded(root, None, true);
        self.state.ensure_projection(&self.model, &self.query);
    }

    pub(in crate::app) fn select_index(&mut self, index: usize) {
        self.state.select_index(Some(index));
    }

    /// The first visible projection row (the viewport offset the latest
    /// render settled on).
    pub(in crate::app) fn offset(&self) -> usize {
        self.state.offset()
    }

    pub(in crate::app) fn set_marked(&mut self, id: usize, marked: bool) -> bool {
        self.state.set_marked(id, marked)
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
        } = self;

        state.ensure_projection(model, query);
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
