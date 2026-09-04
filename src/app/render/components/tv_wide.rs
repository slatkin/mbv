use crate::app::components::media_list::WideMediaList;
use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::hero_left::{self, PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::arrangements::library as library_arrangement;
use crate::app::render::arrangements::padded_rect;
use crate::app::render::components::hero::{wrap_overview_lines, HeroContent};
use crate::app::render::components::hero_model::{Hero, HeroArtwork, HeroArtworkAspect};
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::HomeImagePaint;
use crate::app::render::{render_pill_bar, render_placeholder, MarkerEdge, PillBar};
use crate::app::{palette, App, PanelFocus, PanelMode, SeriesDetail};
use mbv_core::api::EmbyItem;
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::Constraint;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

/// All App-derived data needed to paint the wide TV workspace.
#[derive(Clone)]
pub(in crate::app) struct TvWideRenderCtx {
    pub(in crate::app) list: LibraryListRenderCtx,
    pub(in crate::app) selected_series: Option<EmbyItem>,
    pub(in crate::app) series_detail: Option<SeriesDetail>,
    pub(in crate::app) season_cursor: usize,
    pub(in crate::app) episode_cursor: Option<usize>,
    pub(in crate::app) focused: bool,
    pub(in crate::app) show_letter_pills: bool,
    pub(in crate::app) images_enabled: bool,
    pub(in crate::app) image_loading: bool,
}

impl TvWideRenderCtx {
    pub(in crate::app) fn new(
        list: LibraryListRenderCtx,
        selected_series: Option<EmbyItem>,
        series_detail: Option<SeriesDetail>,
        season_cursor: usize,
        episode_cursor: Option<usize>,
        focused: bool,
        show_letter_pills: bool,
    ) -> Self {
        Self {
            list,
            selected_series,
            series_detail,
            season_cursor,
            episode_cursor,
            focused,
            show_letter_pills,
            images_enabled: true,
            image_loading: true,
        }
    }

    pub(in crate::app) fn with_image_state(
        mut self,
        images_enabled: bool,
        image_loading: bool,
    ) -> Self {
        self.images_enabled = images_enabled;
        self.image_loading = image_loading;
        self
    }

    pub(in crate::app) fn with_local_state(
        mut self,
        cursor: usize,
        scroll: usize,
        season_cursor: usize,
        episode_cursor: Option<usize>,
    ) -> Self {
        self.list = self.list.with_cursor_scroll(cursor, scroll);
        self.season_cursor = season_cursor;
        self.episode_cursor = episode_cursor;
        self
    }

    /// Publish the `tv_wide_*` layout geometry the mounted
    /// `TvWorkspaceComponent` hit-tests (task 5.3d.18d). The legacy
    /// `render_list` wide-TV underpaint is gone; the App frame now only
    /// publishes the hand-off rects before `render_list` runs so input
    /// routing (`is_wide_tv_active`) and the shell's render seam stay
    /// correct while the component owns the picture.
    pub(in crate::app) fn publish_geometry(&self, area: Rect, layout: &mut LayoutMain) {
        layout.tv_wide_area = area;
        let Some(panes) = library_arrangement::wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y)
        else {
            return;
        };
        layout.tv_wide_left_area = panes.left_area;
        layout.tv_wide_right_area = panes.right_area;
        layout.left_area = Rect::default();
        let right_pane = hero_left::hero_on_left_right_pane(panes.right_panel, panes.right_area);
        layout.tv_wide_list_area = padded_rect(right_pane.list_panel, PANE_PAD_X, PANE_PAD_Y);
    }
}

impl App {
    pub(in crate::app::render) fn is_wide_tv_library(&self, lib_idx: usize) -> bool {
        self.libs.get(lib_idx).is_some_and(|lib| {
            lib.library.collection_type == "tvshows"
                && lib.nav_stack.last().is_some_and(|level| {
                    level.items.is_empty()
                        || level.items.iter().all(|item| item.item_type == "Series")
                })
        })
    }

    /// The finalized library content rect when the wide hero-on-left TV
    /// workspace owns `lib_idx`, computed paint-free from the current
    /// terminal size — `None` when the library is not a wide-TV series list
    /// or the breakpoint is narrow. Mirrors the exact gate `render_library`
    /// applies (`is_wide_tv_library` + `shared_hero_presentation` on the
    /// finalized area), so component mount/focus can be routed a frame
    /// earlier than `LayoutMain::is_wide_tv_active` (a previous-frame paint
    /// signal that flashes the narrow browser on entry).
    pub(in crate::app) fn wide_tv_library_area(&self, lib_idx: usize) -> Option<Rect> {
        if !self.is_wide_tv_library(lib_idx) {
            return None;
        }
        let chrome = crate::app::render::arrangements::chrome::chrome_geometry(
            crate::app::render::arrangements::chrome::ChromeGeometryInput {
                area: Rect::new(0, 0, self.terminal_width, self.terminal_height),
                panel_mode: self.effective_panel_mode(),
                panel_focus: self.effective_panel_focus(),
                queue_column_width: self.queue_column_width,
                terminal_width: self.terminal_width,
            },
        );
        if !chrome.right_visible {
            return None;
        }
        let lib_area = crate::app::render::components::widgets::right_panel_content_area(
            chrome.right_area,
            self.effective_panel_mode() != PanelMode::Both,
        );
        hero_left::shared_hero_presentation(lib_area).map(|_| lib_area)
    }

    pub(in crate::app) fn wide_tv_render_ctx(
        &self,
        lib_idx: usize,
        focused: bool,
        cursor_scroll: Option<(usize, usize)>,
    ) -> TvWideRenderCtx {
        let list = self.library_list_render_ctx(
            lib_idx,
            false,
            cursor_scroll.map_or_else(|| 0, |v| v.0),
            cursor_scroll.map_or_else(|| 0, |v| v.1),
        );
        let selected_series = list
            .selected_item()
            .cloned()
            .filter(|item| item.item_type == "Series");
        let series_detail = selected_series
            .as_ref()
            .and_then(|item| self.series_detail_cache.get(&item.id).cloned());
        TvWideRenderCtx::new(
            list,
            selected_series,
            series_detail,
            0,
            None,
            focused && matches!(self.effective_panel_focus(), PanelFocus::Library),
            self.should_show_letter_pills(lib_idx),
        )
    }
}

/// App-free wide TV renderer. The shell builds `TvWideRenderCtx` and the
/// component supplies its local cursor and pane focus through that context.
pub(in crate::app) fn render_wide_tv_with_ctx(
    f: &mut Frame,
    area: Rect,
    ctx: &TvWideRenderCtx,
    layout: &mut LayoutMain,
    media_list: &mut WideMediaList<String>,
) -> (usize, Option<HomeImagePaint>) {
    layout.tv_wide_episode_rows.clear();
    layout.tv_wide_season_tabs.clear();
    layout.tv_wide_area = area;

    let Some(panes) = library_arrangement::wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y) else {
        return (0, None);
    };
    let right_panel = panes.right_panel;
    let right_area = panes.right_area;
    let episode_focused = ctx.focused && ctx.episode_cursor.is_some();
    let right_focused = ctx.focused && !episode_focused;
    let Some(left_area) = hero_left::hero_on_left_pane(
        f,
        area,
        hero_left::LeftPaneFocus::Workspace(ctx.focused && ctx.episode_cursor.is_some()),
    ) else {
        return (0, None);
    };
    layout.tv_wide_left_area = left_area;
    layout.tv_wide_right_area = right_area;
    layout.left_area = Rect::default();

    let (selection_rendered, image_paint) = render_tv_series_selection(
        f,
        left_area,
        episode_focused,
        ctx.selected_series.as_ref(),
        ctx.series_detail.as_ref(),
        ctx.season_cursor,
        ctx.episode_cursor,
        layout,
        ctx.images_enabled,
        ctx.image_loading,
    );
    if !selection_rendered {
        render_placeholder(f, left_area, " Loading\u{2026}");
    }

    let right_pane = hero_left::hero_on_left_right_pane(right_panel, right_area);
    if ctx.list.is_search_active() {
        crate::app::render::components::hero::render_search_box(
            f,
            right_pane.pills_area,
            ctx.list.search_query.as_deref().unwrap_or_default(),
            ctx.list.search_loading,
        );
    } else if ctx.show_letter_pills {
        let selected = ctx
            .list
            .letter_filter
            .as_ref()
            .map(|filter| filter.index)
            .unwrap_or(0);
        let labels = crate::app::render::LetterFilter::labels();
        let ids: Vec<usize> = (0..labels.len()).collect();
        layout.selector_tabs = render_pill_bar(
            f,
            right_pane.pills_area,
            PillBar {
                labels: &labels,
                ids: &ids,
                selected_pos: selected,
                prefix: Some(" \u{2318} "),
            },
        );
    }

    let list_panel = right_pane.list_panel;
    let list_area = padded_rect(list_panel, PANE_PAD_X, PANE_PAD_Y);
    layout.tv_wide_list_area = list_area;
    if list_panel.height > 0 {
        f.render_widget(
            Block::default()
                .style(Style::default().bg(palette::resolve_surface_focus(right_focused))),
            list_panel,
        );
    }
    // The canonical rail owns the full panel row: selection markers and
    // selected backgrounds must reach the panel border, while the layout
    // area remains the padded hit/scroll geometry.
    let paint_area = Rect {
        x: list_panel.x,
        width: list_panel.width,
        ..list_area
    };
    hero_left::hero_on_left_list_panel_border(f, list_panel, right_focused);
    // Legacy rail parity (`item_cell_spans`): the selected row takes the
    // resting surface so it reads against the focused green panel body.
    let paint = super::media_list::render_wide_media_list(
        f,
        paint_area,
        list_area,
        media_list,
        right_focused,
        palette::list_selected_row_bg(),
    );
    layout.left_item_rows = paint.left_item_rows;
    layout.left_row_map = paint.left_row_map;
    let final_scroll = paint.row_geometry.offset();
    // Same key the component sorts the rail rows by, so `left_sorted_indices`
    // matches the painted order; `sort_by_cached_key` computes each key once.
    let mut order: Vec<usize> = (0..ctx.list.items.len()).collect();
    order.sort_by_cached_key(|&index| {
        crate::app::ui_util::natural_sort_key(crate::app::render::effective_sort_str(
            &ctx.list.items[index],
        ))
    });
    layout.left_sorted_indices = order;
    (final_scroll, image_paint)
}

fn render_tv_series_selection(
    f: &mut Frame,
    area: Rect,
    focused: bool,
    selected_series: Option<&EmbyItem>,
    detail: Option<&SeriesDetail>,
    season_cursor: usize,
    episode_cursor: Option<usize>,
    layout: &mut LayoutMain,
    images_enabled: bool,
    image_loading: bool,
) -> (bool, Option<HomeImagePaint>) {
    let Some(item) = selected_series else {
        return (false, None);
    };

    // Artwork-slot-first layout (design.md D-D): a full-width 16:9 landscape
    // slot above the title/metadata/overview, sized with the same formula
    // `prepare_wide_emby_hero_card` uses for Home's wide hero-on-left card.
    let artwork_height = if images_enabled {
        (area.width.saturating_mul(9).saturating_add(31) / 32).max(1)
    } else {
        0
    };
    let slots = hero_left::hero_left_slots(area, artwork_height, None);

    let image_paint = slots.artwork.map(|artwork_area| {
        let image_types = match item.artwork_for(HeroArtworkAspect::Landscape) {
            HeroArtwork::Image { image_types, .. } => image_types,
            HeroArtwork::Placeholder => &["Primary"][..],
        };
        HomeImagePaint::Series {
            area: artwork_area,
            item: Box::new(item.clone()),
            show_placeholder: image_loading,
            image_types,
        }
    });

    let content_area = slots.overview;
    if content_area.height == 0 {
        return (true, image_paint);
    }

    let title = item.title().to_string();
    let meta = item
        .meta_rows(content_area.width)
        .into_iter()
        .next()
        .map(|spans| {
            spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        });
    // Title + ordered metadata only here -- the overview gets its own
    // main-content box below (design.md D-C), matching the season/episode
    // detail box already painted the same way further down.
    let result = crate::app::render::components::hero::paint_hero_content(
        f,
        content_area,
        &HeroContent {
            title: Some(title.as_str()),
            meta_line: meta.as_deref(),
            meta_color: palette::TEXT_DETAIL_META,
            show_playing: false,
            unconditional_spacer_after_meta: true,
            lines: &[],
            image: None,
        },
        focused,
    );
    let mut row = result.next_row;
    let description = item.description();
    if let Some(text) = description.filter(|t| !t.is_empty()) {
        let box_content_width = content_area.width.saturating_sub(PANE_PAD_X * 2) as usize;
        let ov_lines = wrap_overview_lines(&text, |_| box_content_width);
        let ov_height = (ov_lines.len() as u16)
            .max(1)
            .saturating_add(PANE_PAD_Y * 2)
            .min(content_area.bottom().saturating_sub(row));
        if ov_height > PANE_PAD_Y * 2 {
            let box_area = Rect::new(
                content_area.x.saturating_sub(PANE_PAD_X),
                row,
                content_area.width.saturating_add(PANE_PAD_X * 2),
                ov_height,
            );
            let (_, ov_content) = hero_left::hero_on_left_main_content_box(f, box_area);
            let ov_color = if focused {
                palette::TEXT_STRONG
            } else {
                palette::TEXT_MUTED
            };
            f.render_widget(
                Paragraph::new(Span::styled(
                    ov_lines.join(" "),
                    Style::default().fg(ov_color),
                ))
                .wrap(Wrap { trim: true }),
                ov_content,
            );
            row = box_area.y.saturating_add(ov_height);
        }
    }
    let detail_top = row.saturating_add(1);
    if detail_top >= area.bottom() {
        return (true, image_paint);
    }
    let Some(detail) = detail else {
        let box_area = Rect::new(
            area.x.saturating_sub(PANE_PAD_X),
            detail_top,
            area.width.saturating_add(PANE_PAD_X * 2),
            3.min(area.bottom().saturating_sub(detail_top)),
        );
        let (_, content) = hero_left::hero_on_left_main_content_box(f, box_area);
        render_placeholder(f, content, " Loading\u{2026}");
        return (true, image_paint);
    };
    let Some(season) = detail.seasons.get(season_cursor) else {
        return (true, image_paint);
    };
    let episodes = detail
        .episodes
        .get(&season.id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let detail_rows = 1usize.saturating_add(episodes.len().max(1));
    let detail_height = (detail_rows as u16)
        .saturating_add(PANE_PAD_Y * 2)
        .min(area.bottom().saturating_sub(detail_top));
    let box_area = Rect::new(
        area.x.saturating_sub(PANE_PAD_X),
        detail_top,
        area.width.saturating_add(PANE_PAD_X * 2),
        detail_height,
    );
    let (detail_panel, detail_area) = hero_left::hero_on_left_main_content_box(f, box_area);
    if focused {
        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_ACCENT_SOFT)),
            detail_panel,
        );
    }
    if detail_area.height == 0 || detail_area.width == 0 {
        return (true, image_paint);
    }
    let labels: Vec<String> = detail
        .seasons
        .iter()
        .map(|season| season.display_name())
        .collect();
    let ids: Vec<usize> = (0..labels.len()).collect();
    layout.tv_wide_season_tabs = render_pill_bar(
        f,
        Rect::new(detail_area.x, detail_area.y, detail_area.width, 1),
        PillBar {
            labels: &labels,
            ids: &ids,
            selected_pos: season_cursor,
            prefix: Some(" Series: "),
        },
    );
    let first_row = detail_area.y.saturating_add(1);
    let visible = detail_area.height.saturating_sub(1) as usize;
    if episodes.is_empty() {
        render_placeholder(
            f,
            Rect::new(detail_area.x, first_row, detail_area.width, 1),
            if detail.episodes.contains_key(&season.id) {
                " (no episodes)"
            } else {
                " Loading\u{2026}"
            },
        );
        return (true, image_paint);
    }
    let start = episode_cursor
        .map(|cursor| cursor.saturating_sub(visible.saturating_sub(1)))
        .unwrap_or(0)
        .min(episodes.len().saturating_sub(visible));
    let rows = episodes
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .map(|(index, episode)| {
            let selected = episode_cursor == Some(index);
            let marker = super::list_rows::selection_marker(selected, MarkerEdge::Left);
            let number = if episode.index_number > 0 {
                episode.index_number
            } else {
                index as i64 + 1
            };
            let length = episode.runtime_ticks / TICKS_PER_SECOND;
            let duration = if length > 0 {
                crate::app::ui_util::fmt_duration_approx(length)
            } else {
                "\u{2014}".into()
            };
            Row::new(vec![
                Cell::from(format!("{marker}{number}. {}", episode.name)),
                Cell::from(duration),
            ])
            .style(if selected && focused {
                Style::default().fg(palette::TEXT_FOCUS_ACCENT)
            } else {
                Style::default().fg(palette::TEXT_STRONG)
            })
        })
        .collect::<Vec<_>>();
    for (visible_index, (index, _)) in episodes
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .enumerate()
    {
        layout.tv_wide_episode_rows.push((
            Rect::new(
                detail_area.x,
                first_row + visible_index as u16,
                detail_area.width,
                1,
            ),
            index,
        ));
    }
    f.render_widget(
        Table::new(rows, [Constraint::Min(10), Constraint::Length(7)]).column_spacing(1),
        Rect::new(detail_area.x, first_row, detail_area.width, visible as u16),
    );
    (true, image_paint)
}

#[cfg(test)]
#[path = "tv_wide_tests.rs"]
mod tests;
