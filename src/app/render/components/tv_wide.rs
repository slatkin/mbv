use crate::app::components::media_list::WideMediaList;
use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::hero_left::{self, PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::arrangements::library as library_arrangement;
use crate::app::render::arrangements::padded_rect;
use crate::app::render::components::detail_series_view::{SERIES_IMAGE_COLS, SERIES_IMAGE_ROWS};
use crate::app::render::components::hero::{
    inline_hero_text_width, wrap_overview_lines, HeroContent, HeroImage, HeroLine,
};
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::HomeImagePaint;
use crate::app::render::{render_pill_bar, render_placeholder, MarkerEdge, PillBar};
use crate::app::{palette, App, PanelFocus, SeriesDetail};
use mbv_core::api::EmbyItem;
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::Constraint;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Cell, Row, Table};
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
        let right_pane =
            hero_left::hero_on_left_right_pane(panes.right_panel, panes.right_area, PANE_PAD_Y);
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

    pub(in crate::app::render) fn wide_tv_render_ctx(
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
    media_list: &WideMediaList<String>,
) -> (usize, Option<HomeImagePaint>) {
    layout.tv_wide_episode_rows.clear();
    layout.tv_wide_season_tabs.clear();
    layout.tv_wide_area = area;

    let Some(panes) = library_arrangement::wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y) else {
        return (0, None);
    };
    let left_panel = panes.left_panel;
    let right_panel = panes.right_panel;
    let left_area = panes.left_area;
    let right_area = panes.right_area;
    layout.tv_wide_left_area = left_area;
    layout.tv_wide_right_area = right_area;
    layout.left_area = Rect::default();

    let episode_focused = ctx.focused && ctx.episode_cursor.is_some();
    let right_focused = ctx.focused && !episode_focused;
    f.render_widget(
        Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
        left_panel,
    );
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

    let right_pane = hero_left::hero_on_left_right_pane(right_panel, right_area, PANE_PAD_Y);
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
            Block::default().style(palette::resolve_surface_focus(right_focused)),
            list_panel,
        );
    }
    let final_scroll =
        super::media_list::render_wide_media_list(f, list_area, media_list, right_focused, layout);
    let mut order: Vec<usize> = (0..ctx.list.items.len()).collect();
    order.sort_by_key(|&index| ctx.list.items[index].display_name().to_lowercase());
    layout.left_sorted_indices = order;
    hero_left::hero_on_left_list_panel_border(f, list_panel, right_focused);
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
    let title = item.display_name();
    let meta = super::detail_series_view::series_meta_line(item);
    let overview_lines = if item.overview.is_empty() {
        Vec::new()
    } else {
        let overview_start_row = area.y + 1 + (!meta.is_empty()) as u16 + 1;
        wrap_overview_lines(&item.overview, |line_idx| {
            inline_hero_text_width(
                area.width,
                SERIES_IMAGE_COLS,
                SERIES_IMAGE_ROWS,
                overview_start_row
                    .saturating_add(line_idx as u16)
                    .saturating_sub(area.y),
            ) as usize
        })
    };
    let lines: Vec<HeroLine> = overview_lines.into_iter().map(HeroLine::Plain).collect();
    let result = crate::app::render::components::hero::paint_hero_content(
        f,
        area,
        &HeroContent {
            title: Some(title.as_str()),
            meta_line: (!meta.is_empty()).then_some(meta.as_str()),
            meta_color: palette::TEXT_DETAIL_META,
            show_playing: false,
            unconditional_spacer_after_meta: true,
            lines: &lines,
            image: (images_enabled).then_some(HeroImage {
                actual_w: SERIES_IMAGE_COLS,
                height: SERIES_IMAGE_ROWS,
            }),
        },
        focused,
    );
    let image_paint = result.img_rect.map(|image_area| HomeImagePaint::Series {
        area: image_area,
        item: Box::new(item.clone()),
        show_placeholder: image_loading,
    });
    let Some(detail) = detail else {
        render_placeholder(
            f,
            Rect::new(area.x, result.next_row, area.width, 1),
            " Loading\u{2026}",
        );
        return (true, image_paint);
    };
    let Some(season) = detail.seasons.get(season_cursor) else {
        return (true, image_paint);
    };
    if result.next_row >= area.bottom() {
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
        Rect::new(area.x, result.next_row, area.width, 1),
        PillBar {
            labels: &labels,
            ids: &ids,
            selected_pos: season_cursor,
            prefix: Some(" Series: "),
        },
    );
    let episodes = detail
        .episodes
        .get(&season.id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let first_row = result.next_row.saturating_add(1);
    let visible = area.bottom().saturating_sub(first_row).saturating_sub(1) as usize;
    if episodes.is_empty() {
        render_placeholder(
            f,
            Rect::new(area.x, first_row, area.width, 1),
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
            Rect::new(area.x, first_row + visible_index as u16, area.width, 1),
            index,
        ));
    }
    f.render_widget(
        Table::new(rows, [Constraint::Min(10), Constraint::Length(7)]).column_spacing(1),
        Rect::new(area.x, first_row, area.width, visible as u16),
    );
    (true, image_paint)
}

#[cfg(test)]
#[path = "tv_wide_tests.rs"]
mod tests;
