//! The Narrow Library panel skeleton (task 5.7, design D4/D7): Selector row,
//! optional List controls row, and the list box carrying the Inline
//! presentation — whose admitted detail block is the one Narrow inline hero,
//! derived by the panel from the same [`HeroContent`] the Wide header uses.
//! One skeleton for every Narrow library destination: destinations supply
//! typed [`LibraryPanelContent`] and paint nothing themselves.
//!
//! The inline hero (design D7) has no selector, controls or constituent rows
//! inside it: the policy-chosen artwork box right-aligned at a size derived
//! from its shape's aspect, the title and the meta rows (coloured as in
//! Wide, `HERO_META_ROLES[n % 3]`) and the overview wrapping around it, and
//! the text reclaiming the full width below the image.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::components::media_list::Presentation;
use crate::app::palette;
use crate::app::render::arrangements::library::selected_detail_content_area;
use crate::app::render::arrangements::wide_hero::pill_bar_areas;
use crate::app::render::{
    render_artwork_placeholder, render_inline_search, render_placeholder, selected_detail_shell,
    wrap_overview_lines, HERO_BLOCK_EXTRA_ROWS, SELECTED_BLOCK_SIDE_PADDING,
};

use super::content::{LibraryPanelContent, ListSlot, PanelHeroImagePaint, PanelListPaintPolicy};
use super::slots::{
    paint_list_controls_row, paint_pill_bar_row, paint_pill_row_gap, paint_selector_row,
    SELECTOR_ROW_PREFIX,
};
use super::wide::SkeletonHits;

/// Columns the inline hero's wrapping text keeps beside a right-aligned
/// artwork box (the Wide header's rule, applied to the Narrow form).
const INLINE_HERO_MIN_TEXT_COLS: u16 = 16;

/// Row cap for the inline hero's image box: a Narrow detail block replaces
/// the selected row inside a list viewport, so the image never grows past
/// ten painted rows regardless of its aspect.
const INLINE_HERO_MAX_IMAGE_ROWS: u16 = 10;

/// One column of quiet gap between the wrapping text and the image box.
const INLINE_HERO_TEXT_GAP_COLS: u16 = 1;

/// Everything the Narrow skeleton placed this frame, in role-rect form. The
/// panel component (task 5.9) retains these rects; tests assert containment
/// against them, never absolute coordinates (design D15).
#[derive(Clone, Debug, Default)]
pub(in crate::app) struct NarrowSkeletonGeometry {
    /// The Selector row's pill-bar rect, reserved even when the destination
    /// supplies no `SelectorRow` (the Inline Search box takes it).
    pub selector_bar: Rect,
    /// The List controls row's rect, when the destination supplies content.
    pub controls: Option<Rect>,
    /// The list box's row-flow rect the Inline presentation painted into.
    pub list_area: Rect,
    /// The admitted inline hero's detail-block rect, when the flow admitted
    /// it (the fallback paints the ordinary selected row instead).
    pub inline_hero: Option<Rect>,
    /// The projected hero image's paint (task 5.10, design D9), when the
    /// inline hero reserved a ready image's box.
    pub inline_hero_image: Option<PanelHeroImagePaint>,
    /// The painted selection's rect: the admitted inline hero block, else
    /// the selected row's rect (the context-menu anchor's painted truth).
    pub selected: Option<Rect>,
}

/// The inline hero's paint plan (design D7): the image box's size derived
/// from the policy shape's aspect, and the detail-block row count the Inline
/// presentation needs to admit the block. One function serves both the
/// `desired_detail_rows` the panel feeds the paint policy and the hero
/// painter, so the two can never disagree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct InlineHeroPlan {
    /// The full detail-block height including the shared selected-block
    /// shell's extra rows (`HERO_BLOCK_EXTRA_ROWS`).
    pub detail_rows: usize,
    /// The right-aligned image box's cell width.
    pub image_cols: u16,
    /// The right-aligned image box's cell height.
    pub image_rows: u16,
}

/// The cell-folded width:height ratios per artwork shape — the same
/// cell-aspect fold the Wide header's `hero_artwork_box` applies (a terminal
/// cell is roughly twice as tall as it is wide, so a 2:3 portrait box is
/// `h*4/3` cells wide, a square `h*2`, and 16:9 landscape is `w*9/32` tall).
fn shape_ratio(shape: super::content::ArtworkShape) -> (u16, u16) {
    match shape {
        super::content::ArtworkShape::Landscape => (32, 9),
        super::content::ArtworkShape::Square => (2, 1),
        super::content::ArtworkShape::Portrait => (4, 3),
    }
}

/// The paint-free inline-hero layout for one list row's width (design D7):
/// the artwork box is right-aligned, sized from its aspect — the decoded
/// source's aspect when the projection projected a ready image (task 5.10's
/// decoded-size arm), the policy shape's aspect otherwise — capped so the
/// wrapping text keeps [`INLINE_HERO_MIN_TEXT_COLS`] columns and the image
/// never exceeds [`INLINE_HERO_MAX_IMAGE_ROWS`] rows; the detail block is
/// the taller of the image and the wrapped text, plus the shared shell's
/// extra rows. One layout site: the panel calls this for the Inline
/// presentation's `desired_detail_rows` and the hero painter calls it again
/// with the admitted block's width.
pub(in crate::app) fn inline_hero_plan(
    block_width: u16,
    hero: &super::content::HeroContent<'_>,
) -> InlineHeroPlan {
    let content_width = block_width.saturating_sub(SELECTED_BLOCK_SIDE_PADDING.saturating_mul(2));
    // Decoded-size arm (design D7): a ready image's own pixel aspect sets
    // the box ratio (cells are ~2x taller than wide), the policy shape's
    // aspect otherwise.
    let (num, den) = match &hero.facts.artwork.image {
        super::content::HeroImageState::Ready {
            decoded: Some((w, h)),
            ..
        } if *w > 0 && *h > 0 => (
            (*w * 2).min(u16::MAX as u32) as u16,
            (*h).min(u16::MAX as u32) as u16,
        ),
        _ => shape_ratio(hero.facts.artwork.shape),
    };
    let max_cols = content_width.saturating_sub(INLINE_HERO_MIN_TEXT_COLS);
    let image_cols = max_cols
        .min(
            (INLINE_HERO_MAX_IMAGE_ROWS
                .saturating_mul(num)
                .saturating_add(den - 1))
                / den,
        )
        .min(content_width);
    let image_rows = (image_cols.saturating_mul(den) / num.max(1)).min(INLINE_HERO_MAX_IMAGE_ROWS);
    let plan = InlineHeroPlan {
        detail_rows: 0,
        image_cols,
        image_rows,
    };
    let text_rows = inline_hero_text_rows(block_width, hero, &plan);
    InlineHeroPlan {
        detail_rows: text_rows.max(image_rows as usize) + HERO_BLOCK_EXTRA_ROWS as usize,
        image_cols,
        image_rows,
    }
}

/// Wraps the hero's text segments across the changing row widths of the
/// wrap-around form: rows beside the image use the beside-image width, rows
/// at or below the image's bottom reclaim the full content width (design
/// D7). Each segment (title, one meta row, overview) starts a fresh row and
/// occupies at least one row; every row keeps its segment's style.
fn inline_hero_lines(
    block_width: u16,
    hero: &super::content::HeroContent<'_>,
    plan: &InlineHeroPlan,
) -> Vec<(String, Style)> {
    let content_width = block_width.saturating_sub(SELECTED_BLOCK_SIDE_PADDING.saturating_mul(2));
    let content_x = SELECTED_BLOCK_SIDE_PADDING;
    let image_x = content_x.saturating_add(
        content_width
            .saturating_sub(plan.image_cols)
            .min(content_width),
    );
    let beside_width = image_x.saturating_sub(content_x).saturating_sub(1);
    let width_at_row = |row: usize| -> usize {
        if (row as u16) < plan.image_rows {
            beside_width as usize
        } else {
            content_width as usize
        }
    };

    let mut segments: Vec<(String, Style)> =
        Vec::with_capacity(1 + hero.facts.meta_rows.len() + usize::from(hero.overview.is_some()));
    segments.push((
        hero.facts.title.clone(),
        Style::default().fg(palette::TEXT_STRONG),
    ));
    for (index, row) in hero.facts.meta_rows.iter().enumerate() {
        segments.push((
            row.clone(),
            Style::default().fg(palette::HERO_META_ROLES[index % palette::HERO_META_ROLES.len()]),
        ));
    }
    if let Some(overview) = hero
        .overview
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        segments.push((
            overview.to_string(),
            Style::default().fg(palette::TEXT_MUTED),
        ));
    }

    let mut lines: Vec<(String, Style)> = Vec::new();
    for (text, style) in segments {
        let start = lines.len();
        let wrapped = wrap_overview_lines(&text, |row| width_at_row(start + row));
        // Each segment occupies at least one row, so the meta rows stay on
        // distinct lines exactly as the Wide header paints them.
        if wrapped.is_empty() {
            lines.push((String::new(), style));
        } else {
            lines.extend(wrapped.into_iter().map(|line| (line, style)));
        }
    }
    lines
}

/// The wrapped text row count for one plan: the flow the hero painter paints.
fn inline_hero_text_rows(
    block_width: u16,
    hero: &super::content::HeroContent<'_>,
    plan: &InlineHeroPlan,
) -> usize {
    inline_hero_lines(block_width, hero, plan).len()
}

/// Paints one inline hero into the admitted detail block (design D7): the
/// shared selected-block shell, the artwork box right-aligned (sized from
/// the projected image's decoded aspect when ready, the policy shape's
/// aspect otherwise), and the title/meta/overview text wrapping around it in
/// the three meta colours, reclaiming the full width below the image. No
/// selector, controls or constituent rows render inside the block. Returns
/// the image box's rect when a projected ready image should paint there
/// (task 5.10: `Ready` reserves; the shell paints after view).
pub(in crate::app) fn paint_inline_hero(
    f: &mut Frame,
    block: Rect,
    hero: &super::content::HeroContent<'_>,
    focused: bool,
) -> Option<Rect> {
    let plan = inline_hero_plan(block.width, hero);
    if plan.detail_rows == 0 || block.height == 0 {
        return None;
    }
    // The shared selected-block shell: the same ▁/▔ framed shell every
    // inline selected-detail block paints (one implementation, no second
    // loop as underpaint).
    selected_detail_shell(f, block, plan.detail_rows as u16, focused);
    let content =
        selected_detail_content_area(block, SELECTED_BLOCK_SIDE_PADDING, HERO_BLOCK_EXTRA_ROWS);
    // The image box: right-aligned, sized from its aspect — the projected
    // image's decoded aspect when ready, the policy shape's otherwise.
    let image = Rect {
        x: content
            .right()
            .saturating_sub(plan.image_cols)
            .max(content.x),
        y: content.y,
        width: plan.image_cols.min(content.width),
        height: plan.image_rows.min(content.height),
    };
    let image_ready = matches!(
        hero.facts.artwork.image,
        super::content::HeroImageState::Ready { .. }
    );
    if image.width > 0 && image.height > 0 && !image_ready {
        // Placeholder at full box size while loading or imageless (spec: the
        // box keeps its size and shows the shared placeholder).
        render_artwork_placeholder(f, image);
    }

    for (offset, (text, style)) in inline_hero_lines(block.width, hero, &plan)
        .into_iter()
        .enumerate()
    {
        let row = content.y.saturating_add(offset as u16);
        if row >= content.bottom() {
            break;
        }
        let row_width = if (offset as u16) < plan.image_rows {
            image.x.saturating_sub(content.x).saturating_sub(1)
        } else {
            content.width
        };
        if row_width == 0 {
            continue;
        }
        f.render_widget(
            Paragraph::new(text).style(style),
            Rect {
                x: content.x,
                y: row,
                width: row_width.min(content.width),
                height: 1,
            },
        );
    }
    (image.width > 0 && image.height > 0 && image_ready).then_some(image)
}

/// Paints the Narrow Library panel skeleton into `area`. Rows top-to-bottom:
/// the Selector row (one pill bar + the panel's spacer, reserved even without
/// a `SelectorRow` — the Inline Search box takes the bar's rect while a
/// search is active), the optional List controls row, and the list box
/// carrying the Inline presentation. The panel computes the Inline
/// presentation's `desired_detail_rows` from the hero content (design D7) and
/// paints the admitted inline hero into the detail block the presentation
/// reserved.
pub(in crate::app) fn render_narrow_skeleton(
    f: &mut Frame,
    area: Rect,
    content: &mut LibraryPanelContent<'_>,
    browser_focused: bool,
    hits: &mut SkeletonHits,
) -> NarrowSkeletonGeometry {
    // Selector row: one pill bar + the panel's spacer, reserved even without
    // a SelectorRow — while a search is active the box takes the bar's rect
    // (spec: the Selector row's place is reserved for the search box) and
    // the spacer keeps the reserved gap.
    let areas = pill_bar_areas(area);
    let searching = matches!(content.list, ListSlot::Search(_));
    match (&content.selector, searching) {
        (Some(selector), false) => {
            paint_selector_row(
                f,
                areas.pills_area,
                areas.spacer_area,
                selector,
                &mut hits.selector,
            );
        }
        (None, false) => {
            // No `SelectorRow`: still repaint the reserved pills row's own
            // background (task 12.2) through the shared pill-row painter,
            // the same one `paint_selector_row` calls for an empty pill
            // list, so the row never keeps whatever was painted underneath
            // it before this panel owned the placement.
            paint_pill_bar_row(
                f,
                areas.pills_area,
                &[],
                None,
                Some(SELECTOR_ROW_PREFIX),
                &mut hits.selector,
            );
            paint_pill_row_gap(f, areas.spacer_area);
        }
        (_, true) => paint_pill_row_gap(f, areas.spacer_area),
    }

    // List controls row: reserved only when the destination supplies
    // content; without it the rows below move up (spec scenario).
    let (list_panel, controls_area) = match &content.controls {
        Some(controls) if areas.content_area.height > 0 => {
            let row = Rect {
                height: 1,
                ..areas.content_area
            };
            paint_list_controls_row(f, row, controls, &mut hits.controls);
            (
                Rect {
                    y: row.bottom(),
                    height: areas.content_area.height.saturating_sub(1),
                    ..areas.content_area
                },
                Some(row),
            )
        }
        _ => (areas.content_area, None),
    };

    let list_area = list_panel;
    // The panel computes the Inline `desired_detail_rows` from the hero
    // content before the view (design D3/D7): the policy carries it.
    let desired_detail_rows = content
        .hero
        .as_ref()
        .map(|hero| inline_hero_plan(list_area.width, hero).detail_rows)
        .unwrap_or(0);

    let mut inline_hero = None;
    let mut inline_hero_image = None;
    match &mut content.list {
        ListSlot::Search(search) => {
            let items = search.ordered_items();
            let query = search.query().to_string();
            let loading = search.loading();
            let cursor = search.cursor();
            let scroll_in = search.scroll();
            let new_scroll = render_inline_search(
                f,
                areas.pills_area,
                list_area,
                &query,
                loading,
                items,
                cursor,
                scroll_in,
                browser_focused,
                1,
                search.layout_mut(),
            );
            search.set_scroll(new_scroll);
        }
        ListSlot::Media(list) => {
            // The panel drives the presentation and the paint policy (design
            // D3): the Narrow breakpoint selects the Inline presentation, and
            // the slot fixes focus and the list-backdrop selected row.
            list.set_presentation(Presentation::Inline, list_area.height.max(1) as usize);
            list.set_paint_policy(PanelListPaintPolicy::Inline {
                focused: browser_focused,
                desired_detail_rows,
            });
            list.view(f, list_area);
            if let Some(hero) = content.hero.as_ref() {
                if let Some(block) = list.detail_rect() {
                    if let Some(image_box) = paint_inline_hero(f, block, hero, browser_focused) {
                        if let super::content::HeroImageState::Ready { cache_key, .. } =
                            &hero.facts.artwork.image
                        {
                            inline_hero_image = Some(PanelHeroImagePaint {
                                area: image_box,
                                cache_key: cache_key.clone(),
                                centered: false,
                            });
                        }
                    }
                    inline_hero = Some(block);
                }
            }
        }
        ListSlot::Empty { loading, text } => {
            let msg = if *loading {
                " Loading\u{2026}"
            } else {
                text.as_str()
            };
            if !msg.is_empty() {
                render_placeholder(f, list_area, msg);
            }
        }
    }

    NarrowSkeletonGeometry {
        selector_bar: areas.pills_area,
        controls: controls_area,
        list_area,
        inline_hero,
        inline_hero_image,
        selected: inline_hero.or(match &mut content.list {
            ListSlot::Media(list) => list.selected_row_rect(),
            _ => None,
        }),
    }
}

#[cfg(test)]
#[path = "narrow_tests.rs"]
mod narrow_tests;
