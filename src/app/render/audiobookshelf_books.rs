use super::hero::{
    self, hero_block_shell, top_hero_layout, HERO_BLOCK_EXTRA_ROWS, HERO_TITLE_ROWS,
};
use super::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::images::audiobookshelf_book_cover_cache_key;
use crate::app::layout::LayoutMain;
use crate::app::types_audiobookshelf_browse::{AudiobookshelfBookBrowseState, BookRow};
use crate::app::ui_util::{fmt_duration_approx, trunc_str};
use crate::app::{palette, App, TWO_COLUMN_THRESHOLD};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

/// Padding inside recessed wide-book blocks, matching Music's wide layout
/// (`music_wide.rs::PANE_PAD_X`/`PANE_PAD_Y`) -- duplicated rather than
/// shared per this change's design (two consumers isn't yet a strong case
/// for extraction).
pub(super) const PANE_PAD_X: u16 = 2;
pub(super) const PANE_PAD_Y: u16 = 1;
/// Blank row separating the hero from the chapter list in the wide left pane.
const LEFT_SEPARATOR_ROWS: u16 = 1;
/// Height of the bucket-pill row inside the narrow right pane. The wide right
/// pane's pill row height comes from `hero::hero_on_left_right_pane` instead
/// (phase 6, "Adopt: Home and audiobooks").
pub(super) const PILLS_ROW_HEIGHT: u16 = 1;
/// Blank rows below the pills before the book list starts, narrow-pane only
/// (see `PILLS_ROW_HEIGHT`).
pub(super) const PILLS_GAP_ROWS: u16 = 1;

/// The book tab's persistent list renders one row per chapter from the
/// selected book's `chapters[]` (or its `audioFiles` when `chapters[]` is
/// empty). The right pane is the alphabetical-bucket-filtered book browser.
/// Both panes are always visible (book-browsing spec: "Book libraries use
/// the Music tab composition").
impl App {
    pub(super) fn render_audiobookshelf_books(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        layout.audiobookshelf_book_right_area = Rect::default();
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_book_browse.get(index).cloned() else {
            super::render_placeholder(f, area, "Audiobookshelf loading…");
            return;
        };
        if state.books.is_empty() {
            super::render_placeholder(
                f,
                area,
                state
                    .error
                    .as_deref()
                    .unwrap_or(if state.loading_pages.is_empty() {
                        "No audiobooks"
                    } else {
                        "Loading audiobooks…"
                    }),
            );
            return;
        }

        let chapters_focused = state.chapter_selection.is_some();
        let left_focused = focused && chapters_focused;
        let right_focused = focused && !chapters_focused;

        if area.width >= TWO_COLUMN_THRESHOLD
            && area.height.saturating_sub(1) >= hero::HERO_ON_LEFT_MIN_AREA_HEIGHT
        {
            self.render_wide_audiobookshelf_books(
                f,
                area,
                index,
                &state,
                left_focused,
                right_focused,
                layout,
            );
            return;
        }
        self.render_narrow_audiobookshelf_books(
            f,
            area,
            index,
            &state,
            left_focused,
            right_focused,
            layout,
        );
    }

    fn render_wide_audiobookshelf_books(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        state: &AudiobookshelfBookBrowseState,
        left_focused: bool,
        right_focused: bool,
        layout: &mut LayoutMain,
    ) {
        let content_area = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };
        let (mut left_panel, right_panel) = hero::hero_on_left_panes(area);
        left_panel.height = content_area.height;

        // Library-side separator row below the left pane, matching Music.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
            Rect {
                x: left_panel.x,
                y: left_panel.bottom(),
                width: left_panel.width,
                height: 1,
            },
        );

        let left_area = inset_pane_vertically(left_panel);
        let right_area = inset_pane_vertically(right_panel);

        let left_bg = palette::resolve_surface_focus(left_focused);
        f.render_widget(
            Block::default().style(Style::default().bg(left_bg)),
            left_panel,
        );

        let hero_content_area = Rect {
            x: left_area.x.saturating_add(PANE_PAD_X),
            width: left_area.width.saturating_sub(PANE_PAD_X * 2),
            ..left_area
        };
        let hero_rows_wanted = self.audiobookshelf_book_hero_rows(state) + 1; // +1 trailing blank
        let sep = if hero_content_area.height > hero_rows_wanted + LEFT_SEPARATOR_ROWS {
            LEFT_SEPARATOR_ROWS
        } else {
            0
        };
        let hero_h = hero_rows_wanted.min(hero_content_area.height.saturating_sub(sep));
        let hero_area = Rect {
            height: hero_h,
            ..hero_content_area
        };
        let chapters_area = Rect {
            y: hero_content_area.y + hero_h + sep,
            height: hero_content_area.height.saturating_sub(hero_h + sep),
            ..hero_content_area
        };

        layout.left_area = left_area;
        layout.hero_area = hero_area;
        if hero_area.width > 0 && hero_area.height > 0 {
            self.render_audiobookshelf_book_hero(f, hero_area, index, left_focused, layout);
        }
        if chapters_area.width > 0 && chapters_area.height > 0 {
            self.render_audiobookshelf_book_rows(f, chapters_area, state, left_focused, layout);
        }

        layout.audiobookshelf_book_right_area = right_area;
        self.render_audiobookshelf_book_right_pane_wide(
            f,
            right_panel,
            right_area,
            index,
            right_focused,
            layout,
        );
    }

    fn render_narrow_audiobookshelf_books(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        state: &AudiobookshelfBookBrowseState,
        left_focused: bool,
        right_focused: bool,
        layout: &mut LayoutMain,
    ) {
        let desired_rows = self.audiobookshelf_book_hero_rows(state) + HERO_BLOCK_EXTRA_ROWS;
        let top = top_hero_layout(area, desired_rows, false);
        layout.hero_area = top.hero_area;
        if top.hero_rows > 0 {
            hero_block_shell(f, top.hero_area, top.hero_rows, left_focused);
            let content = Rect {
                x: top.hero_area.x + SELECTED_BLOCK_SIDE_PADDING,
                y: top.hero_area.y + 2,
                width: top
                    .hero_area
                    .width
                    .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                height: top.hero_rows - HERO_BLOCK_EXTRA_ROWS,
            };
            self.render_audiobookshelf_book_hero(f, content, index, left_focused, layout);
        }

        // The remaining area below the hero stacks the chapter list (the
        // left pane's persistent list) above the bucket-pill row and the
        // book browser (the right pane), since a narrow terminal has no
        // room for a horizontal split -- both stay rendered per the
        // book-browsing spec's narrow-terminal fallback.
        let remaining = top.list_area;
        let chapters_h = (remaining.height as u32 * 2 / 5) as u16;
        let chapters_area = Rect {
            height: chapters_h,
            ..remaining
        };
        let browser_area = Rect {
            y: remaining.y + chapters_h,
            height: remaining.height.saturating_sub(chapters_h),
            ..remaining
        };
        layout.left_area = chapters_area;
        if chapters_area.height > 0 {
            self.render_audiobookshelf_book_rows(f, chapters_area, state, left_focused, layout);
        }

        layout.audiobookshelf_book_right_area = browser_area;
        self.render_audiobookshelf_book_right_pane_narrow(
            f,
            browser_area,
            index,
            right_focused,
            layout,
        );
    }

    fn audiobookshelf_book_hero_rows(&self, state: &AudiobookshelfBookBrowseState) -> u16 {
        let mut rows = HERO_TITLE_ROWS;
        let book = state.selected_book();
        rows += book.and_then(|book| book.author_display.as_ref()).is_some() as u16;
        rows += 1; // progress row
        rows
    }

    fn render_audiobookshelf_book_hero(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        let Some(state) = self.audiobookshelf_book_browse.get(index).cloned() else {
            return;
        };
        let Some(book) = state.selected_book().cloned() else {
            return;
        };
        let max_y = area.y + area.height;
        let mut row = area.y;
        row = super::hero::render_hero_title_row(
            f,
            area.x,
            row,
            max_y,
            area.width.saturating_sub(1),
            &book.title,
            focused,
        );

        let server_url = self
            .config
            .lock()
            .unwrap()
            .audiobookshelf_setup
            .as_ref()
            .map(|setup| setup.server_url.clone());
        let image_key = server_url.as_ref().map(|server| {
            audiobookshelf_book_cover_cache_key(
                server,
                &book.library_item_id,
                self.current_protocol_suffix(),
            )
        });
        if let Some(server) = server_url {
            self.fetch_audiobookshelf_book_cover(server, book.library_item_id.clone());
        }
        let image_height = image_key
            .as_ref()
            .and_then(|key| self.cached_image_protocol_mut(key))
            .and_then(|image| {
                image
                    .size_for(
                        ratatui_image::Resize::Scale(Some(crate::app::render::RENDER_FILTER)),
                        ratatui::layout::Size {
                            width: super::detail_series::SERIES_IMAGE_COLS,
                            height: super::detail_series::SERIES_IMAGE_ROWS,
                        },
                    )
                    .map(|size| size.height)
            })
            .unwrap_or(0);

        if let Some(author) = book.author_display.as_deref().filter(|a| !a.is_empty()) {
            if row < max_y {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        trunc_str(author, area.width as usize),
                        Style::default().fg(palette::FOAM),
                    )),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                );
                row += 1;
            }
        }
        if row < max_y {
            let progress = state.progress.get(&book.library_item_id);
            let span = match progress {
                Some(progress) if progress.is_finished => Span::styled(
                    "Finished",
                    Style::default()
                        .fg(palette::FOAM)
                        .add_modifier(Modifier::BOLD),
                ),
                Some(progress) if progress.current_time_seconds > 0.0 => {
                    let total = state
                        .detail_cache
                        .get(&book.library_item_id)
                        .map(|(_, files)| files.iter().map(|file| file.duration).sum::<f64>())
                        .filter(|t| *t > 0.0);
                    let pct = total.map(|total| {
                        ((progress.current_time_seconds * 100.0 / total).floor() as u8).clamp(1, 99)
                    });
                    Span::styled(
                        pct.map(|pct| format!("{pct}%")).unwrap_or_default(),
                        Style::default().fg(palette::FOAM),
                    )
                }
                _ => Span::styled("Not started", Style::default().fg(palette::SUBTLE)),
            };
            f.render_widget(
                Paragraph::new(Line::from(vec![span])),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }
        if image_height > 0 {
            let image_rect = Rect {
                x: area.x
                    + area
                        .width
                        .saturating_sub(super::detail_series::SERIES_IMAGE_COLS),
                y: row.saturating_sub(1),
                width: super::detail_series::SERIES_IMAGE_COLS,
                height: image_height,
            };
            layout.inline_image_rect = Some(image_rect);
        }
    }

    /// Chapter (or audioFiles) rows for the selected book: the persistent
    /// list area's provider-native content, always rendered below the hero
    /// (Music's track-list analog; book-browsing spec: "Chapters render as
    /// first-class rows in the persistent list").
    fn render_audiobookshelf_book_rows(
        &self,
        f: &mut Frame,
        area: Rect,
        state: &AudiobookshelfBookBrowseState,
        focused: bool,
        _layout: &mut LayoutMain,
    ) {
        let Some(id) = state.selected_id.as_deref() else {
            return;
        };
        if state.detail_loading {
            super::render_placeholder(f, area, " Loading…");
            return;
        }
        let rows = state.visible_rows(id);
        if rows.is_empty() {
            super::render_placeholder(f, area, " No chapters available");
            return;
        }
        let show_length = area.width > 40;
        let duration_width = if show_length { 7 } else { 0 };
        let title_width = (area.width as usize)
            .saturating_sub(1 + if show_length { duration_width + 1 } else { 0 });
        let table_rows = rows
            .iter()
            .enumerate()
            .map(|(row_index, row)| {
                let selected = state.chapter_selection == Some(row_index);
                let style = if selected && focused {
                    Style::default().fg(palette::YELLOW)
                } else if focused {
                    Style::default().fg(palette::WHITE)
                } else {
                    Style::default().fg(palette::SUBTLE)
                };
                let marker = super::selection_marker(selected, super::MarkerEdge::Left);
                let (title, duration) = match row {
                    BookRow::Chapter { title, end, start } => {
                        let seconds = (*end - *start).max(0.0) as i64;
                        (
                            title.clone(),
                            if seconds > 0 {
                                fmt_duration_approx(seconds)
                            } else {
                                "—".to_string()
                            },
                        )
                    }
                    BookRow::AudioFile { index, duration } => (
                        format!("Part {index}"),
                        fmt_duration_approx(*duration as i64),
                    ),
                };
                let title = trunc_str(&title, title_width);
                let title_cell = Cell::from(Line::from(vec![marker, Span::raw(title)]));
                if show_length {
                    Row::new([title_cell, Cell::from(duration), Cell::from("")]).style(style)
                } else {
                    Row::new([title_cell, Cell::from(""), Cell::from("")]).style(style)
                }
            })
            .collect::<Vec<_>>();
        let mut table_state = TableState::default();
        table_state.select(state.chapter_selection);
        f.render_stateful_widget(
            Table::new(
                table_rows,
                [
                    Constraint::Min(10),
                    Constraint::Length(duration_width as u16),
                    Constraint::Length(1),
                ],
            )
            .column_spacing(1),
            area,
            &mut table_state,
        );
    }
}

fn inset_pane_vertically(area: Rect) -> Rect {
    Rect {
        y: area.y.saturating_add(PANE_PAD_Y),
        height: area.height.saturating_sub(PANE_PAD_Y * 2),
        ..area
    }
}
