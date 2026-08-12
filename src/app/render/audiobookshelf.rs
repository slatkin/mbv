use super::detail::render_hero_title_row;
use super::detail_series::{
    SERIES_DETAIL_DIVIDER_ROWS, SERIES_DETAIL_EPISODE_ROWS_ESTIMATE,
    SERIES_DETAIL_TRAILING_BLANK_ROWS, SERIES_IMAGE_COLS, SERIES_IMAGE_PLACEHOLDER_ROWS,
    SERIES_IMAGE_ROWS,
};
use super::list::{
    hero_block_shell, top_hero_layout, HERO_BLOCK_EXTRA_ROWS, HERO_PLACEHOLDER_ROWS,
    HERO_TITLE_ROWS,
};
use super::list_rows::{
    draw_column_selection_markers, focused_or_subtle, item_cell_spans, SELECTED_BLOCK_SIDE_PADDING,
};
use super::{render_pill_bar, render_placeholder, PillBar, RENDER_FILTER};
use crate::app::images::audiobookshelf_cover_cache_key;
use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{
    library_cell_width, library_column_count, LIBRARY_COLUMN_GAP,
};
use crate::app::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter;
use crate::app::ui_util::{fmt_duration_approx, trunc_str};
use crate::app::{palette, App};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, List, ListItem, Paragraph, Row, Table, TableState};
use ratatui::Frame;

impl App {
    pub(super) fn render_audiobookshelf_podcasts(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get(index).cloned() else {
            render_placeholder(f, area, "Audiobookshelf loading…");
            return;
        };

        let cols = library_column_count(area.width);
        let desired_rows = state
            .selected_show()
            .map(|_| self.audiobookshelf_hero_content_rows(index, cols > 1) + HERO_BLOCK_EXTRA_ROWS)
            .unwrap_or(HERO_PLACEHOLDER_ROWS);
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
            self.render_audiobookshelf_hero(f, content, index, focused, cols > 1, layout);
        }

        if state.shows.is_empty() {
            render_placeholder(
                f,
                top.list_area,
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

        self.render_audiobookshelf_show_rows(f, top.list_area, index, focused, cols, layout);
    }

    fn audiobookshelf_hero_content_rows(&self, index: usize, show_title: bool) -> u16 {
        let state = &self.audiobookshelf_browse[index];
        let mut rows = HERO_TITLE_ROWS.saturating_mul(show_title as u16);
        rows += state
            .selected_show()
            .and_then(|show| show.author.as_ref())
            .is_some() as u16;
        rows += 1 + SERIES_DETAIL_DIVIDER_ROWS as u16;
        if state.episode_selection.is_some() {
            rows += state
                .episodes
                .as_ref()
                .map(|_| state.visible_episodes().len())
                .unwrap_or(SERIES_DETAIL_EPISODE_ROWS_ESTIMATE) as u16;
        }
        rows += SERIES_DETAIL_TRAILING_BLANK_ROWS as u16;
        if self.images_enabled() {
            rows = rows.max(SERIES_IMAGE_ROWS + show_title as u16);
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
        layout: &mut LayoutMain,
    ) {
        let Some(state) = self.audiobookshelf_browse.get(index).cloned() else {
            return;
        };
        let Some(show) = state.selected_show().cloned() else {
            return;
        };
        let max_y = area.y + area.height;
        let mut row = area.y;
        if show_title {
            row = render_hero_title_row(
                f,
                area.x,
                row,
                max_y,
                area.width.saturating_sub(1),
                &show.title,
                focused,
            );
        }

        let image_start = row;
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
        if self.images_enabled() {
            if let Some(server) = server_url {
                self.fetch_audiobookshelf_cover(server, show.library_item_id.clone());
            }
        }
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
        let image_x = area.x + area.width.saturating_sub(image_width);
        let image_end = image_start + image_height;
        let text_width = |current_row: u16| {
            if image_height > 0 && current_row >= image_start && current_row < image_end {
                area.width.saturating_sub(image_width)
            } else {
                area.width
            }
        };

        if let Some(author) = show.author.as_deref() {
            if row < max_y {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        trunc_str(author, text_width(row) as usize),
                        Style::default().fg(palette::SUBTLE),
                    )),
                    Rect {
                        x: area.x,
                        y: row,
                        width: text_width(row),
                        height: 1,
                    },
                );
                row += 1;
            }
        }
        row = (row + 1).min(max_y);

        if image_height > 0 {
            let image_rect = Rect {
                x: image_x,
                y: image_start,
                width: image_width,
                height: image_height,
            };
            layout.inline_image_rect = Some(image_rect);
            if placeholder {
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::OVERLAY)),
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

        if row < max_y {
            let prefix_width = "Series: ".chars().count() as u16;
            f.render_widget(
                Paragraph::new(Span::styled(
                    "Series: ",
                    Style::default()
                        .fg(palette::YELLOW)
                        .add_modifier(Modifier::BOLD),
                )),
                Rect {
                    x: area.x,
                    y: row,
                    width: prefix_width,
                    height: 1,
                },
            );
            if state.episode_selection.is_some() {
                let labels = AudiobookshelfEpisodeFilter::ALL
                    .iter()
                    .map(|filter| filter.label().to_string())
                    .collect::<Vec<_>>();
                let ids = (0..labels.len()).collect::<Vec<_>>();
                layout.selector_tabs = render_pill_bar(
                    f,
                    Rect {
                        x: area.x + prefix_width,
                        y: row,
                        width: text_width(row).saturating_sub(prefix_width),
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
            } else {
                f.render_widget(
                    Paragraph::new(Span::styled("3", Style::default().fg(palette::SOFT_WHITE))),
                    Rect {
                        x: area.x + prefix_width,
                        y: row,
                        width: text_width(row).saturating_sub(prefix_width),
                        height: 1,
                    },
                );
            }
            row += 1;
        }

        if state.episode_selection.is_some() && row < max_y {
            let table_area = Rect {
                x: area.x,
                y: row,
                width: text_width(row),
                height: max_y
                    .saturating_sub(row)
                    .saturating_sub(SERIES_DETAIL_TRAILING_BLANK_ROWS as u16),
            };
            self.render_audiobookshelf_episode_rows(f, table_area, &state, focused);
        }
    }

    fn render_audiobookshelf_episode_rows(
        &self,
        f: &mut Frame,
        area: Rect,
        state: &crate::app::types_audiobookshelf_browse::AudiobookshelfBrowseState,
        focused: bool,
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
                let published = episode
                    .published_at
                    .as_deref()
                    .map(|value| format!(" · {value}"))
                    .unwrap_or_default();
                let progress = state
                    .progress
                    .get(&(episode.library_item_id.clone(), episode.episode_id.clone()))
                    .map(|progress| {
                        if progress.is_finished {
                            " · Played".to_string()
                        } else if progress.current_time_seconds > 0.0 {
                            episode
                                .duration_seconds
                                .filter(|duration| *duration > 0.0)
                                .map(|duration| {
                                    format!(
                                        " · {}%",
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
                let title = trunc_str(
                    &format!("{}. {}{}{}", index + 1, episode.title, published, progress),
                    title_width,
                );
                let title_cell = Cell::from(Line::from(vec![marker, Span::raw(title)]));
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
    }

    fn render_audiobookshelf_show_rows(
        &mut self,
        f: &mut Frame,
        area: Rect,
        index: usize,
        focused: bool,
        cols: usize,
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
        let visible = area.height as usize;
        let lower = (cursor_row + 1).saturating_sub(visible).min(cursor_row);
        state.scroll = state.scroll.clamp(lower, cursor_row);
        let scroll = state.scroll;
        let cell_width = library_cell_width(area, cols) as usize;
        let items = rows
            .iter()
            .skip(scroll)
            .take(visible)
            .map(|indices| {
                let mut spans = Vec::new();
                for (cell, index) in indices.iter().enumerate() {
                    let selected = *index == cursor;
                    let title = trunc_str(&state.shows[*index].title, cell_width.saturating_sub(2));
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
            .take(visible)
            .map(|row| row.first().copied())
            .collect();
        layout.left_item_rows = rows;
        layout.left_screen_offset = scroll;
        layout.cursor_screen_y = Some(area.y + cursor_row.saturating_sub(scroll) as u16);
        f.render_widget(List::new(items), area);
        if focused && layout.left_item_rows.len() > visible {
            super::render_right_scrollbar(
                f,
                area,
                layout.left_item_rows.len().saturating_sub(visible),
                scroll,
            );
        }
        draw_column_selection_markers(f, area, cursor, cols, &layout.left_item_rows, scroll);
    }
}
