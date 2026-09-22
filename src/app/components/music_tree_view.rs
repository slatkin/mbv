// Included into `music_tree` via `include!` (the module's doc and
// imports live there, beside the split's other parts).

/// Whether a row paints the selected-row bar as part of the tree's
/// multi-selection. Album leaves are the only stored multi-selection
/// identities; an artist root is the Heading-equivalent grouping row, keeps
/// its surface fill, and shows its aggregate state through its name role
/// alone.
fn multi_select_bar(model: &MusicTreeModel, id: usize, mark: TreeMarkState) -> bool {
    mark == TreeMarkState::Marked && model.target_of(id).is_some()
}

/// Derives marks from the owner selection rather than the crate's full-model
/// aggregation. This keeps hidden album marks out of a filtered root's
/// Partial/Marked state while preserving them in the mark carrier.
fn computed_mark_state(
    model: &MusicTreeModel,
    query: &TreeQuery<MusicTreeFilter>,
    state: &TreeListViewState<usize>,
    marks: &[MusicTreeTarget],
    id: usize,
) -> TreeMarkState {
    let visible = |candidate| {
        matches!(query.filter_config(), TreeFilterConfig::Disabled)
            || state
                .projection()
                .nodes()
                .iter()
                .any(|node| node.id() == candidate)
    };
    let marked = |candidate| {
        model
            .target_ref_of_node(candidate)
            .is_some_and(|target| marks.iter().any(|item| item == target))
    };
    if model.target_of(id).is_some() {
        return if visible(id) && marked(id) {
            TreeMarkState::Marked
        } else {
            TreeMarkState::Unmarked
        };
    }
    let Some(children) = model.is_artist(id).then(|| &model.children[id]) else {
        return TreeMarkState::Unmarked;
    };
    let children: Vec<usize> = children
        .iter()
        .copied()
        .filter(|child| model.target_of(*child).is_some() && visible(*child))
        .collect();
    let any = children.iter().any(|child| marked(*child));
    let all = !children.is_empty() && children.iter().all(|child| marked(*child));
    if all {
        TreeMarkState::Marked
    } else if any {
        TreeMarkState::Partial
    } else {
        TreeMarkState::Unmarked
    }
}

/// The classified region of a latest-render hit for test assertions: a row
/// carries its stable target, and the non-row regions stay distinguishable
/// without leaking the crate's arena ids.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) enum MusicTreeHit {
    Row(MusicTreeTarget),
    Header,
    VerticalScrollbar,
    HorizontalScrollbar,
}

impl MusicTreeBrowser {
    /// under `at`, if any. A hit row is always interned, so a row the arena no
    /// longer holds is an explicit absent result.
    pub(in crate::app) fn hit_node(&self, at: Position) -> Option<MusicTreeTarget> {
        match self.hit_test_row(at)? {
            TreeHit::Row { id, .. } => self.model.target_of_node(id),
            TreeHit::Header { .. } | TreeHit::VerticalScrollbar | TreeHit::HorizontalScrollbar => {
                None
            }
        }
    }

    /// The classified hit region for render-test assertions: a row carries its
    /// stable target, and the non-row regions stay distinguishable without
    /// leaking the crate's arena ids.
    #[cfg(test)]
    pub(in crate::app) fn hit_region(&self, at: Position) -> Option<MusicTreeHit> {
        match self.hit_test_row(at)? {
            TreeHit::Row { id, .. } => self.model.target_of_node(id).map(MusicTreeHit::Row),
            TreeHit::Header { .. } => Some(MusicTreeHit::Header),
            TreeHit::VerticalScrollbar => Some(MusicTreeHit::VerticalScrollbar),
            TreeHit::HorizontalScrollbar => Some(MusicTreeHit::HorizontalScrollbar),
        }
    }

    /// Latest-completed-render hit resolution into the crate's arena ids, kept
    /// private because the arena index never crosses the seam.
    fn hit_test_row(&self, at: Position) -> Option<TreeHit<usize>> {
        self.paint.is_valid().then(|| self.state.hit_test(at)).flatten()
    }

    /// Moves the selection `delta` visible rows (the tree's own visible-node
    /// movement), clamped at the projection bounds by the shared default.
    pub(in crate::app) fn move_selection(&mut self, delta: i64) {
        let flow = self.row_flow();
        Cursored::move_by(self, &flow, delta as isize);
    }

    /// The tree's established viewport height, from the same geometry the
    /// painter resolves: the parent-configured content rect when the panel has
    /// declared one, otherwise the content rect of the last completed frame.
    /// `None` until one of those exists.
    fn viewport_len(&self) -> Option<usize> {
        self.configured_geometry
            .map(|(_, content_rect)| content_rect.height as usize)
            .or_else(|| self.last_area.map(|area| area.height as usize))
    }

    /// Moves the selection one visible viewport under the shared named
    /// `PagingPolicy::VisibleViewport`, whose page distance is the tree's
    /// established viewport height and which keeps the selection visible. No
    /// geometry means no viewport to page, so this is a deterministic no-op.
    pub(in crate::app) fn page_selection(&mut self, delta: i64) {
        let direction = delta.signum() as isize;
        if direction == 0 {
            return;
        }
        let Some(viewport_len) = self.viewport_len() else {
            return;
        };
        let flow = self.row_flow();
        Viewported::page(
            self,
            &flow,
            viewport_len,
            direction,
            PagingPolicy::VisibleViewport,
        );
    }

    pub(in crate::app) fn select_first_visible(&mut self) {
        let flow = self.row_flow();
        Cursored::first(self, &flow);
    }

    pub(in crate::app) fn select_last_visible(&mut self) {
        let flow = self.row_flow();
        Cursored::last(self, &flow);
    }

    #[cfg(test)]
    fn row_rect_for_index(&self, index: usize) -> Option<Rect> {
        if !self.paint.is_valid() {
            return None;
        }
        raw_row_rect(self.last_area?, self.state.offset(), index)
    }

    /// The selected row's one-line rect from the latest completed view, when
    /// the node is visible (the panel's retained selected-row geometry).
    pub(in crate::app) fn selected_row_rect(&self) -> Option<Rect> {
        PaintRetained::selected_row_rect(self)
    }

    /// A visible node's one-line rect from the latest completed view. A target
    /// absent from the current projection is an explicit absent result.
    #[cfg(test)]
    pub(in crate::app) fn row_rect_for(&self, target: &MusicTreeTarget) -> Option<Rect> {
        let id = self.model.id_of(target)?;
        let index = self.state.visible_index_of(id)?;
        self.row_rect_for_index(index)
    }

    /// Whether the latest completed view's retained geometry claims `at`.
    pub(in crate::app) fn claims_point(&self, at: Position) -> bool {
        PaintRetained::claims_point(self, at)
    }

    /// Arms the crate's `KeepInView` rule for the current selection without
    /// touching expansion (design D3/D4: a geometry or viewport change re-arms
    /// the same owner's visibility rule). Deliberately not `select_by_id`,
    /// whose `expand_to` would promote filter-forced expansion into persistent
    /// expansion on every resize (D5).
    fn rearm_selection_visibility(&mut self) {
        rearm_selection_visibility_for(&mut self.state);
    }

    #[cfg(test)]
    pub(in crate::app) fn projected_node_targets(&self) -> Vec<MusicTreeTarget> {
        self.state
            .projection()
            .nodes()
            .iter()
            .filter_map(|node| self.model.target_of_node(node.id()))
            .collect()
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
        self.state.ensure_projection(&self.model, &self.query);
        let Self {
            model,
            query,
            state,
            focused,
            marquee_key,
            marquee_started,
            configured_geometry,
            last_area,
            marks,
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
        let left_text_inset = content_rect.x.saturating_sub(claim_rect.x) as usize;
        let right_text_inset = claim_rect.right().saturating_sub(content_rect.right()) as usize;

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
            // current projection row; `select_music_target`/`select_by_id` must not be
            // used here because their `expand_to` would promote filter-forced
            // expansion into persistent expansion on every resize (D5).
            rearm_selection_visibility_for(state);
            *last_area = Some(content_rect);
        }
        // `tui-treelistview` resolves the primary column inside the full
        // claim rectangle. The content rectangle still owns the row-flow
        // height and the text's side insets; the extra column keeps the
        // crate's discarded scrollbar outside the claim when there is room.
        let overflow = state.visible_len() > content_rect.height as usize;
        let tree_area = if overflow && paint_area.right() < frame.area().right() {
            Rect {
                width: paint_area.width.saturating_add(1),
                ..paint_area
            }
        } else {
            paint_area
        };
        let tree_col_width = tree_area.width.saturating_sub(u16::from(overflow));
        let text_inset = left_text_inset.saturating_add(right_text_inset);

        // The focused selected row's marquee window, computed once per frame
        // through the shared marquee primitive (design D8). The clock keys on
        // the marqueed title text, exactly like the media-list painter. The
        // window budget matches the renderer's own per-row budget: the
        // six-column date plus trailing gap is reserved only when the selected
        // row carries a year (the pinned gutter contract).
        let selected = state
            .selected_index()
            .and_then(|index| state.projection().nodes().get(index).copied());
        let selected_title_budget = selected.map_or(0, |node| {
            let gutter = usize::from(model.year_of(node.id()).is_some())
                * (YEAR_GUTTER_WIDTH as usize + YEAR_GUTTER_TRAILING_SPACE);
            (tree_col_width as usize)
                .saturating_sub(text_inset)
                .saturating_sub(glyph_prefix_width(node.level()))
                .saturating_sub(gutter)
        });
        let selected_title_spans = selected.filter(|_| *focused).map(|node| {
            let title = model.title_of(node.id());
            let role = name_role(
                model,
                node.id(),
                node.level(),
                computed_mark_state(model, query, state, marks.targets(), node.id()),
            );
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

        let row_background = queue_row_background(*focused);
        let zebra_fill = queue_row_zebra(*focused);
        let visible_mark_states = state
            .projection()
            .nodes()
            .iter()
            .filter_map(|node| {
                let target = model.target_of_node(node.id())?;
                Some((
                    target,
                    computed_mark_state(model, query, state, marks.targets(), node.id()),
                ))
            })
            .collect();
        let label = MusicTreeLabelRenderer {
            tree_col_width,
            left_text_inset,
            right_text_inset,
            row_background,
            zebra_fill,
            selected_title_spans,
            selected_title_budget,
            visible_mark_states,
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
        // The tree crate applies a cell background across its full primary
        // column. Restore the parent buffer in the two side pads for ordinary
        // rows so zebra/base bands stop at the same insets as the Queue list;
        // selected and marked rows intentionally remain full-bleed.
        let selected_row = (*focused).then(|| state.selected_index()).flatten();
        let projection = state.projection().nodes();
        let visible_start = state.offset();
        let visible_end = visible_start.saturating_add(content_rect.height as usize);
        {
            let target = frame.buffer_mut();
            for (projection_row, node) in projection
                .iter()
                .enumerate()
                .skip(visible_start)
                .take(visible_end.saturating_sub(visible_start))
            {
                let full_bleed = selected_row == Some(projection_row)
                    || multi_select_bar(
                        model,
                        node.id(),
                        computed_mark_state(model, query, state, marks.targets(), node.id()),
                    );
                if full_bleed {
                    continue;
                }
                let y = tree_area.y + (projection_row - visible_start) as u16;
                for x in paint_area.x..content_rect.x {
                    if let (Some(source), Some(destination)) = (
                        target.cell(Position { x, y }),
                        tree_buffer.cell_mut(Position { x, y }),
                    ) {
                        destination.clone_from(source);
                    }
                }
                for x in content_rect.right()..paint_area.right() {
                    if let (Some(source), Some(destination)) = (
                        target.cell(Position { x, y }),
                        tree_buffer.cell_mut(Position { x, y }),
                    ) {
                        destination.clone_from(source);
                    }
                }
            }
        }
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
        let retained_selected = state
            .selected_index()
            .and_then(|index| raw_row_rect(content_rect, state.offset(), index));
        let retained_rows: Vec<(Rect, MusicTreeTarget)> = state
            .projection()
            .nodes()
            .iter()
            .enumerate()
            .skip(visible_start)
            .take(visible_end.saturating_sub(visible_start))
            .filter_map(|(projection_row, node)| {
                let target = model.target_ref_of_node(node.id())?.clone();
                let y = content_rect
                    .y
                    .saturating_add((projection_row - visible_start) as u16);
                Some((
                    Rect {
                        x: content_rect.x,
                        y,
                        width: content_rect.width,
                        height: 1,
                    },
                    target,
                ))
            })
            .collect();
        let flow_offset = state.offset();
        PaintRetained::finish(
            self,
            content_rect,
            content_rect,
            flow_offset,
            retained_rows,
            retained_selected,
        );
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

/// The one-line rect a visible projection row occupies in the painted content
/// area, or `None` when the row is outside the viewport. One formula for the
/// shape's retained selected-row geometry and its `row_rect_for` read.
fn raw_row_rect(area: Rect, offset: usize, index: usize) -> Option<Rect> {
    let row = index.checked_sub(offset)?;
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
/// row with the selected-row `SELECTED_ROW_FG` text role (the crate applies it
/// after row cells, so it overrides the zebra and ordinary title roles);
/// hierarchy guides take the muted role via `line_style` and the aggregate
/// mark states take the positive/muted-accent roles. The crate's
/// unconfigurable scrollbar is discarded by `view`; the shared application
/// scrollbar is painted after the tree with the canonical role and glyph set.
fn tree_style(focused: bool) -> tui_treelistview::TreeListViewStyle<'static> {
    use tui_treelistview::{TreeHorizontalScroll, TreeListViewStyle, TreeRowRendering};
    TreeListViewStyle {
        block_style: Style::default(),
        highlight_style: if focused {
            Style::default()
                .bg(palette::SELECTED_ROW_BG)
                .fg(palette::SELECTED_ROW_FG)
        } else {
            Style::default()
        },
        line_style: Style::default().fg(palette::TEXT_MUTED),
        // Mark foregrounds are resolved by the destination label renderer so
        // filtered roots can use their visible-only aggregate state. The
        // crate's row-level mark styles stay neutral rather than reapplying
        // its full-model aggregate over that semantic role.
        marked_style: Style::default(),
        partial_mark_style: Style::default(),
        highlight_symbol: "",
        borders: ratatui::widgets::Borders::NONE,
        column_spacing: 0,
        row_rendering: TreeRowRendering::Virtualized,
        horizontal_scroll: TreeHorizontalScroll::Disabled,
        ..tui_treelistview::TreeListViewStyle::default()
    }
}

