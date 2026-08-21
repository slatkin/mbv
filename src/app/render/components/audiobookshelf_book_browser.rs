use crate::app::layout::{LayoutMain, LibraryRowTarget};
use crate::app::render::arrangements::hero_left::{self, PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::arrangements::padded_rect;
use crate::app::render::components::audiobookshelf_books::{PILLS_GAP_ROWS, PILLS_ROW_HEIGHT};
use crate::app::render::components::hero::{selected_detail_shell, HERO_BLOCK_EXTRA_ROWS};
use crate::app::render::components::list_rows::SELECTED_BLOCK_SIDE_PADDING;
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
    pub(in crate::app::render) fn render_audiobookshelf_book_right_pane_wide(
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

        let right_pane = hero_left::hero_on_left_right_pane(right_panel, right_area, PANE_PAD_Y);
        let pills_area = right_pane.pills_area;
        if pills_area.y + pills_area.height <= right_area.bottom() {
            self.render_audiobookshelf_book_bucket_pills(f, pills_area, index, layout);
        }

        let list_panel = right_pane.list_panel;
        let browser_area = padded_rect(list_panel, PANE_PAD_X, PANE_PAD_Y);
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
                0,
            );
        }
        hero_left::hero_on_left_list_panel_border(f, list_panel, right_focused);
    }

    /// The narrow inline presentation's right pane: no recessed panel
    /// chrome (the pre-redesign narrow book renderer had none either), just
    /// the pill row directly above the single-column book list.
    pub(in crate::app::render) fn render_audiobookshelf_book_right_pane_narrow(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        right_focused: bool,
        layout: &mut LayoutMain,
        detail_rows: u16,
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
                detail_rows,
            );
        }
    }

    /// Renders the alphabetical author-surname-bucket pills (labels from
    /// `state.buckets`, omitting any empty range -- see
    /// `types_audiobookshelf_browse::build_surname_buckets`).
    pub(in crate::app::render) fn render_audiobookshelf_book_bucket_pills(
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
        layout.selector_tabs = crate::app::render::render_pill_bar(
            f,
            row_area,
            crate::app::render::PillBar {
                labels: &labels,
                ids: &ids,
                selected_pos,
                prefix: Some(" \u{2318} "),
            },
        );
    }

    fn render_audiobookshelf_book_browser_rows(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        right_focused: bool,
        layout: &mut LayoutMain,
        detail_rows: u16,
    ) {
        let Some(mut state) = self.audiobookshelf_book_browse.get(index).cloned() else {
            return;
        };
        let Some(bucket) = state.buckets.get(state.selected_bucket).copied() else {
            crate::app::render::render_placeholder(f, area, " (empty)");
            return;
        };
        if bucket.end <= bucket.start {
            crate::app::render::render_placeholder(f, area, " (empty)");
            return;
        }
        let cursor = state.cursor();
        let count = bucket.end - bucket.start;
        let cursor_pos = cursor.saturating_sub(bucket.start).min(count - 1);
        let detail_rows = (detail_rows >= HERO_BLOCK_EXTRA_ROWS + 1)
            .then_some(detail_rows)
            .unwrap_or(0);
        let flow = (detail_rows > 0)
            .then(|| {
                crate::app::render::components::hero::inline_detail_flow(
                    cursor_pos,
                    detail_rows,
                    area.height,
                    state.scroll,
                )
            })
            .flatten();
        let detail_rows = flow.as_ref().map(|_| detail_rows).unwrap_or(0);
        let scroll = flow.map(|flow| flow.offset).unwrap_or_else(|| {
            state.scroll.clamp(
                cursor_pos.saturating_sub(area.height as usize - 1),
                cursor_pos,
            )
        });
        state.scroll = scroll;
        self.audiobookshelf_book_browse[index].scroll = scroll;

        let total_rows = count.saturating_sub((detail_rows > 0) as usize) + detail_rows as usize;
        let right_area = layout.audiobookshelf_book_right_area;
        let row_offset = area.y.saturating_sub(right_area.y) as usize;
        let mut row_targets = vec![None; right_area.height as usize];
        let mut screen_y = 0u16;
        for book_offset in scroll..count {
            if screen_y >= area.height {
                break;
            }
            let book_idx = bucket.start + book_offset;
            let book = &state.books[book_idx];
            let selected = book_idx == cursor;
            if selected && detail_rows > 0 {
                let detail_y = area.y + screen_y;
                if detail_y >= area.bottom() {
                    break;
                }
                let detail_height = detail_rows.min(area.bottom().saturating_sub(detail_y));
                layout.hero_area = Rect {
                    x: area.x,
                    y: detail_y,
                    width: area.width,
                    height: detail_height,
                };
                layout.selected_item_rect = Some(layout.hero_area);
                for offset in 0..detail_height as usize {
                    if let Some(target) =
                        row_targets.get_mut(row_offset + screen_y as usize + offset)
                    {
                        *target = Some(LibraryRowTarget::Book(book_idx));
                    }
                }
                selected_detail_shell(f, layout.hero_area, detail_height, right_focused);
                let hero_rows = self.audiobookshelf_book_hero_rows(&state) + HERO_BLOCK_EXTRA_ROWS;
                let hero_height = hero_rows.min(detail_height);
                if hero_height >= HERO_BLOCK_EXTRA_ROWS {
                    self.render_audiobookshelf_book_hero(
                        f,
                        Rect {
                            x: area.x + SELECTED_BLOCK_SIDE_PADDING,
                            y: detail_y + 2,
                            width: area.width.saturating_sub(SELECTED_BLOCK_SIDE_PADDING * 2),
                            height: hero_height.saturating_sub(HERO_BLOCK_EXTRA_ROWS),
                        },
                        index,
                        right_focused,
                        layout,
                    );
                    let chapters_y = detail_y + hero_height;
                    let chapters_height = detail_height.saturating_sub(hero_height);
                    if chapters_height > 0 {
                        self.render_audiobookshelf_book_rows(
                            f,
                            Rect {
                                x: area.x,
                                y: chapters_y,
                                width: area.width,
                                height: chapters_height,
                            },
                            &state,
                            right_focused,
                            layout,
                        );
                    }
                }
                screen_y += detail_rows;
                continue;
            }
            let row_area = Rect {
                x: area.x,
                y: area.y + screen_y,
                width: area.width,
                height: 1,
            };
            let style = if selected && right_focused {
                Style::default().fg(palette::TEXT_FOCUS_ACCENT)
            } else if right_focused {
                Style::default().fg(palette::TEXT_STRONG)
            } else {
                Style::default().fg(palette::TEXT_SECONDARY)
            };
            if selected && right_focused {
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::SURFACE_FOCUSED)),
                    row_area,
                );
            }
            let marker = crate::app::render::selection_marker(
                selected,
                crate::app::render::MarkerEdge::Left,
            );
            let title = trunc_str(&book.title, area.width.saturating_sub(2) as usize);
            f.render_widget(
                Paragraph::new(Line::from(vec![marker, Span::raw(title)])).style(style),
                row_area,
            );
            if let Some(target) = row_targets.get_mut(row_offset + screen_y as usize) {
                *target = Some(LibraryRowTarget::Book(book_idx));
            }
            screen_y += 1;
        }
        layout.left_row_targets = row_targets;
        if total_rows > area.height as usize && right_focused {
            crate::app::render::render_right_scrollbar(
                f,
                area,
                total_rows.saturating_sub(area.height as usize),
                scroll,
                palette::SCROLLBAR,
            );
        }
    }
}
