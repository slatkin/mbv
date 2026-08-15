use super::list::{hero_block_shell, top_hero_layout, HERO_BLOCK_EXTRA_ROWS, HERO_TITLE_ROWS};
use super::list_rows::{
    draw_column_selection_markers, focused_or_subtle, item_cell_spans, SELECTED_BLOCK_SIDE_PADDING,
};
use crate::app::images::audiobookshelf_book_cover_cache_key;
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{
    library_cell_width, library_column_count, LIBRARY_COLUMN_GAP,
};
use crate::app::types_audiobookshelf_browse::{AudiobookshelfBookBrowseState, BookRow};
use crate::app::ui_util::{fmt_duration_approx, trunc_str};
use crate::app::{palette, App, TWO_COLUMN_THRESHOLD};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, Paragraph, Row, Table, TableState};
use ratatui::Frame;
/// The book tab's persistent list renders one row per chapter from the
/// selected book's `chapters[]` (or its `audioFiles` when `chapters[]` is
/// empty). The browser above it is the author-surname-grouped book list.
impl App {
    pub(super) fn render_audiobookshelf_books(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_book_browse.get(index).cloned() else {
            super::render_placeholder(f, area, "Audiobookshelf loading…");
            return;
        };

        let cols = library_column_count(area.width);
        // Chapter selection renders the hero (title/author/cover/progress) +
        // chapter rows; otherwise the author-surname book browser.
        if state.chapter_selection.is_some() {
            // Music-style composition: hero-on-left, list-on-right at the
            // same two-column breakpoint the Music tab uses; hero-on-top
            // below it (never the always-vertical podcast hero).
            if area.width >= TWO_COLUMN_THRESHOLD {
                let hero_col_width = ((area.width as u32 * 2 / 5) as u16)
                    .max(12)
                    .min(area.width.saturating_sub(12));
                let hero_area = Rect {
                    x: area.x,
                    y: area.y,
                    width: hero_col_width,
                    height: area.height.saturating_sub(1),
                };
                let list_area = Rect {
                    x: area.x + hero_col_width + 2,
                    y: area.y,
                    width: area.width.saturating_sub(hero_col_width + 2),
                    height: area.height.saturating_sub(1),
                };
                layout.hero_area = hero_area;
                layout.left_area = list_area;
                if hero_area.width > 0 && hero_area.height > 0 {
                    self.render_audiobookshelf_book_hero(f, hero_area, index, focused, layout);
                }
                if list_area.width > 0 && list_area.height > 0 {
                    self.render_audiobookshelf_book_rows(f, list_area, &state, focused, layout);
                }
                return;
            }
            let desired_rows =
                self.audiobookshelf_book_hero_rows(&state, cols > 1) + HERO_BLOCK_EXTRA_ROWS;
            let top = top_hero_layout(area, desired_rows, false);
            layout.hero_area = top.hero_area;
            layout.left_area = top.list_area;
            if top.hero_rows > 0 {
                hero_block_shell(f, top.hero_area, top.hero_rows, focused);
                let content = Rect {
                    x: top.hero_area.x + SELECTED_BLOCK_SIDE_PADDING,
                    y: top.hero_area.y + 2,
                    width: top
                        .hero_area
                        .width
                        .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                    height: top.hero_rows - HERO_BLOCK_EXTRA_ROWS,
                };
                self.render_audiobookshelf_book_hero(f, content, index, focused, layout);
            }
            self.render_audiobookshelf_book_rows(f, top.list_area, &state, focused, layout);
            return;
        }

        self.render_audiobookshelf_book_browser(f, area, index, focused, cols, layout);
    }

    fn audiobookshelf_book_hero_rows(
        &self,
        state: &AudiobookshelfBookBrowseState,
        show_title: bool,
    ) -> u16 {
        let mut rows = HERO_TITLE_ROWS.saturating_mul(show_title as u16);
        let book = state.selected_book();
        rows += book.and_then(|book| book.author_display.as_ref()).is_some() as u16;
        rows += 2; // progress row + trailing blank
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
        row = super::detail::render_hero_title_row(
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
    /// list area's provider-native content.
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
                let marker = if selected {
                    super::selection_marker(focused)
                } else {
                    Span::raw(" ")
                };
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

    /// Author-surname-grouped book browser (the persistent grid when no
    /// chapter selection is active).
    fn render_audiobookshelf_book_browser(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        focused: bool,
        cols: usize,
        layout: &mut LayoutMain,
    ) {
        let state = &mut self.audiobookshelf_book_browse[index];
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
        let cursor = state.cursor();
        let rows = state
            .books
            .iter()
            .enumerate()
            .collect::<Vec<_>>()
            .chunks(cols.max(1))
            .map(|chunk| chunk.iter().map(|(i, _)| *i).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let cursor_row = rows
            .iter()
            .position(|row| row.contains(&cursor))
            .unwrap_or(0);
        let visible = area.height as usize;
        let lower = (cursor_row + 1).saturating_sub(visible).min(cursor_row);
        state.scroll = state.scroll.clamp(lower, cursor_row);
        let scroll = state.scroll;
        let cell_width = library_cell_width(area, cols) as usize;
        let items = rows
            .iter()
            .skip(scroll)
            .take(if visible == 0 { 0 } else { visible })
            .map(|indices| {
                let mut spans = Vec::new();
                for (cell, book_index) in indices.iter().enumerate() {
                    let selected = *book_index == cursor;
                    let title = trunc_str(
                        &state.books[*book_index].title,
                        cell_width.saturating_sub(2),
                    );
                    let pad_to = if cell + 1 == indices.len() {
                        cell_width
                    } else {
                        cell_width + LIBRARY_COLUMN_GAP as usize
                    };
                    spans.extend(item_cell_spans(
                        title,
                        String::new(),
                        selected,
                        focused,
                        focused_or_subtle(focused),
                        pad_to,
                        cols,
                    ));
                }
                ListItem::new(Line::from(spans))
            })
            .collect::<Vec<_>>();
        layout.left_row_map = rows
            .iter()
            .skip(scroll)
            .take(if visible == 0 { 0 } else { visible })
            .map(|row| row.first().copied())
            .collect();
        layout.left_item_rows = rows;
        layout.left_screen_offset = scroll;
        layout.cursor_screen_y = Some(area.y + cursor_row.saturating_sub(scroll) as u16);
        f.render_widget(List::new(items), area);
        draw_column_selection_markers(f, area, cursor, cols, &layout.left_item_rows, scroll);
    }
}
