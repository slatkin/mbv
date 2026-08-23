use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::hero_left::{self, PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::arrangements::home as home_arrangement;
use crate::app::render::arrangements::library as library_arrangement;
use crate::app::render::arrangements::padded_rect;
use crate::app::render::components::hero::{self, HERO_BLOCK_EXTRA_ROWS};
use crate::app::render::components::home_hero::{
    HeroData, KeepWatchingHeroLayout, WIDE_OVERVIEW_PAD,
};
use crate::app::render::components::home_list_rows::{render_home_list_rows, DisplayRow};
use crate::app::render::components::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::ui_util::*;
use crate::app::{palette, App};
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::widgets::*;
use ratatui::Frame;

impl App {
    pub(in crate::app::render) fn render_home_list(
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
        let wide_panes = hero_left::shared_hero_presentation(area);
        let two_column = wide_panes.is_some();
        // Single-column Home's whole panel (content plus the shared tab
        // gutters) is painted green while focused in `render_main`, before
        // this function runs.
        let narrow_pill_areas = hero_left::pill_bar_areas(area);
        // Wide (hero-on-left) still pre-reserves its own pill row above
        // `content_area` (its pills sit at the top of the right pane, a
        // hero-on-left concern, `hero_on_left_right_pane`). Narrow
        // inline presentation no longer pre-reserves anything here: its
        // pill row now lives inside `placement-neutral geometry`'s own `pills_area`,
        // outside the selected replacement, same as every other inline browser
        // (design.md decision 6 -- pill *position* is geometry, not a
        // per-screen declaration).
        let content_area = narrow_pill_areas.content_area;

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
        let hero_data: Option<HeroData>;
        let list_area: Rect;
        // Narrow layout's hero shell (area, row count), painted after the
        // pill-gap fill below rather than inline here: `placement-neutral geometry`
        // shifts the hero up into the blank row above `content_area` when
        // one exists, which is the same row the pill-gap fill owns, so the
        // shell must paint last to win that row rather than be painted over.
        let mut narrow_pills_area: Option<Rect> = None;
        let mut narrow_hero_rows = 0;

        if two_column {
            // Two-column layout: hero on left, list on right (hero-on-left,
            // design.md decision 4/5: the pane split and its minimum pane
            // width are the shared arrangement's, not a Home-local ratio).
            let Some((mut hero_panel, right_panel)) = wide_panes else {
                unreachable!("wide_panes is present when two_column is true");
            };
            hero_panel.height = area.height.saturating_sub(1);
            layout.hero_area = hero_panel;
            let mut hero_content = padded_rect(hero_panel, PANE_PAD_X, PANE_PAD_Y);
            let hero_col_height = hero_content.height;

            hero_data = match emby_item {
                Some(item) => {
                    // Shared wide hero-on-left card preparation (design.md
                    // decision 1): the exact same 16:9-artwork-above-metadata
                    // card the wide Movies arrangement renders, so the two
                    // cannot drift in image sizing, metadata order, or
                    // overview treatment.
                    crate::app::render::components::home_hero::prepare_wide_emby_hero_card(&item, hero_content).map(
                        |(meta_layout, meta_area, img_area)| {
                            HeroData::Emby(
                                Box::new(item),
                                meta_area,
                                meta_area, // wide_area same as meta_area in hero-on-left
                                img_area,
                                meta_layout,
                            )
                        },
                    )
                }
                None => current_item
                    .filter(|item| item.as_emby().is_none())
                    .map(|item| {
                        // Size the generic hero to its actual content
                        // (title/overview text, plus a cover for
                        // Audiobookshelf) instead of the full column height —
                        // otherwise short items (feeds have no cover at all)
                        // leave a mostly-empty panel, and the cover -- placed
                        // at the bottom of `area` by `render_home_latest_detail`
                        // -- ends up stranded far below the text.
                        let text_w = hero_content.width as usize;
                        let ov_w = text_w.saturating_sub(WIDE_OVERVIEW_PAD * 2);
                        let text =
                            crate::app::render::components::home_latest_row::home_latest_detail_text(&item, text_w, ov_w);
                        let rows = if matches!(item, QueueItem::Audiobookshelf(_)) {
                            let image_rows =
                                hero_content.width.saturating_mul(9).saturating_add(31) / 32;
                            text.meta_height + 1 + image_rows
                        } else {
                            text.meta_height
                        };
                        hero_content.height = rows.min(hero_col_height);
                        HeroData::Generic(item, hero_content)
                    }),
            };

            if let Some(HeroData::Generic(_, area)) = &hero_data {
                hero_panel.height = area
                    .height
                    .saturating_add(PANE_PAD_Y * 2)
                    .min(hero_panel.height);
            }

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
            // Vertical layout: inline presentation (design.md decision 1),
            // reusing the shared reserved-block geometry and the HeroShell
            // (`▁`/`▔`) border every other inline browser already has
            // (decision 2's "Narrow hero shell is uniform" -- Home was the
            // one screen missing it). The image-beside-metadata content wrap
            // itself is unchanged; it already matches the shared shape.
            let max_allowed = content_area.height.saturating_sub(7);
            let inner_w = content_area
                .width
                .saturating_sub(SELECTED_BLOCK_SIDE_PADDING * 2);

            enum HeroContentDims {
                Emby(mbv_core::api::EmbyItem, u16, KeepWatchingHeroLayout, u16),
                // Audiobookshelf: image beside the metadata column, same
                // shape as `Emby` -- the standard inline arrangement.
                GenericBeside(QueueItem, u16, KeepWatchingHeroLayout, u16),
                // Feed: text-only, no image to sit beside.
                Generic(QueueItem, u16),
                None,
            }
            let dims = if area.width < 24 {
                HeroContentDims::None
            } else {
                // Every inline item with a cover -- Emby and the generic
                // Audiobookshelf hero alike -- gets its image-beside-text
                // dims from the same `beside_image_hero_dims` call, so the
                // two providers' layouts cannot drift apart (image sits
                // beside the metadata column, top-aligned; the overview
                // wraps at the narrower meta width while still beside the
                // image, then at the full hero width once past its bottom
                // edge).
                match emby_item {
                    Some(item) => {
                        let show_name = if item.item_type == "Episode" {
                            item.series_name.clone()
                        } else {
                            String::new()
                        };
                        let overview = if item.overview.is_empty() {
                            String::new()
                        } else {
                            trunc_overview(&item.overview)
                        };
                        let (img_w, meta_layout, image_rows) =
                            crate::app::render::components::home_hero::beside_image_hero_dims(
                                &item.name,
                                &show_name,
                                &overview,
                                inner_w,
                                max_allowed,
                                2, // release-date row + duration row
                            );
                        if meta_layout.height < 4 {
                            HeroContentDims::None
                        } else {
                            HeroContentDims::Emby(item, img_w, meta_layout, image_rows)
                        }
                    }
                    None => current_item
                        .filter(|item| item.as_emby().is_none())
                        .map(|item| {
                            // Feeds have no cover to sit beside and stay
                            // text-only at the full hero width.
                            let QueueItem::Audiobookshelf(episode) = &item else {
                                let text = crate::app::render::components::home_latest_row::home_latest_detail_text(
                                    &item,
                                    inner_w as usize,
                                    inner_w as usize,
                                );
                                return HeroContentDims::Generic(item, text.meta_height);
                            };
                            // Strip URLs (an unbroken URL is one giant
                            // unbreakable word to the wrapper) and cap long
                            // descriptions at 400 chars, matching
                            // `home_latest_detail_text`'s Feed/two-column
                            // overview handling and Emby's home-video/podcast
                            // library views (`trunc_overview`) -- podcast
                            // overviews routinely carry ad copy.
                            let overview = episode
                                .description
                                .as_deref()
                                .map(trunc_overview)
                                .unwrap_or_default();
                            let (img_w, layout, image_rows) =
                                crate::app::render::components::home_hero::beside_image_hero_dims(
                                    item.title(),
                                    episode.show_title.as_deref().unwrap_or(""),
                                    &overview,
                                    inner_w,
                                    max_allowed,
                                    1,
                                );
                            HeroContentDims::GenericBeside(item, img_w, layout, image_rows)
                        })
                        .unwrap_or(HeroContentDims::None),
                }
            };
            let content_rows = match &dims {
                HeroContentDims::Emby(_, _, meta_layout, image_rows) => {
                    meta_layout.height.max(*image_rows)
                }
                HeroContentDims::GenericBeside(_, _, layout, image_rows) => {
                    layout.height.max(*image_rows)
                }
                HeroContentDims::Generic(_, rows) => *rows,
                HeroContentDims::None => 0,
            };
            let desired_hero_rows = if content_rows > 0 {
                content_rows + HERO_BLOCK_EXTRA_ROWS
            } else {
                0
            };
            let cursor_row = rows
                .iter()
                .position(|row| matches!(row, DisplayRow::Item(flat_idx, _) if *flat_idx == cursor))
                .unwrap_or(0);
            let flow = (desired_hero_rows > 0)
                .then(|| {
                    hero::inline_detail_flow(
                        cursor_row,
                        desired_hero_rows,
                        content_area.height,
                        self.home.home_scroll,
                    )
                })
                .flatten();
            let hero_rows = flow.as_ref().map(|_| desired_hero_rows).unwrap_or(0);
            narrow_hero_rows = hero_rows;
            let hero_area = flow.map(|flow| {
                home_arrangement::inline_hero_area(content_area, flow.detail_screen_row, hero_rows)
            });
            if let Some(hero_area) = hero_area {
                layout.hero_area = hero_area;
            }
            let hero_content = hero_area.map(|hero_area| {
                library_arrangement::selected_detail_content_area(
                    hero_area,
                    SELECTED_BLOCK_SIDE_PADDING,
                    HERO_BLOCK_EXTRA_ROWS,
                )
            });
            hero_data = match (dims, hero_content) {
                (
                    HeroContentDims::Emby(item, img_w, meta_layout, image_rows),
                    Some(hero_content),
                ) => {
                    let (meta_area, img_area) =
                        crate::app::render::components::home_hero::beside_image_hero_rects(
                            hero_content,
                            img_w,
                            meta_layout.height,
                            image_rows,
                        );
                    Some(HeroData::Emby(
                        Box::new(item),
                        meta_area,
                        hero_content,
                        img_area,
                        meta_layout,
                    ))
                }
                (
                    HeroContentDims::GenericBeside(item, img_w, layout, image_rows),
                    Some(hero_content),
                ) => {
                    let (meta_area, img_area) =
                        crate::app::render::components::home_hero::beside_image_hero_rects(
                            hero_content,
                            img_w,
                            layout.height,
                            image_rows,
                        );
                    Some(HeroData::GenericBeside(
                        item,
                        meta_area,
                        hero_content,
                        img_area,
                        layout,
                    ))
                }
                (HeroContentDims::Generic(item, _), Some(hero_content)) => {
                    Some(HeroData::Generic(item, hero_content))
                }
                _ => None,
            };
            narrow_pills_area = Some(narrow_pill_areas.pills_area);
            list_area = content_area;
        }

        // Hero-on-left's right pane: pill row at the pane's top, then the
        // list panel below it (design.md decision 6, shared with Music and
        // audiobooks via `hero_left::hero_on_left_right_pane`). With no hero item
        // there is no right pane at all -- pills span the full row and the
        // list takes the full width, same as the single-column layout.
        let wide_pill_section = two_column && hero_data.is_some();
        let (pills_area, spacer_area, green_panel_full): (Rect, Rect, Option<Rect>) =
            if wide_pill_section {
                let right_area = padded_rect(list_area, 0, PANE_PAD_Y);
                let right_pane =
                    hero_left::hero_on_left_right_pane(list_area, right_area, PANE_PAD_Y);
                (
                    right_pane.pills_area,
                    right_pane.spacer_area,
                    Some(right_pane.list_panel),
                )
            } else if two_column {
                // Wide layout, no hero item: same top-of-area fallback the
                // hero-on-left pane would have used.
                let areas = hero_left::pill_bar_areas(area);
                (areas.pills_area, areas.spacer_area, None)
            } else {
                // Narrow: section pills stay outside the selected detail flow.
                (
                    narrow_pills_area.unwrap_or_default(),
                    narrow_pill_areas.spacer_area,
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
            padded_rect(list_panel, PANE_PAD_X, PANE_PAD_Y)
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
        // library background, same as every other inline browser), so it
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
        // inline browser's regardless of focus).
        if spacer_area.y < area.bottom() && spacer_area.width > 0 {
            let panel_bg = palette::SURFACE_BACKDROP;
            f.render_widget(
                Paragraph::new(" ".repeat(spacer_area.width as usize))
                    .style(Style::default().bg(panel_bg)),
                spacer_area,
            );
        }

        layout.left_area = list_area;
        // Render hero (shared between both layout modes).
        if narrow_hero_rows > 0 {
            hero::selected_detail_shell(f, layout.hero_area, narrow_hero_rows, focused);
        }
        if let Some(hero_data) = &hero_data {
            self.render_home_hero_data(f, hero_data, two_column, focused);
        }

        render_home_list_rows(
            self,
            f,
            &rows,
            list_area,
            selection_bg_full,
            selection_bg,
            cursor,
            focused,
            layout,
            narrow_hero_rows,
        );

        if let Some(panel) = green_panel_full {
            hero_left::hero_on_left_list_panel_border(f, panel, focused);
        }
    }
}

#[cfg(test)]
#[path = "home_tests.rs"]
mod tests;
