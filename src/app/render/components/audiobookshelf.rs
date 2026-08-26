use crate::app::images::audiobookshelf_cover_cache_key;
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{
    library_cell_width, library_column_count, LIBRARY_COLUMN_GAP,
};
use crate::app::render::arrangements::hero_left;
use crate::app::render::components::detail_series_view::{
    SERIES_DETAIL_DIVIDER_ROWS, SERIES_DETAIL_EPISODE_ROWS_ESTIMATE,
    SERIES_DETAIL_TRAILING_BLANK_ROWS, SERIES_IMAGE_COLS, SERIES_IMAGE_PLACEHOLDER_ROWS,
    SERIES_IMAGE_ROWS,
};
use crate::app::render::components::hero::{
    inline_detail_flow, inline_display_row, inline_display_row_count, inline_hero_text_width,
    selected_detail_shell, wrap_overview_lines, HeroContent, HeroImage, HeroLine, InlineDisplayRow,
    HERO_BLOCK_EXTRA_ROWS, HERO_TITLE_ROWS,
};
use crate::app::render::components::list_rows::{
    draw_column_selection_markers, focused_or_subtle, item_cell_spans, SELECTED_BLOCK_SIDE_PADDING,
};
use crate::app::render::{render_pill_bar, render_placeholder, PillBar, RENDER_FILTER};
use crate::app::types_audiobookshelf_browse::{
    build_show_title_buckets, AudiobookshelfEpisodeFilter,
};
use crate::app::ui_util::{fmt_duration_approx, trunc_str};
use crate::app::{palette, App};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, List, ListItem, Row, Table, TableState};
use ratatui::Frame;

fn format_episode_date(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some((year, month, day)) = value.split_once('T').and_then(|(date, _)| {
        let mut parts = date.split('-');
        Some((
            parts.next()?.parse::<i64>().ok()?,
            parts.next()?.parse::<u32>().ok()?,
            parts.next()?.parse::<u32>().ok()?,
        ))
    }) {
        return Some(format!("{day:02}/{month:02}/{year:04}"));
    }
    if let Some((year, month, day)) = value.split_once('-').and_then(|_| {
        let mut parts = value.split('-');
        Some((
            parts.next()?.parse::<i64>().ok()?,
            parts.next()?.parse::<u32>().ok()?,
            parts.next()?.parse::<u32>().ok()?,
        ))
    }) {
        return Some(format!("{day:02}/{month:02}/{year:04}"));
    }
    let timestamp = value.parse::<i128>().ok()?;
    let seconds = if timestamp.abs() >= 100_000_000_000 {
        timestamp / 1_000
    } else {
        timestamp
    };
    let dt = time::OffsetDateTime::from_unix_timestamp(seconds as i64).ok()?;
    let year = dt.year();
    let (month, day) = (u8::from(dt.month()), dt.day());
    Some(format!("{day:02}/{month:02}/{year:04}"))
}

fn episode_title_without_number(title: &str) -> &str {
    let title = title.trim_start();
    let title = title.strip_prefix('#').unwrap_or(title).trim_start();
    let digit_end = title
        .char_indices()
        .take_while(|(_, character)| character.is_ascii_digit())
        .last()
        .map(|(index, character)| index + character.len_utf8());
    let Some(digit_end) = digit_end else {
        return title;
    };
    let suffix = title[digit_end..].trim_start();
    if matches!(suffix.chars().next(), Some('.' | ':'))
        && suffix
            .chars()
            .nth(1)
            .is_some_and(|character| character.is_ascii_digit())
    {
        return title;
    }
    let Some(separator_end) = suffix
        .char_indices()
        .take_while(|(_, character)| matches!(character, '.' | ')' | ':' | '-' | '|'))
        .last()
        .map(|(index, character)| index + character.len_utf8())
    else {
        return title;
    };
    let stripped = suffix[separator_end..].trim_start();
    if stripped.is_empty() {
        title
    } else {
        stripped
    }
}

impl App {
    pub(in crate::app::render) fn render_audiobookshelf_podcasts(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        layout.audiobookshelf_podcast_area = area;
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get(index).cloned() else {
            render_placeholder(f, area, "Audiobookshelf loading…");
            return;
        };

        let cols = library_column_count(area.width);
        if let Some((hero_panel, right_panel)) = hero_left::shared_hero_presentation(area) {
            layout.hero_area = hero_panel;
            layout.left_area = right_panel;
            layout.audiobookshelf_podcast_right_area = right_panel;
            let content = Rect {
                x: hero_panel.x + SELECTED_BLOCK_SIDE_PADDING,
                y: hero_panel.y + SELECTED_BLOCK_SIDE_PADDING,
                width: hero_panel
                    .width
                    .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                height: hero_panel
                    .height
                    .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
            };
            if content.width > 0 && content.height > 0 {
                self.render_audiobookshelf_hero(f, content, index, focused, false, true, layout);
            }
            if state.shows.is_empty() {
                render_placeholder(f, right_panel, "No podcast shows");
            } else {
                self.render_audiobookshelf_show_rows(f, right_panel, index, focused, 1, 0, layout);
            }
            return;
        }
        if state.shows.is_empty() {
            layout.left_area = area;
            render_placeholder(
                f,
                area,
                state
                    .error
                    .as_deref()
                    .unwrap_or(if state.loading_pages.is_empty() {
                        "No podcast shows"
                    } else {
                        "Loading podcast shows…"
                    }),
            );
            return;
        }

        // Narrow-mode panel row: alphabetical show-title bucket pills, above
        // the scrolling show list (design.md "alphabetical panel pills
        // handle tab-level browsing"; matches `render_letter_pills_row`'s
        // shape). Wide mode has no equivalent row -- its right pane is
        // always the full unfiltered show workspace.
        let areas = hero_left::pill_bar_areas(area);
        let pills_area = areas.pills_area;
        self.render_audiobookshelf_podcast_bucket_pills(f, pills_area, index, layout);
        let list_area = areas.content_area;
        layout.left_area = list_area;
        let hero_content_width = list_area
            .width
            .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING);
        let desired_rows = self.audiobookshelf_hero_content_rows(index, true, hero_content_width)
            + HERO_BLOCK_EXTRA_ROWS;
        let hero_rows = if desired_rows >= HERO_BLOCK_EXTRA_ROWS && desired_rows < list_area.height
        {
            desired_rows
        } else {
            0
        };
        self.render_audiobookshelf_show_rows(f, list_area, index, focused, cols, hero_rows, layout);
        if hero_rows > 0 {
            let cursor_row = self.audiobookshelf_browse[index].cursor() / cols.max(1);
            let detail_screen_row =
                inline_detail_flow(cursor_row, hero_rows, list_area.height, state.scroll)
                    .expect("admitted inline detail must fit")
                    .detail_screen_row;
            layout.hero_area = Rect {
                x: list_area.x,
                y: list_area.y + detail_screen_row as u16,
                width: list_area.width,
                height: hero_rows,
            };
            layout.inline_hero_area = layout.hero_area;
            layout.selected_item_rect = Some(layout.hero_area);
            selected_detail_shell(f, layout.hero_area, hero_rows, focused);
            let content = Rect {
                x: list_area.x + SELECTED_BLOCK_SIDE_PADDING,
                y: layout.hero_area.y + 2,
                width: list_area
                    .width
                    .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                height: hero_rows - HERO_BLOCK_EXTRA_ROWS,
            };
            self.render_audiobookshelf_hero(f, content, index, focused, true, false, layout);
        } else {
            layout.hero_area = Rect::default();
        }
    }

    /// Renders the narrow podcast panel's alphabetical show-title bucket
    /// pills (labels from `build_show_title_buckets`, omitting any empty
    /// range), matching `render_letter_pills_row`'s shape. Show titles have
    /// no separate sort key the way Emby libraries' `LetterFilter` name
    /// ranges do, so buckets are computed directly from `state.shows`
    /// (already title-sorted, see `AudiobookshelfBrowseState::append_page`)
    /// each render rather than persisted -- the selected pill just reflects
    /// whichever bucket the cursor's show currently falls in.
    fn render_audiobookshelf_podcast_bucket_pills(
        &mut self,
        f: &mut Frame,
        row_area: Rect,
        index: usize,
        layout: &mut LayoutMain,
    ) {
        let Some(state) = self.audiobookshelf_browse.get(index) else {
            layout.selector_tabs = Vec::new();
            return;
        };
        let buckets = build_show_title_buckets(&state.shows);
        if buckets.is_empty() || row_area.width == 0 {
            layout.selector_tabs = Vec::new();
            return;
        }
        let cursor = state.cursor();
        let selected_pos = buckets
            .iter()
            .position(|bucket| cursor >= bucket.start && cursor < bucket.end)
            .unwrap_or(0);
        let labels: Vec<String> = buckets
            .iter()
            .map(|bucket| bucket.label.to_string())
            .collect();
        let ids: Vec<usize> = (0..labels.len()).collect();
        layout.selector_tabs = render_pill_bar(
            f,
            row_area,
            PillBar {
                labels: &labels,
                ids: &ids,
                selected_pos,
                prefix: Some(" ⌘ "),
            },
        );
    }

    fn audiobookshelf_hero_content_rows(&self, index: usize, show_title: bool, width: u16) -> u16 {
        let state = &self.audiobookshelf_browse[index];
        let mut rows = HERO_TITLE_ROWS.saturating_mul(show_title as u16);
        rows += state
            .selected_show()
            .and_then(|show| show.author.as_ref())
            .is_some() as u16;
        if let Some(description) = state
            .selected_show()
            .and_then(|show| show.description.as_deref())
            .filter(|description| !description.is_empty())
        {
            rows += 1;
            let (image_width, image_height) = if self.images_enabled() {
                (SERIES_IMAGE_COLS, SERIES_IMAGE_ROWS)
            } else {
                (0, 0)
            };
            let description_start = HERO_TITLE_ROWS.saturating_mul(show_title as u16)
                + state
                    .selected_show()
                    .and_then(|show| show.author.as_ref())
                    .is_some() as u16
                + 1;
            rows += wrap_overview_lines(description, |line| {
                let row = description_start + line as u16;
                inline_hero_text_width(width, image_width, image_height, row) as usize
            })
            .len()
            .min(4) as u16;
        }
        if state.episode_selection.is_some() {
            rows += 1 + SERIES_DETAIL_DIVIDER_ROWS as u16;
            rows += state
                .episodes
                .as_ref()
                .map(|_| state.visible_episodes().len())
                .unwrap_or(SERIES_DETAIL_EPISODE_ROWS_ESTIMATE) as u16;
        }
        rows += SERIES_DETAIL_TRAILING_BLANK_ROWS as u16;
        if self.images_enabled() {
            rows = rows.max(SERIES_IMAGE_ROWS + 1);
        }
        rows
    }

    fn render_audiobookshelf_hero(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        focused: bool,
        show_title: bool,
        persistent: bool,
        layout: &mut LayoutMain,
    ) {
        let Some(state) = self.audiobookshelf_browse.get(index).cloned() else {
            return;
        };
        let Some(show) = state.selected_show().cloned() else {
            return;
        };
        let max_y = area.y + area.height;
        let server_url = self
            .config
            .lock()
            .unwrap()
            .audiobookshelf_setup
            .as_ref()
            .map(|setup| setup.server_url.clone());
        let image_key = server_url.as_ref().map(|server| {
            audiobookshelf_cover_cache_key(
                server,
                &show.library_item_id,
                self.current_protocol_suffix(),
            )
        });
        let image_loading = image_key
            .as_ref()
            .is_some_and(|key| self.card_image_loading.contains(key));
        let (image_width, image_height, placeholder) = image_key
            .as_ref()
            .and_then(|key| self.cached_image_protocol_mut(key))
            .and_then(|image| {
                image
                    .size_for(
                        ratatui_image::Resize::Scale(Some(RENDER_FILTER)),
                        ratatui::layout::Size {
                            width: SERIES_IMAGE_COLS,
                            height: SERIES_IMAGE_ROWS,
                        },
                    )
                    .map(|size| (size.width, size.height, false))
            })
            .unwrap_or({
                if image_loading {
                    (SERIES_IMAGE_COLS, SERIES_IMAGE_PLACEHOLDER_ROWS, true)
                } else {
                    (0, 0, false)
                }
            });
        // Title row (paints into `area`, same shape as the movie/series
        // hero's top-row title) plus the image's right-aligned reservation,
        // via the shared `Hero` component. Author/description are plain
        // `HeroLine`s (design.md decision 2): the spacer choreography
        // (spacer only before a present description, then an unconditional
        // trailing spacer) is reproduced with empty `HeroLine::Plain`
        // entries, which `paint_hero_content` skips painting but still
        // advances the row for (`hero.rs:427-440`).
        // Pre-render width estimate for wrapping the description.
        let text_width_pre = |current_row: u16| -> u16 {
            inline_hero_text_width(
                area.width,
                image_width,
                image_height,
                current_row.saturating_sub(area.y),
            )
        };
        let mut hero_lines: Vec<HeroLine> = Vec::new();
        if let Some(author) = show.author.as_deref() {
            hero_lines.push(HeroLine::Plain(author.to_string()));
        }
        if let Some(description) = show
            .description
            .as_deref()
            .filter(|description| !description.is_empty())
        {
            hero_lines.push(HeroLine::Plain(String::new()));
            let description_start_row = area.y
                + HERO_TITLE_ROWS.saturating_mul(show_title as u16)
                + show.author.is_some() as u16
                + 1;
            let description_lines = wrap_overview_lines(description, |line| {
                text_width_pre(description_start_row + line as u16) as usize
            });
            for line_text in description_lines.into_iter().take(4) {
                hero_lines.push(HeroLine::Plain(line_text));
            }
        }
        hero_lines.push(HeroLine::Plain(String::new()));

        let hero_content = HeroContent {
            title: show_title.then_some(show.title.as_str()),
            meta_line: None,
            meta_color: palette::TEXT_SECONDARY,
            show_playing: false,
            unconditional_spacer_after_meta: false,
            lines: &hero_lines,
            image: (image_height > 0).then_some(HeroImage {
                actual_w: image_width,
                height: image_height,
            }),
        };
        let hero_result = crate::app::render::components::hero::paint_hero_content(
            f,
            area,
            &hero_content,
            focused,
        );
        let mut row = hero_result.next_row;
        let text_width = |current_row: u16| {
            inline_hero_text_width(
                area.width,
                image_width,
                image_height,
                current_row.saturating_sub(area.y),
            )
        };

        if let Some(image_rect) = hero_result.img_rect {
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

        // Wide-only: narrow shows hero content only and routes Enter to the
        // selection modal instead (`open_podcast_selection_modal`).
        if persistent && state.episode_selection.is_some() && row < max_y {
            let labels = AudiobookshelfEpisodeFilter::ALL
                .iter()
                .map(|filter| filter.label().to_string())
                .collect::<Vec<_>>();
            let ids = (0..labels.len()).collect::<Vec<_>>();
            layout.selector_tabs = render_pill_bar(
                f,
                Rect {
                    x: area.x,
                    y: row,
                    width: text_width(row),
                    height: 1,
                },
                PillBar {
                    labels: &labels,
                    ids: &ids,
                    selected_pos: AudiobookshelfEpisodeFilter::ALL
                        .iter()
                        .position(|filter| *filter == state.episode_filter)
                        .unwrap_or(0),
                    prefix: Some(" ⌘ "),
                },
            );
            row += 1;
        }

        if persistent && state.episode_selection.is_some() && row < max_y {
            let table_area = Rect {
                x: area.x,
                y: row,
                width: text_width(row),
                height: max_y
                    .saturating_sub(row)
                    .saturating_sub(SERIES_DETAIL_TRAILING_BLANK_ROWS as u16),
            };
            self.render_audiobookshelf_episode_rows(f, table_area, &state, focused, layout);
        }
    }

    fn render_audiobookshelf_episode_rows(
        &self,
        f: &mut Frame,
        area: Rect,
        state: &crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        if state.detail_loading || state.episodes.is_none() {
            render_placeholder(f, area, " Loading…");
            return;
        }
        let episodes = state.visible_episodes();
        if episodes.is_empty() {
            render_placeholder(f, area, " No matching episodes");
            return;
        }
        let show_length = area.width > 40;
        let duration_width = if show_length { 7 } else { 0 };
        let title_width = (area.width as usize)
            .saturating_sub(1 + if show_length { duration_width + 1 } else { 0 });
        let rows = episodes
            .iter()
            .enumerate()
            .map(|(index, episode)| {
                let selected = state.episode_selection == Some(index);
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
                let published = episode
                    .published_at
                    .as_deref()
                    .and_then(format_episode_date)
                    .unwrap_or_default();
                let progress = state
                    .progress
                    .get(&(episode.library_item_id.clone(), episode.episode_id.clone()))
                    .map(|progress| {
                        if progress.is_finished {
                            "Played".to_string()
                        } else if progress.current_time_seconds > 0.0 {
                            episode
                                .duration_seconds
                                .filter(|duration| *duration > 0.0)
                                .map(|duration| {
                                    format!(
                                        "{}%",
                                        (progress.current_time_seconds * 100.0 / duration)
                                            .floor()
                                            .clamp(1.0, 99.0)
                                            as u8
                                    )
                                })
                                .unwrap_or_default()
                        } else {
                            String::new()
                        }
                    })
                    .unwrap_or_default();
                let date_width = if published.is_empty() {
                    0
                } else {
                    published.len() + 1
                };
                let progress_width = if progress.is_empty() {
                    0
                } else {
                    progress.len() + 1
                };
                let title = trunc_str(
                    episode_title_without_number(&episode.title),
                    title_width.saturating_sub(date_width + progress_width),
                );
                let mut title_spans = vec![marker, Span::raw(title)];
                if !published.is_empty() {
                    title_spans.push(Span::raw(" "));
                    title_spans.push(Span::styled(
                        published,
                        Style::default().fg(palette::TEXT_FOCUS_ACCENT),
                    ));
                }
                if !progress.is_empty() {
                    title_spans.push(Span::raw(" "));
                    title_spans.push(Span::styled(
                        progress,
                        Style::default().fg(palette::TEXT_METADATA),
                    ));
                }
                let title_cell = Cell::from(Line::from(title_spans));
                let duration = episode
                    .duration_seconds
                    .filter(|seconds| *seconds > 0.0)
                    .map(|seconds| fmt_duration_approx(seconds as i64))
                    .unwrap_or_else(|| "—".to_string());
                if show_length {
                    Row::new([title_cell, Cell::from(duration), Cell::from("")]).style(style)
                } else {
                    Row::new([title_cell, Cell::from(""), Cell::from("")]).style(style)
                }
            })
            .collect::<Vec<_>>();
        let mut table_state = TableState::default();
        table_state.select(state.episode_selection);
        f.render_stateful_widget(
            Table::new(
                rows,
                [
                    Constraint::Min(10),
                    Constraint::Length(duration_width as u16),
                    Constraint::Length(1),
                ],
            )
            .column_spacing(1)
            .row_highlight_style(Style::default()),
            area,
            &mut table_state,
        );
        let offset = table_state.offset();
        layout.audiobookshelf_episode_rows = episodes
            .iter()
            .enumerate()
            .skip(offset)
            .take(area.height as usize)
            .enumerate()
            .map(|(screen_row, (index, _))| {
                (
                    Rect {
                        y: area.y + screen_row as u16,
                        height: 1,
                        ..area
                    },
                    index,
                )
            })
            .collect();
    }

    fn render_audiobookshelf_show_rows(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        focused: bool,
        cols: usize,
        hero_rows: u16,
        layout: &mut LayoutMain,
    ) {
        let state = &mut self.audiobookshelf_browse[index];
        let cursor = state.cursor();
        let rows = state
            .shows
            .iter()
            .enumerate()
            .collect::<Vec<_>>()
            .chunks(cols.max(1))
            .map(|chunk| chunk.iter().map(|(index, _)| *index).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let cursor_row = rows
            .iter()
            .position(|row| row.contains(&cursor))
            .unwrap_or(0);
        let total_display = inline_display_row_count(rows.len(), cursor_row, hero_rows);
        let visible = area.height as usize;
        let scroll = if hero_rows > 0 {
            inline_detail_flow(cursor_row, hero_rows, area.height, state.scroll)
                .expect("admitted inline detail must fit")
                .offset
        } else {
            let lower = cursor_row.saturating_add(1).saturating_sub(visible);
            state.scroll.clamp(lower, cursor_row)
        };
        state.scroll = scroll;
        let cell_width = library_cell_width(area, cols) as usize;
        let items = (scroll..total_display)
            .take(visible)
            .map(|display_row| {
                match inline_display_row(rows.len(), cursor_row, hero_rows, display_row)
                    .expect("visible row is within the replacement flow")
                {
                    InlineDisplayRow::Replacement => ListItem::new(Line::default()),
                    InlineDisplayRow::Source(source_row) => {
                        let indices = &rows[source_row];
                        let mut spans = Vec::new();
                        for (cell, index) in indices.iter().enumerate() {
                            let selected = *index == cursor;
                            let title =
                                trunc_str(&state.shows[*index].title, cell_width.saturating_sub(2));
                            let pad_to = if cell + 1 == indices.len() {
                                cell_width
                            } else {
                                cell_width + LIBRARY_COLUMN_GAP as usize
                            };
                            spans.extend(item_cell_spans(
                                title,
                                String::new(),
                                selected,
                                focused_or_subtle(focused),
                                pad_to,
                            ));
                        }
                        ListItem::new(Line::from(spans))
                    }
                }
            })
            .collect::<Vec<_>>();
        layout.left_row_map = (scroll..total_display)
            .take(visible)
            .map(|display_row| {
                match inline_display_row(rows.len(), cursor_row, hero_rows, display_row)
                    .expect("visible row is within the replacement flow")
                {
                    InlineDisplayRow::Replacement => (display_row == cursor_row).then_some(cursor),
                    InlineDisplayRow::Source(source_row) => rows[source_row].first().copied(),
                }
            })
            .collect();
        layout.left_item_rows = (0..total_display)
            .map(|display_row| {
                match inline_display_row(rows.len(), cursor_row, hero_rows, display_row)
                    .expect("display row is within the replacement flow")
                {
                    InlineDisplayRow::Replacement => {
                        if display_row == cursor_row {
                            vec![cursor]
                        } else {
                            Vec::new()
                        }
                    }
                    InlineDisplayRow::Source(source_row) => rows[source_row].clone(),
                }
            })
            .collect();
        layout.left_screen_offset = scroll;
        f.render_widget(List::new(items), area);
        if focused && total_display > visible {
            crate::app::render::render_right_scrollbar(
                f,
                area,
                total_display.saturating_sub(visible),
                scroll,
                palette::SCROLLBAR,
            );
        }
        if hero_rows == 0 {
            draw_column_selection_markers(f, area, cursor, &layout.left_item_rows, scroll);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::episode_title_without_number;

    #[test]
    fn episode_title_without_number_removes_common_prefixes() {
        for (input, expected) in [
            ("1. First episode", "First episode"),
            ("01 - First episode", "First episode"),
            ("#12: First episode", "First episode"),
            ("3) First episode", "First episode"),
        ] {
            assert_eq!(episode_title_without_number(input), expected);
        }
    }

    #[test]
    fn episode_title_without_number_preserves_non_prefix_numbers() {
        for title in ["Episode 12", "2026 election", "The 1.5 hour episode"] {
            assert_eq!(episode_title_without_number(title), title);
        }
    }
}
