use super::super::ui_util::*;
use super::home_hero::KeepWatchingHeroLayout;
use super::home_video::power_home_panel_scroll;

use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    pub(super) fn render_power_home_list(
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
            items: Vec<mbv_core::api::MediaItem>,
        }
        enum DisplayRow {
            Empty,
            Item(usize, Box<mbv_core::api::MediaItem>),
        }

        let continue_items = self.home.continue_items.clone();
        let latest = self.home.latest.clone();

        let mut flat = continue_items.len();
        let mut new_sections: Vec<Section> = Vec::new();
        for (idx, (_title, _lib, items, _cur)) in latest.iter().enumerate() {
            if items.is_empty() {
                flat += items.len();
                continue;
            }
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
            y: area.y.saturating_add(2),
            height: area.height.saturating_sub(2),
            ..area
        };

        let mut rows: Vec<DisplayRow> = Vec::new();
        if self.home.section == 0 {
            for (idx, item) in continue_items.into_iter().enumerate() {
                rows.push(DisplayRow::Item(idx, Box::new(item)));
            }
        } else if let Some(section) = selected_new {
            for (idx, item) in section.items.iter().cloned().enumerate() {
                rows.push(DisplayRow::Item(section.flat_start + idx, Box::new(item)));
            }
        }
        if rows.is_empty() {
            rows.push(DisplayRow::Empty);
        }

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
        // of the Newest sections.
        let hero_item = self.power_home_current_item();
        let two_column = area.width >= 80;

        // Hero data: (item, meta_area, img_area, meta_layout)
        let hero_data: Option<(mbv_core::api::MediaItem, Rect, Rect, KeepWatchingHeroLayout)>;
        let list_area: Rect;

        if two_column {
            // Two-column layout: hero on left, list on right
            let hero_col_width = ((area.width as u32 * 2 / 5) as u16)
                .max(12)
                .min(area.width.saturating_sub(12));
            // The right column reserves three rows for the pill row and its
            // dark top/bottom padding. The left hero starts at the panel top.
            let hero_col_height = area.height;

            hero_data = hero_item.and_then(|item| {
                let meta_w = hero_col_width as usize;
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
                        x: area.x,
                        y: area.y,
                        width: hero_col_width,
                        height: image_height,
                    };
                    let meta_area = Rect {
                        x: area.x,
                        y: area.y + img_area.height + 1,
                        width: hero_col_width,
                        height: meta_layout.height,
                    };
                    Some((item, meta_area, img_area, meta_layout))
                }
            });

            list_area = if hero_data.is_some() {
                Rect {
                    x: content_area.x + hero_col_width + 2,
                    y: area.y.saturating_add(3),
                    width: content_area.width.saturating_sub(hero_col_width + 2),
                    height: area.height.saturating_sub(3),
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
                hero_item.and_then(|item| {
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
                        Some((item, meta_area, img_area, meta_layout))
                    }
                })
            };

            let hero_h = hero_data
                .as_ref()
                .map(|(_, meta_area, _, _)| meta_area.height)
                .unwrap_or(0);
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
                y: area.y.saturating_add(1),
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
        let pill_section_area = if wide_pill_section {
            Rect {
                y: area.y,
                height: 3,
                ..pills_area
            }
        } else {
            pills_area
        };
        f.render_widget(
            Block::default().style(Style::default().bg(palette::PLAYBACK_PANEL_BG)),
            pill_section_area,
        );
        self.render_power_home_section_pills_row(f, pills_area, layout);
        if wide_pill_section {
            if let Some((selected_pill, _)) = layout
                .selector_tabs
                .iter()
                .find(|(_, section)| *section == self.home.section)
            {
                for y in [
                    pill_section_area.y,
                    pill_section_area.bottom().saturating_sub(1),
                ] {
                    f.render_widget(
                        Block::default().style(Style::default().bg(palette::YELLOW)),
                        Rect {
                            y,
                            height: 1,
                            ..*selected_pill
                        },
                    );
                }
            }
        }

        // In the wide Home layout, the list body is a separate right-column
        // green surface. The pill section stays outside it. Keep one blank
        // green row at its top and bottom, then inset its list content.
        // `green_panel_full` tracks the full green panel rect (before inset)
        // so focused rows can span its entire width.
        let green_panel_full: Option<Rect>;
        let list_area = if two_column && hero_data.is_some() {
            const RIGHT_COLUMN_INNER_INSET: u16 = 2;
            green_panel_full = Some(list_area);
            f.render_widget(
                Block::default().style(Style::default().bg(palette::BG_GREEN)),
                list_area,
            );
            let interior_area = Rect {
                y: list_area.y.saturating_add(1),
                height: list_area.height.saturating_sub(2),
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

        layout.left_area = list_area;

        // Render hero (shared between both layout modes)
        if let Some((item, meta_area, img_area, meta_layout)) = &hero_data {
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

        let content_h = rows.len().max(1) as u16;
        let wide_home_panel_unfocused = two_column && hero_data.is_some() && !focused;
        let needs_scrollbar = content_h > list_area.height;
        let list_w = super::power_content_width(list_area.width, needs_scrollbar) as u16;
        let cursor_row = rows
            .iter()
            .position(|row| matches!(row, DisplayRow::Item(flat_idx, _) if *flat_idx == cursor))
            .unwrap_or(0) as u16;
        let scroll_y = power_home_panel_scroll(
            self.home.home_scroll as u16,
            cursor_row,
            cursor_row + 1,
            content_h,
            list_area.height,
        );
        self.home.home_scroll = scroll_y as usize;

        let playing_item_id = {
            let playback = self.effective_playback_state();
            playback
                .active
                .then(|| {
                    self.playback_queue()
                        .items
                        .get(playback.active_idx)
                        .map(|item| item.id.clone())
                })
                .flatten()
        };

        let mut hitmap: Vec<(Rect, usize)> = Vec::new();

        let visible = list_area.height.min(content_h.saturating_sub(scroll_y));
        for k in 0..visible {
            let row_idx = scroll_y as usize + k as usize;
            let sy = list_area.y + k;
            let row_rect = Rect {
                x: list_area.x,
                y: sy,
                width: list_w,
                height: 1,
            };
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
                    let is_playing = playing_item_id.as_deref() == Some(item.id.as_str());
                    if selected_row {
                        layout.cursor_screen_y = Some(sy);
                    }

                    let dur_str = if !item.is_folder && item.runtime_ticks > 0 {
                        let mins = (item.runtime_ticks / TICKS_PER_SECOND / 60).max(1);
                        format!("{}m", mins)
                    } else {
                        String::new()
                    };
                    let avail = (row_rect.width as usize).saturating_sub(2); // 2-col gutter (marker/icon)
                                                                             // Reserve a 6-column gap before the duration column so the title
                                                                             // truncates well before running up against it, plus a 1-column
                                                                             // pad after the duration so it isn't flush against the right edge.
                    const DUR_GAP: usize = 6;
                    let dur_reserve = if dur_str.is_empty() {
                        0
                    } else {
                        dur_str.width() + DUR_GAP + 1
                    };
                    let name_w = avail.saturating_sub(dur_reserve);
                    let is_episode = item.item_type == "Episode" && !item.series_name.is_empty();
                    let title_width: usize;

                    let mut spans: Vec<Span> = if is_episode {
                        // Episode: show name in yellow, episode title in white.
                        let show_w = name_w * 2 / 5;
                        let show = trunc_str(&item.series_name, show_w);
                        let show_actual_w = show.width();
                        let ep_title =
                            trunc_str(&item.name, name_w.saturating_sub(show_actual_w + 1));
                        title_width = show_actual_w + 1 + ep_title.width();
                        let bold = if selected_row && focused {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        };
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
                        vec![
                            if is_playing {
                                Span::styled(
                                    super::LIST_PLAY_ICON,
                                    Style::default().fg(palette::AQUA),
                                )
                            } else if selected_row && focused {
                                Span::styled("■", Style::default().fg(palette::RED))
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
                        // Non-episode: single title span.
                        let title = trunc_str(&item.display_name(), name_w);
                        title_width = title.width();
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
                                vec![
                                    if is_playing {
                                        Span::styled(
                                            super::LIST_PLAY_ICON,
                                            Style::default().fg(palette::AQUA),
                                        )
                                    } else {
                                        Span::styled("■", Style::default().fg(palette::RED))
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
                                            .add_modifier(Modifier::BOLD),
                                    ),
                                ]
                            } else {
                                vec![
                                    if is_playing {
                                        Span::styled(
                                            super::LIST_PLAY_ICON,
                                            Style::default().fg(palette::AQUA),
                                        )
                                    } else {
                                        Span::styled("■", Style::default().fg(palette::RED))
                                    },
                                    Span::raw(" "),
                                    Span::styled(
                                        title,
                                        Style::default()
                                            .fg(palette::WHITE)
                                            .add_modifier(Modifier::BOLD),
                                    ),
                                ]
                            }
                        } else {
                            vec![
                                if is_playing {
                                    Span::styled(
                                        super::LIST_PLAY_ICON,
                                        Style::default().fg(palette::AQUA),
                                    )
                                } else {
                                    Span::raw(" ")
                                },
                                Span::raw(" "),
                                Span::styled(
                                    title,
                                    Style::default().fg(if wide_home_panel_unfocused {
                                        palette::MUTED
                                    } else {
                                        palette::WHITE
                                    }),
                                ),
                            ]
                        }
                    };
                    let pad = avail.saturating_sub(title_width + dur_str.width() + 1);
                    if !dur_str.is_empty() {
                        spans.push(Span::raw(" ".repeat(pad)));
                        spans.push(Span::styled(dur_str, Style::default().fg(palette::MUTED)));
                    }
                    f.render_widget(Paragraph::new(Line::from(spans)), row_rect);
                    hitmap.push((row_rect, *flat_idx));
                }
            }
        }

        layout.home.hitmap = hitmap;

        if needs_scrollbar && focused {
            let max_off = content_h.saturating_sub(list_area.height) as usize;
            super::render_power_right_scrollbar(f, list_area, max_off, scroll_y as usize);
        }
    }

    pub(super) fn render_power_home_section_pills_row(
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
        for (idx, (title, _lib, items, _cur)) in self.home.latest.iter().enumerate() {
            if !items.is_empty() {
                labels.push((idx + 1, title.clone()));
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
                prefix: None,
                underlay: super::PillUnderlay::Blank { fill: false },
            },
        );
    }
}

#[cfg(test)]
#[path = "home_tests.rs"]
mod tests;
