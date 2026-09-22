// Included into `music_tree` via `include!` (the module's doc and
// imports live there, beside the split's other parts).

/// their text to the selected-row Ink role.
struct MusicTreeLabelRenderer<'a> {
    tree_col_width: u16,
    left_text_inset: usize,
    right_text_inset: usize,
    row_background: ratatui::style::Color,
    zebra_fill: ratatui::style::Color,
    /// The focused selected row's marquee spans for this frame (`None` when
    /// unfocused or nothing is selected). Computed in `view` so the renderer
    /// itself can stay `&self` inside the crate's trait signature.
    selected_title_spans: Option<Vec<Span<'static>>>,
    selected_title_budget: usize,
    visible_mark_states: HashMap<MusicTreeTarget, TreeMarkState>,
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
        let mark = model
            .target_of_node(id)
            .and_then(|target| self.visible_mark_states.get(&target).copied())
            .unwrap_or(TreeMarkState::Unmarked);
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
                .saturating_sub(self.left_text_inset)
                .saturating_sub(self.right_text_inset)
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
            // two excess leading columns from album rows and two more from
            // track rows. The glyphs remain crate-owned; only this label
            // composition changes.
            let mut remaining = if context.level == 2 { 4 } else { 2 };
            for prefix in line.spans.iter_mut().take(context.level) {
                let removed = remaining.min(prefix.content.chars().count());
                prefix.content = prefix
                    .content
                    .chars()
                    .skip(removed)
                    .collect::<String>()
                    .into();
                remaining -= removed;
                if remaining == 0 {
                    break;
                }
            }
        }
        // The tree widget paints the full claim rectangle so row fills bleed
        // through the parent panel's side pads. Keep the text at the inset
        // content rectangle by adding only the parent-provided left inset to
        // the label line.
        if self.left_text_inset > 0 {
            line.spans
                .insert(0, Span::raw(" ".repeat(self.left_text_inset)));
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
                Style::default().fg(name_role(model, id, context.level, mark)),
            ));
        }

        // Pad the name slot to its budget, then paint the album year once in
        // the right-aligned fixed six-column gutter at the row's right edge in
        // the green status role, followed by its two-column trailing gap. A
        // yearless row appends nothing, so its title keeps the full width (the
        // pinned gutter contract).
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
        // Every selected bar resolves its row content to Ink, including a
        // playback-live title; the equivalent row highlight from the crate is
        // kept as a second line of defence for cells the label does not own.
        if (context.render.is_selected && self.selected_title_spans.is_some())
            || multi_select_bar(model, id, mark)
        {
            for span in line.spans.iter_mut().skip(composed) {
                span.style = span.style.fg(palette::SELECTED_ROW_FG);
            }
        }
        let mut style = Style::default();
        if multi_select_bar(model, id, mark) {
            style = style
                .bg(palette::SELECTED_ROW_BG)
                .fg(palette::SELECTED_ROW_FG);
        } else {
            // Match the Queue's row palette, while keeping zebra bands inside
            // the panel's two-column text insets. The selected bar is the one
            // deliberate full-bleed exception and is applied as the Cell
            // style above.
            let row_fill = if model.is_striped(id) {
                self.zebra_fill
            } else {
                self.row_background
            };
            let first_painted_span = usize::from(self.left_text_inset > 0);
            for span in line.spans.iter_mut().skip(first_painted_span) {
                span.style = span.style.bg(row_fill);
            }
            let painted_width: usize = line.spans[first_painted_span..]
                .iter()
                .map(|span| span.content.width())
                .sum();
            let target_width = (self.tree_col_width as usize).saturating_sub(self.right_text_inset);
            if target_width > painted_width {
                line.spans.push(Span::styled(
                    " ".repeat(target_width - painted_width),
                    Style::default().bg(row_fill),
                ));
            }
        }
        Cell::from(line).style(style)
    }
}

/// field; music rows are always ordinary unless playback is live.
/// The row's name role, resolved only from semantic inputs: the crate's mark
/// state, the row's hierarchy level, and an album leaf's playback-live
/// [`MediaSemanticState`]. Nothing here reads a raw `EmbyItem` played/resume
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
        TreeMarkState::Unmarked => {
            let ordinary = match level {
                1 => palette::TEXT_FOCUS_ACCENT,
                _ => palette::ACCENT,
            };
            model
                .semantic_state_of(id)
                .map_or(ordinary, |state| semantic_role(state, ordinary))
        }
    }
}

/// The playback-live semantic palette applied to a tree row. Music rows
/// otherwise keep their ordinary depth colour; active/now-playing rows retain
/// emphasis without an inline progress slot.
fn semantic_role(
    state: &MediaSemanticState,
    ordinary: ratatui::style::Color,
) -> ratatui::style::Color {
    if matches!(
        state,
        MediaSemanticState::Active { .. } | MediaSemanticState::NowPlaying { .. }
    ) {
        palette::TEXT_EMPHASIS
    } else {
        // `Played` is normalized out of the arena, but the fallback remains
        // the row's ordinary depth colour if a future caller supplies another
        // non-live state.
        ordinary
    }
}
