use super::super::ui_util::*;
use super::home_hero::KeepWatchingHeroLayout;
use super::home_video::power_home_panel_scroll;
use super::list_rows::focused_or_subtle;
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

        self.render_power_home_section_pills_row(
            f,
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
            layout,
        );
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
        // Row budget for the hero image is the primary size control: an
        // independent, terminal-height-aware target computed up front, not
        // derived from how long a given item's overview text happens to be.
        // Mirrors render_card_image's terminal-height-aware cap.
        let max_allowed = content_area.height.saturating_sub(7);
        let hero_img_rows = max_allowed
            .min(if self.terminal_height <= 30 { 12 } else { 24 })
            .max(10.min(max_allowed));
        let hero: Option<(mbv_core::api::MediaItem, u16, KeepWatchingHeroLayout)> =
            if area.width < 24 {
                None
            } else {
                hero_item.and_then(|item| {
                    let img_w = ((area.width as u32 * 2 / 5) as u16)
                        .max(12)
                        .min(area.width.saturating_sub(12));
                    let meta_w = area.width.saturating_sub(img_w + 1) as usize;
                    let mut meta_layout = Self::keep_watching_hero_layout(&item, meta_w);
                    meta_layout.height = meta_layout.height.min(hero_img_rows);
                    if meta_layout.height < 4 {
                        None
                    } else {
                        Some((item, img_w, meta_layout))
                    }
                })
            };
        let hero_h: u16 = if hero.is_some() { hero_img_rows } else { 0 };

        let list_area = Rect {
            y: content_area.y + hero_h + 2,
            height: content_area.height.saturating_sub(hero_h + 2),
            ..content_area
        };
        layout.left_area = list_area;

        if let Some((item, img_w, meta_layout)) = &hero {
            let img_w = *img_w;
            let hero_area = Rect {
                x: content_area.x,
                y: content_area.y,
                width: content_area.width,
                height: hero_h,
            };
            let meta_area = Rect {
                x: hero_area.x,
                y: hero_area.y,
                width: hero_area.width.saturating_sub(img_w + 1),
                height: hero_h,
            };
            let img_area = Rect {
                x: hero_area.x + hero_area.width.saturating_sub(img_w),
                y: hero_area.y,
                width: img_w,
                height: hero_h,
            };

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
            self.render_keep_watching_hero_image(f, img_area, &cache_key);
            self.render_keep_watching_hero_meta(f, meta_area, item, meta_layout, focused);
        }

        let content_h = rows.len().max(1) as u16;
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
                    if selected_row {
                        layout.cursor_screen_y = Some(sy);
                    }

                    let dur_str = if !item.is_folder && item.runtime_ticks > 0 {
                        let mins = (item.runtime_ticks / TICKS_PER_SECOND / 60).max(1);
                        format!("{}m", mins)
                    } else {
                        String::new()
                    };
                    let avail = (row_rect.width as usize).saturating_sub(1);
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
                    let title = trunc_str(&item.display_name(), name_w);
                    // The gap between title and duration grows to fill whatever
                    // `name_w` didn't need, so it's just what's left of `avail`
                    // after the title and duration (DUR_GAP only sets where
                    // truncation kicks in, above).
                    let pad = avail.saturating_sub(title.width() + dur_str.width() + 1);

                    let fg = focused_or_subtle(focused);
                    let mut spans: Vec<Span> = if selected_row && focused {
                        vec![
                            super::selection_marker(true),
                            Span::styled(
                                title,
                                Style::default()
                                    .fg(palette::IRIS)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]
                    } else {
                        vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
                    };
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
