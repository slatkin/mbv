use crate::app::components::media_list::{
    InlineMediaBrowser, InlineMediaBrowserPaintPolicy, SelectedRowSurface, WideMediaList,
    WideMediaListPaintPolicy,
};
use crate::app::layout::LayoutMain;
use crate::app::palette;
use crate::app::render::arrangements::{padded_rect, wide_hero};
use crate::app::render::components::hero::{
    paint_hero_content, selected_detail_shell, HeroContent, HERO_BLOCK_EXTRA_ROWS,
};
use crate::app::render::components::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::render::components::widgets::{render_pill_bar, render_placeholder, PillBar};
use crate::app::render::render_artwork_placeholder;
use crate::app::render::screens::feeds_model::{feed_entry_meta_line, feed_hero_content_rows};
use crate::app::types_feed_tab::WatchedFilter;
use crate::app::ui_util::trunc_str;
use mbv_core::config::FeedSubscription;
use mbv_core::playback_queue::FeedEntry;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;

use tuirealm::component::Component;

/// The parent-owned Feeds chrome model: the subscription/group selector pills
/// and the watched-filter selector stay outside the canonical control, which
/// only ever paints the sub-rect below that pill strip.
pub(in crate::app) struct FeedsRenderModel<'a> {
    pub subscriptions: &'a [FeedSubscription],
    pub visible_entries: &'a [FeedEntry],
    pub watched_filter: WatchedFilter,
    pub selected_group: usize,
    pub loading: bool,
    /// The cursor-selected entry, for the parent-owned detail hero (Wide left
    /// pane and Narrow inline replacement block). `None` when nothing is
    /// selectable.
    pub selected_entry: Option<&'a FeedEntry>,
    pub images_enabled: bool,
}

/// The active media-list presentation Feeds paints this frame (design.md D1):
/// the Wide presentation for the Wide hero list or the Inline presentation for
/// inline Narrow. Exactly one is handed over per view; the same shared
/// `MediaList` owner moves between them.
pub(in crate::app) enum FeedsPresentation<'a> {
    Wide(&'a mut WideMediaList<String>),
    Inline(&'a mut InlineMediaBrowser<String>),
}

/// Paints the Feeds destination's parent-owned pill strip + watched-filter
/// chrome + Wide hero detail pane, then mounts the active canonical
/// presentation (`FeedsPresentation::Wide` for Wide hero Wide,
/// `FeedsPresentation::Inline` for inline Narrow) into the list sub-rect below
/// the pill strip. Only the handed-over presentation paints; the shared
/// `MediaList` owner keeps cursor/scroll and there is no render write-back.
pub(in crate::app) fn render_feeds_content(
    f: &mut Frame,
    area: Rect,
    focused: bool,
    layout: &mut LayoutMain,
    model: FeedsRenderModel<'_>,
    presentation: FeedsPresentation<'_>,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    layout.feeds_area = area;
    let subscriptions = model.subscriptions;
    let has_subs = !subscriptions.is_empty();

    // The shared arrangement owns the pill row and spacer. The watched
    // filter remains Feeds chrome immediately below that spacer, with the
    // existing trailing gap before the list.
    let render_selector_content = |f: &mut Frame, pane: Rect| {
        let areas = wide_hero::pill_bar_areas(pane);
        let mut selector_tabs = Vec::new();
        if has_subs && areas.pills_area.height > 0 {
            const MAX_LABEL: usize = 18;
            let labels: Vec<String> = std::iter::once("All".to_string())
                .chain(
                    subscriptions
                        .iter()
                        .map(|sub| trunc_str(&sub.name, MAX_LABEL)),
                )
                .collect();
            let ids: Vec<usize> = (0..labels.len()).collect();
            selector_tabs = render_pill_bar(
                f,
                areas.pills_area,
                PillBar {
                    labels: &labels,
                    ids: &ids,
                    selected_pos: model.selected_group,
                    prefix: Some(" ⌘ "),
                },
            );
        }

        let filter_area = Rect {
            y: areas.spacer_area.bottom(),
            height: if has_subs {
                1.min(areas.content_area.height)
            } else {
                0
            },
            ..areas.content_area
        };
        if has_subs && filter_area.height > 0 {
            let filters = [
                WatchedFilter::All,
                WatchedFilter::Watched,
                WatchedFilter::Unwatched,
            ];
            let labels: Vec<String> = filters
                .iter()
                .map(|filter| filter.label().to_string())
                .collect();
            let filter_base = subscriptions.len() + 1;
            let ids: Vec<usize> = (0..filters.len())
                .map(|index| filter_base + index)
                .collect();
            let filter_tabs = render_pill_bar(
                f,
                filter_area,
                PillBar {
                    labels: &labels,
                    ids: &ids,
                    selected_pos: model.watched_filter.position(),
                    prefix: None,
                },
            );
            selector_tabs.extend(filter_tabs);
        }

        let list_y = filter_area
            .y
            .saturating_add(if has_subs { 2 } else { 1 })
            .min(pane.y.saturating_add(pane.height));
        let list_area = Rect {
            y: list_y,
            height: pane.y.saturating_add(pane.height).saturating_sub(list_y),
            ..pane
        };
        (selector_tabs, list_area)
    };

    // The shared arrangement owns the pill row and spacer (and the status-row
    // reserve on both returned panes).
    let wide_panes = wide_hero::wide_hero_presentation(area);
    let selector_pane = wide_panes.map(|panes| panes.browser).unwrap_or(area);
    let (selector_tabs, list_panel) = render_selector_content(f, selector_pane);
    layout.selector_tabs = selector_tabs;
    layout.left_area = list_panel;
    if list_panel.height == 0 {
        return;
    }

    // Empty / help states: no canonical control, one-line placeholder.
    if !has_subs {
        render_placeholder(
            f,
            Rect {
                height: 1,
                ..list_panel
            },
            " No feed subscriptions configured",
        );
        return;
    }
    if model.visible_entries.is_empty() {
        let msg = if model.loading {
            " Loading…"
        } else {
            " Press r to load feeds"
        };
        render_placeholder(
            f,
            Rect {
                height: 1,
                ..list_panel
            },
            msg,
        );
        return;
    }

    let wide = wide_panes.is_some();

    // Wide: paint the parent-owned Wide hero detail pane, then frame the
    // left browser pane and inset the canonical control inside it so the border can
    // never replace a heading or the last visible entry at a scroll boundary.
    let (list_area, outer_panel) = if let Some(hero_panel) = wide_panes.map(|panes| panes.hero) {
        layout.hero_area = hero_panel;
        // Wide right hero pane: unconditional fill via the shared primitive
        // (D1, persistent pane -- painted even with no selected entry). Feeds
        // is read-only and never focus-green (D3/D8).
        let hero_content_area =
            wide_hero::wide_hero_hero_pane(f, area, wide_hero::LeftPaneFocus::ReadOnly)
                .expect("wide branch already confirmed wide_hero_presentation fits");
        let (_, hero_content_area) = wide_hero::wide_hero_hero_content_box(f, hero_content_area);
        if let Some(entry) = model.selected_entry {
            paint_feed_hero(f, hero_content_area, entry, focused, model.images_enabled);
        }
        f.render_widget(
            Block::default().style(Style::default().bg(palette::resolve_surface_focus(focused))),
            list_panel,
        );
        // `list_area` is the inset content rect (row/hit geometry); the
        // painter is handed a full-width, vertically-inset paint rect below.
        (
            padded_rect(list_panel, wide_hero::PANE_PAD_X, wide_hero::PANE_PAD_Y),
            Some(list_panel),
        )
    } else {
        (list_panel, None)
    };
    layout.left_area = list_area;
    if list_area.height == 0 {
        return;
    }

    if wide {
        let FeedsPresentation::Wide(canonical_list) = presentation else {
            return;
        };
        let paint_rect = Rect {
            x: outer_panel.map_or(list_area.x, |panel| panel.x),
            width: outer_panel.map_or(list_area.width, |panel| panel.width),
            ..list_area
        };
        if let Some(panel) = outer_panel {
            wide_hero::wide_hero_browser_border(f, panel, focused);
        }
        canonical_list.set_geometry(paint_rect, list_area);
        canonical_list.set_paint_policy(WideMediaListPaintPolicy::new(
            focused,
            SelectedRowSurface::ListBackdrop,
            None,
        ));
        canonical_list.view(f, paint_rect);
        layout.selected_item_rect = canonical_list.current_selected_row_rect();
        layout.inline_hero_area = Rect::default();
    } else {
        let FeedsPresentation::Inline(inline_list) = presentation else {
            return;
        };
        let desired_detail_rows =
            feed_hero_content_rows(true).saturating_add(HERO_BLOCK_EXTRA_ROWS) as usize;
        inline_list.set_geometry(list_area, list_area);
        inline_list.set_paint_policy(InlineMediaBrowserPaintPolicy::new(
            focused,
            SelectedRowSurface::ListBackdrop,
            desired_detail_rows,
        ));
        inline_list.view(f, list_area);
        match inline_list.current_detail_rect() {
            Some(hero_area) => {
                layout.hero_area = hero_area;
                layout.inline_hero_area = hero_area;
                layout.selected_item_rect = Some(hero_area);
                if let Some(entry) = model.selected_entry {
                    selected_detail_shell(f, hero_area, hero_area.height, focused);
                    paint_feed_hero(
                        f,
                        Rect {
                            x: hero_area.x + SELECTED_BLOCK_SIDE_PADDING,
                            y: hero_area.y + 2,
                            width: hero_area
                                .width
                                .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                            height: hero_area.height.saturating_sub(HERO_BLOCK_EXTRA_ROWS),
                        },
                        entry,
                        focused,
                        model.images_enabled,
                    );
                }
            }
            None => {
                layout.hero_area = Rect::default();
                layout.inline_hero_area = Rect::default();
                layout.selected_item_rect = inline_list.current_selected_row_rect();
            }
        }
    }
}

/// Paint the feeds detail hero (title + one metadata line, no artwork) into the
/// already-inset `content` rect. Wide passes the plain pane's padded rect (like
/// the sibling hero painters); Narrow passes the rect inset inside its `▔`/`▁`
/// HeroShell.
fn paint_feed_hero(
    f: &mut Frame,
    content: Rect,
    entry: &FeedEntry,
    focused: bool,
    images_enabled: bool,
) {
    let meta = feed_entry_meta_line(entry);
    let image_width = if images_enabled {
        (content.width / 8).max(1)
    } else {
        0
    };
    let image_height = (image_width.saturating_mul(9).saturating_add(31) / 32)
        .min(content.height)
        .max(1);
    let result = paint_hero_content(
        f,
        content,
        &HeroContent {
            title: Some(entry.title.as_str()),
            meta_line: Some(meta.as_str()),
            meta_color: palette::PLAYBACK_META_FG,
            show_playing: false,
            unconditional_spacer_after_meta: false,
            lines: &[],
            image: images_enabled.then_some(crate::app::render::components::hero::HeroImage {
                actual_w: image_width,
                height: image_height,
            }),
        },
        focused,
    );
    if let Some(image_area) = result.img_rect {
        render_artwork_placeholder(f, image_area);
    }
}
