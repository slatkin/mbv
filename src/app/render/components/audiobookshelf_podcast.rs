use crate::app::render::arrangements::hero_left;
use crate::app::render::arrangements::hero_left::{
    hero_on_left_list_panel_border, PANE_PAD_X, PANE_PAD_Y,
};
use crate::app::render::arrangements::padded_rect;
use crate::app::render::components::detail_series_view::{
    SERIES_DETAIL_DIVIDER_ROWS, SERIES_DETAIL_EPISODE_ROWS_ESTIMATE,
    SERIES_DETAIL_TRAILING_BLANK_ROWS, SERIES_IMAGE_COLS, SERIES_IMAGE_ROWS,
};
use crate::app::render::components::hero::{
    inline_detail_flow, inline_display_row, inline_display_row_count, inline_hero_text_width,
    selected_detail_shell, wrap_overview_lines, HeroContent, HeroImage, HeroLine,
    HERO_BLOCK_EXTRA_ROWS, HERO_TITLE_ROWS,
};
use crate::app::render::components::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::render::{render_pill_bar, render_placeholder, HomeImagePaint, PillBar};
use crate::app::types_audiobookshelf_browse::{
    build_show_title_buckets, AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter,
};
use crate::app::ui_util::trunc_str;
use unicode_width::UnicodeWidthStr;

/// Podcast hero content row budget, shared by the legacy `App` narrow
/// renderer and `AudiobookshelfPodcastComponent`'s narrow path so both admit
/// the same inline-detail height. Mirrors the prior
/// `App::audiobookshelf_hero_content_rows` behavior exactly: title row,
/// optional author row, blank before a nonempty description, wrapped
/// description (capped at four rows) using `wrap_overview_lines` +
/// `inline_hero_text_width` with the image dimensions, episode
/// divider/visible-or-estimated episode rows, trailing blank, and the
/// image-height minimum when images are enabled.
pub(in crate::app::render) fn podcast_hero_content_rows(
    state: &AudiobookshelfBrowseState,
    interaction: PodcastInteraction,
    show_title: bool,
    width: u16,
    images_enabled: bool,
) -> u16 {
    let title_rows = HERO_TITLE_ROWS.saturating_mul(show_title as u16);
    let author_rows = state
        .selected_show()
        .and_then(|show| show.author.as_ref())
        .is_some() as u16;
    let mut rows = title_rows + author_rows;
    if let Some(description) = state
        .selected_show()
        .and_then(|show| show.description.as_deref())
        .filter(|description| !description.is_empty())
    {
        rows += 1;
        let (image_width, image_height) = if images_enabled {
            (SERIES_IMAGE_COLS, SERIES_IMAGE_ROWS)
        } else {
            (0, 0)
        };
        let description_start = title_rows + author_rows + 1;
        rows += wrap_overview_lines(description, |line| {
            let row = description_start + line as u16;
            inline_hero_text_width(width, image_width, image_height, row) as usize
        })
        .len()
        .min(4) as u16;
    }
    if interaction.episode_selection.is_some() {
        rows += 1 + SERIES_DETAIL_DIVIDER_ROWS as u16;
        rows += state
            .episodes
            .as_ref()
            .map(|_| state.visible_episodes(interaction.episode_filter).len())
            .unwrap_or(SERIES_DETAIL_EPISODE_ROWS_ESTIMATE) as u16;
    }
    rows += SERIES_DETAIL_TRAILING_BLANK_ROWS as u16;
    if images_enabled {
        rows = rows.max(SERIES_IMAGE_ROWS + 1);
    }
    rows
}
use crate::app::{library_column_width, palette};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Paragraph};
use ratatui::Frame;

/// The component-owned interaction values the podcast renderer needs, passed
/// in rather than read off the projected content type
/// (split-browse-state-interaction-fields task 3.2).
#[derive(Clone, Copy)]
pub(in crate::app) struct PodcastInteraction {
    pub episode_filter: AudiobookshelfEpisodeFilter,
    pub episode_selection: Option<usize>,
}

/// Geometry painted by the podcast component. Input uses this same geometry,
/// so selector and show targets cannot drift from the rendered surface.
#[derive(Default)]
pub(in crate::app) struct AudiobookshelfPodcastGeometry {
    /// Column count used by the painted show grid and keyboard navigation.
    pub columns: usize,
    pub selector_tabs: Vec<(Rect, usize)>,
    pub pill_bar_area: Rect,
    pub show_rows: Vec<(Rect, usize)>,
    pub episode_rows: Vec<(Rect, usize)>,
    /// Painted list/browser area: the wide right panel, or the narrow content
    /// area below the pill bar. Mirrors the legacy `LayoutMain.left_area` so
    /// the shell can anchor overlays after render ownership moved to the
    /// component (task 5.3d.10c).
    pub list_area: Rect,
    /// Wide-only right panel rect; zero in the narrow layout. Mirrors the
    /// legacy `LayoutMain.audiobookshelf_podcast_right_area`.
    pub right_area: Rect,
    /// Hero rect the component painted (wide hero panel, or narrow
    /// inline-detail hero). Zero when no hero was painted.
    pub hero_area: Rect,
    /// Narrow-only inline hero rect; zero in the wide layout or when the
    /// inline hero was rejected. Equals `hero_area` when set.
    pub inline_hero_area: Rect,
    /// Selected-item rect the component painted (only the narrow inline hero
    /// shell today; `None` in the wide layout, which has no selected-item
    /// shell). Mirrors the legacy `LayoutMain.selected_item_rect`.
    pub selected_item_rect: Option<Rect>,
    /// Full-width selected show panel in the wide rail.
    pub selected_panel_rect: Option<Rect>,
}

pub(in crate::app) fn render_audiobookshelf_podcast_content(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    images_enabled: bool,
    state: &mut AudiobookshelfBrowseState,
    interaction: PodcastInteraction,
    scroll: &mut usize,
    geometry: &mut AudiobookshelfPodcastGeometry,
) -> Option<HomeImagePaint> {
    *geometry = AudiobookshelfPodcastGeometry::default();
    let Some((hero_panel, right_panel)) = hero_left::shared_hero_presentation(area) else {
        return render_narrow_podcast(
            frame,
            area,
            focused,
            images_enabled,
            state,
            interaction,
            scroll,
            geometry,
        );
    };

    // Wide hero: title lives in the right show-list panel, so the hero
    // body carries only author/description/image (matches legacy
    // `render_audiobookshelf_hero` `show_title = false`). Persistent-
    // mode episode pills + table are wide-only (narrow routes Enter to
    // the selection modal instead).
    let image_paint = render_podcast_hero(
        frame,
        hero_panel,
        state,
        interaction,
        focused,
        false,
        images_enabled,
        true,
        geometry,
    );
    // Wide layout: the list/browser occupies the right panel; the hero panel
    // is the painted hero. No inline hero and no selected-item shell exist in
    // this layout (the right panel paints an ordinary show grid).
    geometry.right_area = right_panel;
    geometry.columns = 1;
    let right_pane = hero_left::hero_on_left_right_pane(right_panel, right_panel, PANE_PAD_Y);
    frame.render_widget(
        ratatui::widgets::Block::default()
            .style(crate::app::palette::resolve_surface_focus(focused)),
        right_panel,
    );
    let buckets = build_show_title_buckets(&state.shows);
    let labels: Vec<String> = buckets.iter().map(|bucket| bucket.label.into()).collect();
    let ids: Vec<usize> = (0..labels.len()).collect();
    let selected_bucket = buckets
        .iter()
        .position(|bucket| state.cursor() >= bucket.start && state.cursor() < bucket.end)
        .unwrap_or(0);
    geometry.pill_bar_area = right_pane.pills_area;
    geometry.selector_tabs = render_pill_bar(
        frame,
        right_pane.pills_area,
        PillBar {
            labels: &labels,
            ids: &ids,
            selected_pos: selected_bucket,
            prefix: Some(" ⌘ "),
        },
    );
    let browser = padded_rect(right_pane.list_panel, PANE_PAD_X, PANE_PAD_Y);
    geometry.list_area = browser;
    if state.shows.is_empty() {
        render_placeholder(frame, browser, "No podcast shows");
        return image_paint;
    }
    if state.selected_show().is_some() {
        geometry.hero_area = hero_panel;
    }
    render_show_rows(
        frame,
        browser,
        Some(right_pane.list_panel),
        focused,
        state,
        1,
        0,
        *scroll,
        geometry,
    );
    hero_on_left_list_panel_border(frame, right_pane.list_panel, focused);
    image_paint
}

#[allow(clippy::too_many_arguments)]
fn render_narrow_podcast(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    images_enabled: bool,
    state: &mut AudiobookshelfBrowseState,
    interaction: PodcastInteraction,
    scroll: &mut usize,
    geometry: &mut AudiobookshelfPodcastGeometry,
) -> Option<HomeImagePaint> {
    if state.shows.is_empty() {
        render_placeholder(
            frame,
            area,
            state.error.as_deref().unwrap_or("No podcast shows"),
        );
        // Narrow with no shows: the whole area is the (empty) browser; no
        // hero, right panel, or selected-item shell exists.
        geometry.list_area = area;
        return None;
    }
    let parts = hero_left::pill_bar_areas(area);
    let buckets = build_show_title_buckets(&state.shows);
    let selected_bucket = buckets
        .iter()
        .position(|bucket| state.cursor() >= bucket.start && state.cursor() < bucket.end)
        .unwrap_or(0);
    let labels: Vec<String> = buckets.iter().map(|bucket| bucket.label.into()).collect();
    let ids: Vec<usize> = (0..labels.len()).collect();
    geometry.pill_bar_area = parts.pills_area;
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

    let list_area = parts.content_area;
    geometry.list_area = list_area;
    geometry.columns = library_column_width::library_column_count(list_area.width).max(1);
    let hero_content_width = list_area
        .width
        .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING);
    let desired_rows =
        podcast_hero_content_rows(state, interaction, true, hero_content_width, images_enabled)
            + HERO_BLOCK_EXTRA_ROWS;
    let hero_rows = if desired_rows >= HERO_BLOCK_EXTRA_ROWS && desired_rows < list_area.height {
        desired_rows
    } else {
        0
    };
    render_show_rows(
        frame,
        list_area,
        None,
        focused,
        state,
        geometry.columns,
        hero_rows,
        *scroll,
        geometry,
    );
    if hero_rows == 0 {
        return None;
    }
    let cursor_row = state.cursor() / geometry.columns.max(1);
    let flow = inline_detail_flow(cursor_row, hero_rows, list_area.height, *scroll)?;
    *scroll = flow.offset;
    let hero_area = Rect {
        x: list_area.x,
        y: list_area.y + flow.detail_screen_row as u16,
        width: list_area.width,
        height: hero_rows,
    };
    selected_detail_shell(frame, hero_area, hero_rows, focused);
    // Narrow inline hero admitted: the painted hero is both the inline hero
    // and the selected-item shell the shell anchors overlays to.
    geometry.hero_area = hero_area;
    geometry.inline_hero_area = hero_area;
    geometry.selected_item_rect = Some(hero_area);
    // Narrow inline hero: title is painted (the selected show row is
    // replaced, so the hero must carry its own title); persistent-
    // mode episode pills + table are suppressed (Enter opens the
    // selection modal instead). Matches legacy `show_title = true`,
    // `persistent = false`.
    render_podcast_hero(
        frame,
        hero_area,
        state,
        interaction,
        focused,
        true,
        images_enabled,
        false,
        geometry,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_podcast_hero(
    frame: &mut Frame,
    area: Rect,
    state: &AudiobookshelfBrowseState,
    interaction: PodcastInteraction,
    focused: bool,
    show_title: bool,
    images_enabled: bool,
    wide: bool,
    geometry: &mut AudiobookshelfPodcastGeometry,
) -> Option<HomeImagePaint> {
    let show = state.selected_show()?;
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
            image: images_enabled.then_some(HeroImage {
                actual_w: SERIES_IMAGE_COLS,
                height: SERIES_IMAGE_ROWS,
            }),
        },
        focused,
    );
    // Episode filter pills + table are wide-only (`persistent` legacy
    // gate); narrow routes Enter to the selection modal instead, so
    // `episode_selection` is never set in narrow in practice.
    if wide && interaction.episode_selection.is_some() && result.next_row < area.bottom() {
        let filter = interaction.episode_filter;
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
        for (index, episode) in state.visible_episodes(filter).iter().enumerate() {
            if row_y + index as u16 >= area.bottom() {
                break;
            }
            let row = Rect {
                x: area.x,
                y: row_y + index as u16,
                width: area.width,
                height: 1,
            };
            let marker = if interaction.episode_selection == Some(index) {
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
    (images_enabled && result.img_rect.is_some()).then(|| HomeImagePaint::AudiobookshelfCover {
        area: result.img_rect.unwrap(),
        library_item_id: show.library_item_id.clone(),
        show_placeholder: true,
    })
}

fn render_show_rows(
    frame: &mut Frame,
    area: Rect,
    selected_panel: Option<Rect>,
    focused: bool,
    state: &AudiobookshelfBrowseState,
    cols: usize,
    hero_rows: u16,
    scroll: usize,
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
        inline_detail_flow(cursor_row, hero_rows, area.height, scroll)
            .map(|flow| flow.offset)
            .unwrap_or(0)
    } else {
        scroll.min(total_display.saturating_sub(area.height as usize))
    };
    let items: Vec<ListItem> = (scroll..total_display)
        .take(area.height as usize)
        .enumerate()
        .map(|(screen_row, display_row)| {
            match inline_display_row(rows.len(), cursor_row, hero_rows, display_row) {
                Some(crate::app::render::components::hero::InlineDisplayRow::Replacement) => {
                    ListItem::new(Line::default())
                }
                Some(crate::app::render::components::hero::InlineDisplayRow::Source(
                    source_row,
                )) => {
                    let cell_width = library_column_width::library_cell_width(area, cols) as usize;
                    let text = rows[source_row]
                        .iter()
                        .enumerate()
                        .map(|(column, index)| {
                            let selected = *index == state.cursor();
                            if selected {
                                let panel = selected_panel.map(|panel| Rect {
                                    x: panel.x,
                                    y: area.y + screen_row as u16,
                                    width: panel.width,
                                    height: 1,
                                });
                                if let Some(panel) = panel {
                                    frame.render_widget(
                                        Block::default().style(
                                            Style::default()
                                                .bg(palette::resolve_surface_focus(focused)),
                                        ),
                                        panel,
                                    );
                                    geometry.selected_panel_rect = Some(panel);
                                }
                                let x = area.x
                                    + column as u16
                                        * (cell_width as u16
                                            + library_column_width::LIBRARY_COLUMN_GAP);
                                let width = if column + 1 < rows[source_row].len() {
                                    cell_width as u16 + library_column_width::LIBRARY_COLUMN_GAP
                                } else {
                                    cell_width as u16
                                };
                                frame.render_widget(
                                    Block::default().style(
                                        Style::default()
                                            .bg(palette::resolve_surface_focus(focused)),
                                    ),
                                    ratatui::layout::Rect {
                                        x,
                                        y: area.y + screen_row as u16,
                                        width,
                                        height: 1,
                                    },
                                );
                            }
                            let marker = if selected && focused { "> " } else { "  " };
                            let title_width = cell_width.saturating_sub(marker.width());
                            let cell = format!(
                                "{marker}{}",
                                trunc_str(&state.shows[*index].title, title_width)
                            );
                            let padding = " ".repeat(cell_width.saturating_sub(cell.width()));
                            if column + 1 < rows[source_row].len() {
                                format!(
                                    "{cell}{padding}{}",
                                    " ".repeat(library_column_width::LIBRARY_COLUMN_GAP as usize)
                                )
                            } else {
                                format!("{cell}{padding}")
                            }
                        })
                        .collect::<String>();
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
            let cell_width = library_column_width::library_cell_width(area, cols);
            for (column, index) in rows[source_row].iter().enumerate() {
                geometry.show_rows.push((
                    Rect {
                        x: area.x
                            + column as u16
                                * (cell_width + library_column_width::LIBRARY_COLUMN_GAP),
                        y: area.y + screen_row as u16,
                        width: cell_width,
                        height: 1,
                    },
                    *index,
                ));
            }
        }
    }
    frame.render_widget(List::new(items), area);
}

#[cfg(test)]
mod tests {
    use super::{podcast_hero_content_rows, PodcastInteraction};
    use crate::app::render::components::detail_series_view::{
        SERIES_DETAIL_DIVIDER_ROWS, SERIES_DETAIL_EPISODE_ROWS_ESTIMATE,
        SERIES_DETAIL_TRAILING_BLANK_ROWS, SERIES_IMAGE_COLS, SERIES_IMAGE_ROWS,
    };
    use crate::app::render::components::hero::{
        inline_hero_text_width, wrap_overview_lines, HERO_TITLE_ROWS,
    };
    use crate::app::types_audiobookshelf_browse::{
        AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter,
    };
    use mbv_core::audiobookshelf::{
        AudiobookshelfDownloadedEpisode, AudiobookshelfLibrary, AudiobookshelfShow,
    };

    fn interaction(episode_selection: Option<usize>) -> PodcastInteraction {
        PodcastInteraction {
            episode_filter: AudiobookshelfEpisodeFilter::All,
            episode_selection,
        }
    }

    fn make_state(show: AudiobookshelfShow) -> AudiobookshelfBrowseState {
        let library = AudiobookshelfLibrary {
            id: "lib".into(),
            name: "Podcasts".into(),
            media_type: "podcast".into(),
        };
        let mut state = AudiobookshelfBrowseState::new(library);
        state.append_page(0, 20, 1, vec![show]);
        state.select(0);
        state
    }

    /// Independent oracle reproducing the pre-extraction
    /// `App::audiobookshelf_hero_content_rows` body, so the shared helper is
    /// proved equivalent to the legacy rule it replaces (author, long
    /// description, episode, and image cases all shift the budget exactly as
    /// before).
    fn legacy_hero_content_rows(
        state: &AudiobookshelfBrowseState,
        interaction: PodcastInteraction,
        show_title: bool,
        width: u16,
        images_enabled: bool,
    ) -> u16 {
        let title_rows = HERO_TITLE_ROWS.saturating_mul(show_title as u16);
        let author_rows = state
            .selected_show()
            .and_then(|show| show.author.as_ref())
            .is_some() as u16;
        let mut rows = title_rows + author_rows;
        if let Some(description) = state
            .selected_show()
            .and_then(|show| show.description.as_deref())
            .filter(|description| !description.is_empty())
        {
            rows += 1;
            let (image_width, image_height) = if images_enabled {
                (SERIES_IMAGE_COLS, SERIES_IMAGE_ROWS)
            } else {
                (0, 0)
            };
            let description_start = title_rows + author_rows + 1;
            rows += wrap_overview_lines(description, |line| {
                let row = description_start + line as u16;
                inline_hero_text_width(width, image_width, image_height, row) as usize
            })
            .len()
            .min(4) as u16;
        }
        if interaction.episode_selection.is_some() {
            rows += 1 + SERIES_DETAIL_DIVIDER_ROWS as u16;
            rows += state
                .episodes
                .as_ref()
                .map(|_| state.visible_episodes(interaction.episode_filter).len())
                .unwrap_or(SERIES_DETAIL_EPISODE_ROWS_ESTIMATE) as u16;
        }
        rows += SERIES_DETAIL_TRAILING_BLANK_ROWS as u16;
        if images_enabled {
            rows = rows.max(SERIES_IMAGE_ROWS + 1);
        }
        rows
    }

    fn assert_matches_legacy(
        state: &AudiobookshelfBrowseState,
        interaction: PodcastInteraction,
        width: u16,
        images_enabled: bool,
    ) {
        let got = podcast_hero_content_rows(state, interaction, true, width, images_enabled);
        let expected = legacy_hero_content_rows(state, interaction, true, width, images_enabled);
        assert_eq!(got, expected, "shared helper must match legacy rule");
    }

    #[test]
    fn narrow_podcast_budget_matches_legacy_for_author_only() {
        let state = make_state(AudiobookshelfShow {
            library_item_id: "s".into(),
            title: "Show".into(),
            author: Some("Author".into()),
            description: None,
            cover_path: None,
        });
        // title(1) + author(1) + trailing(1) = 3; no image minimum.
        assert_eq!(
            podcast_hero_content_rows(&state, interaction(None), true, 40, false),
            3
        );
        assert_matches_legacy(&state, interaction(None), 40, false);
    }

    #[test]
    fn narrow_podcast_budget_matches_legacy_for_long_description() {
        let state = make_state(AudiobookshelfShow {
            library_item_id: "s".into(),
            title: "Show".into(),
            author: Some("Author".into()),
            description: Some("word ".repeat(80)),
            cover_path: None,
        });
        assert_matches_legacy(&state, interaction(None), 40, false);
    }

    #[test]
    fn narrow_podcast_budget_matches_legacy_for_episodes() {
        let mut state = make_state(AudiobookshelfShow {
            library_item_id: "s".into(),
            title: "Show".into(),
            author: None,
            description: None,
            cover_path: None,
        });
        state.episodes = Some(vec![
            AudiobookshelfDownloadedEpisode {
                library_item_id: "s".into(),
                episode_id: "e1".into(),
                title: "E1".into(),
                published_at: None,
                duration_seconds: None,
            },
            AudiobookshelfDownloadedEpisode {
                library_item_id: "s".into(),
                episode_id: "e2".into(),
                title: "E2".into(),
                published_at: None,
                duration_seconds: None,
            },
            AudiobookshelfDownloadedEpisode {
                library_item_id: "s".into(),
                episode_id: "e3".into(),
                title: "E3".into(),
                published_at: None,
                duration_seconds: None,
            },
        ]);
        assert_matches_legacy(&state, interaction(Some(0)), 40, false);
    }

    #[test]
    fn narrow_podcast_budget_matches_legacy_for_images_minimum() {
        let state = make_state(AudiobookshelfShow {
            library_item_id: "s".into(),
            title: "Show".into(),
            author: None,
            description: None,
            cover_path: None,
        });
        // Images enabled lifts even a title-only budget to SERIES_IMAGE_ROWS+1.
        assert_eq!(
            podcast_hero_content_rows(&state, interaction(None), true, 40, true),
            SERIES_IMAGE_ROWS + 1
        );
        assert_matches_legacy(&state, interaction(None), 40, true);
    }
}
