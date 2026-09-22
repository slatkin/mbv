//! Test-only arena-id accessors for the tree's own white-box suites.
//!
//! The production seam addresses rows by `MusicTreeTarget` only. The tree's
//! white-box tests still assert arena intern behaviour (monotonic ids,
//! tombstoning, revision stamps) and need the arena `usize` directly, so this
//! `#[cfg(test)]` module keeps those handles private to the tree module
//! instead of exposing them through a `pub(in crate::app)` surface.

use ratatui::layout::{Position, Rect};
use tui_treelistview::{ProjectedNode, TreeHit, TreeMarkState};

use super::{computed_mark_state, MusicTreeBrowser};

impl MusicTreeBrowser {
    /// The selected node's arena id (white-box tests only).
    pub(super) fn selected_id(&self) -> Option<usize> {
        self.state.selected_id()
    }

    /// The album target of an arena node id (white-box tests only).
    pub(super) fn target_of(&self, id: usize) -> Option<&str> {
        self.model.target_of(id)
    }

    /// The current visible projection as the crate's arena nodes (white-box
    /// tests only).
    pub(super) fn projected_nodes(&self) -> Vec<ProjectedNode<usize>> {
        self.state.projection().nodes().to_vec()
    }

    /// Selects a projection row without touching expansion (white-box tests).
    pub(super) fn select_index(&mut self, index: usize) {
        self.state.select_index(Some(index));
    }

    /// Selects an arena node id, loading its ancestor path (white-box tests).
    pub(super) fn select_id_by_arena(&mut self, id: usize) -> bool {
        let selected = self.state.select_by_id(&self.model, &self.query, id);
        if selected {
            self.invalidate();
        }
        selected
    }

    /// Expands an artist root by arena id (white-box tests).
    pub(super) fn expand_root_id(&mut self, id: usize) {
        self.expand_node_id(id);
    }

    /// Expands a node by arena id (white-box tests).
    pub(super) fn expand_node_id(&mut self, id: usize) {
        let parent = self.projected_parent_of(id);
        self.state.set_expanded(id, parent, true);
        self.state.ensure_projection(&self.model, &self.query);
        self.invalidate();
    }

    /// Collapses an artist root by arena id (white-box tests).
    pub(super) fn collapse_root_id(&mut self, id: usize) {
        self.state.set_expanded(id, None, false);
        self.state.ensure_projection(&self.model, &self.query);
        self.invalidate();
    }

    /// Whether an artist root is persistently expanded by arena id (white-box
    /// tests).
    pub(super) fn root_is_expanded_id(&self, id: usize) -> bool {
        self.state.node_is_expanded(id, None)
    }

    /// Stores a mark on an album leaf by arena id (white-box tests).
    pub(super) fn set_marked_id(&mut self, id: usize, marked: bool) -> bool {
        let Some(target) = self.model.target_of_node(id) else {
            return false;
        };
        self.set_marked(&target, marked)
    }

    /// Toggles an album leaf or artist root mark by arena id (white-box tests).
    pub(super) fn toggle_mark_id(&mut self, id: usize) -> bool {
        let Some(target) = self.model.target_of_node(id) else {
            return false;
        };
        self.toggle_mark(&target)
    }

    /// The node's derived mark state by arena id (white-box tests).
    pub(super) fn mark_state_id(&self, id: usize) -> TreeMarkState {
        computed_mark_state(
            &self.model,
            &self.query,
            &self.state,
            &self.selection_order,
            id,
        )
    }

    /// The node's painted title by arena id (white-box tests).
    pub(super) fn title_of_id(&self, id: usize) -> &str {
        self.model.title_of(id)
    }

    /// Latest-completed-render hit resolution into arena ids (white-box tests).
    pub(super) fn hit_node_id(&self, at: Position) -> Option<(usize, usize)> {
        match self.hit_test_row(at)? {
            TreeHit::Row { id, index, .. } => Some((id, index)),
            TreeHit::Header { .. } | TreeHit::VerticalScrollbar | TreeHit::HorizontalScrollbar => {
                None
            }
        }
    }

    /// The crate's raw hit result by arena id (white-box tests).
    pub(super) fn hit_test(&self, position: Position) -> Option<TreeHit<usize>> {
        self.hit_test_row(position)
    }

    /// A visible node's one-line rect by arena id (white-box tests).
    pub(super) fn row_rect_for_id(&self, id: usize) -> Option<Rect> {
        let index = self.state.visible_index_of(id)?;
        self.row_rect_for_index(index)
    }

    /// Replaces the owner-local matching projection by arena ids (white-box
    /// tests).
    pub(super) fn set_filter_matches_id(&mut self, matching: Option<&[usize]>) {
        self.apply_filter_match_ids(matching);
    }
}
