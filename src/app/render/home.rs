use super::super::ui_util::*;
use super::hero::{self, HERO_BLOCK_EXTRA_ROWS};
use super::hero_left;
use super::home_hero::{HeroData, KeepWatchingHeroLayout, WIDE_OVERVIEW_PAD};
use super::home_list_rows::{render_home_list_rows, DisplayRow};
use super::list_rows::SELECTED_BLOCK_SIDE_PADDING;

use crate::app::layout::LayoutMain;
use crate::app::{palette, App, TWO_COLUMN_THRESHOLD};
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::widgets::*;
use ratatui::Frame;

/// Padding around the wide (hero-on-left) hero column's content, matching
/// the hero-on-left arrangement's shared `PANE_PAD_X`/`PANE_PAD_Y` convention
/// (`music_wide.rs`, `audiobookshelf_books.rs`).
const HOME_HERO_PAD_X: u16 = 2;
const HOME_HERO_PAD_Y: u16 = 1;

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
        // Wide (hero-on-left) still pre-reserves its own pill row above
        // `content_area` (its pills sit at the top of the right pane, a
        // hero-on-left concern, `hero_on_left_right_pane`). Narrow
        // (hero-on-top fallback) no longer pre-reserves anything here: its
        // pill row now lives inside `top_hero_layout`'s own `pills_area`,
        // below the hero, same as every other hero-on-top screen
        // (design.md decision 6 -- pill *position* is geometry, not a
        // per-screen declaration).
        let content_offset = if two_column { 1 + pill_gap_rows } else { 0 };
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
        let hero_data: Option<HeroData>;
        let list_area: Rect;
        // Narrow layout's hero shell (area, row count), painted after the
        // pill-gap fill below rather than inline here: `top_hero_layout`
        // shifts the hero up into the blank row above `content_area` when
        // one exists, which is the same row the pill-gap fill owns, so the
        // shell must paint last to win that row rather than be painted over.
        let mut narrow_shell: Option<(Rect, u16)> = None;
        let mut narrow_pills_area: Option<Rect> = None;

        if two_column {
            // Two-column layout: hero on left, list on right (hero-on-left,
            // design.md decision 4/5: the pane split and its minimum pane
            // width are the shared arrangement's, not a Home-local ratio).
            let (mut hero_panel, right_panel) = hero_left::hero_on_left_panes(area);
            hero_panel.height = area.height.saturating_sub(1);
            let mut hero_content = Rect {
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
                    // 16:9 image needs 9 rows for every 32 columns. Ceiling (matching
                    // `render_keep_watching_hero_image`'s own budget) so the reserved
                    // box never ends above where the image actually draws — a smaller
                    // box would let the image's last row overlap the title's first row.
                    let image_height = (hero_content.width.saturating_mul(9).saturating_add(31)
                        / 32)
                        .max(1)
                        .min(hero_col_height.saturating_sub(meta_layout.height));
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
                            super::home_latest_row::home_latest_detail_text(&item, text_w, ov_w);
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
                    .saturating_add(HOME_HERO_PAD_Y * 2)
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
                // Audiobookshelf: image beside the metadata column, same
                // shape as `Emby` -- the standard hero-on-top arrangement.
                GenericBeside(QueueItem, u16, KeepWatchingHeroLayout, u16),
                // Feed: text-only, no image to sit beside.
                Generic(QueueItem, u16),
                None,
            }
            let dims = if area.width < 24 {
                HeroContentDims::None
            } else {
                // Every hero-on-top item with a cover -- Emby and the generic
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
                            super::home_hero::beside_image_hero_dims(
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
                                let text = super::home_latest_row::home_latest_detail_text(
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
                                super::home_hero::beside_image_hero_dims(
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
            let top = hero::top_hero_layout(content_area, desired_hero_rows, true);
            if top.hero_rows > 0 {
                narrow_shell = Some((top.hero_area, top.hero_rows));
            }
            narrow_pills_area = Some(top.pills_area);
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
                    let (meta_area, img_area) = super::home_hero::beside_image_hero_rects(
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
                HeroContentDims::GenericBeside(item, img_w, layout, image_rows) => {
                    let (meta_area, img_area) = super::home_hero::beside_image_hero_rects(
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
                HeroContentDims::Generic(item, _) => Some(HeroData::Generic(item, hero_content)),
                HeroContentDims::None => None,
            };

            list_area = top.list_area;
        }

        // Hero-on-left's right pane: pill row at the pane's top, then the
        // list panel below it (design.md decision 6, shared with Music and
        // audiobooks via `hero_left::hero_on_left_right_pane`). With no hero item
        // there is no right pane at all -- pills span the full row and the
        // list takes the full width, same as the single-column layout.
        let wide_pill_section = two_column && hero_data.is_some();
        let (pills_area, green_panel_full): (Rect, Option<Rect>) = if wide_pill_section {
            let right_area = inset_pane_vertically(list_area);
            let right_pane =
                hero_left::hero_on_left_right_pane(list_area, right_area, HOME_HERO_PAD_Y);
            (right_pane.pills_area, Some(right_pane.list_panel))
        } else if two_column {
            // Wide layout, no hero item: same top-of-area fallback the
            // hero-on-left pane would have used.
            (
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 1,
                },
                None,
            )
        } else {
            // Narrow: pills sit below the hero, in the shared
            // `top_hero_layout` pills slot (unified with Movies/TV's
            // letter-range pills, see `list.rs`).
            (narrow_pills_area.unwrap_or_default(), None)
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

        // Render hero (shared between both layout modes).
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
            content_h,
            focused,
            layout,
        );

        if let Some(panel) = green_panel_full {
            hero_left::hero_on_left_list_panel_border(f, panel, focused);
        }
    }
}

#[cfg(test)]
#[path = "home_tests.rs"]
mod tests;
