use crate::app::components::media_list::{InlineMediaBrowser, RowGeometry, WideMediaList};
use crate::app::palette;
use crate::app::render::arrangements::hero_left::{self, PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::arrangements::library as library_arrangement;
use crate::app::render::arrangements::padded_rect;
use crate::app::render::components::hero::{self, HERO_BLOCK_EXTRA_ROWS};
use crate::app::render::components::home_hero;
use crate::app::render::components::home_hero::{HeroData, HomeImagePaint, KeepWatchingHeroLayout};
use crate::app::render::components::home_pills::{home_pill_labels, render_home_pills};
use crate::app::render::components::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::types_playback::HomeLatestSource;
use crate::app::ui_util::*;
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::widgets::*;
use ratatui::Frame;

/// Output of [`render_home_content`]: painted geometry the caller owns.
/// `hero_area`/`selected_item_rect` are `None` when this render touched no
/// hero / painted no visible selection.
pub(in crate::app) struct HomeContentOutput {
    pub(in crate::app) hitmap: Vec<(Rect, usize)>,
    pub(in crate::app) pill_targets: Vec<(Rect, usize)>,
    pub(in crate::app) image_paint: Option<HomeImagePaint>,
    pub(in crate::app) hero_area: Option<Rect>,
    pub(in crate::app) left_area: Rect,
    pub(in crate::app) selected_item_rect: Option<Rect>,
    /// The `section` actually rendered, after the invalid-section clamp.
    /// `HomeComponent::view()` writes it back into its own section state.
    pub(in crate::app) resolved_section: usize,
}

/// The QueueItem at flat `cursor` in the continue-watching + latest-sections
/// flat ordering (mirrors `App::home_current_item` without `App`).
fn home_item_at(
    continue_items: &[QueueItem],
    latest: &[(String, HomeLatestSource, Vec<QueueItem>)],
    cursor: usize,
) -> Option<QueueItem> {
    let mut pos = 0usize;
    for item in continue_items {
        if pos == cursor {
            return Some(item.clone());
        }
        pos += 1;
    }
    for (_, _, items) in latest {
        for item in items {
            if pos == cursor {
                return Some(item.clone());
            }
            pos += 1;
        }
    }
    None
}

/// Paints Home's parent-owned hero + section pills + list-surface chrome
/// without `App` (design D2), then mounts the active canonical control
/// (`canonical_list` for hero-on-left Wide, `inline_list` for inline Narrow)
/// into the list area and rebuilds the pre-#638 hit map from its exported row
/// geometry. `section` is the already-resolved selected pill; `cursor` is the
/// component's already-clamped flat cursor (used only to pick the hero item
/// and anchor the replacement block). Only the image pixel paint is deferred
/// to the shell.
#[allow(clippy::too_many_arguments)]
pub(in crate::app) fn render_home_content(
    f: &mut Frame,
    area: Rect,
    focused: bool,
    continue_items: &[QueueItem],
    latest: &[(String, HomeLatestSource, Vec<QueueItem>)],
    section: usize,
    cursor: usize,
    canonical_list: &WideMediaList<String>,
    inline_list: &InlineMediaBrowser<String>,
    use_nerd_fonts: bool,
) -> HomeContentOutput {
    if area.height == 0 || area.width == 0 {
        return HomeContentOutput {
            hitmap: Vec::new(),
            pill_targets: Vec::new(),
            image_paint: None,
            hero_area: None,
            left_area: Rect::default(),
            selected_item_rect: None,
            resolved_section: section,
        };
    }

    struct Section {
        section_idx: usize,
        flat_start: usize,
        items: Vec<QueueItem>,
    }
    let mut flat = continue_items.len();
    let mut new_sections: Vec<Section> = Vec::new();
    for (idx, (_title, _source, items)) in latest.iter().enumerate() {
        new_sections.push(Section {
            section_idx: idx + 1,
            flat_start: flat,
            items: items.clone(),
        });
        flat += items.len();
    }

    // The caller resolves which section is *persisted*; a section that no
    // longer exists (e.g. a provider went away) still falls back to the
    // first available new section here, matching the legacy clamp.
    let section = if section != 0 && !new_sections.iter().any(|s| s.section_idx == section) {
        new_sections.first().map(|s| s.section_idx).unwrap_or(0)
    } else {
        section
    };

    let selected_new = new_sections.iter().find(|s| s.section_idx == section);

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

    // The active section's flat indices (Continue Watching is section 0; each
    // latest pill is section N). Only this section is projected into the
    // canonical control; the parent keeps section identity. `cursor` is the
    // already-clamped flat cursor the component derived from the control.
    let active_flat: Vec<usize> = if section == 0 {
        (0..continue_items.len()).collect()
    } else if let Some(sec) = selected_new {
        (sec.flat_start..sec.flat_start + sec.items.len()).collect()
    } else {
        Vec::new()
    };
    let control_empty = active_flat.is_empty();

    // --- Home hero panel ----------------------------------------------
    // Shared hero above the selected Home list. It reflects the current
    // flat cursor item whether the active pill is Continue Watching or one
    // of the Newest sections. Emby rows keep the full two-column/hero
    // treatment; non-Emby rows (Audiobookshelf today, Feeds in Part 3) get
    // the generic detail block added in Part 2 (#543).
    let current_item = home_item_at(continue_items, latest, cursor);
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
    let mut narrow_dims: Option<HeroContentDims> = None;
    let mut narrow_desired_hero_rows: u16 = 0;
    let mut hero_area_out: Option<Rect> = None;

    if two_column {
        // Two-column layout: hero on left, list on right (hero-on-left,
        // design.md decision 4/5: the pane split and its minimum pane
        // width are the shared arrangement's, not a Home-local ratio).
        let Some((mut hero_panel, right_panel)) = wide_panes else {
            unreachable!("wide_panes is present when two_column is true");
        };
        hero_panel.height = area.height.saturating_sub(1);
        hero_area_out = Some(hero_panel);
        let mut hero_content = padded_rect(hero_panel, PANE_PAD_X, PANE_PAD_Y);
        let hero_col_height = hero_content.height;

        hero_data = match emby_item {
            Some(item) => {
                // Shared wide hero-on-left card preparation (design.md
                // decision 1): the exact same 16:9-artwork-above-metadata
                // card the wide Movies arrangement renders, so the two
                // cannot drift in image sizing, metadata order, or
                // overview treatment.
                crate::app::render::components::home_hero::prepare_wide_emby_hero_card(
                    &item,
                    hero_content,
                )
                .map(|(meta_layout, meta_area, img_area)| {
                    HeroData::Emby(
                        Box::new(item),
                        meta_area,
                        meta_area, // wide_area same as meta_area in hero-on-left
                        img_area,
                        meta_layout,
                    )
                })
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
                    // The recessed overview box applies the shared pane
                    // padding twice (panel and content), so measure against
                    // its actual text width.
                    let ov_w = text_w;
                    let text =
                        crate::app::render::components::home_latest_row::home_latest_detail_text(
                            &item, text_w, ov_w,
                        );
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
                            HeroContentDims::Emby(Box::new(item), img_w, meta_layout, image_rows)
                        }
                    }
                    None => current_item
                        .filter(|item| item.as_emby().is_none())
                        .map(|item| {
                            // Feeds have no cover to sit beside and stay
                            // text-only at the full hero width.
                            let QueueItem::Audiobookshelf(_) = &item else {
                                let text = crate::app::render::components::home_latest_row::home_latest_detail_text(
                                    &item,
                                    inner_w as usize,
                                    inner_w as usize,
                                );
                                return HeroContentDims::Generic(item, text.meta_height);
                            };
                            let layout = crate::app::render::components::home_latest_row::home_latest_detail_text(
                                &item,
                                inner_w as usize,
                                inner_w as usize,
                            );
                            let image_rows = inner_w.saturating_mul(9).saturating_add(31) / 32;
                            HeroContentDims::Generic(
                                item,
                                (layout.meta_height + 1 + image_rows).min(max_allowed),
                            )
                        })
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
        // Size the hero from its content; placement and admission are the
        // canonical `InlineMediaBrowser`'s replacement-flow decision, resolved
        // when the control paints below.
        narrow_desired_hero_rows = if content_rows > 0 {
            content_rows + HERO_BLOCK_EXTRA_ROWS
        } else {
            0
        };
        narrow_dims = Some(dims);
        hero_data = None;
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
            let right_pane = hero_left::hero_on_left_right_pane(list_area, right_area, PANE_PAD_Y);
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
    let labels = home_pill_labels(latest);
    let pill_targets = render_home_pills(f, pills_area, &labels, section);

    let list_area = if let Some(list_panel) = green_panel_full {
        let panel_bg = palette::resolve_surface_focus(focused);
        f.render_widget(
            Block::default().style(Style::default().bg(panel_bg)),
            list_panel,
        );
        padded_rect(list_panel, PANE_PAD_X, PANE_PAD_Y)
    } else {
        // Narrow Home (and the empty-wide fallback) still owns a focus-aware
        // list surface; only wide hero/list layouts have a separate panel.
        let panel_bg = palette::resolve_surface_focus(focused);
        f.render_widget(
            Block::default().style(Style::default().bg(panel_bg)),
            list_area,
        );
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

    let left_area = list_area;
    let mut image_paint = None;
    // Two-column: the hero-on-left card paints independently of the list flow
    // (its geometry was resolved above, before the pill/list split).
    if two_column {
        if let Some(hero_data) = &hero_data {
            image_paint =
                home_hero::render_home_hero_content(f, hero_data, true, focused, use_nerd_fonts);
        }
    }

    // Paint the active canonical control into the list area and rebuild the
    // pre-#638 Home hit map from its exported row geometry.
    let (hitmap, selected_item_rect) = if control_empty {
        crate::app::render::render_placeholder(f, list_area, " (empty)");
        (Vec::new(), None)
    } else if two_column {
        let mut scratch = crate::app::layout::LayoutMain::default();
        super::media_list::render_wide_media_list(
            f,
            list_area,
            canonical_list,
            focused,
            selection_bg,
            &mut scratch,
        );
        let geometry = canonical_list.row_geometry(list_area.height as usize);
        (
            home_hitmap(&geometry, list_area, &active_flat),
            geometry.selected_row_rect(list_area),
        )
    } else {
        let result = super::media_list::render_inline_media_browser(
            f,
            list_area,
            inline_list,
            narrow_desired_hero_rows as usize,
            focused,
            selection_bg,
        );
        let mut hitmap = home_hitmap(&result.row_geometry, list_area, &active_flat);
        let selected_item_rect = match result.hero_area {
            Some(hero_area) => {
                hitmap.push((hero_area, cursor));
                hero_area_out = Some(hero_area);
                hero::selected_detail_shell(f, hero_area, hero_area.height, focused);
                let hero_content = library_arrangement::selected_detail_content_area(
                    hero_area,
                    SELECTED_BLOCK_SIDE_PADDING,
                    HERO_BLOCK_EXTRA_ROWS,
                );
                if let Some(hero_data) = narrow_dims
                    .take()
                    .and_then(|dims| narrow_hero_data(dims, hero_content))
                {
                    image_paint = home_hero::render_home_hero_content(
                        f,
                        &hero_data,
                        false,
                        focused,
                        use_nerd_fonts,
                    );
                }
                Some(hero_area)
            }
            None => result.row_geometry.selected_row_rect(list_area),
        };
        (hitmap, selected_item_rect)
    };

    if let Some(panel) = green_panel_full {
        hero_left::hero_on_left_list_panel_border(f, panel, focused);
    }

    HomeContentOutput {
        hitmap,
        pill_targets,
        image_paint,
        hero_area: hero_area_out,
        left_area,
        selected_item_rect,
        resolved_section: section,
    }
}

/// Sized hero content for the narrow inline flow, resolved before the control
/// admits (or rejects) the replacement block.
enum HeroContentDims {
    Emby(
        Box<mbv_core::api::EmbyItem>,
        u16,
        KeepWatchingHeroLayout,
        u16,
    ),
    // Feed and Audiobookshelf use the shared stacked detail block;
    // Audiobookshelf artwork is painted above its metadata; Feed stays
    // text-only in the shared renderer.
    Generic(QueueItem, u16),
    None,
}

/// Build the parent-owned narrow `HeroData` once the canonical control has
/// resolved the on-screen detail-block rect.
fn narrow_hero_data(dims: HeroContentDims, hero_content: Rect) -> Option<HeroData> {
    match dims {
        HeroContentDims::Emby(item, img_w, meta_layout, image_rows) => {
            let (meta_area, img_area) =
                crate::app::render::components::home_hero::beside_image_hero_rects(
                    hero_content,
                    img_w,
                    meta_layout.height,
                    image_rows,
                );
            Some(HeroData::Emby(
                item,
                meta_area,
                hero_content,
                img_area,
                meta_layout,
            ))
        }
        HeroContentDims::Generic(item, _) => Some(HeroData::Generic(item, hero_content)),
        HeroContentDims::None => None,
    }
}

/// Rebuild the pre-#638 Home hit map from a canonical control's exported
/// `RowGeometry`: each visible display row that resolves to a source row maps
/// to that active-section item's flat index. Replacement/continuation rows
/// (source row `None`) are skipped; the caller adds the selected replacement
/// block separately.
fn home_hitmap(
    geometry: &RowGeometry<String>,
    area: Rect,
    active_flat: &[usize],
) -> Vec<(Rect, usize)> {
    let offset = geometry.offset();
    geometry
        .visible_rows(area)
        .into_iter()
        .enumerate()
        .filter_map(|(row, rect)| {
            let source = geometry.source_row(offset + row)?;
            Some((rect, *active_flat.get(source)?))
        })
        .collect()
}
