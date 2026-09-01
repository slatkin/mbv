use crate::app::layout::LayoutMain;
use crate::app::library_column_width::{
    library_cell_width, library_column_count, LIBRARY_COLUMN_GAP,
};
use crate::app::palette;
use crate::app::render::arrangements::hero_left;
use crate::app::render::components::feed_row::render_feed_entry_cell;
use crate::app::render::components::hero::{
    inline_detail_flow, inline_display_row, inline_display_row_count, paint_hero_content,
    selected_detail_shell, HeroContent, InlineDisplayRow, HERO_BLOCK_EXTRA_ROWS,
};
use crate::app::render::components::list_rows::{
    draw_column_selection_markers, SELECTED_BLOCK_SIDE_PADDING,
};
use crate::app::render::components::widgets::{
    render_pill_bar, render_placeholder, render_right_scrollbar_with_viewport, PillBar,
};
use crate::app::render::screens::feeds_model::{
    current_time_secs, feed_display_rows, feed_entry_meta_line, feed_hero_content_rows,
    pack_feed_rows, PackedFeedRow,
};
use crate::app::types_feed_tab::WatchedFilter;
use mbv_core::config::FeedSubscription;
use mbv_core::playback_queue::FeedEntry;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub(in crate::app) struct FeedsRenderModel<'a> {
    pub subscriptions: &'a [FeedSubscription],
    pub visible_entries: &'a [FeedEntry],
    pub watched_filter: WatchedFilter,
    pub selected_group: usize,
    pub loading: bool,
    pub cursor: &'a mut usize,
    pub scroll: &'a mut usize,
}

pub(in crate::app) fn render_feeds_content(
    f: &mut Frame,
    area: Rect,
    focused: bool,
    layout: &mut LayoutMain,
    model: FeedsRenderModel<'_>,
) {
    if area.height == 0 {
        return;
    }

    layout.feeds_area = area;
    let subscriptions = model.subscriptions;
    let has_subs = !subscriptions.is_empty();
    let loading = model.loading;

    // The shared arrangement owns the pill row and spacer. The watched
    // filter remains Feeds content immediately below that spacer, with
    // the existing trailing gap before the list.
    let render_selector_content = |f: &mut Frame, pane: Rect| {
        let areas = hero_left::pill_bar_areas(pane);
        let mut selector_tabs = Vec::new();
        if has_subs && areas.pills_area.height > 0 {
            const MAX_LABEL: usize = 12;
            let labels: Vec<String> = std::iter::once("All".to_string())
                .chain(subscriptions.iter().map(|sub| {
                    if sub.name.len() > MAX_LABEL {
                        format!("{}…", &sub.name[..MAX_LABEL])
                    } else {
                        sub.name.clone()
                    }
                }))
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
            let filter = model.watched_filter;
            let mut spans = Vec::new();
            for (i, f_variant) in [
                crate::app::types_feed_tab::WatchedFilter::All,
                crate::app::types_feed_tab::WatchedFilter::Watched,
                crate::app::types_feed_tab::WatchedFilter::Unwatched,
            ]
            .iter()
            .enumerate()
            {
                if i > 0 {
                    spans.push(Span::styled(
                        " · ",
                        Style::default().fg(palette::TEXT_MUTED),
                    ));
                }
                let active = *f_variant == filter;
                spans.push(Span::styled(
                    f_variant.label().to_string(),
                    if active {
                        Style::default()
                            .fg(palette::ACCENT)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(palette::TEXT_MUTED)
                    },
                ));
            }
            f.render_widget(
                Paragraph::new(Line::from(spans))
                    .style(Style::default().bg(palette::SURFACE_BACKDROP)),
                filter_area,
            );
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

    // Whether this frame will show a hero (has entries to select from):
    // if so, the selector rows move below it, unified with the
    // Movies/TV inline browser arrangement; otherwise (no subs / no
    // entries yet) they stay above the placeholder message, since
    // there's no hero to be below.
    let will_show_hero = has_subs && !model.visible_entries.is_empty();

    // When a hero will show, the selector rows move below it and are
    // painted by the post-hero `render_selector_content` call further down
    // (design decision 6, unified with the Movies/TV inline arrangement).
    // Painting them here too would double the pill bar (regressed in
    // 1a4fb6cf, restored to the 45907507 behaviour).
    let (selector_tabs, list_area) = if will_show_hero {
        (Vec::new(), area)
    } else {
        render_selector_content(f, area)
    };
    layout.selector_tabs = selector_tabs;
    layout.left_area = list_area;
    if list_area.height == 0 {
        return;
    }
    // Empty / help state.
    if !has_subs {
        render_placeholder(
            f,
            Rect {
                x: list_area.x,
                y: list_area.y,
                width: list_area.width,
                height: 1,
            },
            " No feed subscriptions configured",
        );
        return;
    }

    let n = model.visible_entries.len();
    if n == 0 {
        let msg = if loading {
            " Loading…"
        } else {
            " Press r to load feeds"
        };
        render_placeholder(
            f,
            Rect {
                x: list_area.x,
                y: list_area.y,
                width: list_area.width,
                height: 1,
            },
            msg,
        );
        return;
    }

    // Render the entry list. Headings are presentation-only: the cursor
    // and all actions continue to address entries by their canonical
    // index in `visible_entries()`.
    let cursor = (*model.cursor).min(n.saturating_sub(1));
    let display_rows = feed_display_rows(model.visible_entries, current_time_secs());

    // Hero: the cursor-selected entry's title + metadata, painted above
    // the (now packed) list -- feeds' inline presentation
    // (design.md decision 6: no image, since feed entries carry none).
    // The group pill bar + watched-filter row render below the hero
    // (unified with Movies/TV's letter-range pills), so they're carved
    // out of the hero's own leftover space rather than `placement-neutral geometry`'s
    // built-in single-pill-row slot, which is too short for both rows.
    let selected_entry = model.visible_entries[cursor].clone();
    let wide_panes = hero_left::shared_hero_presentation(area);
    let wide = wide_panes.is_some();
    let (hero_area, post_hero_area, _) = if wide {
        let Some((hero_panel, right_panel)) = wide_panes else {
            unreachable!("wide_panes is present when wide is true");
        };
        (hero_panel, right_panel, 0)
    } else {
        (Rect::default(), list_area, 0)
    };
    let show_title = library_column_count(post_hero_area.width) > 1;
    let hero_rows = if wide {
        feed_hero_content_rows(show_title)
            .saturating_add(HERO_BLOCK_EXTRA_ROWS)
            .min(hero_area.height)
    } else {
        feed_hero_content_rows(true).saturating_add(HERO_BLOCK_EXTRA_ROWS)
    };
    layout.hero_area = hero_area;
    if wide && hero_rows > 0 {
        selected_detail_shell(f, hero_area, hero_rows, focused);
        let content_rect = Rect {
            x: hero_area.x + SELECTED_BLOCK_SIDE_PADDING,
            y: hero_area.y + 2,
            width: hero_area
                .width
                .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
            height: hero_rows.saturating_sub(HERO_BLOCK_EXTRA_ROWS),
        };
        let meta = feed_entry_meta_line(&selected_entry);
        let hero_content = HeroContent {
            title: show_title.then_some(selected_entry.title.as_str()),
            meta_line: Some(meta.as_str()),
            meta_color: palette::PLAYBACK_META_FG,
            show_playing: false,
            unconditional_spacer_after_meta: false,
            lines: &[],
            image: None,
        };
        paint_hero_content(f, content_rect, &hero_content, focused);
    }

    // Paint the post-hero selector content now that layout publication is
    // already complete for this frame's natural checkpoint.
    let (selector_tabs, list_area) = render_selector_content(f, post_hero_area);
    layout.selector_tabs = selector_tabs;
    layout.left_area = list_area;
    if list_area.height == 0 {
        return;
    }
    // The selector preserves pane width, so pack rows only after it yields
    // the final right-list area in wide presentation.
    let cols = library_column_count(list_area.width);
    let packed = pack_feed_rows(&display_rows, cols);
    let hero_rows = if wide {
        0
    } else {
        if hero_rows >= HERO_BLOCK_EXTRA_ROWS && hero_rows < list_area.height {
            hero_rows
        } else {
            0
        }
    };
    let visible = list_area.height as usize;
    let packed_cursor_row = packed
        .iter()
        .position(|row| matches!(row, PackedFeedRow::Items(idxs) if idxs.contains(&cursor)))
        .unwrap_or(0);
    let total_display = inline_display_row_count(packed.len(), packed_cursor_row, hero_rows);
    let (scroll, detail_screen_row) = if !wide && hero_rows > 0 {
        let flow = inline_detail_flow(
            packed_cursor_row,
            hero_rows,
            list_area.height,
            *model.scroll,
        )
        .expect("admitted inline detail must fit");
        (flow.offset, Some(flow.detail_screen_row))
    } else {
        let lower_bound = packed_cursor_row.saturating_add(1).saturating_sub(visible);
        let upper_bound = packed_cursor_row.min(total_display.saturating_sub(visible));
        ((*model.scroll).clamp(lower_bound, upper_bound), None)
    };
    *model.scroll = scroll;
    let cell_w = library_cell_width(list_area, cols);
    let row_w = list_area.width.saturating_sub(1);
    let visible_count = total_display.saturating_sub(scroll).min(visible);
    let mut row_map: Vec<Option<usize>> = Vec::with_capacity(list_area.height as usize);
    let entries = model.visible_entries;
    for (row_y, display_row) in (list_area.y..).zip((scroll..total_display).take(visible)) {
        match inline_display_row(packed.len(), packed_cursor_row, hero_rows, display_row)
            .expect("visible row is within the replacement flow")
        {
            InlineDisplayRow::Replacement => {
                row_map.push((display_row == packed_cursor_row).then_some(cursor));
            }
            InlineDisplayRow::Source(source_row) => match &packed[source_row] {
                PackedFeedRow::Spacer => {
                    f.render_widget(
                        Paragraph::new(Line::default())
                            .style(Style::default().bg(palette::SURFACE_BACKDROP)),
                        Rect {
                            x: list_area.x,
                            y: row_y,
                            width: row_w,
                            height: 1,
                        },
                    );
                    row_map.push(None);
                }
                PackedFeedRow::Heading(group) => {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(
                                group.label(),
                                Style::default()
                                    .fg(palette::TEXT_FOCUS_ACCENT)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]))
                        .style(Style::default().bg(palette::SURFACE_BACKDROP)),
                        Rect {
                            x: list_area.x,
                            y: row_y,
                            width: row_w,
                            height: 1,
                        },
                    );
                    row_map.push(None);
                }
                PackedFeedRow::Items(idxs) => {
                    for (cell_idx, &i) in idxs.iter().enumerate() {
                        let entry = &entries[i];
                        let selected = i == cursor;
                        let cell_x = list_area.x + cell_idx as u16 * (cell_w + LIBRARY_COLUMN_GAP);
                        render_feed_entry_cell(
                            f,
                            entry,
                            cell_x,
                            row_y,
                            cell_w,
                            selected,
                            focused,
                            !selected || wide,
                        );
                    }
                    row_map.push(idxs.first().copied());
                }
            },
        }
    }
    row_map.resize(list_area.height as usize, None);
    layout.left_row_map = row_map;
    layout.left_item_rows = (0..total_display)
        .map(|display_row| {
            match inline_display_row(packed.len(), packed_cursor_row, hero_rows, display_row)
                .expect("display row is within the replacement flow")
            {
                InlineDisplayRow::Replacement => {
                    if display_row == packed_cursor_row {
                        vec![cursor]
                    } else {
                        Vec::new()
                    }
                }
                InlineDisplayRow::Source(source_row) => match &packed[source_row] {
                    PackedFeedRow::Items(idxs) => idxs.clone(),
                    PackedFeedRow::Spacer | PackedFeedRow::Heading(_) => Vec::new(),
                },
            }
        })
        .collect();

    if visible_count > 0 && visible_count < total_display {
        render_right_scrollbar_with_viewport(
            f,
            list_area,
            total_display,
            visible_count,
            scroll,
            palette::SCROLLBAR,
        );
    }
    if wide {
        draw_column_selection_markers(f, list_area, cursor, &layout.left_item_rows, scroll);
    }
    if !wide {
        if let Some(detail_screen_row) = detail_screen_row {
            layout.hero_area = Rect {
                x: list_area.x,
                y: list_area.y + detail_screen_row as u16,
                width: list_area.width,
                height: hero_rows,
            };
            layout.inline_hero_area = layout.hero_area;
            layout.selected_item_rect = Some(layout.hero_area);
            selected_detail_shell(f, layout.hero_area, hero_rows, focused);
            let meta = feed_entry_meta_line(&selected_entry);
            paint_hero_content(
                f,
                Rect {
                    x: layout.hero_area.x + SELECTED_BLOCK_SIDE_PADDING,
                    y: layout.hero_area.y + 2,
                    width: layout
                        .hero_area
                        .width
                        .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                    height: hero_rows.saturating_sub(HERO_BLOCK_EXTRA_ROWS),
                },
                &HeroContent {
                    title: Some(selected_entry.title.as_str()),
                    meta_line: Some(meta.as_str()),
                    meta_color: palette::PLAYBACK_META_FG,
                    show_playing: false,
                    unconditional_spacer_after_meta: false,
                    lines: &[],
                    image: None,
                },
                focused,
            );
        }
    }
}
