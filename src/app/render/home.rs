use super::super::ui_util::*;
use super::hero::{self, HERO_BLOCK_EXTRA_ROWS};
use super::home_hero::KeepWatchingHeroLayout;
use super::home_video::home_panel_scroll;
use super::list_rows::SELECTED_BLOCK_SIDE_PADDING;

use crate::app::layout::LayoutMain;
use crate::app::{palette, App, TWO_COLUMN_THRESHOLD};
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

/// Padding around the wide (hero-on-left) hero column's content, matching
/// the hero-on-left arrangement's shared `PANE_PAD_X`/`PANE_PAD_Y` convention
/// (`music_wide.rs`, `audiobookshelf_books.rs`).
const HOME_HERO_PAD_X: u16 = 2;
const HOME_HERO_PAD_Y: u16 = 1;
/// The two-column (wide) hero's original 2-col horizontal padding around
/// the overview text block. The single-column hero has none (flush with
/// the title above it).
const WIDE_OVERVIEW_PAD: usize = 2;

fn inset_pane_vertically(area: Rect) -> Rect {
    Rect {
        y: area.y.saturating_add(HOME_HERO_PAD_Y),
        height: area.height.saturating_sub(HOME_HERO_PAD_Y * 2),
        ..area
    }
}

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

        // Same threshold the library list uses to switch to two columns, so
        // Home's hero/list split and the library list cross over together.
        let two_column = area.width >= TWO_COLUMN_THRESHOLD;
        // Single-column Home's whole panel (content plus the shared tab
        // gutters) is painted green while focused in `render_main`, before
        // this function runs.
        // The wide layout keeps two blank rows below the pill bar (panel
        // surface transition); the single-column layout only needs one.
        let pill_gap_rows: u16 = if two_column { 2 } else { 1 };
        let content_offset = 1 + pill_gap_rows;
        let content_area = Rect {
            y: area.y.saturating_add(content_offset),
            height: area.height.saturating_sub(content_offset),
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
        // Hero data: Emby keeps (item, meta_area, wide_area, img_area,
        // meta_layout) — `wide_area` is where overview lines past the
        // image's bottom edge render at full width; the generic detail
        // block renders into a single content area.
        enum HeroData {
            Emby(
                Box<mbv_core::api::EmbyItem>,
                Rect,
                Rect,
                Rect,
                KeepWatchingHeroLayout,
            ),
            Generic(QueueItem, Rect),
        }
        let hero_data: Option<HeroData>;
        let list_area: Rect;
        // Narrow layout's hero shell (area, row count), painted after the
        // pill-gap fill below rather than inline here: `top_hero_layout`
        // shifts the hero up into the blank row above `content_area` when
        // one exists, which is the same row the pill-gap fill owns, so the
        // shell must paint last to win that row rather than be painted over.
        let mut narrow_shell: Option<(Rect, u16)> = None;

        if two_column {
            // Two-column layout: hero on left, list on right (hero-on-left,
            // design.md decision 4/5: the pane split and its minimum pane
            // width are the shared arrangement's, not a Home-local ratio).
            let (mut hero_panel, right_panel) = hero::hero_on_left_panes(area);
            hero_panel.height = area.height.saturating_sub(1);
            let hero_col_width = hero_panel.width;
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
                    // Image sits above metadata in this layout (not beside
                    // it), so the overview always wraps at the full meta
                    // width — no wrap-around split needed.
                    let meta_layout = Self::keep_watching_hero_layout(
                        &item,
                        meta_w,
                        meta_w,
                        0,
                        WIDE_OVERVIEW_PAD,
                    );
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
                    Block::default().style(Style::default().bg(palette::SURFACE_RESTING)),
                    hero_panel,
                );
            }

            list_area = if hero_data.is_some() {
                right_panel
            } else {
                // No hero item: list takes full width
                content_area
            };
        } else {
            // Vertical layout: hero-on-top fallback (design.md decision 1),
            // reusing the shared reserved-block geometry and the HeroShell
            // (`▁`/`▔`) border every other hero-on-top screen already has
            // (decision 2's "Narrow hero shell is uniform" -- Home was the
            // one screen missing it). The image-beside-metadata content wrap
            // itself is unchanged; it already matches the shared shape.
            let max_allowed = content_area.height.saturating_sub(7);
            let inner_w = content_area
                .width
                .saturating_sub(SELECTED_BLOCK_SIDE_PADDING * 2);

            enum HeroContentDims {
                Emby(mbv_core::api::EmbyItem, u16, KeepWatchingHeroLayout, u16),
                Generic(QueueItem, u16),
                None,
            }
            let dims = if area.width < 24 {
                HeroContentDims::None
            } else {
                match emby_item {
                    Some(item) => {
                        let img_w = inner_w / 2;
                        let meta_w = inner_w.saturating_sub(img_w + 1) as usize;
                        // Image sits beside the metadata column, top-aligned.
                        // Compute its row extent before laying out the
                        // overview, so overview text wraps around it: at
                        // the narrower meta width for rows beside the
                        // image, then at the full hero width for any rows
                        // once past the image's bottom edge.
                        let image_rows =
                            (img_w.saturating_mul(9).saturating_add(31) / 32).min(max_allowed);
                        let meta_layout = Self::keep_watching_hero_layout(
                            &item,
                            meta_w,
                            inner_w as usize,
                            image_rows,
                            0,
                        );
                        if meta_layout.height < 4 {
                            HeroContentDims::None
                        } else {
                            HeroContentDims::Emby(item, img_w, meta_layout, image_rows)
                        }
                    }
                    None => current_item
                        .filter(|item| item.as_emby().is_none())
                        .map(|item| HeroContentDims::Generic(item, max_allowed))
                        .unwrap_or(HeroContentDims::None),
                }
            };
            let content_rows = match &dims {
                HeroContentDims::Emby(_, _, meta_layout, image_rows) => {
                    meta_layout.height.max(*image_rows)
                }
                HeroContentDims::Generic(_, rows) => *rows,
                HeroContentDims::None => 0,
            };
            let desired_hero_rows = if content_rows > 0 {
                content_rows + HERO_BLOCK_EXTRA_ROWS
            } else {
                0
            };
            let top = hero::top_hero_layout(content_area, desired_hero_rows, false);
            if top.hero_rows > 0 {
                narrow_shell = Some((top.hero_area, top.hero_rows));
            }
            let hero_content = Rect {
                x: top.hero_area.x.saturating_add(SELECTED_BLOCK_SIDE_PADDING),
                y: top.hero_area.y.saturating_add(2),
                width: top
                    .hero_area
                    .width
                    .saturating_sub(SELECTED_BLOCK_SIDE_PADDING * 2),
                height: top.hero_rows.saturating_sub(HERO_BLOCK_EXTRA_ROWS),
            };
            hero_data = match dims {
                HeroContentDims::Emby(item, img_w, meta_layout, image_rows) => {
                    let hero_height = image_rows.max(meta_layout.height);
                    let meta_area = Rect {
                        x: hero_content.x,
                        y: hero_content.y,
                        width: hero_content.width.saturating_sub(img_w + 1),
                        height: hero_height,
                    };
                    let img_area = Rect {
                        x: hero_content.x + hero_content.width.saturating_sub(img_w),
                        y: hero_content.y,
                        width: img_w,
                        height: hero_height,
                    };
                    Some(HeroData::Emby(
                        Box::new(item),
                        meta_area,
                        hero_content,
                        img_area,
                        meta_layout,
                    ))
                }
                HeroContentDims::Generic(item, _) => Some(HeroData::Generic(item, hero_content)),
                HeroContentDims::None => None,
            };

            list_area = top.list_area;
        }

        // Hero-on-left's right pane: pill row at the pane's top, then the
        // list panel below it (design.md decision 6, shared with Music and
        // audiobooks via `hero::hero_on_left_right_pane`). With no hero item
        // there is no right pane at all -- pills span the full row and the
        // list takes the full width, same as the single-column layout.
        let wide_pill_section = two_column && hero_data.is_some();
        let (pills_area, green_panel_full): (Rect, Option<Rect>) = if wide_pill_section {
            let right_area = inset_pane_vertically(list_area);
            let right_pane = hero::hero_on_left_right_pane(list_area, right_area, HOME_HERO_PAD_Y);
            (right_pane.pills_area, Some(right_pane.list_panel))
        } else {
            (
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 1,
                },
                None,
            )
        };
        self.render_home_section_pills_row(f, pills_area, layout);

        let list_area = if let Some(list_panel) = green_panel_full {
            let panel_bg = palette::resolve_surface_focus(focused);
            f.render_widget(
                Block::default().style(Style::default().bg(panel_bg)),
                list_panel,
            );
            Rect {
                x: list_panel.x.saturating_add(HOME_HERO_PAD_X),
                y: list_panel.y.saturating_add(HOME_HERO_PAD_Y),
                width: list_panel.width.saturating_sub(HOME_HERO_PAD_X * 2),
                height: list_panel.height.saturating_sub(HOME_HERO_PAD_Y * 2),
            }
        } else {
            list_area
        };
        // The selected row's full-width background fill uses this rect in
        // both layouts — the wide layout's dedicated green panel, or (with
        // no separate panel) `list_area` itself in the single-column
        // layout — so the selected row always gets the same full-row
        // highlight style. `green_panel_full` alone stays `None` in the
        // single-column layout since it also drives the wide panel's
        // top/bottom border rule, which the single-column layout doesn't
        // have.
        let selection_bg_full = green_panel_full.unwrap_or(list_area);
        // Selected-row highlight colour: the wide layout's list panel is
        // itself green while focused, so the dark `SURFACE_BACKDROP` bar
        // reads against it. The single-column layout has no such green
        // panel (its surrounding surface is the ordinary `SURFACE_BACKDROP`
        // library background, same as every other hero-on-top tab), so it
        // uses the same lighter `SURFACE_RESTING` highlight movies/TV lists
        // use (`list_rows.rs`'s `build_list_row_spans`) to stay visible
        // against that darker backdrop.
        let selection_bg = if green_panel_full.is_some() {
            palette::SURFACE_BACKDROP
        } else {
            palette::SURFACE_RESTING
        };

        // Keep the row immediately below the Home pill bar free of list text.
        // The wide layout uses the list panel surface; the single-column
        // layout inherits the ordinary library panel surface (no green
        // focus fill -- Home's panel background matches every other
        // hero-on-top tab's regardless of focus).
        let pill_gap = Rect {
            x: pills_area.x,
            y: pills_area.y.saturating_add(1),
            width: pills_area.width,
            height: 1,
        };
        if pill_gap.y < area.bottom() && pill_gap.width > 0 {
            let panel_bg = palette::SURFACE_BACKDROP;
            f.render_widget(
                Paragraph::new(" ".repeat(pill_gap.width as usize))
                    .style(Style::default().bg(panel_bg)),
                pill_gap,
            );
            if pill_gap_rows > 1 {
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
        }

        layout.left_area = list_area;

        // Painted last so it wins the row it shares with the pill-gap fill
        // above (see `narrow_shell`'s doc comment).
        if let Some((hero_area, hero_rows)) = narrow_shell {
            hero::hero_block_shell(f, hero_area, hero_rows, focused);
        }

        // Render hero (shared between both layout modes)
        match &hero_data {
            Some(HeroData::Emby(item, meta_area, wide_area, img_area, meta_layout)) => {
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
                let overview_pad = if two_column { WIDE_OVERVIEW_PAD } else { 0 };
                self.render_keep_watching_hero_meta(
                    f,
                    *meta_area,
                    *wide_area,
                    item,
                    meta_layout,
                    focused,
                    overview_pad as u16,
                );
            }
            Some(HeroData::Generic(item, area)) => {
                let overview_pad = if two_column { WIDE_OVERVIEW_PAD } else { 0 };
                self.render_home_latest_detail(f, *area, item, focused, overview_pad);
            }
            None => {}
        }

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
            let row_x = selection_bg_full.x;
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

                    // Selected row's full-width background fill, shared by
                    // every row kind (Emby and the generic ABS/Feed
                    // renderer) so the highlight always spans the whole
                    // panel, matching the wide layout's selected-row style.
                    // The marker always draws in a 2-col gutter left of the
                    // text (design.md decision 2: a thin edge marker, no
                    // inline glyph, the same convention every other list
                    // uses), whether that gutter is inside the row's own
                    // background fill (wide, where `list_area` is already
                    // inset from the panel edge) or borrowed from the
                    // chrome margin outside `list_area` (single-column,
                    // which has no such inset).
                    if selected_row && focused {
                        f.render_widget(
                            Block::default().style(Style::default().bg(selection_bg)),
                            Rect {
                                x: selection_bg_full.x,
                                y: sy,
                                width: selection_bg_full.width,
                                height: 1,
                            },
                        );
                        let gutter_x = if list_area.x > row_x {
                            row_x
                        } else {
                            row_x.saturating_sub(2)
                        };
                        f.render_widget(
                            Block::default().style(Style::default().bg(selection_bg)),
                            Rect {
                                x: gutter_x,
                                y: sy,
                                width: 2,
                                height: 1,
                            },
                        );
                        f.render_widget(
                            Paragraph::new(super::selection_marker(true, super::MarkerEdge::Left)),
                            Rect {
                                x: gutter_x,
                                y: sy,
                                width: 1,
                                height: 1,
                            },
                        );
                    }

                    let text_rect = Rect {
                        x: row_x + 2,
                        width: row_rect.width.saturating_sub(2),
                        ..row_rect
                    };

                    // Non-Emby rows (Audiobookshelf today, Feeds in Part 3) use
                    // the generic single-line renderer.
                    let Some(emby) = item.as_emby() else {
                        super::home_latest_row::render_home_latest_row(
                            f,
                            text_rect,
                            item,
                            selected_row,
                            focused,
                        );
                        hitmap.push((row_rect, *flat_idx));
                        continue;
                    };
                    super::home_latest_row::render_home_emby_row(
                        f,
                        text_rect,
                        emby,
                        selected_row,
                        focused,
                    );
                    hitmap.push((row_rect, *flat_idx));
                }
            }
        }

        layout.home.hitmap = hitmap;

        if needs_scrollbar && focused {
            let max_off = content_h.saturating_sub(list_area.height) as usize;
            super::render_right_scrollbar(
                f,
                list_area,
                max_off,
                scroll_y as usize,
                palette::SCROLLBAR,
            );
        }

        if let Some(panel) = green_panel_full {
            hero::hero_on_left_list_panel_border(f, panel, focused);
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

        let mut labels: Vec<(usize, String)> = vec![(0, "Continue".to_string())];
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
