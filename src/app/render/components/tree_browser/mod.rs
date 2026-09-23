//! Destination-neutral painting for the embedded [`TreeBrowser`].
//!
//! This module receives semantic row data only.  It owns indentation, depth
//! roles, selected and aggregate bars, zebra fills, trailing metadata, and
//! the selected-title marquee.

use std::time::Instant;

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::components::list::tree_browser::{
    TreeAggregateMark, TreePaintRow, TreePaintRowKind, TreeTitleRole,
};
use crate::app::components::media_list::MediaSemanticState;
use crate::app::palette;
use crate::app::render::components::marquee::marquee_spans;

/// The pinned trailing-metadata slot (design D8): a fixed six-column
/// right-aligned date cell, plus the two-column gap between it and the row's
/// trailing edge. Reserved only on rows that carry trailing metadata, so a
/// metadata-free row's title keeps the full width.
pub(in crate::app) const TREE_METADATA_SLOT_WIDTH: usize = 6;
pub(in crate::app) const TREE_METADATA_TRAILING_SPACE: usize = 2;

/// The rows that paint the canonical full-width bar: the focused selected row
/// and directly marked leaves. Aggregate states resolve title roles, not bars
/// (design D5's aggregate-mark roles). The retained hit geometry mirrors this
/// predicate, so it stays one paint-policy decision owned here.
pub(in crate::app) fn tree_row_is_full_width(selected: bool, marked: bool) -> bool {
    selected || marked
}

/// The columns a row's trailing metadata reserves inside the content area.
pub(in crate::app) fn tree_metadata_gutter_width(trailing: &str) -> usize {
    trailing
        .width()
        .max(TREE_METADATA_SLOT_WIDTH)
        .saturating_add(TREE_METADATA_TRAILING_SPACE)
}

#[allow(dead_code)]
pub(in crate::app) fn render_tree_browser(
    frame: &mut Frame,
    claim_rect: Rect,
    content_rect: Rect,
    rows: &[TreePaintRow],
    total_rows: usize,
    viewport_offset: usize,
    focused: bool,
    marquee_text: &mut String,
    marquee_started_at: &mut Instant,
) {
    let zebra = palette::surface_colors(palette::Surface::QueueColumn, focused).fill;
    let base = palette::surface_colors(palette::Surface::QueuePanel, focused).fill;
    let left_inset = usize::from(content_rect.x.saturating_sub(claim_rect.x));
    let right_inset = usize::from(claim_rect.right().saturating_sub(content_rect.right()));
    let content_width = usize::from(content_rect.width);
    let mut group_root_index = 0;

    for (index, row) in rows.iter().enumerate() {
        if row.kind == TreePaintRowKind::Heading {
            group_root_index = row.root_index;
        }
        let Some(y) = content_rect.y.checked_add(index as u16) else {
            break;
        };
        if y >= content_rect.bottom() || content_width == 0 {
            break;
        }
        // Selected and directly marked rows claim the complete panel width.
        // Ordinary rows remain inside the parent's text insets, so the
        // panel—not a destination—owns the side bands around a tree. The bar
        // is the focused selected row's canonical claim; aggregate states
        // resolve title roles, not bars (design D5).
        let full_width = tree_row_is_full_width(focused && row.selected, row.marked);
        let row_area = if full_width {
            Rect::new(claim_rect.x, y, claim_rect.width, 1)
        } else {
            Rect::new(content_rect.x, y, content_rect.width, 1)
        };
        let fill = if full_width {
            palette::SELECTED_ROW_BG
        } else if row.root_index.saturating_sub(group_root_index) % 2 == 0 {
            zebra
        } else {
            base
        };
        let (prefix, title) = match row.kind {
            TreePaintRowKind::Node => (" ".repeat(row.depth.saturating_mul(2)), row.title.clone()),
            TreePaintRowKind::Heading => (String::new(), row.title.to_uppercase()),
            TreePaintRowKind::Spacer => (String::new(), String::new()),
        };
        let gutter = row
            .trailing
            .as_deref()
            .map_or(0, tree_metadata_gutter_width);
        let budget = content_width.saturating_sub(prefix.width() + gutter);
        let title_color = if full_width {
            palette::SELECTED_ROW_FG
        } else if row.kind == TreePaintRowKind::Heading {
            palette::TEXT_METADATA
        } else {
            title_color(row)
        };
        let mut spans = if full_width && left_inset > 0 {
            vec![Span::raw(" ".repeat(left_inset))]
        } else {
            Vec::new()
        };
        spans.push(Span::raw(prefix));
        if focused && row.selected {
            spans.extend(marquee_spans(
                &title,
                &[(title.as_str(), title_color)],
                budget,
                marquee_text,
                marquee_started_at,
                false,
            ));
        } else if row.kind != TreePaintRowKind::Spacer {
            let mut title_style = Style::default().fg(title_color);
            if row.kind == TreePaintRowKind::Heading {
                title_style = title_style.add_modifier(ratatui::style::Modifier::BOLD);
            }
            spans.push(Span::styled(
                crate::app::ui_util::trunc_str(&title, budget),
                title_style,
            ));
        }
        let painted = spans.iter().map(|span| span.content.width()).sum::<usize>();
        let target_width = content_width
            .saturating_sub(gutter)
            .saturating_add(if full_width { left_inset } else { 0 });
        if painted < target_width {
            spans.push(Span::raw(" ".repeat(target_width - painted)));
        }
        if let Some(trailing) = &row.trailing {
            spans.push(Span::styled(
                format!(
                    "{:>width$}{}",
                    trailing,
                    " ".repeat(TREE_METADATA_TRAILING_SPACE),
                    width = trailing.width().max(TREE_METADATA_SLOT_WIDTH)
                ),
                Style::default().fg(if full_width {
                    palette::SELECTED_ROW_FG
                } else {
                    palette::STATUS_AVAILABLE
                }),
            ));
        }
        if full_width && right_inset > 0 {
            spans.push(Span::raw(" ".repeat(right_inset)));
        }
        let mut style = Style::default().bg(fill);
        if full_width {
            style = style.fg(palette::SELECTED_ROW_FG);
        }
        frame.render_widget(Paragraph::new(Line::from(spans)).style(style), row_area);
    }

    // The tree uses the same focus-gated scrollbar policy as the flat list.
    if focused && total_rows > usize::from(content_rect.height) && claim_rect.width > 0 {
        crate::app::render::components::widgets::render_right_scrollbar_with_viewport(
            frame,
            claim_rect,
            total_rows,
            usize::from(content_rect.height),
            viewport_offset,
            palette::SCROLLBAR,
        );
    }
}

#[allow(dead_code)]
fn title_color(row: &TreePaintRow) -> ratatui::style::Color {
    // Directly marked rows paint the full-width bar and never reach this
    // role; aggregate states keep their own title roles (design D5).
    match row.aggregate_mark {
        TreeAggregateMark::Partial => palette::TEXT_ACCENT_MUTED,
        TreeAggregateMark::Full => palette::STATUS_AVAILABLE,
        TreeAggregateMark::None => {
            if matches!(
                row.semantic_state,
                MediaSemanticState::Active { .. } | MediaSemanticState::NowPlaying { .. }
            ) {
                palette::TEXT_EMPHASIS
            } else {
                match row.title_role {
                    TreeTitleRole::Heading => palette::TEXT_EMPHASIS,
                    TreeTitleRole::Secondary => palette::TEXT_FOCUS_ACCENT,
                    TreeTitleRole::Standard => match row.depth {
                        0 => palette::TEXT_EMPHASIS,
                        1 => palette::TEXT_FOCUS_ACCENT,
                        _ => palette::ACCENT,
                    },
                }
            }
        }
    }
}
