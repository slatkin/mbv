use crate::app::images::audiobookshelf_book_cover_cache_key;
use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::hero_left;
use crate::app::render::components::hero::{HERO_BLOCK_EXTRA_ROWS, HERO_TITLE_ROWS};
use crate::app::render::components::home_hero::{
    beside_image_hero_dims, beside_image_hero_rects, HeroMetaBlock,
};
use crate::app::types_audiobookshelf_browse::{AudiobookshelfBookBrowseState, BookRow};
use crate::app::ui_util::{fmt_duration_approx, trunc_str};
use crate::app::{palette, App};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Row, Table, TableState};
use ratatui::Frame;

/// Padding inside recessed wide-book blocks, matching Music's wide layout
/// (`music_wide.rs::PANE_PAD_X`/`PANE_PAD_Y`) -- duplicated rather than
/// shared per this change's design (two consumers isn't yet a strong case
/// for extraction).
pub(in crate::app::render) const PANE_PAD_X: u16 = 2;
pub(in crate::app::render) const PANE_PAD_Y: u16 = 1;
/// Blank row separating the hero from the chapter list in the wide left pane.
const LEFT_SEPARATOR_ROWS: u16 = 1;
/// Height of the bucket-pill row inside the narrow right pane. The wide right
/// pane's pill row height comes from `hero_left::hero_on_left_right_pane` instead
/// (phase 6, "Adopt: Home and audiobooks").
pub(in crate::app::render) const PILLS_ROW_HEIGHT: u16 = 1;
/// Blank rows below the pills before the book list starts, narrow-pane only
/// (see `PILLS_ROW_HEIGHT`).
pub(in crate::app::render) const PILLS_GAP_ROWS: u16 = 1;

/// The book tab's persistent list renders one row per chapter from the
/// selected book's `chapters[]` (or its `audioFiles` when `chapters[]` is
/// empty). The right pane is the alphabetical-bucket-filtered book browser.
/// Both panes are always visible (book-browsing spec: "Book libraries use
/// the Music tab composition").
impl App {
    pub(in crate::app::render) fn render_audiobookshelf_books(
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
            crate::app::render::render_placeholder(f, area, "Audiobookshelf loading…");
            return;
        };
        if state.books.is_empty() {
            crate::app::render::render_placeholder(
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

        if hero_left::shared_hero_presentation(area).is_some() {
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
        let Some((mut left_panel, right_panel)) = hero_left::shared_hero_presentation(area) else {
            return;
        };
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
        _left_focused: bool,
        right_focused: bool,
        layout: &mut LayoutMain,
    ) {
        // Narrow books use one scrolling browser. Selected detail, including
        // chapters, replaces the active book row.
        let pills_area = Rect {
            height: PILLS_ROW_HEIGHT.min(area.height),
            ..area
        };
        self.render_audiobookshelf_book_bucket_pills(f, pills_area, index, layout);
        let browser_area = Rect {
            y: area.y + PILLS_ROW_HEIGHT + PILLS_GAP_ROWS,
            height: area
                .height
                .saturating_sub(PILLS_ROW_HEIGHT + PILLS_GAP_ROWS),
            ..area
        };
        layout.left_area = browser_area;
        layout.audiobookshelf_book_right_area = browser_area;

        let chapter_rows = state
            .selected_id
            .as_deref()
            .map(|id| state.visible_rows(id).len() as u16 + LEFT_SEPARATOR_ROWS)
            .unwrap_or(0);
        let detail_rows =
            self.audiobookshelf_book_hero_rows(state) + HERO_BLOCK_EXTRA_ROWS + chapter_rows;
        self.render_audiobookshelf_book_right_pane_narrow(
            f,
            browser_area,
            index,
            right_focused,
            layout,
            detail_rows,
        );
        if layout.hero_area.width > 0 && layout.hero_area.height > 0 {
            layout.inline_hero_area = layout.hero_area;
        }
    }

    pub(in crate::app::render) fn audiobookshelf_book_hero_rows(
        &self,
        state: &AudiobookshelfBookBrowseState,
    ) -> u16 {
        let Some(book) = state.selected_book() else {
            return HERO_TITLE_ROWS;
        };
        // The hero layout is: title (wrapped), author row, meta row (duration
        // / progress), blank separator, description (wrapped around image).
        // `beside_image_hero_dims` computes the exact layout we render; we
        // need the same row count it will ask for so the reserved block fits.
        // Use a provisional width (the widest we'd get in the narrow path);
        // the actual render clamps to the granted area, so an over-estimate
        // just reserves a row or two more than needed -- safe.
        let inner_w = 60u16;
        let max_allowed = 30u16;
        let overview = book
            .description
            .as_deref()
            .filter(|d| !d.is_empty())
            .map(crate::app::ui_util::trunc_overview)
            .unwrap_or_default();
        let (_, layout, image_rows) = beside_image_hero_dims(
            &book.title,
            book.author_display.as_deref().unwrap_or(""),
            &overview,
            inner_w,
            max_allowed,
            1,
        );
        layout.height.max(image_rows)
    }

    pub(in crate::app::render) fn render_audiobookshelf_book_hero(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        focused: bool,
        _layout: &mut LayoutMain,
    ) {
        let Some(state) = self.audiobookshelf_book_browse.get(index).cloned() else {
            return;
        };
        let Some(book) = state.selected_book().cloned() else {
            return;
        };

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
        if self.images_enabled() {
            if let Some(server) = server_url {
                self.fetch_audiobookshelf_book_cover(server, book.library_item_id.clone());
            }
        }

        // Build the metadata spans: duration, then progress (matching the
        // podcast tab's episode-progress style: a % or Finished span).
        let mut meta_spans: Vec<Span<'static>> = Vec::new();
        if book.duration_seconds > 0.0 {
            meta_spans.push(Span::styled(
                trunc_str(
                    &fmt_duration_approx(book.duration_seconds as i64),
                    area.width as usize,
                ),
                Style::default().fg(palette::TEXT_SECONDARY),
            ));
        }
        let progress = state.progress.get(&book.library_item_id);
        let progress_span = match progress {
            Some(p) if p.is_finished => Some(Span::styled(
                "Finished",
                Style::default()
                    .fg(palette::TEXT_METADATA)
                    .add_modifier(Modifier::BOLD),
            )),
            Some(p) if p.current_time_seconds > 0.0 => {
                // Prefer the book's total duration for the percentage; fall
                // back to the sum of audio-file durations from the detail
                // cache if the list page didn't carry one.
                let total = if book.duration_seconds > 0.0 {
                    Some(book.duration_seconds)
                } else {
                    state
                        .detail_cache
                        .get(&book.library_item_id)
                        .map(|(_, files)| files.iter().map(|f| f.duration).sum::<f64>())
                        .filter(|t| *t > 0.0)
                };
                let pct = total.map(|total| {
                    ((p.current_time_seconds * 100.0 / total).floor() as u8).clamp(1, 99)
                });
                pct.map(|pct| {
                    Span::styled(
                        format!("{pct}%"),
                        Style::default().fg(palette::TEXT_METADATA),
                    )
                })
            }
            _ => Some(Span::styled(
                "Not started",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
        };
        if let Some(span) = progress_span {
            if !meta_spans.is_empty() {
                meta_spans.push(Span::raw("  "));
            }
            meta_spans.push(span);
        }
        // Add narrator and year as subtle text after progress, matching the
        // Emby hero's release-date/duration/progress ordering.
        if let Some(narrator) = book.narrator.as_deref().filter(|n| !n.is_empty()) {
            if !meta_spans.is_empty() {
                meta_spans.push(Span::raw("  "));
            }
            meta_spans.push(Span::styled(
                trunc_str(&format!("Read by {narrator}"), area.width as usize),
                Style::default().fg(palette::TEXT_SECONDARY),
            ));
        }
        if let Some(year) = book.published_year.as_deref().filter(|y| !y.is_empty()) {
            if !meta_spans.is_empty() {
                meta_spans.push(Span::raw("  "));
            }
            meta_spans.push(Span::styled(
                year.to_string(),
                Style::default().fg(palette::TEXT_SECONDARY),
            ));
        }

        // The overview/description, URL-stripped and capped like the
        // podcast hero's description (`trunc_overview`).
        let overview = book
            .description
            .as_deref()
            .filter(|d| !d.is_empty())
            .map(crate::app::ui_util::trunc_overview)
            .unwrap_or_default();

        // The standard inline beside-image layout: image on the right,
        // metadata column on the left, overview wrapping around the image.
        // This is the same path Emby Keep Watching and the generic ABS hero
        // use, so the book tab's hero can't drift from theirs.
        let inner_w = area.width;
        let max_allowed = area.height;
        let author = book.author_display.as_deref().unwrap_or("");
        let (img_w, meta_layout, image_rows) =
            beside_image_hero_dims(&book.title, author, &overview, inner_w, max_allowed, 1);
        let (meta_area, img_area) =
            beside_image_hero_rects(area, img_w, meta_layout.height, image_rows);

        let cache_key = image_key.clone().unwrap_or_default();
        self.render_beside_image_hero(
            f,
            meta_area,
            area,
            img_area,
            &meta_layout,
            HeroMetaBlock {
                title_suffix: None,
                meta_rows: vec![meta_spans],
            },
            &cache_key,
            0,
            focused,
            false,
        );
        // The shared `render_beside_image_hero` renders the image into
        // `img_area` via the image protocol.
    }

    /// Chapter (or audioFiles) rows for the selected book: the persistent
    /// list area's provider-native content, always rendered below the hero
    /// (Music's track-list analog; book-browsing spec: "Chapters render as
    /// first-class rows in the persistent list").
    pub(in crate::app::render) fn render_audiobookshelf_book_rows(
        &self,
        f: &mut Frame,
        area: Rect,
        state: &AudiobookshelfBookBrowseState,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        let Some(id) = state.selected_id.as_deref() else {
            return;
        };
        if state.detail_loading {
            crate::app::render::render_placeholder(f, area, " Loading…");
            return;
        }
        let rows = state.visible_rows(id);
        if rows.is_empty() {
            crate::app::render::render_placeholder(f, area, " No chapters available");
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
                    Style::default().fg(palette::TEXT_FOCUS_ACCENT)
                } else if focused {
                    Style::default().fg(palette::TEXT_STRONG)
                } else {
                    Style::default().fg(palette::TEXT_SECONDARY)
                };
                let marker = crate::app::render::selection_marker(
                    selected,
                    crate::app::render::MarkerEdge::Left,
                );
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
                table_rows.clone(),
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
        layout.audiobookshelf_book_chapter_rows = table_rows
            .iter()
            .enumerate()
            .skip(table_state.offset())
            .take(area.height as usize)
            .enumerate()
            .map(|(screen_row, _)| {
                (
                    Rect {
                        y: area.y + screen_row as u16,
                        height: 1,
                        ..area
                    },
                    table_state.offset() + screen_row,
                )
            })
            .collect();
    }
}

fn inset_pane_vertically(area: Rect) -> Rect {
    Rect {
        y: area.y.saturating_add(PANE_PAD_Y),
        height: area.height.saturating_sub(PANE_PAD_Y * 2),
        ..area
    }
}
