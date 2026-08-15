use super::super::ui_util::*;
use super::home_hero::KeepWatchingHeroLayout;
use super::home_video::home_panel_scroll;

use crate::app::layout::LayoutMain;
use crate::app::{palette, App, TWO_COLUMN_THRESHOLD};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

const HOME_HERO_PAD_X: u16 = 2;
const HOME_HERO_PAD_Y: u16 = 1;

impl App {
    pub(super) fn render_home_list(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        struct Section {
            section_idx: usize,
            flat_start: usize,
            items: Vec<QueueItem>,
        }
        // Cross-provider home row. Emby rows keep the full two-column/hero
        // treatment; non-Emby rows (Audiobookshelf today, Feeds in Part 3) use
        // the generic `render_home_latest_row`/`render_home_latest_detail`.
        enum DisplayRow {
            Empty,
            Item(usize, Box<QueueItem>),
        }

        let continue_items = self.home.continue_items.clone();
        let latest = self.home.latest.clone();

        let mut flat = continue_items.len();
        let mut new_sections: Vec<Section> = Vec::new();
        for (idx, (_title, _lib, items, _cur)) in latest.iter().enumerate() {
            new_sections.push(Section {
                section_idx: idx + 1,
                flat_start: flat,
                items: items.clone(),
            });
            flat += items.len();
        }

        if self.home.section != 0
            && !new_sections
                .iter()
                .any(|section| section.section_idx == self.home.section)
        {
            self.home.section = new_sections
                .first()
                .map(|section| section.section_idx)
                .unwrap_or(0);
        }

        let selected_new = new_sections
            .iter()
            .find(|section| section.section_idx == self.home.section);

        let content_area = Rect {
            y: area.y.saturating_add(3),
            height: area.height.saturating_sub(3),
            ..area
        };

        let mut rows: Vec<DisplayRow> = Vec::new();
        if self.home.section == 0 {
            for (idx, item) in continue_items.into_iter().enumerate() {
                rows.push(DisplayRow::Item(
                    idx,
                    Box::new(QueueItem::Emby(Box::new(item))),
                ));
            }
        } else if let Some(section) = selected_new {
            for (idx, item) in section.items.iter().enumerate() {
                rows.push(DisplayRow::Item(
                    section.flat_start + idx,
                    Box::new(item.clone()),
                ));
            }
        }
        if rows.is_empty() {
            rows.push(DisplayRow::Empty);
        }

        let content_h = rows.len().max(1) as u16;

        let visible_flat_indices: Vec<usize> = rows
            .iter()
            .filter_map(|row| match row {
                DisplayRow::Item(flat_idx, _) => Some(*flat_idx),
                _ => None,
            })
            .collect();
        if let Some(first) = visible_flat_indices.first() {
            if !visible_flat_indices.contains(&self.home.home_cursor) {
                self.home.home_cursor = *first;
            }
        } else {
            self.home.home_cursor = 0;
        }
        let cursor = self.home.home_cursor;

        // --- Home hero panel ----------------------------------------------
        // Shared hero above the selected Home list. It reflects the current
        // flat cursor item whether the active pill is Continue Watching or one
        // of the Newest sections. Emby rows keep the full two-column/hero
        // treatment; non-Emby rows (Audiobookshelf today, Feeds in Part 3) get
        // the generic detail block added in Part 2 (#543).
        let current_item = self.home_current_item();
        let emby_item = current_item
            .as_ref()
            .and_then(|item| item.as_emby().cloned());
        // Same threshold the library list uses to switch to two columns, so
        // Home's hero/list split and the library list cross over together.
        let two_column = area.width >= TWO_COLUMN_THRESHOLD;

        // Hero data: Emby keeps (item, meta_area, img_area, meta_layout); the
        // generic detail block renders into a single content area.
        enum HeroData {
            Emby(
                Box<mbv_core::api::EmbyItem>,
                Rect,
                Rect,
                KeepWatchingHeroLayout,
            ),
            Generic(QueueItem, Rect),
        }
        let hero_data: Option<HeroData>;
        let list_area: Rect;

        if two_column {
            // Two-column layout: hero on left, list on right.
            let hero_col_width = ((area.width as u32 * 2 / 5) as u16)
                .max(12)
                .min(area.width.saturating_sub(12));
            let hero_panel = Rect {
                x: area.x,
                y: area.y,
                width: hero_col_width,
                height: area.height.saturating_sub(1),
            };
            let hero_content = Rect {
                x: hero_panel.x.saturating_add(HOME_HERO_PAD_X),
                y: hero_panel.y.saturating_add(HOME_HERO_PAD_Y),
                width: hero_panel.width.saturating_sub(HOME_HERO_PAD_X * 2),
                height: hero_panel.height.saturating_sub(HOME_HERO_PAD_Y * 2),
            };
            let hero_col_height = hero_content.height;

            hero_data = match emby_item {
                Some(item) => {
                    let meta_w = hero_content.width as usize;
                    let meta_layout = Self::keep_watching_hero_layout(&item, meta_w);
                    // Terminal cells are roughly twice as tall as they are wide, so a
                    // 16:9 image needs 9 rows for every 32 columns. Keep the artwork
                    // at its natural display height, then leave one row before metadata.
                    let image_height = (hero_col_width.saturating_mul(9).saturating_add(31) / 32)
                        .max(1)
                        .min(hero_col_height.saturating_sub(meta_layout.height + 1));
                    if meta_layout.height < 4 || image_height == 0 {
                        None
                    } else {
                        let img_area = Rect {
                            x: hero_content.x,
                            y: hero_content.y,
                            width: hero_content.width,
                            height: image_height,
                        };
                        let meta_area = Rect {
                            x: hero_content.x,
                            y: hero_content.y + img_area.height + 1,
                            width: hero_content.width,
                            height: meta_layout.height,
                        };
                        Some(HeroData::Emby(
                            Box::new(item),
                            meta_area,
                            img_area,
                            meta_layout,
                        ))
                    }
                }
                None => current_item
                    .filter(|item| item.as_emby().is_none())
                    .map(|item| HeroData::Generic(item, hero_content)),
            };

            if hero_data.is_some() {
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::PLAYBACK_PANEL_BG)),
                    hero_panel,
                );
            }

            list_area = if hero_data.is_some() {
                Rect {
                    x: content_area.x + hero_col_width + 2,
                    y: area.y.saturating_add(2),
                    width: content_area.width.saturating_sub(hero_col_width + 2),
                    height: area.height.saturating_sub(2),
                }
            } else {
                // No hero item: list takes full width
                content_area
            };
        } else {
            // Vertical layout: hero on top, list below (unchanged behavior)
            let max_allowed = content_area.height.saturating_sub(7);

            hero_data = if area.width < 24 {
                None
            } else {
                match emby_item {
                    Some(item) => {
                        let img_w = area.width / 2;
                        let meta_w = area.width.saturating_sub(img_w + 1) as usize;
                        let meta_layout = Self::keep_watching_hero_layout(&item, meta_w);
                        let image_rows =
                            (img_w.saturating_mul(9).saturating_add(31) / 32).min(max_allowed);
                        let hero_height = image_rows.max(meta_layout.height);
                        if meta_layout.height < 4 {
                            None
                        } else {
                            let hero_area = Rect {
                                x: content_area.x,
                                y: content_area.y,
                                width: content_area.width,
                                height: hero_height,
                            };
                            let meta_area = Rect {
                                x: hero_area.x,
                                y: hero_area.y,
                                width: hero_area.width.saturating_sub(img_w + 1),
                                height: hero_height,
                            };
                            let img_area = Rect {
                                x: hero_area.x + hero_area.width.saturating_sub(img_w),
                                y: hero_area.y,
                                width: img_w,
                                height: hero_height,
                            };
                            Some(HeroData::Emby(
                                Box::new(item),
                                meta_area,
                                img_area,
                                meta_layout,
                            ))
                        }
                    }
                    None => current_item
                        .filter(|item| item.as_emby().is_none())
                        .map(|item| {
                            HeroData::Generic(
                                item,
                                Rect {
                                    x: content_area.x,
                                    y: content_area.y,
                                    width: content_area.width,
                                    height: max_allowed,
                                },
                            )
                        }),
                }
            };

            let hero_h = match &hero_data {
                Some(HeroData::Emby(_, meta_area, _, _)) => meta_area.height,
                Some(HeroData::Generic(_, area)) => area.height,
                None => 0,
            };
            let list_gap = if hero_data.is_some() { 1 } else { 2 };
            list_area = Rect {
                y: content_area.y + hero_h + list_gap,
                height: content_area.height.saturating_sub(hero_h + list_gap),
                ..content_area
            };
        }

        let wide_pill_section = two_column && hero_data.is_some();
        let pills_area = if wide_pill_section {
            Rect {
                x: list_area.x,
                y: area.y,
                width: list_area.width,
                height: 1,
            }
        } else {
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            }
        };
        self.render_home_section_pills_row(f, pills_area, layout);

        // In the wide Home layout, the list body is a separate right-column
        // green surface directly below the pill row. Keep one blank green row
        // at its top and bottom.
        // `green_panel_full` tracks the painted green panel so focused rows and
        // its top/bottom rules can span the entire width.
        let green_panel_full: Option<Rect>;
        let list_area = if two_column && hero_data.is_some() {
            const RIGHT_COLUMN_INNER_INSET: u16 = 2;
            let panel_h = list_area.height.saturating_sub(1);
            let panel_area = Rect {
                height: panel_h,
                ..list_area
            };
            green_panel_full = Some(panel_area);
            let green_bg = if focused {
                palette::BG_GREEN
            } else {
                palette::PLAYBACK_PANEL_BG
            };
            f.render_widget(
                Block::default().style(Style::default().bg(green_bg)),
                panel_area,
            );
            let interior_area = Rect {
                y: list_area.y.saturating_add(1),
                height: panel_h.saturating_sub(3),
                ..list_area
            };
            Rect {
                x: interior_area.x + RIGHT_COLUMN_INNER_INSET,
                width: interior_area
                    .width
                    .saturating_sub(2 * RIGHT_COLUMN_INNER_INSET),
                ..interior_area
            }
        } else {
            green_panel_full = None;
            list_area
        };

        // Keep the row immediately below the Home pill bar free of list text.
        // The wide layout uses the list panel surface; other layouts inherit
        // the library panel surface.
        let pill_gap = Rect {
            x: pills_area.x,
            y: pills_area.y.saturating_add(1),
            width: pills_area.width,
            height: 1,
        };
        if pill_gap.y < area.bottom() && pill_gap.width > 0 {
            let panel_bg = if wide_pill_section {
                if focused {
                    palette::BG_GREEN
                } else {
                    palette::PLAYBACK_PANEL_BG
                }
            } else {
                palette::LIBRARY_SIDE_BG
            };
            f.render_widget(
                Paragraph::new(" ".repeat(pill_gap.width as usize))
                    .style(Style::default().bg(palette::LIBRARY_SIDE_BG)),
                pill_gap,
            );
            let second_pill_gap = Rect {
                y: pill_gap.y.saturating_add(1),
                ..pill_gap
            };
            if second_pill_gap.y < area.bottom() {
                f.render_widget(
                    Paragraph::new(" ".repeat(second_pill_gap.width as usize))
                        .style(Style::default().bg(panel_bg)),
                    second_pill_gap,
                );
            }
        }

        layout.left_area = list_area;

        // Render hero (shared between both layout modes)
        match &hero_data {
            Some(HeroData::Emby(item, meta_area, img_area, meta_layout)) => {
                let cache_key = format!("{}:pwr_kw", item.id);
                if self.images_enabled() {
                    let img_types = Self::keep_watching_hero_image_types(item);
                    self.fetch_card_image(
                        cache_key.clone(),
                        item.id.clone(),
                        item.series_id.clone(),
                        img_types,
                    );
                }
                self.render_keep_watching_hero_image(f, *img_area, &cache_key, two_column);
                self.render_keep_watching_hero_meta(f, *meta_area, item, meta_layout, focused);
            }
            Some(HeroData::Generic(item, area)) => {
                self.render_home_latest_detail(f, *area, item);
            }
            None => {}
        }

        let wide_home_panel_unfocused = two_column && hero_data.is_some() && !focused;
        let needs_scrollbar = content_h > list_area.height;
        let list_w = super::content_width(list_area.width, needs_scrollbar) as u16;
        let cursor_row = rows
            .iter()
            .position(|row| matches!(row, DisplayRow::Item(flat_idx, _) if *flat_idx == cursor))
            .unwrap_or(0) as u16;
        let scroll_y = home_panel_scroll(
            self.home.home_scroll as u16,
            cursor_row,
            cursor_row + 1,
            content_h,
            list_area.height,
        );
        self.home.home_scroll = scroll_y as usize;

        let mut hitmap: Vec<(Rect, usize)> = Vec::new();

        let visible = list_area.height.min(content_h.saturating_sub(scroll_y));
        for k in 0..visible {
            let row_idx = scroll_y as usize + k as usize;
            let sy = list_area.y + k;
            let row_x = green_panel_full.map(|panel| panel.x).unwrap_or(list_area.x);
            let row_rect = Rect {
                x: row_x,
                y: sy,
                width: list_w.saturating_add(list_area.x.saturating_sub(row_x)),
                height: 1,
            };
            if row_idx >= rows.len() {
                continue;
            }
            match &rows[row_idx] {
                DisplayRow::Empty => {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::raw(" "),
                            Span::styled("(empty)", Style::default().fg(palette::MUTED)),
                        ])),
                        row_rect,
                    );
                }
                DisplayRow::Item(flat_idx, item) => {
                    let selected_row = *flat_idx == cursor;
                    if selected_row {
                        layout.cursor_screen_y = Some(sy);
                    }

                    // Non-Emby rows (Audiobookshelf today, Feeds in Part 3) use
                    // the generic single-line renderer.
                    let Some(emby) = item.as_emby() else {
                        super::home_latest_row::render_home_latest_row(
                            f,
                            row_rect,
                            item,
                            selected_row,
                            focused,
                            wide_home_panel_unfocused,
                        );
                        hitmap.push((row_rect, *flat_idx));
                        continue;
                    };
                    let item = emby;

                    let avail = (row_rect.width as usize).saturating_sub(2);

                    let dur_str = if !item.is_folder && item.runtime_ticks > 0 {
                        fmt_duration_short(item.runtime_ticks / TICKS_PER_SECOND)
                    } else {
                        String::new()
                    };

                    let pct_str = if item.playback_position_ticks > 0
                        && !item.played
                        && item.runtime_ticks > 0
                    {
                        let pct =
                            (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
                        if pct > 0 {
                            Some(format!("{}%", pct))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    let meta_text = dur_str;
                    let meta_w = meta_text.width();
                    // pad + 8-char "HH:MM:SS" + pad
                    const META_COL_W: usize = 10;
                    const META_RIGHT_MARGIN: usize = 0;
                    const META_INNER_PAD: usize = 1;
                    // " 100%" - space + up to 4 chars, reserved next to the title.
                    const PCT_COL_W: usize = 5;
                    let title_col_w = avail.saturating_sub(
                        META_COL_W + META_RIGHT_MARGIN + META_INNER_PAD * 2 + PCT_COL_W,
                    );

                    // Render selection background
                    if selected_row && focused {
                        if let Some(full) = green_panel_full {
                            f.render_widget(
                                Block::default()
                                    .style(Style::default().bg(palette::LIBRARY_SIDE_BG)),
                                Rect {
                                    x: full.x,
                                    y: sy,
                                    width: full.width,
                                    height: 1,
                                },
                            );
                        }
                    }

                    // Build left column (title)
                    let is_episode = item.item_type == "Episode" && !item.series_name.is_empty();
                    let mut title_spans: Vec<Span> = if is_episode {
                        let show_w = title_col_w * 2 / 5;
                        let show = trunc_str(&item.series_name, show_w);
                        let show_actual_w = show.width();
                        let ep_title =
                            trunc_str(&item.name, title_col_w.saturating_sub(show_actual_w + 1));
                        let bold = if selected_row && focused {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        };
                        vec![
                            if selected_row && focused {
                                Span::styled("▍", Style::default().fg(palette::AQUA))
                            } else {
                                Span::raw(" ")
                            },
                            Span::raw(" "),
                            Span::styled(
                                show,
                                Style::default().fg(palette::YELLOW).add_modifier(bold),
                            ),
                            Span::raw(" "),
                            Span::styled(
                                ep_title,
                                Style::default()
                                    .fg(if wide_home_panel_unfocused {
                                        palette::MUTED
                                    } else {
                                        palette::WHITE
                                    })
                                    .add_modifier(bold),
                            ),
                        ]
                    } else {
                        let title = trunc_str(&item.display_name(), title_col_w);
                        vec![
                            if selected_row && focused {
                                Span::styled("▍", Style::default().fg(palette::AQUA))
                            } else {
                                Span::raw(" ")
                            },
                            Span::raw(" "),
                            Span::styled(
                                title,
                                Style::default()
                                    .fg(if wide_home_panel_unfocused {
                                        palette::MUTED
                                    } else {
                                        palette::WHITE
                                    })
                                    .add_modifier(if selected_row && focused {
                                        Modifier::BOLD
                                    } else {
                                        Modifier::empty()
                                    }),
                            ),
                        ]
                    };

                    // Add playback progress percentage to the right of the title
                    if let Some(pct) = &pct_str {
                        title_spans.push(Span::raw(" "));
                        title_spans.push(Span::styled(
                            pct.clone(),
                            Style::default().fg(palette::FOAM),
                        ));
                    }

                    // Calculate actual title width for right-alignment
                    let actual_title_w: usize = title_spans.iter().map(|s| s.content.width()).sum();

                    // Build right column (metadata) - flush with right edge.
                    // Use the row's full width here, not `avail`: `avail` already
                    // subtracts the 2-column left prefix, and `actual_title_w`
                    // includes that same prefix, so reusing `avail` would
                    // double-subtract it and push the metadata box too far left.
                    let mut meta_spans: Vec<Span> = Vec::new();
                    let pad_to_right =
                        (row_rect.width as usize).saturating_sub(actual_title_w + META_COL_W);
                    if pad_to_right > 0 {
                        meta_spans.push(Span::raw(" ".repeat(pad_to_right)));
                    }
                    // Create fixed-width metadata column, text right-aligned inside.
                    // No explicit background: the row's own background (panel or
                    // focused-selection fill) already shows through underneath.
                    let content_w = META_COL_W - META_INNER_PAD * 2;
                    let inner_pad = content_w.saturating_sub(meta_w);
                    let left_pad_str = " ".repeat(META_INNER_PAD + inner_pad);
                    let full_meta = format!("{}{}", left_pad_str, meta_text);
                    let full_meta = format!("{:width$}", full_meta, width = META_COL_W);
                    meta_spans.push(Span::styled(full_meta, Style::default().fg(palette::GREEN)));

                    // Combine and render
                    let mut all_spans = title_spans;
                    all_spans.extend(meta_spans);
                    f.render_widget(Paragraph::new(Line::from(all_spans)), row_rect);
                    hitmap.push((row_rect, *flat_idx));
                }
            }
        }

        layout.home.hitmap = hitmap;

        if needs_scrollbar && focused {
            let max_off = content_h.saturating_sub(list_area.height) as usize;
            super::render_right_scrollbar(f, list_area, max_off, scroll_y as usize);
        }

        if let Some(panel) = green_panel_full {
            if panel.height > 0 {
                let border_style = Style::default().fg(palette::SEEK_TRACK);
                for (y, glyph) in [
                    (panel.y, '\u{2594}'),
                    (panel.bottom().saturating_sub(1), '\u{2581}'),
                ] {
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            glyph.to_string().repeat(panel.width as usize),
                            border_style,
                        ))),
                        Rect {
                            y,
                            height: 1,
                            ..panel
                        },
                    );
                }
            }
        }
    }

    pub(super) fn render_home_section_pills_row(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutMain,
    ) {
        if area.width == 0 || area.height == 0 {
            layout.selector_tabs = Vec::new();
            return;
        }

        let mut labels: Vec<(usize, String)> = vec![(0, "Continue Watching".to_string())];
        for (idx, (title, _lib, _items, _cur)) in self.home.latest.iter().enumerate() {
            // Every section in `home.latest` is a real pill (an ABS library,
            // an Emby view, or Feeds). Match the Continue Watching convention:
            // the pill always renders even when its section is empty (which
            // shows an "(empty)" row), so a bare section is still discoverable.
            labels.push((idx + 1, title.clone()));
        }
        // Restore the last-selected Home pill from prefs once a section with
        // that source identity exists (sections arrive asynchronously across
        // providers). Keep it pending until the section appears.
        if let Some(pending) = self.home_section_pending.as_ref() {
            if let Some((idx, _)) = self
                .home
                .latest
                .iter()
                .enumerate()
                .find(|(_, (_, source, _, _))| source == pending)
            {
                self.home.section = idx + 1;
                self.home_section_pending = None;
            }
        }
        if !labels
            .iter()
            .any(|(section_idx, _)| *section_idx == self.home.section)
        {
            self.home.section = labels[0].0;
        }

        const MAX_LABEL: usize = 18;
        let selected_pos = labels
            .iter()
            .position(|(section_idx, _)| *section_idx == self.home.section)
            .unwrap_or(0);
        // Pre-truncated pill labels; ids are the section indices (idx+1) used
        // as click targets, distinct from the pill's display position.
        let label_strs: Vec<String> = labels
            .iter()
            .map(|(_, label)| trunc_str(label, MAX_LABEL).to_string())
            .collect();
        let ids: Vec<usize> = labels.iter().map(|(section_idx, _)| *section_idx).collect();
        layout.selector_tabs = super::render_pill_bar(
            f,
            area,
            super::PillBar {
                labels: &label_strs,
                ids: &ids,
                selected_pos,
                prefix: Some(" ⌘ "),
            },
        );
    }
}

#[cfg(test)]
#[path = "home_tests.rs"]
mod tests;
