use crate::app::images::audiobookshelf_book_cover_cache_key;
use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::hero_left::{self, PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::arrangements::library as library_arrangement;
use crate::app::render::arrangements::padded_rect;
use crate::app::render::components::hero::{
    paint_hero_content, wrap_overview_lines, HeroContent, HeroImage, HeroLine, ImageTop,
    HERO_BLOCK_EXTRA_ROWS, HERO_TITLE_ROWS,
};
use crate::app::render::components::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::render::RENDER_FILTER;
use crate::app::types_audiobookshelf_browse::{AudiobookshelfBookBrowseState, BookRow};
use crate::app::ui_util::{fmt_duration_approx, trunc_str};
use crate::app::{palette, App};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Row, Table, TableState};
use ratatui::Frame;

/// Blank row separating the hero from the chapter list in the wide left pane.
const LEFT_SEPARATOR_ROWS: u16 = 1;
const BOOK_IMAGE_COLS: u16 = 18;
const BOOK_IMAGE_ROWS: u16 = 12;
const BOOK_IMAGE_PLACEHOLDER_ROWS: u16 = 6;

#[derive(Clone)]
pub(in crate::app::render) struct BookHeroPlan {
    pub(in crate::app::render) image_key: Option<String>,
    pub(in crate::app::render) image_width: u16,
    pub(in crate::app::render) image_height: u16,
    pub(in crate::app::render) placeholder: bool,
    pub(in crate::app::render) content_rows: u16,
}

impl BookHeroPlan {
    pub(in crate::app::render) fn constrained_to_height(&self, height: u16) -> Self {
        let image_height = self
            .image_height
            .min(height.saturating_sub(HERO_TITLE_ROWS));
        Self {
            image_height,
            content_rows: self.content_rows.min(height),
            ..self.clone()
        }
    }
}

/// The book tab's wide workspace renders one row per chapter from the selected
/// book's `chapters[]` (or its `audioFiles` when `chapters[]` is empty). The
/// right pane is the alphabetical-bucket-filtered book browser.
impl App {
    pub(in crate::app::render) fn render_audiobookshelf_books(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        layout.audiobookshelf_book_right_area = Rect::default();
        layout.audiobookshelf_book_wide_right_area = Rect::default();
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

        if hero_left::shared_hero_presentation(area).is_some() {
            let chapters_focused = state.chapter_selection.is_some();
            let left_focused = focused && chapters_focused;
            let right_focused = focused && !chapters_focused;
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
        self.render_narrow_audiobookshelf_books(f, area, index, &state, focused, layout);
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
        let Some(panes) = library_arrangement::wide_library_panes(area, 0, PANE_PAD_Y) else {
            return;
        };
        let left_panel = panes.left_panel;
        let right_panel = panes.right_panel;

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

        let left_area = panes.left_area;
        let right_area = panes.right_area;

        let left_bg = palette::resolve_surface_focus(left_focused);
        f.render_widget(
            Block::default().style(Style::default().bg(left_bg)),
            left_panel,
        );

        let hero_content_area = padded_rect(left_area, PANE_PAD_X, 0);
        let hero_plan = self.audiobookshelf_book_hero_plan(state, hero_content_area.width);
        let hero_rows_wanted = hero_plan.content_rows + 1; // +1 trailing blank
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
            let paint_plan = hero_plan.constrained_to_height(hero_area.height);
            self.render_audiobookshelf_book_hero(
                f,
                hero_area,
                index,
                left_focused,
                layout,
                &paint_plan,
            );
        }
        if chapters_area.width > 0 && chapters_area.height > 0 {
            self.render_audiobookshelf_book_rows(f, chapters_area, state, left_focused, layout);
        }

        layout.audiobookshelf_book_right_area = right_area;
        layout.audiobookshelf_book_wide_right_area = right_area;
        self.render_audiobookshelf_book_right_pane_wide(
            f,
            right_panel,
            right_area,
            index,
            right_focused,
            layout,
            &hero_plan,
        );
    }

    fn render_narrow_audiobookshelf_books(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        state: &AudiobookshelfBookBrowseState,
        right_focused: bool,
        layout: &mut LayoutMain,
    ) {
        // Narrow books use one scrolling browser. Selected detail replaces the
        // active book row; chapters remain in the wide workspace only.
        let areas = hero_left::pill_bar_areas(area);
        let pills_area = areas.pills_area;
        let browser_area = areas.content_area;
        self.render_audiobookshelf_book_bucket_pills(f, pills_area, index, layout);
        layout.left_area = browser_area;
        layout.audiobookshelf_book_right_area = browser_area;

        let hero_plan = self.audiobookshelf_book_hero_plan(
            state,
            browser_area
                .width
                .saturating_sub(SELECTED_BLOCK_SIDE_PADDING * 2),
        );
        let detail_rows = hero_plan.content_rows + HERO_BLOCK_EXTRA_ROWS;
        self.render_audiobookshelf_book_right_pane_narrow(
            f,
            browser_area,
            index,
            right_focused,
            layout,
            detail_rows,
            &hero_plan,
        );
        if layout.hero_area.width > 0 && layout.hero_area.height > 0 {
            layout.inline_hero_area = layout.hero_area;
        }
    }

    fn audiobookshelf_book_hero_plan(
        &mut self,
        state: &AudiobookshelfBookBrowseState,
        width: u16,
    ) -> BookHeroPlan {
        let Some(book) = state.selected_book() else {
            return BookHeroPlan {
                image_key: None,
                image_width: 0,
                image_height: 0,
                placeholder: false,
                content_rows: HERO_TITLE_ROWS,
            };
        };
        let server_url = self
            .config
            .lock()
            .unwrap()
            .audiobookshelf_setup
            .as_ref()
            .map(|setup| setup.server_url.clone());
        let images_enabled = self.images_enabled();
        let has_cover = images_enabled && book.cover_path.is_some();
        let image_key = has_cover
            .then(|| {
                server_url.as_ref().map(|server| {
                    audiobookshelf_book_cover_cache_key(
                        server,
                        &book.library_item_id,
                        self.current_protocol_suffix(),
                    )
                })
            })
            .flatten();
        let image_loading = image_key
            .as_ref()
            .is_some_and(|key| self.card_image_loading.contains(key));
        let (image_width, image_height, placeholder) = if has_cover {
            image_key
                .as_ref()
                .and_then(|key| self.cached_image_protocol_mut(key))
                .and_then(|image| {
                    image
                        .size_for(
                            ratatui_image::Resize::Scale(Some(RENDER_FILTER)),
                            ratatui::layout::Size {
                                width: BOOK_IMAGE_COLS,
                                height: BOOK_IMAGE_ROWS,
                            },
                        )
                        .map(|size| (size.width, size.height, false))
                })
                .unwrap_or((
                    BOOK_IMAGE_COLS,
                    if image_loading {
                        BOOK_IMAGE_PLACEHOLDER_ROWS
                    } else {
                        BOOK_IMAGE_ROWS
                    },
                    true,
                ))
        } else {
            (0, 0, false)
        };
        let image_width = image_width.min(width);
        let overview = book
            .description
            .as_deref()
            .filter(|d| !d.is_empty())
            .map(crate::app::ui_util::trunc_overview)
            .unwrap_or_default();
        let overview_rows = wrap_overview_lines(&overview, |line| {
            if line < image_height as usize {
                width.saturating_sub(image_width) as usize
            } else {
                width as usize
            }
        })
        .len() as u16;
        let author_rows = (!book.author_display.as_deref().unwrap_or("").is_empty()) as u16;
        let content_rows =
            (HERO_TITLE_ROWS + image_height).max(HERO_TITLE_ROWS + 2 + author_rows + overview_rows);
        BookHeroPlan {
            image_key,
            image_width,
            image_height,
            placeholder,
            content_rows,
        }
    }

    pub(in crate::app::render) fn render_audiobookshelf_book_hero(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        focused: bool,
        _layout: &mut LayoutMain,
        plan: &BookHeroPlan,
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
        if self.images_enabled() && book.cover_path.is_some() {
            if let Some(server) = server_url {
                self.fetch_audiobookshelf_book_cover(server, book.library_item_id.clone());
            }
        }

        // Build the metadata spans: duration, then progress (matching the
        // podcast tab's episode-progress style: a % or Finished span).
        let mut meta_parts = Vec::new();
        if book.duration_seconds > 0.0 {
            meta_parts.push(fmt_duration_approx(book.duration_seconds as i64));
        }
        let progress = state.progress.get(&book.library_item_id);
        let progress_span = match progress {
            Some(p) if p.is_finished => Some("Finished".to_string()),
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
                pct.map(|pct| format!("{pct}%"))
            }
            _ => Some("Not started".to_string()),
        };
        if let Some(part) = progress_span {
            meta_parts.push(part);
        }
        // Add narrator and year as subtle text after progress, matching the
        // Emby hero's release-date/duration/progress ordering.
        if let Some(narrator) = book.narrator.as_deref().filter(|n| !n.is_empty()) {
            meta_parts.push(format!("Read by {narrator}"));
        }
        if let Some(year) = book.published_year.as_deref().filter(|y| !y.is_empty()) {
            meta_parts.push(year.to_string());
        }

        // The overview/description, URL-stripped and capped like the
        // podcast hero's description (`trunc_overview`).
        let overview = book
            .description
            .as_deref()
            .filter(|d| !d.is_empty())
            .map(crate::app::ui_util::trunc_overview)
            .unwrap_or_default();

        let author = book.author_display.as_deref().unwrap_or("");
        let image_key = plan.image_key.as_ref();
        let image_width = plan.image_width;
        let image_height = plan.image_height;
        let placeholder = plan.placeholder;
        let title_rows = HERO_TITLE_ROWS;
        let image_start_row = area.y + title_rows;
        let image_end_row = image_start_row + image_height;
        let text_width = |row: u16| {
            if image_height > 0 && row >= image_start_row && row < image_end_row {
                area.width.saturating_sub(image_width) as usize
            } else {
                area.width as usize
            }
        };
        let mut lines = Vec::new();
        if !author.is_empty() {
            lines.push(HeroLine::Plain(author.to_string()));
        }
        let overview_lines =
            wrap_overview_lines(&overview, |line| text_width(image_start_row + line as u16));
        lines.extend(overview_lines.into_iter().map(HeroLine::Plain));
        let meta = (!meta_parts.is_empty()).then(|| meta_parts.join("  "));
        let content = HeroContent {
            title: Some(book.title.as_str()),
            meta_line: meta.as_deref(),
            meta_color: palette::TEXT_DETAIL_META,
            show_playing: false,
            unconditional_spacer_after_meta: false,
            lines: &lines,
            image: (image_height > 0).then_some(HeroImage {
                actual_w: image_width,
                height: image_height,
                top: ImageTop::AfterTitle,
            }),
        };
        let result = paint_hero_content(f, area, &content, focused);
        if let Some(image_rect) = result.img_rect {
            if placeholder {
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::BORDER_UNFOCUSED)),
                    image_rect,
                );
            } else if let Some(key) = image_key.as_ref() {
                if let Some(image) = self.cached_image_protocol_mut(key) {
                    type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
                    f.render_stateful_widget(
                        SImg::default().resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
                        image_rect,
                        image,
                    );
                }
            }
        }
    }

    /// Chapter (or audio-file) rows for the selected book in the wide
    /// workspace: the provider-native persistent list content below the hero.
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
