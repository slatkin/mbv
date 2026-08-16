use super::audiobookshelf_books::{PANE_PAD_X, PANE_PAD_Y, PILLS_GAP_ROWS, PILLS_ROW_HEIGHT};
use super::hero;
use crate::app::layout::{LayoutMain, LibraryRowTarget};
use crate::app::ui_util::trunc_str;
use crate::app::{palette, App};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

/// The book tab's right pane: alphabetical author-surname-bucket pills
/// (Music's artist-grouping-pills analog) above a persistent, single-column,
/// bucket-filtered book list (Music's album-list-within-artist-filter
/// analog). Reuses `render_pill_bar`/`PillBar` per Music's
/// `render_music_group_pills_row` shape, and the panel chrome (recessed
/// background, `▔`/`▁` border glyphs) `render_wide_music_group` uses for its
/// right rail -- duplicated rather than shared, per this change's design.
impl App {
    pub(super) fn render_audiobookshelf_book_right_pane_wide(
        &mut self,
        f: &mut Frame,
        right_panel: Rect,
        right_area: Rect,
        index: usize,
        right_focused: bool,
        layout: &mut LayoutMain,
    ) {
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
            right_panel,
        );

        let right_pane = hero::hero_on_left_right_pane(right_panel, right_area, PANE_PAD_Y);
        let pills_area = right_pane.pills_area;
        if pills_area.y + pills_area.height <= right_area.bottom() {
            self.render_audiobookshelf_book_bucket_pills(f, pills_area, index, layout);
        }

        let list_panel = right_pane.list_panel;
        let browser_area = Rect {
            x: list_panel.x.saturating_add(PANE_PAD_X),
            y: list_panel.y.saturating_add(PANE_PAD_Y),
            width: list_panel.width.saturating_sub(PANE_PAD_X * 2),
            height: list_panel.height.saturating_sub(PANE_PAD_Y * 2),
        };
        if list_panel.height > 0 {
            let list_bg = palette::resolve_surface_focus(right_focused);
            f.render_widget(
                Block::default().style(Style::default().bg(list_bg)),
                list_panel,
            );
        }
        if browser_area.height > 0 && browser_area.width > 0 {
            self.render_audiobookshelf_book_browser_rows(
                f,
                browser_area,
                index,
                right_focused,
                layout,
            );
        }
        if list_panel.height > 0 {
            super::render_selected_block_borders(
                f,
                list_panel,
                0,
                list_panel.height as usize,
                1,
                (list_panel.height as usize).saturating_sub(2),
            );
        }
    }

    /// The narrow hero-on-top fallback's right pane: no recessed panel
    /// chrome (the pre-redesign narrow book renderer had none either), just
    /// the pill row directly above the single-column book list.
    pub(super) fn render_audiobookshelf_book_right_pane_narrow(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        right_focused: bool,
        layout: &mut LayoutMain,
    ) {
        if area.height == 0 {
            return;
        }
        let pills_area = Rect {
            height: PILLS_ROW_HEIGHT.min(area.height),
            ..area
        };
        self.render_audiobookshelf_book_bucket_pills(f, pills_area, index, layout);
        let list_area = Rect {
            y: area.y + PILLS_ROW_HEIGHT + PILLS_GAP_ROWS,
            height: area
                .height
                .saturating_sub(PILLS_ROW_HEIGHT + PILLS_GAP_ROWS),
            ..area
        };
        if list_area.height > 0 {
            self.render_audiobookshelf_book_browser_rows(
                f,
                list_area,
                index,
                right_focused,
                layout,
            );
        }
    }

    /// Renders the alphabetical author-surname-bucket pills (labels from
    /// `state.buckets`, omitting any empty range -- see
    /// `types_audiobookshelf_browse::build_surname_buckets`).
    fn render_audiobookshelf_book_bucket_pills(
        &mut self,
        f: &mut Frame,
        row_area: Rect,
        index: usize,
        layout: &mut LayoutMain,
    ) {
        let Some(state) = self.audiobookshelf_book_browse.get(index) else {
            layout.selector_tabs = Vec::new();
            return;
        };
        if state.buckets.is_empty() || row_area.width == 0 {
            layout.selector_tabs = Vec::new();
            if row_area.width > 0 {
                f.render_widget(
                    Paragraph::new(Line::from(Span::raw(" ".repeat(row_area.width as usize)))),
                    row_area,
                );
            }
            return;
        }
        let labels: Vec<String> = state.buckets.iter().map(|b| b.label.to_string()).collect();
        let ids: Vec<usize> = (0..labels.len()).collect();
        let selected_pos = state.selected_bucket.min(labels.len().saturating_sub(1));
        layout.selector_tabs = super::render_pill_bar(
            f,
            row_area,
            super::PillBar {
                labels: &labels,
                ids: &ids,
                selected_pos,
                prefix: Some(" \u{2318} "),
            },
        );
    }

    /// The persistent, single-column, bucket-filtered book list (Music's
    /// album-list-within-artist-filter analog, `render_wide_right_album_browser`
    /// shape without header rows -- bucket grouping is already expressed by
    /// the pill row, not by in-list headers).
    fn render_audiobookshelf_book_browser_rows(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        right_focused: bool,
        layout: &mut LayoutMain,
    ) {
        let Some(state) = self.audiobookshelf_book_browse.get(index) else {
            return;
        };
        let Some(bucket) = state.buckets.get(state.selected_bucket).copied() else {
            super::render_placeholder(f, area, " (empty)");
            return;
        };
        if bucket.end <= bucket.start {
            super::render_placeholder(f, area, " (empty)");
            return;
        }
        let cursor = state.cursor();
        let count = bucket.end - bucket.start;
        let cursor_pos = cursor.saturating_sub(bucket.start).min(count - 1);
        let visible = area.height as usize;

        let state = &mut self.audiobookshelf_book_browse[index];
        let lower = (cursor_pos + 1).saturating_sub(visible).min(cursor_pos);
        state.scroll = state.scroll.clamp(lower, cursor_pos);
        let scroll = state.scroll;

        let right_area = layout.audiobookshelf_book_right_area;
        let row_offset = area.y.saturating_sub(right_area.y) as usize;
        let mut row_targets = vec![None; right_area.height as usize];
        for screen_y in 0..visible.min(count.saturating_sub(scroll)) {
            let book_idx = bucket.start + scroll + screen_y;
            let book = &state.books[book_idx];
            let selected = book_idx == cursor;
            let row_area = Rect {
                x: area.x,
                y: area.y + screen_y as u16,
                width: area.width,
                height: 1,
            };
            let style = if selected && right_focused {
                Style::default().fg(palette::YELLOW)
            } else if right_focused {
                Style::default().fg(palette::WHITE)
            } else {
                Style::default().fg(palette::SUBTLE)
            };
            if selected && right_focused {
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::SURFACE_FOCUSED)),
                    row_area,
                );
                layout.cursor_screen_y = Some(row_area.y);
            }
            let marker = super::selection_marker(selected, super::MarkerEdge::Left);
            let title = trunc_str(&book.title, area.width.saturating_sub(2) as usize);
            f.render_widget(
                Paragraph::new(Line::from(vec![marker, Span::raw(title)])).style(style),
                row_area,
            );
            if let Some(target) = row_targets.get_mut(row_offset + screen_y) {
                *target = Some(LibraryRowTarget::Book(book_idx));
            }
        }
        layout.left_row_targets = row_targets;

        if count > visible && right_focused {
            let max_offset = count.saturating_sub(visible);
            super::render_right_scrollbar(f, area, max_offset, scroll, palette::SCROLLBAR);
        }
    }
}
