//! Paint projection: prepare shared render rows and retained pointer targets
//! from the owner's visible flow.

use std::hash::Hash;

use ratatui::layout::Rect;

use crate::list::AggregateMarkState;
use mbv_render::components::tree_browser::{
    TreeAggregateMark, TreePaintRow, TreePaintRowKind, TreeTitleRole,
};
// The full-width bar is paint policy, so its predicate lives with the shared
// tree painter and is imported through the app-level render seam.
use mbv_render::tree_row_is_full_width;

use super::super::StructuralRow;
use super::{TreeBrowser, TreeMarkPolicy, VisibleRow};

impl<Target: Clone + Eq + Hash> TreeBrowser<Target> {
    pub fn visible_rows(&self, visible_rows: &[VisibleRow]) -> Vec<TreePaintRow> {
        let mut rows = Vec::new();
        let mut group_root_index = 0;
        let grouped = self.root_structures.values().any(|structures| {
            structures
                .iter()
                .any(|structure| matches!(structure, StructuralRow::Heading(_)))
        });
        let mut group_item_index = 0;
        for (flow_index, &visible) in visible_rows.iter().enumerate() {
            if let VisibleRow::Structural(root, index) = visible {
                let Some(entry) = self.arena.get(&root) else {
                    continue;
                };
                let Some(structure) = self
                    .root_structures
                    .get(&entry.node.target)
                    .and_then(|structures| structures.get(index))
                else {
                    continue;
                };
                let (kind, title) = match structure {
                    StructuralRow::Heading(title) => {
                        group_root_index = entry.root_index;
                        group_item_index = 0;
                        (TreePaintRowKind::Heading, title.clone())
                    }
                    StructuralRow::Spacer => (TreePaintRowKind::Spacer, String::new()),
                };
                if flow_index >= self.viewport_offset {
                    rows.push(TreePaintRow {
                        kind,
                        title,
                        title_role: TreeTitleRole::Standard,
                        trailing: None,
                        depth: 0,
                        root_index: entry.root_index,
                        group_root_index,
                        zebra_striped: false,
                        selected: false,
                        marked: false,
                        aggregate_mark: TreeAggregateMark::None,
                        semantic_state:
                            mbv_render::components::media_list::MediaSemanticState::Ordinary,
                    });
                }
                continue;
            }
            let VisibleRow::Node(id) = visible else {
                continue;
            };
            let Some(entry) = self.arena.get(&id) else {
                continue;
            };
            let target = &entry.node.target;
            let zebra_striped = if grouped {
                let striped = group_item_index % 2 == 0;
                group_item_index += 1;
                striped
            } else {
                entry.root_index.saturating_sub(group_root_index) % 2 == 0
            };
            if flow_index < self.viewport_offset {
                continue;
            }
            let marked = self.marks.contains(target);
            let aggregate_mark = if entry.node.mark_policy == TreeMarkPolicy::Aggregate {
                match self.aggregate_mark_state_for(target) {
                    AggregateMarkState::Marked => TreeAggregateMark::Full,
                    AggregateMarkState::Partial => TreeAggregateMark::Partial,
                    AggregateMarkState::Unmarked => TreeAggregateMark::None,
                }
            } else {
                TreeAggregateMark::None
            };
            rows.push(TreePaintRow {
                kind: TreePaintRowKind::Node,
                title: entry.node.title.clone(),
                title_role: entry.node.title_role,
                trailing: entry
                    .node
                    .trailing
                    .as_ref()
                    .map(|trailing| trailing.text.clone()),
                depth: entry.depth,
                root_index: entry.root_index,
                group_root_index,
                zebra_striped,
                selected: self.selected.as_ref() == Some(target),
                marked,
                aggregate_mark,
                semantic_state: entry.node.semantic_state.clone(),
            });
        }
        rows
    }

    fn retained_row(
        &self,
        row: VisibleRow,
        index: usize,
        claim_rect: Rect,
        content_rect: Rect,
    ) -> Option<(Rect, Target)> {
        let VisibleRow::Node(id) = row else {
            return None;
        };
        let y = content_rect
            .y
            .checked_add(u16::try_from(index).unwrap_or(u16::MAX))?;
        if y >= content_rect.bottom() {
            return None;
        }
        let entry = self.arena.get(&id)?;
        let target = entry.node.target.clone();
        let marked = self.marks.contains(&target);
        let selected = self.focused && self.selected.as_ref() == Some(&target);
        let full_width = tree_row_is_full_width(selected, marked);
        let rect = if full_width {
            Rect::new(claim_rect.x, y, claim_rect.width, 1)
        } else {
            Rect::new(content_rect.x, y, content_rect.width, 1)
        };
        Some((rect, target))
    }

    pub fn retained_rows(
        &self,
        visible_rows: &[VisibleRow],
        claim_rect: Rect,
        content_rect: Rect,
    ) -> Vec<(Rect, Target)> {
        visible_rows
            .iter()
            .copied()
            .skip(self.viewport_offset)
            .enumerate()
            .filter_map(|(index, row)| self.retained_row(row, index, claim_rect, content_rect))
            .collect()
    }
}
