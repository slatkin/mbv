use crate::app::render::arrangements::hero_left;
use crate::app::render::components::hero::{
    inline_detail_flow, inline_display_row, inline_display_row_count, selected_detail_shell,
    HeroContent, HeroLine, HERO_BLOCK_EXTRA_ROWS, HERO_TITLE_ROWS,
};
use crate::app::render::components::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::render::{render_pill_bar, render_placeholder, PillBar};
use crate::app::types_audiobookshelf_browse::{
    build_show_title_buckets, AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter,
};
use crate::app::{library_column_width, palette};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::Frame;

/// Geometry painted by the podcast component. Input uses this same geometry,
/// so selector and show targets cannot drift from the rendered surface.
#[derive(Default)]
pub(in crate::app) struct AudiobookshelfPodcastGeometry {
    pub selector_tabs: Vec<(Rect, usize)>,
    pub show_rows: Vec<(Rect, usize)>,
    pub episode_rows: Vec<(Rect, usize)>,
}

pub(in crate::app) fn render_audiobookshelf_podcast_content(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    state: &mut AudiobookshelfBrowseState,
    geometry: &mut AudiobookshelfPodcastGeometry,
) {
    *geometry = AudiobookshelfPodcastGeometry::default();
    let Some((hero_panel, right_panel)) = hero_left::shared_hero_presentation(area) else {
        render_narrow_podcast(frame, area, focused, state, geometry);
        return;
    };

    render_podcast_hero(frame, hero_panel, state, focused, true, geometry);
    if state.shows.is_empty() {
        render_placeholder(frame, right_panel, "No podcast shows");
        return;
    }
    render_show_rows(frame, right_panel, focused, state, 1, 0, geometry);
}

fn render_narrow_podcast(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    state: &mut AudiobookshelfBrowseState,
    geometry: &mut AudiobookshelfPodcastGeometry,
) {
    if state.shows.is_empty() {
        render_placeholder(
            frame,
            area,
            state.error.as_deref().unwrap_or("No podcast shows"),
        );
        return;
    }
    let parts = hero_left::pill_bar_areas(area);
    let buckets = build_show_title_buckets(&state.shows);
    let selected_bucket = buckets
        .iter()
        .position(|bucket| state.cursor() >= bucket.start && state.cursor() < bucket.end)
        .unwrap_or(0);
    let labels: Vec<String> = buckets.iter().map(|bucket| bucket.label.into()).collect();
    let ids: Vec<usize> = (0..labels.len()).collect();
    geometry.selector_tabs = render_pill_bar(
        frame,
        parts.pills_area,
        PillBar {
            labels: &labels,
            ids: &ids,
            selected_pos: selected_bucket,
            prefix: Some(" ⌘ "),
        },
    );

    let hero_rows = HERO_TITLE_ROWS + HERO_BLOCK_EXTRA_ROWS;
    let list_area = parts.content_area;
    render_show_rows(
        frame,
        list_area,
        focused,
        state,
        library_column_width::library_column_count(list_area.width),
        hero_rows,
        geometry,
    );
    if hero_rows >= list_area.height {
        return;
    }
    let cursor_row =
        state.cursor() / library_column_width::library_column_count(list_area.width).max(1);
    let Some(flow) = inline_detail_flow(cursor_row, hero_rows, list_area.height, state.scroll)
    else {
        return;
    };
    state.scroll = flow.offset;
    let hero_area = Rect {
        x: list_area.x,
        y: list_area.y + flow.detail_screen_row as u16,
        width: list_area.width,
        height: hero_rows,
    };
    selected_detail_shell(frame, hero_area, hero_rows, focused);
    render_podcast_hero(frame, hero_area, state, focused, false, geometry);
}

fn render_podcast_hero(
    frame: &mut Frame,
    area: Rect,
    state: &AudiobookshelfBrowseState,
    focused: bool,
    show_title: bool,
    geometry: &mut AudiobookshelfPodcastGeometry,
) {
    let Some(show) = state.selected_show() else {
        return;
    };
    let mut lines = Vec::new();
    if let Some(author) = &show.author {
        lines.push(HeroLine::Plain(author.clone()));
    }
    if let Some(description) = &show.description {
        if !description.is_empty() {
            lines.push(HeroLine::Plain(String::new()));
            lines.push(HeroLine::Plain(description.clone()));
        }
    }
    lines.push(HeroLine::Plain(String::new()));
    let result = crate::app::render::components::hero::paint_hero_content(
        frame,
        Rect {
            x: area.x + SELECTED_BLOCK_SIDE_PADDING,
            y: area.y + SELECTED_BLOCK_SIDE_PADDING,
            width: area.width.saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
            height: area.height.saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
        },
        &HeroContent {
            title: show_title.then_some(show.title.as_str()),
            meta_line: None,
            meta_color: palette::TEXT_SECONDARY,
            show_playing: false,
            unconditional_spacer_after_meta: false,
            lines: &lines,
            image: None,
        },
        focused,
    );
    if state.episode_selection.is_some() && result.next_row < area.bottom() {
        let filter = state.episode_filter;
        let labels: Vec<String> = AudiobookshelfEpisodeFilter::ALL
            .iter()
            .map(|filter| filter.label().into())
            .collect();
        let ids: Vec<usize> = (0..labels.len()).collect();
        let tabs = render_pill_bar(
            frame,
            Rect {
                x: area.x,
                y: result.next_row,
                width: area.width,
                height: 1,
            },
            PillBar {
                labels: &labels,
                ids: &ids,
                selected_pos: AudiobookshelfEpisodeFilter::ALL
                    .iter()
                    .position(|candidate| *candidate == filter)
                    .unwrap_or(0),
                prefix: Some(" ⌘ "),
            },
        );
        geometry.selector_tabs.extend(tabs);
        let row_y = result.next_row + 1;
        for (index, episode) in state.visible_episodes().iter().enumerate() {
            if row_y + index as u16 >= area.bottom() {
                break;
            }
            let row = Rect {
                x: area.x,
                y: row_y + index as u16,
                width: area.width,
                height: 1,
            };
            let marker = if state.episode_selection == Some(index) {
                "> "
            } else {
                "  "
            };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(marker, Style::default().fg(palette::TEXT_FOCUS_ACCENT)),
                    Span::raw(episode.title.clone()),
                ])),
                row,
            );
            geometry.episode_rows.push((row, index));
        }
    }
}

fn render_show_rows(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    state: &AudiobookshelfBrowseState,
    cols: usize,
    hero_rows: u16,
    geometry: &mut AudiobookshelfPodcastGeometry,
) {
    let cols = cols.max(1);
    let rows: Vec<Vec<usize>> = state
        .shows
        .iter()
        .enumerate()
        .collect::<Vec<_>>()
        .chunks(cols)
        .map(|chunk| chunk.iter().map(|(index, _)| *index).collect())
        .collect();
    let cursor_row = rows
        .iter()
        .position(|row| row.contains(&state.cursor()))
        .unwrap_or(0);
    let total_display = inline_display_row_count(rows.len(), cursor_row, hero_rows);
    let scroll = if hero_rows > 0 {
        inline_detail_flow(cursor_row, hero_rows, area.height, state.scroll)
            .map(|flow| flow.offset)
            .unwrap_or(0)
    } else {
        state
            .scroll
            .min(total_display.saturating_sub(area.height as usize))
    };
    let items: Vec<ListItem> = (scroll..total_display)
        .take(area.height as usize)
        .map(|display_row| {
            match inline_display_row(rows.len(), cursor_row, hero_rows, display_row) {
                Some(crate::app::render::components::hero::InlineDisplayRow::Replacement) => {
                    ListItem::new(Line::default())
                }
                Some(crate::app::render::components::hero::InlineDisplayRow::Source(
                    source_row,
                )) => {
                    let text = rows[source_row]
                        .iter()
                        .map(|index| {
                            let marker = if *index == state.cursor() && focused {
                                "> "
                            } else {
                                "  "
                            };
                            format!("{marker}{}", state.shows[*index].title)
                        })
                        .collect::<Vec<_>>()
                        .join("  ");
                    ListItem::new(text)
                }
                None => ListItem::new(Line::default()),
            }
        })
        .collect();
    for (screen_row, display_row) in (scroll..total_display)
        .take(area.height as usize)
        .enumerate()
    {
        if let Some(crate::app::render::components::hero::InlineDisplayRow::Source(source_row)) =
            inline_display_row(rows.len(), cursor_row, hero_rows, display_row)
        {
            for index in &rows[source_row] {
                geometry.show_rows.push((
                    Rect {
                        y: area.y + screen_row as u16,
                        height: 1,
                        ..area
                    },
                    *index,
                ));
            }
        }
    }
    frame.render_widget(List::new(items), area);
}
