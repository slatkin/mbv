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

// Some tree accessors are still reached only by tests; the allowance lapses
// once every Grouped Music integration path uses them.
#![cfg_attr(not(test), allow(dead_code))]
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

use crate::app::components::media_list::MediaSemanticState;
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
    Track { album: String, track: String },
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
    /// The settled item's canonical semantic state, derived at the projection
    /// boundary through [`MediaSemanticState::from_emby`]. Music collapse
    /// keeps every album leaf `Ordinary`, so the label renderer can never
    /// re-derive played/resume decoration from raw item fields.
    pub(in crate::app) semantic_state: MediaSemanticState,
}

/// A cached track projected below an album leaf. The browser only needs the
/// stable Workspace target and label; the owning Music component retains the
/// full `EmbyItem` to resolve activation through the existing playback arm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct MusicTreeTrack {
    pub(in crate::app) target: String,
    pub(in crate::app) title: String,
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
        semantic_state: MediaSemanticState,
    },
    Track {
        album_target: String,
        target: String,
        title: String,
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
    /// Per node: its top-level artist group's settled order (the group zebra
    /// phase). Descendant albums and tracks share their root's phase;
    /// `usize::MAX` marks a tombstone.
    root_position: Vec<usize>,
    revision: TreeRevision,
}

impl MusicTreeModel {
    pub(in crate::app) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            intern: HashMap::new(),
            roots: Vec::new(),
            children: Vec::new(),
            root_position: Vec::new(),
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
        self.reconcile_with_tracks(entries, &HashMap::new());
    }

    /// Reconciles the settled album projection plus the already cached track
    /// children. The map is keyed by the stable album target, so duplicate
    /// album IDs remain distinct tree branches.
    pub(in crate::app) fn reconcile_with_tracks(
        &mut self,
        entries: &[MusicTreeEntry],
        tracks_by_album: &HashMap<String, Vec<MusicTreeTrack>>,
    ) {
        let mut next_roots = Vec::new();
        let mut next_children: HashMap<usize, Vec<usize>> = HashMap::new();
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
                &entry.semantic_state,
                &mut display_changed,
            );
            next_children.entry(root).or_default().push(leaf);
            let track_ids = tracks_by_album
                .get(&entry.target)
                .into_iter()
                .flatten()
                .map(|track| self.intern_track(&entry.target, track, &mut display_changed))
                .collect();
            next_children.insert(leaf, track_ids);
        }

        let mut children = vec![Vec::new(); self.nodes.len()];
        for (root, leaves) in next_children {
            children[root] = leaves;
        }
        let mut root_position = vec![usize::MAX; self.nodes.len()];
        for (position, &root) in next_roots.iter().enumerate() {
            root_position[root] = position;
            for &leaf in &children[root] {
                root_position[leaf] = position;
                for &track in &children[leaf] {
                    root_position[track] = position;
                }
            }
        }

        let changed = display_changed || self.roots != next_roots || self.children != children;
        self.roots = next_roots;
        self.children = children;
        self.root_position = root_position;
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
        self.root_position.clear();
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
    fn intern_track(
        &mut self,
        album_target: &str,
        track: &MusicTreeTrack,
        display_changed: &mut bool,
    ) -> usize {
        let node_key = MusicNodeKey::Track {
            album: album_target.to_string(),
            track: track.target.clone(),
        };
        if let Some(id) = self.intern.get(&node_key).copied() {
            if let Some(MusicNode::Track { title, .. }) = self.nodes.get_mut(id) {
                if title != &track.title {
                    *title = track.title.clone();
                    *display_changed = true;
                }
            }
            return id;
        }
        let id = self.nodes.len();
        self.nodes.push(MusicNode::Track {
            album_target: album_target.to_string(),
            target: track.target.clone(),
            title: track.title.clone(),
        });
        self.intern.insert(node_key, id);
        id
    }

    fn intern_album(
        &mut self,
        target: &str,
        title: &str,
        year: Option<&str>,
        semantic_state: &MediaSemanticState,
        display_changed: &mut bool,
    ) -> usize {
        let node_key = MusicNodeKey::Album(target.to_string());
        if let Some(id) = self.intern.get(&node_key).copied() {
            if let Some(MusicNode::Album {
                title: existing_title,
                year: existing_year,
                semantic_state: existing_state,
                ..
            }) = self.nodes.get_mut(id)
            {
                if existing_title != title
                    || existing_year.as_deref() != year
                    || existing_state != semantic_state
                {
                    *existing_title = title.to_string();
                    *existing_year = year.map(str::to_string);
                    *existing_state = semantic_state.clone();
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
            semantic_state: semantic_state.clone(),
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

    /// The artist root's settled display name; album leaves have none.
    pub(in crate::app) fn artist_name_of(&self, id: usize) -> Option<&str> {
        match self.nodes.get(id) {
            Some(MusicNode::Artist { name, .. }) => Some(name),
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
            MusicNode::Album { title, .. } | MusicNode::Track { title, .. } => title,
        }
    }

    pub(in crate::app) fn year_of(&self, id: usize) -> Option<&str> {
        match &self.nodes[id] {
            MusicNode::Album { year, .. } => year.as_deref().filter(|year| !year.is_empty()),
            MusicNode::Artist { .. } | MusicNode::Track { .. } => None,
        }
    }

    pub(in crate::app) fn target_of(&self, id: usize) -> Option<&str> {
        match self.nodes.get(id) {
            Some(MusicNode::Album { target, .. }) => Some(target),
            _ => None,
        }
    }

    /// Resolves an album identity for an album or one of its track children.
    pub(in crate::app) fn album_target_of(&self, id: usize) -> Option<&str> {
        match self.nodes.get(id) {
            Some(MusicNode::Album { target, .. }) => Some(target),
            Some(MusicNode::Track { album_target, .. }) => Some(album_target),
            _ => None,
        }
    }

    /// Resolves the stable `(album target, track target)` identity of a track
    /// node. Track rows never use projection indexes as identities.
    pub(in crate::app) fn track_identity_of(&self, id: usize) -> Option<(&str, &str)> {
        match self.nodes.get(id) {
            Some(MusicNode::Track {
                album_target,
                target,
                ..
            }) => Some((album_target, target)),
            _ => None,
        }
    }

    /// The album leaf's canonical semantic state; artist roots are ordinary
    /// grouping rows and carry none.
    pub(in crate::app) fn semantic_state_of(&self, id: usize) -> Option<&MediaSemanticState> {
        match self.nodes.get(id) {
            Some(MusicNode::Album { semantic_state, .. }) => Some(semantic_state),
            _ => None,
        }
    }

    /// The node's top-level group zebra phase. The header and every visible
    /// descendant deliberately share one band, so expanding a group does not
    /// introduce row-based colour changes.
    pub(in crate::app) fn is_striped(&self, id: usize) -> bool {
        self.root_position
            .get(id)
            .is_some_and(|position| *position != usize::MAX && position.is_multiple_of(2))
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
            MusicNode::Album { .. } if self.children[id].is_empty() => TreeChildren::Leaf,
            MusicNode::Album { .. } => TreeChildren::Loaded(&self.children[id]),
            MusicNode::Track { .. } => TreeChildren::Leaf,
        }
    }

    fn revision(&self) -> TreeRevision {
        self.revision
    }

    fn size_hint(&self) -> usize {
        self.nodes.len()
    }
}

/// The destination-local matching projection used by the tree owner. The
/// eventual fuzzy filter (task 5.1) supplies matching album/root ids through
/// this same seam; expansion never participates in the action scope.
#[derive(Clone, Debug, Default)]
struct MusicTreeFilter {
    matching: HashSet<usize>,
}

impl TreeFilter<MusicTreeModel> for MusicTreeFilter {
    fn is_match(&self, _model: &MusicTreeModel, id: usize) -> bool {
        self.matching.contains(&id)
    }
}

/// Width of the plain-space indentation + separator prefix
/// `tree_label_line` paints before a row's name. Roots have no prefix; nested
/// rows have one three-column space span per level and one separator before
/// the title. The state glyph is intentionally empty.
fn glyph_prefix_width(level: usize) -> usize {
    if level == 0 {
        0
    } else {
        // The renderer trims two columns from the crate's plain-space leaf
        // prefix; keep the title budget in step with that composition.
        3 * level + 1 - 2
    }
}

/// The tree's primary-cell renderer. It receives the crate's own row context
/// (level, tail stack, expansion glyph selection, mark state, selected bit)
/// and composes the label line through `tree_label_line`, then applies mbv's
/// semantic roles: hierarchy glyphs in the muted role, artist roots in the
/// metadata role, album leaves in the emphasis role, marks in the positive
/// status role, the top-level group zebra fill on the whole cell, and the
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
        let year = model.year_of(id);
        // The pinned year-gutter contract (design D8): the six-column date
        // plus its trailing gap is reserved only on rows that carry a year,
        // so a yearless row's title budget keeps those columns.
        let gutter =
            usize::from(year.is_some()) * (YEAR_GUTTER_WIDTH as usize + YEAR_GUTTER_TRAILING_SPACE);
        let budget = if context.render.is_selected {
            self.selected_title_budget
        } else {
            (self.tree_col_width as usize)
                .saturating_sub(glyph_prefix_width(context.level))
                .saturating_sub(gutter)
        };

        // The crate composes the hierarchy guides and expansion glyph; the
        // renderer fills the name slot itself so the focused selected row can
        // carry this frame's marquee window.
        let mut line = tree_label_line(context, TreeLabelPrefix::borrowed(""), glyphs);
        line.spans.pop(); // the empty borrowed name span; its separator stays
        if context.level > 0 {
            // Grouped Music keeps the tree's plain-space hierarchy but drops
            // the two excess leading columns from every non-root row. The
            // glyphs remain crate-owned; only this label composition changes.
            let prefix = line
                .spans
                .first_mut()
                .expect("a nested tree row has a prefix span");
            prefix.content = prefix.content.chars().skip(2).collect::<String>().into();
        }
        let composed = line.spans.len();

        if context.render.is_selected {
            if let Some(spans) = &self.selected_title_spans {
                line.spans.extend(spans.iter().cloned());
            }
        }
        if line.spans.len() == composed {
            // Ordinary truncation: whole-title ellipsis cut to the budget.
            line.spans.push(Span::styled(
                trunc_str(model.title_of(id), budget).to_string(),
                Style::default().fg(name_role(model, id, context.level, context.node.mark)),
            ));
        }

        // Pad the name slot to its budget, then paint the album year once in
        // the right-aligned fixed six-column gutter at the row's right edge in
        // the `STATUS_AVAILABLE` role, followed by its two-column trailing
        // gap. A yearless row appends nothing, so its title keeps the full
        // width (the pinned gutter contract).
        let painted: usize = line.spans[composed..]
            .iter()
            .map(|span| span.content.width())
            .sum();
        if let Some(pad) = budget.checked_sub(painted).filter(|pad| *pad > 0) {
            line.spans.push(Span::raw(" ".repeat(pad)));
        }
        if let Some(year) = year {
            line.spans.push(Span::styled(
                format!(
                    "{:>width$}",
                    trunc_str(year, YEAR_GUTTER_WIDTH as usize),
                    width = YEAR_GUTTER_WIDTH as usize
                ),
                Style::default().fg(palette::STATUS_AVAILABLE),
            ));
            line.spans
                .push(Span::raw(" ".repeat(YEAR_GUTTER_TRAILING_SPACE)));
        }

        // Everything the crate composed raw (guides, expansion glyph,
        // separators) takes the muted hierarchy role; the name spans above
        // keep theirs. The crate supplies no glyph-style parameter: the state
        // glyph is an unstyled `Span::raw`, so the label renderer's own cell is
        // the supported seam for it (the state glyph is styleable only here,
        // while the guides also arrive through `line_style`).
        for span in line.spans.iter_mut().take(composed) {
            span.style = span.style.fg(palette::TEXT_MUTED);
        }

        // The row's fill: a multi-selected album leaf paints the selected-row
        // bar (the style half of the canonical multi-selection contract; task
        // 4.2 drives the marks), overriding its group band; otherwise the
        // header and every descendant share their top-level group's phase. The
        // focused selected row's bar comes from the crate's `highlight_style`,
        // applied after this cell.
        let mut style = Style::default();
        if multi_select_bar(model, id, context.node.mark) {
            style = style.bg(palette::SELECTED_ROW_BG);
        } else if model.is_striped(id) {
            style = style.bg(self.zebra_fill);
        }
        Cell::from(line).style(style)
    }
}

/// Whether a row paints the selected-row bar as part of the tree's
/// multi-selection. Album leaves are the only stored multi-selection
/// identities; an artist root is the Heading-equivalent grouping row, keeps
/// its surface fill, and shows its aggregate state through its name role
/// alone.
fn multi_select_bar(model: &MusicTreeModel, id: usize, mark: TreeMarkState) -> bool {
    mark == TreeMarkState::Marked && model.target_of(id).is_some()
}

/// The row's name role, resolved only from semantic inputs: the crate's mark
/// state, the row's hierarchy level, and an album leaf's canonical
/// [`MediaSemanticState`]. Nothing here reads a raw `EmbyItem` played/resume
/// field, and music collapse keeps every leaf `Ordinary`.
fn name_role(
    model: &MusicTreeModel,
    id: usize,
    level: usize,
    mark: TreeMarkState,
) -> ratatui::style::Color {
    match mark {
        TreeMarkState::Marked => palette::STATUS_AVAILABLE,
        TreeMarkState::Partial => palette::TEXT_ACCENT_MUTED,
        TreeMarkState::Unmarked if level == 0 => palette::MUSIC_HEADER,
        TreeMarkState::Unmarked => model
            .semantic_state_of(id)
            .map_or(palette::TEXT_EMPHASIS, semantic_role),
    }
}

/// The canonical media-list semantic palette applied to a tree album leaf: an
/// ordinary leaf keeps the emphasis role, a played one mutes, and an active
/// one keeps the emphasis role without any inline progress decoration (the
/// tree paints no progress slot).
fn semantic_role(state: &MediaSemanticState) -> ratatui::style::Color {
    match state {
        MediaSemanticState::Ordinary => palette::TEXT_EMPHASIS,
        MediaSemanticState::Played => palette::TEXT_MUTED,
        MediaSemanticState::Active { .. } | MediaSemanticState::NowPlaying { .. } => {
            palette::TEXT_EMPHASIS
        }
    }
}

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
    /// Whether the latest `view` completed and retained hit geometry (the
    /// panel's `set_paint_policy`/`set_geometry`/`clamp_viewport` invalidate
    /// it before the next view, so a skipped frame claims no point).
    paint_complete: bool,
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
            marquee_key: String::new(),
            marquee_started: Instant::now(),
            configured_geometry: None,
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
        self.model.reconcile_with_tracks(entries, &self.track_items);
        let rebuilt = self.state.ensure_projection(&self.model, &self.query);
        self.state.ensure_mark_states(&self.model);
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

    pub(in crate::app) fn set_geometry(&mut self, claim_rect: Rect, content_rect: Rect) {
        self.configured_geometry = Some((claim_rect, content_rect));
        self.last_area = None;
        self.invalidate();
    }

    pub(in crate::app) fn expand_root(&mut self, root: usize) {
        self.expand_node(root);
    }

    /// Expands an artist or cached-track album node without changing
    /// selection. Track children are loaded from the existing projection;
    /// this operation never starts a fetch.
    pub(in crate::app) fn expand_node(&mut self, id: usize) {
        let parent = self
            .state
            .projection()
            .nodes()
            .iter()
            .find(|node| node.id() == id)
            .and_then(|node| node.parent());
        self.state.set_expanded(id, parent, true);
        self.state.ensure_projection(&self.model, &self.query);
        self.invalidate();
    }

    pub(in crate::app) fn node_is_expanded(&self, id: usize) -> bool {
        let parent = self
            .state
            .projection()
            .nodes()
            .iter()
            .find(|node| node.id() == id)
            .and_then(|node| node.parent());
        self.state.node_is_expanded(id, parent)
    }

    /// Collapses an artist root (task 2.4 Left): its leaves leave the visible
    /// projection while the selection stays on the root, and the crate's
    /// projection rebuild re-arms the viewport visibility rule.
    pub(in crate::app) fn collapse_root(&mut self, root: usize) {
        self.state.set_expanded(root, None, false);
        self.state.ensure_projection(&self.model, &self.query);
        self.invalidate();
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
        let selected = self.state.select_by_id(&self.model, &self.query, id);
        if selected {
            self.invalidate();
        }
        selected
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
            .and_then(|id| self.model.album_target_of(id))
    }

    /// The selected track's stable album and track targets, if the tree is on
    /// a track item rather than an artist or album row.
    pub(in crate::app) fn selected_track_identity(&self) -> Option<(&str, &str)> {
        self.state
            .selected_id()
            .and_then(|id| self.model.track_identity_of(id))
    }

    /// The selected artist root's settled identity (design D7): the stable
    /// `ArtistItems` key or the deterministic fallback grouping key.
    pub(in crate::app) fn selected_artist_key(&self) -> Option<&ArtistKey> {
        self.selected_id()
            .and_then(|id| self.model.artist_key_of(id))
    }

    /// The selected artist root's settled display name.
    pub(in crate::app) fn selected_artist_name(&self) -> Option<&str> {
        self.selected_id()
            .and_then(|id| self.model.artist_name_of(id))
    }

    /// Returns the focused artist's album targets in settled order. The walk
    /// intentionally reads the model's child list rather than the expanded
    /// projection: a collapsed root has the same action scope as an expanded
    /// root. When filtering is enabled, the current tree projection is the
    /// visibility predicate, so only matching leaves cross the component
    /// boundary. A non-artist selection is not an artist action; an empty
    /// artist returns an empty list so callers can handle that case explicitly.
    pub(in crate::app) fn selected_artist_album_targets(&self) -> Option<Vec<String>> {
        let root = self.selected_id()?;
        if !self.model.is_artist(root) {
            return None;
        }
        let filtered = !matches!(self.query.filter_config(), TreeFilterConfig::Disabled);
        let visible = |id| {
            !filtered
                || self
                    .state
                    .projection()
                    .nodes()
                    .iter()
                    .any(|node| node.id() == id && node.parent() == Some(root))
        };
        Some(
            self.model.children[root]
                .iter()
                .copied()
                .filter(|id| visible(*id))
                .filter_map(|id| self.model.target_of(id).map(str::to_owned))
                .collect(),
        )
    }

    /// Replaces the owner-local matching projection without creating a second
    /// filter owner. `None` disables filtering; `Some(&[])` is an active
    /// no-match filter. Task 5.1 can feed fuzzy-matched node ids here after
    /// its debounce while this task's action walk already respects them.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn set_filter_matches(&mut self, matching: Option<&[usize]>) {
        {
            let filter = self.query.filter_mut();
            filter.matching.clear();
            if let Some(matching) = matching {
                filter.matching.extend(matching.iter().copied());
            }
        }
        self.query.set_filter_config(match matching {
            Some(_) => TreeFilterConfig::enabled(),
            None => TreeFilterConfig::Disabled,
        });
        self.state.ensure_projection(&self.model, &self.query);
        self.state.ensure_mark_states(&self.model);
        self.invalidate();
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

    /// The node's painted title (the tree render tests locate rows by title
    /// rather than by hard-coded settled sort positions).
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
        self.invalidate();
    }

    /// Expands every artist root (component-test fixture for the tree's
    /// settled visible album order).
    #[cfg(test)]
    pub(in crate::app) fn expand_all_roots(&mut self) {
        let _ = self.state.expand_all(&self.model);
        self.state.ensure_projection(&self.model, &self.query);
        self.invalidate();
    }

    /// Whether the selected node is an artist root (task 2.3: an artist focus
    /// resolves to no album, so callers never treat it as a selectable album).
    pub(in crate::app) fn selected_is_artist(&self) -> bool {
        self.state
            .selected_id()
            .is_some_and(|id| self.model.is_artist(id))
    }

    pub(in crate::app) fn selected_is_track(&self) -> bool {
        self.state
            .selected_id()
            .is_some_and(|id| self.model.track_identity_of(id).is_some())
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
        let selected = self.state.select_by_id(&self.model, &self.query, id);
        if selected {
            self.invalidate();
        }
        selected
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

    /// The neighbour album-artwork targets the shell prefetches (task 6.5,
    /// design D4): from the **latest completed paint**'s visible projection,
    /// up to one album leaf behind the selected leaf and up to three ahead, in
    /// visible order, skipping artist roots and the selected leaf itself. The
    /// owner resolves the window here so the shell receives stable targets and
    /// never a cursor or enough tree state to re-resolve one.
    ///
    /// `None` when no paint completed (retained geometry is not the painted
    /// projection), when an artist root is focused (the shipped suppression),
    /// or when the window has no album leaf.
    pub(in crate::app) fn neighbour_prefetch_targets(&self) -> Option<Vec<String>> {
        if !self.paint_complete || self.selected_is_artist() {
            return None;
        }
        let nodes = self.state.projection().nodes();
        let selected_index = nodes
            .iter()
            .position(|node| Some(node.id()) == self.state.selected_id())?;
        let target_of =
            |node: &ProjectedNode<usize>| self.model.target_of(node.id()).map(str::to_owned);
        let mut targets: Vec<String> = nodes[..selected_index]
            .iter()
            .rev()
            .filter_map(target_of)
            .take(NEIGHBOUR_PREFETCH_BEHIND)
            .collect();
        targets.extend(
            nodes[selected_index + 1..]
                .iter()
                .filter_map(target_of)
                .take(NEIGHBOUR_PREFETCH_AHEAD),
        );
        (!targets.is_empty()).then_some(targets)
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
        match self.hit_test(at)? {
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

    fn row_rect_for_index(&self, index: usize) -> Option<Rect> {
        if !self.paint_complete {
            return None;
        }
        let area = self.last_area?;
        let row = index.checked_sub(self.state.offset())?;
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

    /// The selected row's one-line rect from the latest completed view, when
    /// the node is visible (the panel's retained selected-row geometry).
    pub(in crate::app) fn selected_row_rect(&self) -> Option<Rect> {
        self.state
            .selected_index()
            .and_then(|index| self.row_rect_for_index(index))
    }

    /// A visible node's one-line rect from the latest completed view.
    #[cfg(test)]
    pub(in crate::app) fn row_rect_for(&self, id: usize) -> Option<Rect> {
        let index = self
            .state
            .projection()
            .nodes()
            .iter()
            .position(|node| node.id() == id)?;
        self.row_rect_for_index(index)
    }

    /// Whether the latest completed view's retained geometry claims `at`.
    pub(in crate::app) fn claims_point(&self, at: Position) -> bool {
        self.hit_test(at).is_some()
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
        self.paint_complete
            .then(|| self.state.hit_test(position))
            .flatten()
    }

    pub(in crate::app) fn projection_len(&self) -> usize {
        self.state.visible_len()
    }

    pub(in crate::app) fn projected_nodes(&self) -> &[ProjectedNode<usize>] {
        self.state.projection().nodes()
    }

    /// Injects the marquee clock directly (no sleeps): `key` must be the
    /// title the selected row marquees, `elapsed_ms` the age of its start
    /// time. The no-sleep marquee coverage and the later title-clock reset
    /// coverage both go through here.
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
            configured_geometry,
            last_area,
            ..
        } = self;
        let (claim_rect, content_rect) = configured_geometry.unwrap_or((area, area));
        // The panel's claim keeps selected-row ownership and the shared
        // scrollbar at the canonical full-width position. The tree rows
        // themselves use the panel's two-column inset on both sides; the
        // right-hand gap is kept clear while the scrollbar remains at the
        // claim edge.
        let paint_area = Rect {
            y: content_rect.y,
            height: content_rect.height,
            ..claim_rect
        };
        let row_area = content_rect;

        state.ensure_projection(model, query);
        // A geometry change re-applies the viewport visibility rule to this
        // same owner (design D3/D4): re-arm the selected node's visibility so
        // the crate's `KeepInView` policy scrolls the minimum needed at the
        // new height instead of leaving the selection outside a clamped
        // viewport. A settled-content change already re-arms through
        // `reconcile`, and a no-op frame does not touch the offset.
        if *last_area != Some(content_rect) {
            // Re-arm the selected row's visibility for the new height without
            // touching expansion. The crate only arms its `KeepInView` rule
            // when the selection actually changes, so clear and restore the
            // current projection row; `select_id`/`select_by_id` must not be
            // used here because their `expand_to` would promote filter-forced
            // expansion into persistent expansion on every resize (D5).
            rearm_selection_visibility_for(state);
            *last_area = Some(content_rect);
        }
        // `tui-treelistview` resolves the primary column inside `tree_area`,
        // not `paint_area`: when overflow has room beyond the claimed area,
        // the adapter gives the crate one extra column so its own scrollbar
        // lands outside the painted content. Budget labels from that same
        // resolved cell width, after removing the crate-owned scrollbar.
        let overflow = state.visible_len() > content_rect.height as usize;
        let tree_area = if overflow && row_area.right() < frame.area().right() {
            Rect {
                width: row_area.width.saturating_add(1),
                ..row_area
            }
        } else {
            row_area
        };
        let tree_col_width = tree_area.width.saturating_sub(u16::from(overflow));

        // The focused selected row's marquee window, computed once per frame
        // through the shared marquee primitive (design D8). The clock keys on
        // the marqueed title text, exactly like the media-list painter. The
        // window budget matches the renderer's own per-row budget: the
        // six-column date plus trailing gap is reserved only when the selected
        // row carries a year (the pinned gutter contract).
        let selected = state
            .selected_index()
            .and_then(|index| state.projection().nodes().get(index).copied());
        state.ensure_mark_states(model);
        let selected_title_budget = selected.map_or(0, |node| {
            let gutter = usize::from(model.year_of(node.id()).is_some())
                * (YEAR_GUTTER_WIDTH as usize + YEAR_GUTTER_TRAILING_SPACE);
            (tree_col_width as usize)
                .saturating_sub(glyph_prefix_width(node.level()))
                .saturating_sub(gutter)
        });
        let selected_title_spans = selected.filter(|_| *focused).map(|node| {
            let title = model.title_of(node.id());
            let role = name_role(model, node.id(), node.level(), state.mark_state(node.id()));
            let parts = vec![(title.to_string(), role)];
            marquee_spans(
                title,
                &parts,
                selected_title_budget,
                marquee_key,
                marquee_started,
                false,
            )
        });

        let zebra_fill = palette::music_tree_zebra(*focused);
        let label = MusicTreeLabelRenderer {
            tree_col_width,
            zebra_fill,
            selected_title_spans,
            selected_title_budget,
            _marker: std::marker::PhantomData,
        };

        // One primary tree column: the pinned six-column year gutter and its
        // trailing gap are painted inside that cell by the label renderer, so
        // they can be reserved per row (no gutter on artist roots or yearless
        // leaves) and no second or inline year column exists.
        let columns = TreeColumnSet::new(vec![ColumnDef::tree(
            "",
            ColumnWidth::flexible(1, u16::MAX)
                .expect("the tree column's width range is always valid"),
        )])
        .expect("the tree adapter's column set is always valid")
        .without_header();

        let widget = TreeListView::new(model, query, &label, &columns, tree_style(*focused))
            .glyphs(tree_glyphs());

        // `tui-treelistview` 0.2.2 has no scrollbar policy or scrollbar style
        // seam: an overflowing render always appends Ratatui's default
        // scrollbar inside the supplied area. Render into a cloned buffer and
        // omit that one crate-owned column, then paint the app's shared
        // scrollbar in its normal position. Extending the widget area by one
        // column when there is room makes the crate's table occupy the same
        // content width as the other library lists while its discarded
        // scrollbar lands exactly where the shared scrollbar does. At the
        // frame edge both widgets necessarily use the final content column.
        let mut tree_buffer = Buffer::empty(tree_area);
        {
            let target = frame.buffer_mut();
            for y in tree_area.y..tree_area.bottom() {
                for x in tree_area.x..tree_area.right() {
                    if let (Some(source), Some(destination)) = (
                        target.cell(Position { x, y }),
                        tree_buffer.cell_mut(Position { x, y }),
                    ) {
                        destination.clone_from(source);
                    }
                }
            }
        }
        StatefulWidget::render(widget, tree_area, &mut tree_buffer, state);
        let crate_scrollbar_x = overflow.then(|| tree_area.right().saturating_sub(1));
        {
            let target = frame.buffer_mut();
            for y in paint_area.y..paint_area.bottom() {
                for x in paint_area.x..paint_area.right() {
                    if crate_scrollbar_x == Some(x) {
                        continue;
                    }
                    if let (Some(source), Some(destination)) = (
                        tree_buffer.cell(Position { x, y }),
                        target.cell_mut(Position { x, y }),
                    ) {
                        destination.clone_from(source);
                    }
                }
            }
        }
        // The crate scrollbar has been discarded above. The shared helper is
        // focus-gated exactly like the canonical media-list painter, so an
        // unfocused tree has no scrollbar rather than retaining a second
        // crate-default indicator.
        if *focused && overflow {
            crate::app::render::components::widgets::render_right_scrollbar_with_viewport(
                frame,
                paint_area,
                state.visible_len(),
                content_rect.height as usize,
                state.offset(),
                palette::SCROLLBAR,
            );
        }
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

/// The tree's glyph set. Grouped Music deliberately has no symbols: levels
/// remain readable through plain-space indentation alone.
fn tree_glyphs() -> TreeGlyphs<'static> {
    TreeGlyphs {
        indent: "   ",
        branch_last: "",
        branch: "",
        vert: "",
        empty: "   ",
        leaf: "",
        expanded: "",
        collapsed: "",
        unloaded: "",
        loading: "",
    }
}

/// The tree's visual configuration through `TreeListViewStyle`: no border,
/// no header, no highlight symbol, no horizontal scroll; the focused
/// selected row paints the canonical `SELECTED_ROW_BG` bar across the whole
/// row (the crate applies it after row cells, so it overrides the zebra);
/// hierarchy guides take the muted role via `line_style` and the aggregate
/// mark states take the positive/muted-accent roles. The crate's
/// unconfigurable scrollbar is discarded by `view`; the shared application
/// scrollbar is painted after the tree with the canonical role and glyph set.
fn tree_style(focused: bool) -> tui_treelistview::TreeListViewStyle<'static> {
    use tui_treelistview::{TreeHorizontalScroll, TreeListViewStyle, TreeRowRendering};
    TreeListViewStyle {
        block_style: Style::default(),
        highlight_style: if focused {
            Style::default().bg(palette::SELECTED_ROW_BG)
        } else {
            Style::default()
        },
        line_style: Style::default().fg(palette::TEXT_MUTED),
        marked_style: Style::default().fg(palette::STATUS_AVAILABLE),
        partial_mark_style: Style::default().fg(palette::TEXT_ACCENT_MUTED),
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
