// Included into `music_tree` via `include!` (the module's doc and
// imports live there, beside the split's other parts).

impl MusicTreeBrowser {
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

    /// Whether an album target is in the retained mark order.
    fn is_marked(&self, target: &str) -> bool {
        self.selection_order.iter().any(|item| item == target)
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
        self.sync_manual_marks();
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
    /// D6), so marking one is a no-op. The owner retains membership separately
    /// because the tree dependency's mark set has no insertion order.
    pub(in crate::app) fn set_marked(&mut self, id: usize, marked: bool) -> bool {
        let Some(target) = self.model.target_of(id).map(str::to_owned) else {
            return false;
        };
        let was_marked = self.is_marked(&target);
        if was_marked == marked {
            return false;
        }
        if marked {
            self.selection_order.push(target);
        } else {
            self.selection_order.retain(|item| item != &target);
        }
        self.sync_manual_marks();
        true
    }

    /// Toggles an album leaf, or all currently visible album descendants of an
    /// artist root. The caller must resolve the target from the latest paint
    /// before invoking this operation; this method only mutates stable node
    /// identities.
    pub(in crate::app) fn toggle_mark(&mut self, id: usize) -> bool {
        if let Some(target) = self.model.target_of(id).map(str::to_owned) {
            let marked = self.is_marked(&target);
            return self.set_marked(id, !marked);
        }
        if !self.model.is_artist(id) {
            return false;
        }
        let descendants = self.visible_album_ids(id);
        if descendants.is_empty() {
            return false;
        }
        let all_marked = descendants.iter().all(|child| {
            self.model
                .target_of(*child)
                .is_some_and(|target| self.is_marked(target))
        });
        let mut changed = false;
        for child in descendants {
            changed |= self.set_marked(child, !all_marked);
        }
        changed
    }

    /// Resolves a modified click against the latest completed tree frame,
    /// selects the clicked node, and only then toggles its album/root mark.
    /// Resolution is deliberately complete before any local mutation.
    pub(in crate::app) fn toggle_mark_at(&mut self, at: Position) -> Option<usize> {
        let (id, index) = self.hit_node(at)?;
        self.state.select_index(Some(index));
        self.toggle_mark(id);
        Some(id)
    }

    /// Album targets selected in insertion order. Hidden marks are masked while
    /// a filter is active but remain in `selection_order` for dismissal.
    pub(in crate::app) fn selected_album_targets(&self) -> Vec<String> {
        self.selection_order
            .iter()
            .filter(|target| {
                self.model
                    .node_id(&MusicNodeKey::Album((*target).clone()))
                    .is_some_and(|id| self.album_is_visible(id))
            })
            .cloned()
            .collect()
    }

    /// Album targets selected in the current settled tree display order. This
    /// is the order boundary-crossing actions should use, independent of click
    /// order.
    pub(in crate::app) fn selected_album_targets_in_display_order(&self) -> Vec<String> {
        self.model
            .roots
            .iter()
            .flat_map(|root| self.model.children[*root].iter())
            .copied()
            .filter(|id| self.album_is_visible(*id))
            .filter_map(|id| self.model.target_of(id))
            .filter(|target| self.is_marked(target))
            .map(str::to_owned)
            .collect()
    }

    /// The node's derived mark state (the tree render tests locate rows by
    /// aggregate state rather than hard-coded colours).
    #[cfg(test)]
    pub(in crate::app) fn mark_state(&self, id: usize) -> TreeMarkState {
        computed_mark_state(
            &self.model,
            &self.query,
            &self.state,
            &self.selection_order,
            id,
        )
    }

    /// The node's painted title (the tree render tests locate rows by title
    /// rather than by hard-coded settled sort positions).
    #[cfg(test)]
    pub(in crate::app) fn title_of(&self, id: usize) -> &str {
        self.model.title_of(id)
    }

    /// The album leaf's stable target (the identity that crosses to the
    /// shell); artist roots have none.
    pub(in crate::app) fn target_of(&self, id: usize) -> Option<&str> {
        self.model.target_of(id)
    }

    /// Whether a hit-tested node is an artist root. This keeps context
    /// resolution on the tree owner rather than exposing its model to the
    /// mounted destination.
    pub(in crate::app) fn model_is_artist(&self, id: usize) -> bool {
        self.model.is_artist(id)
    }

    /// Resolves an artist root's currently visible album descendants in tree
    /// order for context-hit membership checks.
    pub(in crate::app) fn artist_album_targets(&self, root: usize) -> Vec<String> {
        self.visible_album_ids(root)
            .into_iter()
            .filter_map(|id| self.model.target_of(id).map(str::to_owned))
            .collect()
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
        self.select_id(id)
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
        if !self.paint_complete || self.selected_is_artist() || self.filter_active {
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

    /// The painted viewport's deepest album target with the viewport height
    /// in rows — the source-pagination hint the shell arms the next artist
    /// page from (design D3). Derived from the **latest completed paint**, so
    /// scrolling the viewport alone advances pagination exactly like a
    /// selection move; the near-edge margin and page size are the shell's
    /// decision, floored at this height so one fetch always fills the visible
    /// list. `None` when no paint completed or no album leaf is visible.
    pub(in crate::app) fn painted_edge_album_page(&self) -> Option<(String, usize)> {
        if !self.paint_complete {
            return None;
        }
        let nodes = self.state.projection().nodes();
        let start = self.state.offset();
        let height = self.last_area?.height as usize;
        let end = start.saturating_add(height).min(nodes.len());
        nodes[start..end]
            .iter()
            .rev()
            .find_map(|node| self.model.target_of(node.id()).map(str::to_owned))
            .map(|target| (target, height.max(1)))
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
        if self.selection_order.is_empty() {
            return;
        }
        self.selection_order.clear();
        self.sync_manual_marks();
    }

    /// Returns the album leaves currently visible beneath an artist root. An
    /// active filter masks hidden descendants without changing stored marks.
    fn visible_album_ids(&self, root: usize) -> Vec<usize> {
        self.model.children[root]
            .iter()
            .copied()
            .filter(|id| self.model.target_of(*id).is_some())
            .filter(|id| self.album_is_visible(*id))
            .collect()
    }

    fn album_is_visible(&self, id: usize) -> bool {
        if matches!(self.query.filter_config(), TreeFilterConfig::Disabled) {
            return true;
        }
        self.state
            .projection()
            .nodes()
            .iter()
            .any(|node| node.id() == id)
    }

    /// Rebuilds the crate's mark cache from the owner selection. During a
    /// filter session only visible album marks are handed to the dependency;
    /// hidden membership remains in `selection_order` and is restored when the
    /// filter is dismissed.
    fn sync_manual_marks(&mut self) {
        self.state.clear_marks();
        for target in &self.selection_order {
            let Some(id) = self.model.node_id(&MusicNodeKey::Album(target.clone())) else {
                continue;
            };
            if self.album_is_visible(id) {
                self.state.set_marked(id, true);
            }
        }
        self.state.ensure_mark_states(&self.model);
    }
}
